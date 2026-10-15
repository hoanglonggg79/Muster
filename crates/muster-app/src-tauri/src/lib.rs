use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use muster_core::browser::{detect, download, login};
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
use muster_core::roblox::account_ext::{self, FollowPrivacy, PinState};
use muster_core::roblox::api::{
    fetch_authenticated_user, fetch_user_avatar_headshot, fetch_user_robux,
};
use muster_core::roblox::auth::{self as roblox_auth, AuthClient};
use muster_core::roblox::games::{FavoriteGame, GameDetails, RecentGame};
use muster_core::roblox::limiter::{RateLimiter, RetryTransport};
use muster_core::roblox::presence::{PresenceType, fetch_presences};
use muster_core::roblox::reqwest_transport::ReqwestTransport;
use muster_core::roblox::servers::{ServerPick, ServersPage};
use muster_core::roblox::vip::{extract_link_code, fetch_access_code, strip_vip_prefix};
use muster_core::roblox::{batch, games, groups, servers};
use muster_core::settings::Settings;
use muster_core::vault::{self, Branch, Format};
use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri::{AppHandle, Emitter, State};
use tauri_specta::{Builder, collect_commands};

pub const SAVED_PLACE_ID_FIELD: &str = "SavedPlaceId";
pub const SAVED_JOB_ID_FIELD: &str = "SavedJobId";
pub const HTTP_MIN_INTERVAL_MS: u64 = 200;
pub const DEFAULT_FPS_CAP: u32 = 60;

#[derive(Debug, Serialize, Type)]
pub struct AppInitState {
    pub unlock_fps: bool,
    pub max_fps: u32,
    pub multi_roblox: bool,
}

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

#[derive(Debug, Serialize, Type)]
pub struct PinInfo {
    pub enabled: bool,

    #[specta(type = f64)]
    pub unlocked_until: i64,

    pub is_unlocked: bool,
}

impl From<PinState> for PinInfo {
    fn from(state: PinState) -> Self {
        Self {
            enabled: state.enabled,
            unlocked_until: state.unlocked_until,
            is_unlocked: state.is_unlocked(),
        }
    }
}

pub struct AppState {
    pub store: Mutex<AccountStore>,
    pub mutex_guard: Mutex<Option<SingletonMutex>>,
    pub data_file: PathBuf,
    pub data_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub limiter: Arc<RateLimiter>,
}

impl AppState {
    fn save_accounts(&self) -> Result<(), String> {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        vault::write_accounts(&self.data_file, store.accounts()).map_err(|e| e.to_string())
    }

