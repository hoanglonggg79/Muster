use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use muster_core::datetime::{now_local_json_net, now_unix_millis};
use muster_core::import;
use muster_core::launch::windows::{
    SingletonMutex, SystemClientControl, launch_roblox_url, list_running_clients,
};
use muster_core::launch::{
    LaunchTarget, build_launch_url, close_previous_instances, generate_browser_tracker_id,
};
use muster_core::models::Account;
use muster_core::patch::{
    DEFAULT_UNLOCK_FPS, PatchSettings, apply_client_settings, find_version_folder,
};
use muster_core::roblox::account::AccountStore;
use muster_core::roblox::api::{
    fetch_authenticated_user, fetch_user_avatar_headshot, fetch_user_robux,
};
use muster_core::roblox::auth::AuthClient;
use muster_core::roblox::limiter::{RateLimiter, RetryTransport};
use muster_core::roblox::presence::{PresenceType, fetch_presences};
use muster_core::roblox::reqwest_transport::ReqwestTransport;
use muster_core::roblox::vip::{extract_link_code, fetch_access_code, strip_vip_prefix};
use muster_core::settings::Settings;
use muster_core::vault::{self, Branch, Format};
use serde::Serialize;
use specta::Type;
use specta_typescript::Typescript;
use tauri::State;
use tauri_specta::{Builder, collect_commands};

pub const SAVED_PLACE_ID_FIELD: &str = "SavedPlaceId";
pub const SAVED_JOB_ID_FIELD: &str = "SavedJobId";
pub const HTTP_MIN_INTERVAL_MS: u64 = 200;

#[derive(Debug, Serialize, Type)]
pub struct VaultInspection {
    pub path: String,
    pub size_bytes: u32,
    pub branch: Branch,
}

#[derive(Debug, Serialize, Type)]
pub struct VaultOpenSummary {
    pub path: String,
    pub format: Format,
    pub account_count: u32,
    pub usernames: Vec<String>,
}

#[derive(Debug, Serialize, Type)]
pub struct PresenceInfo {
    pub user_id: f64,

    pub presence_type: PresenceType,

    #[specta(type = Option<f64>)]
    pub place_id: Option<i64>,

    pub last_online: Option<String>,
}

pub struct AppState {
    pub store: Mutex<AccountStore>,
    pub mutex_guard: Mutex<Option<SingletonMutex>>,
    pub data_file: PathBuf,
    pub settings: Settings,
    pub limiter: Arc<RateLimiter>,
}

impl AppState {
    fn save_accounts(&self) -> Result<(), String> {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        vault::write_accounts(&self.data_file, store.accounts()).map_err(|e| e.to_string())
    }
}

fn http_transport(state: &AppState) -> Result<RetryTransport<ReqwestTransport>, String> {
    ReqwestTransport::new()
        .map(|transport| RetryTransport::new(transport).with_limiter(Arc::clone(&state.limiter)))
        .map_err(|error| error.to_string())
}

fn resolve_launch_target<T: muster_core::roblox::http::Transport>(
    transport: &T,
    place_id: i64,
    job_id: Option<&str>,
    csrf: &str,
    cookie: &str,
) -> Result<LaunchTarget, String> {
    let Some(job) = job_id.map(strip_vip_prefix).filter(|job| !job.is_empty()) else {
        return Ok(LaunchTarget::Game {
            place_id,
            job_id: None,
        });
    };

    match extract_link_code(job) {
        Some(link_code) => {
            let access_code = fetch_access_code(transport, place_id, link_code, csrf, cookie)
                .map_err(|error| error.to_string())?;

            Ok(LaunchTarget::PrivateServer {
                place_id,
                access_code,
                link_code: link_code.to_owned(),
            })
        }
        None => Ok(LaunchTarget::Game {
            place_id,
            job_id: Some(job.to_owned()),
        }),
    }
}

#[tauri::command]
#[specta::specta]
fn vault_inspect(path: String) -> Result<VaultInspection, String> {
    let data = std::fs::read(&path).map_err(|error| format!("không đọc được {path}: {error}"))?;

    Ok(VaultInspection {
        path,
        size_bytes: u32::try_from(data.len()).unwrap_or(u32::MAX),
        branch: vault::detect_branch(&data),
    })
}

#[tauri::command]
#[specta::specta]
fn vault_open(path: String, password: Option<String>) -> Result<VaultOpenSummary, String> {
    let (accounts, format) = vault::read_accounts_from_path(Path::new(&path), password.as_deref())
        .map_err(|error| error.to_string())?;

    Ok(VaultOpenSummary {
        path,
        format,
        account_count: u32::try_from(accounts.len()).unwrap_or(u32::MAX),
        usernames: accounts
            .iter()
            .map(|account| account.display_name().to_owned())
            .collect(),
    })
}

#[tauri::command]
#[specta::specta]
fn account_list(state: State<AppState>) -> Result<Vec<Account>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(store.accounts().to_vec())
}

#[tauri::command]
#[specta::specta]
fn account_add_cookie(
    cookie: String,
    alias: Option<String>,
    state: State<AppState>,
) -> Result<Account, String> {
    let transport = http_transport(&state)?;
    let user_info = fetch_authenticated_user(&transport, &cookie).map_err(|e| e.to_string())?;

    let robux = fetch_user_robux(&transport, user_info.id, &cookie).unwrap_or(0);
    let avatar = fetch_user_avatar_headshot(&transport, user_info.id).unwrap_or(None);
    let tracker_id = generate_browser_tracker_id().map_err(|e| e.to_string())?;

    let mut fields = BTreeMap::new();
    fields.insert("DisplayName".to_owned(), user_info.display_name.clone());
    fields.insert("Robux".to_owned(), robux.to_string());
    if let Some(avatar_url) = avatar {
        fields.insert("AvatarUrl".to_owned(), avatar_url);
    }

    let account = Account {
        valid: true,
        security_token: Some(cookie),
        username: Some(user_info.name),
        last_use: now_local_json_net(),
        alias: alias.unwrap_or(user_info.display_name),
        description: String::new(),
        password: String::new(),
        group: None,
        user_id: user_info.id,
        fields,
        last_attempted_refresh: now_local_json_net(),
        browser_tracker_id: Some(tracker_id),
        extra: BTreeMap::new(),
    };

    let index = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        let (_, index) = store.add(account).map_err(|e| e.to_string())?;
        index
    };
    state.save_accounts()?;

    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.get(index).cloned().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn account_add_credentials(
    username: String,
    password: String,
    alias: Option<String>,
    state: State<AppState>,
) -> Result<Account, String> {
    let transport = http_transport(&state)?;
    let auth = AuthClient::new(&transport);

    let cookie = auth
        .login(&username, &password)
        .map_err(|e| e.to_string())?;
    account_add_cookie(cookie.to_string(), alias, state)
}

#[tauri::command]
#[specta::specta]
fn account_remove(user_id: f64, state: State<AppState>) -> Result<(), String> {
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.remove_by_user_id(user_id as i64);
    }
    state.save_accounts()
}

#[tauri::command]
#[specta::specta]
fn account_refresh(user_id: f64, state: State<AppState>) -> Result<Account, String> {
    let uid = user_id as i64;
    let cookie = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let idx = store
            .find_by_user_id(uid)
            .ok_or_else(|| "không tìm thấy tài khoản".to_string())?;
        let acc = store.get(idx).map_err(|e| e.to_string())?;
        acc.cookie()
            .ok_or_else(|| "tài khoản không có cookie".to_string())?
            .to_owned()
    };

    let transport = http_transport(&state)?;
    let user_info = fetch_authenticated_user(&transport, &cookie).map_err(|e| e.to_string())?;
    let robux = fetch_user_robux(&transport, uid, &cookie).unwrap_or(0);
    let avatar = fetch_user_avatar_headshot(&transport, uid).unwrap_or(None);

    let updated = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        let idx = store
            .find_by_user_id(uid)
            .ok_or_else(|| "không tìm thấy tài khoản".to_string())?;
        let acc = store.get_mut(idx).map_err(|e| e.to_string())?;
        acc.username = Some(user_info.name);
        acc.set_field("DisplayName", user_info.display_name);
        acc.set_field("Robux", robux.to_string());
        if let Some(url) = avatar {
            acc.set_field("AvatarUrl", url);
        }
        acc.last_attempted_refresh = now_local_json_net();
        acc.clone()
    };

    state.save_accounts()?;
    Ok(updated)
}