    fn save_settings(&self) -> Result<(), String> {
        let settings = self.settings.lock().map_err(|e| e.to_string())?;
        settings
            .save_json(&import::settings_json_path())
            .map_err(|e| e.to_string())
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

fn add_account_from_cookie(
    state: &AppState,
    cookie: &str,
    alias: Option<String>,
) -> Result<Account, String> {
    let cookie = roblox_auth::sanitize_cookie(cookie);
    if cookie.is_empty() {
        return Err("Cookie .ROBLOSECURITY rỗng".to_owned());
    }

    let transport = http_transport(state)?;
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
fn account_add_cookie(
    cookie: String,
    alias: Option<String>,
    state: State<AppState>,
) -> Result<Account, String> {
    add_account_from_cookie(&state, &cookie, alias)
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

    if state
        .settings
        .lock()
        .map_err(|e| e.to_string())?
        .general
        .auto_close_last_process
    {
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
fn app_init_state(state: State<AppState>) -> Result<AppInitState, String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    Ok(AppInitState {
        unlock_fps: settings.general.unlock_fps,
        max_fps: if settings.general.max_fps_value > 0 {
            settings.general.max_fps_value as u32
        } else {
            DEFAULT_FPS_CAP
        },
        multi_roblox: settings.general.enable_multi_rbx,
    })
}

#[tauri::command]
#[specta::specta]
fn patch_apply_fps(
    unlock_fps: bool,
    max_fps: Option<u32>,
    state: State<AppState>,
) -> Result<String, String> {
    let folder = find_version_folder().map_err(|e| e.to_string())?;
    let patch = PatchSettings {
        custom_client_settings: None,
        unlock_fps,
        max_fps_value: max_fps,
    };
    apply_client_settings(&folder, &patch).map_err(|e| e.to_string())?;
    {
        let mut settings = state.settings.lock().map_err(|e| e.to_string())?;
        settings.general.unlock_fps = unlock_fps;
        if let Some(fps) = max_fps {
            settings.general.max_fps_value = i32::try_from(fps).unwrap_or(i32::MAX);
        }
    }
    state.save_settings()?;
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
    } else {
        *guard = None;
    }
    {
        let mut settings = state.settings.lock().map_err(|e| e.to_string())?;
        settings.general.enable_multi_rbx = enabled;
    }
    state.save_settings()?;
    Ok(enabled)
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

fn account_csrf(
    transport: &RetryTransport<ReqwestTransport>,
    cookie: &str,
) -> Result<String, String> {
    AuthClient::new(transport)
        .csrf_token(cookie)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
fn account_check_pin(user_id: f64, state: State<AppState>) -> Result<PinInfo, String> {
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;
    let csrf = account_csrf(&transport, &cookie)?;

    let pin =
        account_ext::check_pin(&transport, &cookie, Some(&csrf)).map_err(|e| e.to_string())?;
    Ok(PinInfo::from(pin))
}

#[tauri::command]
#[specta::specta]
fn account_unlock_pin(
    user_id: f64,
    pin: String,
    state: State<AppState>,
) -> Result<PinInfo, String> {
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;
    let csrf = account_csrf(&transport, &cookie)?;

    let state_pin =
        account_ext::unlock_pin(&transport, &cookie, &csrf, &pin).map_err(|e| e.to_string())?;
    Ok(PinInfo::from(state_pin))
}

#[tauri::command]
#[specta::specta]
fn account_set_follow_privacy(
    user_id: f64,
    privacy: i32,
    state: State<AppState>,
) -> Result<(), String> {
    let privacy = FollowPrivacy::from_index(privacy)
        .ok_or_else(|| "mức quyền riêng tư không hợp lệ".to_string())?;

    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;
    let csrf = account_csrf(&transport, &cookie)?;

    account_ext::set_follow_privacy(&transport, &cookie, &csrf, privacy).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn account_set_display_name(
    user_id: f64,
    name: String,
    state: State<AppState>,
) -> Result<(), String> {
    let uid = user_id as i64;
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;
    let csrf = account_csrf(&transport, &cookie)?;

    account_ext::set_display_name(&transport, &cookie, &csrf, uid, &name)
        .map_err(|e| e.to_string())?;

    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        if let Some(index) = store.find_by_user_id(uid) {
            let account = store.get_mut(index).map_err(|e| e.to_string())?;
            account.set_field("DisplayName", name);
        }
    }

    state.save_accounts()
}

#[tauri::command]
#[specta::specta]
fn account_set_avatar(
    user_id: f64,
    avatar_json: String,
    state: State<AppState>,
) -> Result<Vec<f64>, String> {
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;
    let csrf = account_csrf(&transport, &cookie)?;

    let outcome = account_ext::set_avatar(&transport, &cookie, &csrf, &avatar_json)
        .map_err(|e| e.to_string())?;

    Ok(outcome
        .invalid_asset_ids
        .into_iter()
        .map(|id| id as f64)
        .collect())
}

#[tauri::command]
#[specta::specta]
fn account_change_password(
    user_id: f64,
    current: String,
    new_password: String,
    state: State<AppState>,
) -> Result<(), String> {
    let uid = user_id as i64;
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;
    let csrf = account_csrf(&transport, &cookie)?;

    let rotated = account_ext::change_password(&transport, &cookie, &csrf, &current, &new_password)
        .map_err(|e| e.to_string())?;

    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        if let Some(index) = store.find_by_user_id(uid) {
            let account = store.get_mut(index).map_err(|e| e.to_string())?;
            account.password = new_password;
            if let Some(token) = rotated {
                account.security_token = Some(token);
            }
        }
    }

    state.save_accounts()
}

#[tauri::command]
#[specta::specta]
fn account_change_email(
    user_id: f64,
    password: String,
    email: String,
    state: State<AppState>,
) -> Result<(), String> {
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;
    let csrf = account_csrf(&transport, &cookie)?;

    account_ext::change_email(&transport, &cookie, &csrf, &password, &email)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn account_logout_other_sessions(user_id: f64, state: State<AppState>) -> Result<(), String> {
    let uid = user_id as i64;
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;
    let csrf = account_csrf(&transport, &cookie)?;

    let rotated = account_ext::logout_other_sessions(&transport, &cookie, &csrf)
        .map_err(|e| e.to_string())?;

    if let Some(token) = rotated {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        if let Some(index) = store.find_by_user_id(uid) {
            store
                .get_mut(index)
                .map_err(|e| e.to_string())?
                .security_token = Some(token);
        }
        drop(store);
        state.save_accounts()?;
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
fn account_block_user(
    user_id: f64,
    target_id: f64,
    state: State<AppState>,
) -> Result<bool, String> {
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;
    let csrf = account_csrf(&transport, &cookie)?;

    let status = account_ext::block_user(&transport, &cookie, &csrf, target_id as i64, false)
        .map_err(|e| e.to_string())?;
    Ok((200..300).contains(&status))
}

#[tauri::command]
#[specta::specta]
fn account_unblock_user(
    user_id: f64,
    target_id: f64,
    state: State<AppState>,
) -> Result<bool, String> {
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;
    let csrf = account_csrf(&transport, &cookie)?;

    let status = account_ext::block_user(&transport, &cookie, &csrf, target_id as i64, true)
        .map_err(|e| e.to_string())?;
    Ok((200..300).contains(&status))
}

#[tauri::command]
#[specta::specta]
fn account_blocked_list(user_id: f64, state: State<AppState>) -> Result<Vec<f64>, String> {
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let transport = http_transport(&state)?;

    let body = account_ext::blocked_list(&transport, &cookie).map_err(|e| e.to_string())?;

    Ok(account_ext::parse_blocked_user_ids(&body)
        .into_iter()
        .map(|id| id as f64)
        .collect())
}

#[tauri::command]
#[specta::specta]
async fn account_unblock_everyone(user_id: f64, state: State<'_, AppState>) -> Result<u32, String> {
    let cookie = resolve_cookie(&state, Some(user_id))?;
    let limiter = Arc::clone(&state.limiter);

    tauri::async_runtime::spawn_blocking(move || {
        let transport = ReqwestTransport::new()
            .map(|transport| RetryTransport::new(transport).with_limiter(limiter))
            .map_err(|error| error.to_string())?;
        let csrf = account_csrf(&transport, &cookie)?;

        let done = account_ext::unblock_everyone(&transport, &cookie, &csrf)
            .map_err(|error| error.to_string())?;
        Ok::<u32, String>(done as u32)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[derive(Debug, Serialize, Type)]
pub struct GroupInfo {
    pub name: String,
    pub label: String,
    pub count: u32,
}

#[derive(Debug, Serialize, Clone, Type)]
pub struct BrowserStatus {
    pub chrome_for_testing: Option<String>,
    pub chrome_for_testing_version: Option<String>,
    pub ready: bool,
}

#[derive(Debug, Serialize, Clone, Type)]
pub struct BrowserProgress {
    pub phase: String,
    pub downloaded: u32,
    pub total: u32,
    pub percent: u32,
}

#[derive(Debug, Deserialize, Type)]
pub struct ThumbnailRequestInput {
    pub asset_id: f64,
    pub kind: String,
    pub size: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Type)]
pub struct ThumbnailResult {
    pub asset_id: f64,
    pub url: String,
}

pub const BROWSER_PROGRESS_EVENT: &str = "browser-download";
pub const BROWSER_DEVTOOLS_TIMEOUT_SECONDS: u64 = 30;
pub const BROWSER_LOGIN_TIMEOUT_SECONDS: u64 = 600;
pub const BROWSER_FORM_TIMEOUT_SECONDS: u64 = 60;

fn favorites_path(state: &AppState) -> PathBuf {
    state.data_dir.join(games::FAVORITES_FILE)
}

fn recent_path(state: &AppState) -> PathBuf {
    state.data_dir.join(games::RECENT_GAMES_FILE)
}

fn resolve_cookie(state: &AppState, user_id: Option<f64>) -> Result<String, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;

    let index = match user_id {
        Some(id) => store
            .find_by_user_id(id as i64)
            .ok_or_else(|| "không tìm thấy tài khoản".to_string())?,
        None => (0..store.len())
            .find(|index| {
                store
                    .get(*index)
                    .is_ok_and(|account| account.cookie().is_some())
            })
            .ok_or_else(|| "chưa có tài khoản nào đăng nhập".to_string())?,
    };

    store
        .get(index)
        .map_err(|e| e.to_string())?
        .cookie()
        .map(str::to_owned)
        .ok_or_else(|| "tài khoản không có cookie".to_string())
}

#[tauri::command]
#[specta::specta]
fn groups_list(state: State<AppState>) -> Result<Vec<GroupInfo>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;

    Ok(groups::sorted_groups(store.accounts())
        .into_iter()
        .map(|name| {
            let count = groups::members(&store, &name).len() as u32;
            let label = groups::display_name(&name).to_owned();
            GroupInfo { name, label, count }
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
fn group_rename(from: String, to: String, state: State<AppState>) -> Result<u32, String> {
    let changed = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        groups::rename_group(&mut store, &from, &to).map_err(|e| e.to_string())?
    };

    state.save_accounts()?;
    Ok(changed as u32)
}

#[tauri::command]
#[specta::specta]
fn group_assign(user_id: f64, group: String, state: State<AppState>) -> Result<(), String> {
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        let index = store
            .find_by_user_id(user_id as i64)
            .ok_or_else(|| "không tìm thấy tài khoản".to_string())?;
        groups::assign_group(&mut store, index, &group).map_err(|e| e.to_string())?;
    }

    state.save_accounts()
}

#[tauri::command]
#[specta::specta]
fn servers_list(place_id: f64, cursor: Option<String>) -> Result<ServersPage, String> {
    let transport = ReqwestTransport::new().map_err(|e| e.to_string())?;
    servers::fetch_public_servers(&transport, place_id as i64, cursor.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn servers_pick_job(
    place_id: f64,
    max_pages: u32,
    lowest_players: bool,
) -> Result<Option<String>, String> {
    let transport = ReqwestTransport::new().map_err(|e| e.to_string())?;
    let pick = if lowest_players {
        ServerPick::LowestPlayers
    } else {
        ServerPick::Random
    };

    servers::choose_job_id(&transport, place_id as i64, max_pages, pick, |count| {
        servers::random_index_below(count).unwrap_or(0)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn game_details(
    place_ids: Vec<f64>,
    user_id: Option<f64>,
    state: State<AppState>,
) -> Result<Vec<GameDetails>, String> {
    let ids: Vec<i64> = place_ids.into_iter().map(|id| id as i64).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let cookie = resolve_cookie(&state, user_id)?;
    let transport = http_transport(&state)?;

    games::fetch_place_details(&transport, &cookie, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn thumbnails_batch(requests: Vec<ThumbnailRequestInput>) -> Result<Vec<ThumbnailResult>, String> {
    if requests.is_empty() {
        return Ok(Vec::new());
    }

    let transport = ReqwestTransport::new().map_err(|e| e.to_string())?;

    let inputs: Vec<batch::ThumbnailRequest> = requests
        .into_iter()
        .map(|request| {
            let mut built = batch::ThumbnailRequest::new(request.asset_id as i64, request.kind);
            if let Some(size) = request.size {
                built = built.with_size(size);
            }
            if let Some(format) = request.format {
                built = built.with_format(format);
            }
            built
        })
        .collect();

    let results = batch::fetch_thumbnails(&transport, &inputs).map_err(|e| e.to_string())?;

    Ok(results
        .into_iter()
        .map(|(asset_id, url)| ThumbnailResult {
            asset_id: asset_id as f64,
            url,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
fn favorites_list(state: State<AppState>) -> Result<Vec<FavoriteGame>, String> {
    Ok(games::load_favorites(&favorites_path(&state)))
}

#[tauri::command]
#[specta::specta]
fn favorite_add(
    name: String,
    place_id: f64,
    private_server: Option<String>,
    state: State<AppState>,
) -> Result<Vec<FavoriteGame>, String> {
    let path = favorites_path(&state);
    let mut favorites = games::load_favorites(&path);

    let mut game = FavoriteGame::new(name, place_id as i64);
    if let Some(server) = private_server.filter(|value| !value.trim().is_empty()) {
        game = game.with_private_server(server);
    }

    let _ = games::add_favorite(&mut favorites, game);
    games::save_favorites(&path, &favorites).map_err(|e| e.to_string())?;

    Ok(favorites)
}

#[tauri::command]
#[specta::specta]
fn favorite_remove(
    place_id: f64,
    private_server: Option<String>,
    state: State<AppState>,
) -> Result<Vec<FavoriteGame>, String> {
    let path = favorites_path(&state);
    let mut favorites = games::load_favorites(&path);

    let _ = games::remove_favorite(
        &mut favorites,
        place_id as i64,
        private_server.as_deref().unwrap_or(""),
    );
    games::save_favorites(&path, &favorites).map_err(|e| e.to_string())?;

    Ok(favorites)
}

#[tauri::command]
#[specta::specta]
fn recent_games(state: State<AppState>) -> Result<Vec<RecentGame>, String> {
    Ok(games::load_recent(&recent_path(&state)))
}

#[tauri::command]
#[specta::specta]
fn recent_push(
    name: String,
    place_id: f64,
    state: State<AppState>,
) -> Result<Vec<RecentGame>, String> {
    let path = recent_path(&state);
    let mut recent = games::load_recent(&path);
    let max = state
        .settings
        .lock()
        .map_err(|e| e.to_string())?
        .general
        .max_recent_games
        .max(1) as usize;

    games::push_recent(&mut recent, RecentGame::new(name, place_id as i64), max);
    games::save_recent(&path, &recent).map_err(|e| e.to_string())?;

    Ok(recent)
}

#[tauri::command]
#[specta::specta]
fn browser_status() -> BrowserStatus {
    let data_dir = import::app_data_dir();
    let chrome_for_testing = detect::installed_chrome_for_testing(&data_dir)
        .map(|path| path.to_string_lossy().into_owned());

    BrowserStatus {
        ready: chrome_for_testing.is_some(),
        chrome_for_testing,
        chrome_for_testing_version: detect::installed_version(&data_dir),
    }
}

fn to_u32(value: f64) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    value.min(f64::from(u32::MAX)) as u32
}

fn emit_browser_progress(app: &AppHandle, phase: &str, downloaded: f64, total: f64) {
    let percent = if total > 0.0 {
        to_u32((downloaded / total * 100.0).clamp(0.0, 100.0))
    } else {
        0
    };

    let _ = app.emit(
        BROWSER_PROGRESS_EVENT,
        BrowserProgress {
            phase: phase.to_owned(),
            downloaded: to_u32(downloaded),
            total: to_u32(total),
            percent,
        },
    );
}

fn download_chromium_blocking(app: &AppHandle) -> Result<String, String> {
    let data_dir = import::ensure_app_data_dir();
    let release = download::fetch_stable_release().map_err(|e| e.to_string())?;

    let version_dir = detect::chrome_for_testing_root(&data_dir).join(&release.version);
    if detect::chrome_for_testing_binary(&version_dir).is_file() {
        emit_browser_progress(app, "granting", 1.0, 1.0);
        download::grant_app_container_access(&version_dir).map_err(|e| e.to_string())?;
        emit_browser_progress(app, "ready", 1.0, 1.0);
        return Ok(version_dir.to_string_lossy().into_owned());
    }

    let archive = data_dir.join("browser").join("chrome-win64.zip");
    if let Some(parent) = archive.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    emit_browser_progress(app, "connecting", 0.0, 0.0);

    let progress_app = app.clone();
    let mut last_progress = std::time::Instant::now() - Duration::from_secs(1);
    download::download_to_file(&release.url, &archive, move |downloaded, total| {
        if last_progress.elapsed() >= Duration::from_millis(100) || downloaded == total {
            emit_browser_progress(
                &progress_app,
                "downloading",
                downloaded as f64,
                total as f64,
            );
            last_progress = std::time::Instant::now();
        }
    })
    .map_err(|e| e.to_string())?;

    if let Some(expected) = &release.sha256 {
        if !download::verify_sha256(&archive, expected) {
            let _ = std::fs::remove_file(&archive);
            return Err(
                "Gói Chromium tải về không khớp SHA-256; đã xoá để không chạy mã lạ".to_owned(),
            );
        }
    }

    emit_browser_progress(app, "extracting", 1.0, 1.0);

    let binary = download::extract_chrome(&archive, &version_dir).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&archive);

    emit_browser_progress(app, "granting", 1.0, 1.0);
    download::grant_app_container_access(&version_dir).map_err(|e| e.to_string())?;

    emit_browser_progress(app, "ready", 1.0, 1.0);
    Ok(binary.to_string_lossy().into_owned())
}

#[tauri::command]
#[specta::specta]
async fn browser_download_chromium(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || download_chromium_blocking(&app))
        .await
        .map_err(|e| e.to_string())?
}

fn emit_login_phase(app: &AppHandle, phase: &login::LoginPhase) {
    let label = match phase {
        login::LoginPhase::Launching => "launching",
        login::LoginPhase::WaitingLogin => "waiting-login",
        login::LoginPhase::FillingForm => "filling-form",
        login::LoginPhase::ManualLoginRequired(_) => "manual-login",
    };

    emit_browser_progress(app, label, 0.0, 0.0);
}

fn run_browser_login(
    app: &AppHandle,
    credentials: Option<(String, String)>,
) -> Result<String, String> {
    let data_dir = import::ensure_app_data_dir();

    let executable = detect::installed_chrome_for_testing(&data_dir)
        .ok_or_else(|| login::NO_BROWSER_MESSAGE.to_owned())?;

    let request = login::LoginRequest {
        executable: &executable,
        data_dir: &data_dir,
        credentials: credentials
            .as_ref()
            .map(|(username, password)| (username.as_str(), password.as_str())),
    };

    let timings = login::LoginTimings {
        devtools: Duration::from_secs(BROWSER_DEVTOOLS_TIMEOUT_SECONDS),
        form: Duration::from_secs(BROWSER_FORM_TIMEOUT_SECONDS),
        login: Duration::from_secs(BROWSER_LOGIN_TIMEOUT_SECONDS),
    };

    login::run_login(&request, &timings, &mut |phase| {
        emit_login_phase(app, &phase)
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
async fn browser_open_login(
    app: AppHandle,
    state: State<'_, AppState>,
    username: Option<String>,
    password: Option<String>,
) -> Result<Account, String> {
    let credentials = match (username, password) {
        (Some(user), Some(pass)) if !user.trim().is_empty() && !pass.is_empty() => {
            Some((user.trim().to_owned(), pass))
        }
        _ => None,
    };

    let cookie = tauri::async_runtime::spawn_blocking(move || run_browser_login(&app, credentials))
        .await
        .map_err(|e| e.to_string())??;

    add_account_from_cookie(&state, &cookie, None)
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
            account_check_pin,
            account_unlock_pin,
            account_set_follow_privacy,
            account_set_display_name,
            account_set_avatar,
            account_change_password,
            account_change_email,
            account_logout_other_sessions,
            account_block_user,
            account_unblock_user,
            account_blocked_list,
            account_unblock_everyone,
            app_init_state,
            groups_list,
            group_rename,
            group_assign,
            servers_list,
            servers_pick_job,
            game_details,
            thumbnails_batch,
            favorites_list,
            favorite_add,
            favorite_remove,
            recent_games,
            recent_push,
            browser_status,
            browser_download_chromium,
            browser_open_login,
        ])
        .typ::<Account>()
        .typ::<BrowserProgress>()
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

    import::migrate_legacy_portable_data();

    let data_dir = import::ensure_app_data_dir();
    let data_file = data_dir.join(import::ACCOUNT_FILE);

    let settings_path = import::settings_json_path();
    let settings = Settings::load_preferred(&settings_path, &import::settings_toml_legacy_path());
    if !settings_path.is_file() {
        let _ = settings.save_json(&settings_path);
    }

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
        data_dir,
        settings: Mutex::new(settings),
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