#[tauri::command]
#[specta::specta]
fn account_launch(
    user_id: f64,
    place_id: f64,
    job_id: Option<String>,
    state: State<AppState>,
) -> Result<String, String> {
    let uid = user_id as i64;
    let pid = place_id as i64;

    let (cookie, tracker, tracker_generated) = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        let idx = store
            .find_by_user_id(uid)
            .ok_or_else(|| "không tìm thấy tài khoản".to_string())?;
        let acc = store.get_mut(idx).map_err(|e| e.to_string())?;

        let cookie = acc
            .cookie()
            .ok_or_else(|| "tài khoản không có cookie".to_string())?
            .to_owned();

        let generated = match acc.browser_tracker_id.as_deref() {
            Some(tracker) if !tracker.is_empty() => false,
            _ => {
                let tracker = generate_browser_tracker_id().map_err(|e| e.to_string())?;
                acc.browser_tracker_id = Some(tracker);
                true
            }
        };

        let tracker = acc
            .browser_tracker_id
            .clone()
            .ok_or_else(|| "tài khoản không có BrowserTrackerID".to_string())?;

        (cookie, tracker, generated)
    };

    if tracker_generated {
        state.save_accounts()?;
    }

    let transport = http_transport(&state)?;
    let auth = AuthClient::new(&transport);
    let csrf = auth.csrf_token(&cookie).map_err(|e| e.to_string())?;
    let ticket = auth
        .auth_ticket_with_csrf(&cookie, &csrf)
        .map_err(|e| e.to_string())?;

    if state.settings.general.auto_close_last_process {
        let clients = list_running_clients();
        let mut control = SystemClientControl;
        close_previous_instances(&mut control, &clients, &tracker);
    }

    let target = resolve_launch_target(&transport, pid, job_id.as_deref(), &csrf, &cookie)?;

    let launch_url = build_launch_url(ticket.as_str(), &target, &tracker, now_unix_millis(), false);

    launch_roblox_url(&launch_url).map_err(|e| e.to_string())?;

    Ok(format!("Đã khởi chạy tài khoản {uid} vào game {pid}"))
}

#[tauri::command]
#[specta::specta]
fn patch_apply_fps(unlock_fps: bool, max_fps: Option<u32>) -> Result<String, String> {
    let folder = find_version_folder().map_err(|e| e.to_string())?;
    let settings = PatchSettings {
        custom_client_settings: None,
        unlock_fps,
        max_fps_value: max_fps,
    };
    apply_client_settings(&folder, &settings).map_err(|e| e.to_string())?;
    let effective = max_fps.unwrap_or(DEFAULT_UNLOCK_FPS);
    Ok(format!(
        "Đã áp dụng cấu hình FPS ({effective} FPS) tại {}",
        folder.display()
    ))
}

#[tauri::command]
#[specta::specta]
fn multi_roblox_toggle(enabled: bool, state: State<AppState>) -> Result<bool, String> {
    let mut guard = state.mutex_guard.lock().map_err(|e| e.to_string())?;
    if enabled {
        if guard.is_none() {
            let m = SingletonMutex::acquire().map_err(|e| e.to_string())?;
            *guard = Some(m);
        }
        Ok(true)
    } else {
        *guard = None;
        Ok(false)
    }
}

#[tauri::command]
#[specta::specta]
fn vault_import_ram(
    dir: String,
    password: Option<String>,
    state: State<AppState>,
) -> Result<u32, String> {
    let path = Path::new(&dir).join(import::ACCOUNT_FILE);
    if !path.is_file() {
        return Err("Không tìm thấy file AccountData.json trong thư mục chỉ định".to_string());
    }

    let (accounts, _) =
        vault::read_accounts_from_path(&path, password.as_deref()).map_err(|e| e.to_string())?;
    let count = accounts.len() as u32;

    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        for acc in accounts {
            store.import_account(acc).map_err(|e| e.to_string())?;
        }
    }
    state.save_accounts()?;

    Ok(count)
}

#[tauri::command]
#[specta::specta]
fn presence_batch(user_ids: Vec<f64>) -> Result<Vec<PresenceInfo>, String> {
    let ids: Vec<i64> = user_ids.into_iter().map(|id| id as i64).collect();

    let transport = ReqwestTransport::new().map_err(|e| e.to_string())?;
    let presences = fetch_presences(&transport, &ids).map_err(|e| e.to_string())?;

    Ok(presences
        .into_iter()
        .map(|presence| PresenceInfo {
            user_id: presence.user_id as f64,
            presence_type: presence.user_presence_type,
            place_id: presence.place_id,
            last_online: presence.last_online,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
fn account_update(
    user_id: f64,
    alias: String,
    group: Option<String>,
    description: String,
    saved_place_id: Option<String>,
    saved_job_id: Option<String>,
    state: State<AppState>,
) -> Result<Account, String> {
    let uid = user_id as i64;

    let updated = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        let idx = store
            .find_by_user_id(uid)
            .ok_or_else(|| "không tìm thấy tài khoản".to_string())?;
        let account = store.get_mut(idx).map_err(|e| e.to_string())?;

        account.alias = alias;
        account.description = description;
        account.group = group.filter(|value| !value.trim().is_empty());

        match saved_place_id {
            Some(value) if !value.trim().is_empty() => {
                account.set_field(SAVED_PLACE_ID_FIELD, value.trim().to_owned());
            }
            _ => {
                account.remove_field(SAVED_PLACE_ID_FIELD);
            }
        }

        match saved_job_id {
            Some(value) if !value.trim().is_empty() => {
                account.set_field(SAVED_JOB_ID_FIELD, value.trim().to_owned());
            }
            _ => {
                account.remove_field(SAVED_JOB_ID_FIELD);
            }
        }

        account.clone()
    };

    state.save_accounts()?;
    Ok(updated)
}

#[must_use]
pub fn bindings_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bindings.ts")
}

#[must_use]
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            vault_inspect,
            vault_open,
            account_list,
            account_add_cookie,
            account_add_credentials,
            account_remove,
            account_refresh,
            account_launch,
            patch_apply_fps,
            multi_roblox_toggle,
            vault_import_ram,
            presence_batch,
            account_update,
        ])
        .typ::<Account>()
}

#[must_use]
pub fn typescript_exporter() -> Typescript {
    Typescript::default()
}

pub fn export_bindings() -> Result<(), String> {
    specta_builder()
        .export(typescript_exporter(), bindings_path())
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    export_bindings().expect("không ghi được bindings TypeScript");

    let data_file = import::portable_account_file();

    let settings_file = import::portable_data_dir().join("settings.toml");
    let settings = Settings::load(&settings_file).unwrap_or_default();

    let initial_accounts = if data_file.is_file() {
        vault::read_accounts_from_path(&data_file, None)
            .map(|(accs, _)| accs)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let app_state = AppState {
        store: Mutex::new(AccountStore::from_accounts(initial_accounts)),
        mutex_guard: Mutex::new(None),
        data_file,
        settings,
        limiter: Arc::new(RateLimiter::new(Duration::from_millis(
            HTTP_MIN_INTERVAL_MS,
        ))),
    };

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("không khởi động được ứng dụng");
}
