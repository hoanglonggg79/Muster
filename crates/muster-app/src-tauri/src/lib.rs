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

/// The hosts Roblox may set `.ROBLOSECURITY` on. WebView2 answers `GetCookies` per URI, and the
/// login flow legitimately lands on `www.roblox.com`, bare `roblox.com` or `auth.roblox.com`; the
/// cookie itself is scoped to `.roblox.com`, so we ask for the two stable entry points and take
/// whichever returns it first.
#[cfg(windows)]
const COOKIE_PROBE_URIS: [&str; 2] = ["https://www.roblox.com", "https://roblox.com"];

/// Read `.ROBLOSECURITY` out of a WebView2 cookie list.
///
/// Safe wrapper: the COM calls inside need `unsafe`, but every one is checked for HRESULT failure
/// and the pointers are only dereferenced while `list` is alive.
#[cfg(windows)]
#[must_use]
fn roblosecurity_from_cookie_list(
    list: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2CookieList,
) -> Option<String> {
    unsafe { roblosecurity_from_cookie_list_inner(list) }
}

/// # Safety
/// The caller must be on the UI thread with `list` still alive (only true inside the `GetCookies`
/// callback).
#[cfg(windows)]
#[must_use]
unsafe fn roblosecurity_from_cookie_list_inner(
    list: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2CookieList,
) -> Option<String> {
    unsafe {
        let mut count = 0u32;
        list.Count(&mut count).ok()?;

        for i in 0..count {
            let Ok(cookie) = list.GetValueAtIndex(i) else {
                continue;
            };
            let mut name_pwstr = windows_core::PWSTR::null();
            let mut value_pwstr = windows_core::PWSTR::null();
            if cookie.Name(&mut name_pwstr).is_err() || cookie.Value(&mut value_pwstr).is_err() {
                continue;
            }

            let name = name_pwstr.to_string().unwrap_or_default();
            let value = value_pwstr.to_string().unwrap_or_default();
            if name == ".ROBLOSECURITY" && !value.trim().is_empty() {
                return Some(value);
            }
        }

        None
    }
}

/// Query every probe URI and send the first `.ROBLOSECURITY` found through `tx`.
///
/// Each `GetCookies` is asynchronous, so the sender is shared through an `Arc<Mutex<Option<_>>>`
/// and `take()`n exactly once. Results are only *sent*, never dropped: a probe that finds nothing
/// simply leaves the sender in place for the next URI, and the caller may reuse the same channel on
/// a later poll.
#[cfg(windows)]
fn extract_roblox_security_cookie_tx(
    window: &tauri::WebviewWindow,
    tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<String>>>>,
) -> Result<(), String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use windows_core::Interface;

    window
        .with_webview(move |wv| unsafe {
            let Ok(controller) = wv.controller().CoreWebView2() else {
                return;
            };
            let Ok(wv2) = controller.cast::<ICoreWebView2_2>() else {
                return;
            };
            let Ok(manager) = wv2.CookieManager() else {
                return;
            };

            for uri in COOKIE_PROBE_URIS {
                if tx.lock().is_ok_and(|guard| guard.is_none()) {
                    // Already answered by an earlier URI in this batch.
                    break;
                }

                let tx = Arc::clone(&tx);
                let handler = webview2_com::GetCookiesCompletedHandler::create(Box::new(
                    move |_res, list| {
                        let Some(list) = list else {
                            return Ok(());
                        };
                        let Some(value) = roblosecurity_from_cookie_list(&list) else {
                            return Ok(());
                        };

                        if let Ok(mut guard) = tx.lock() {
                            if let Some(sender) = guard.take() {
                                let _ = sender.send(value);
                            }
                        }
                        Ok(())
                    },
                ));

                let wide: Vec<u16> = uri.encode_utf16().chain(std::iter::once(0)).collect();
                let uri_pcwstr = windows_core::PCWSTR(wide.as_ptr());
                let _ = manager.GetCookies(uri_pcwstr, &handler);
            }
        })
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn extract_roblox_security_cookie_tx(
    _window: &tauri::WebviewWindow,
    _tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<String>>>>,
) -> Result<(), String> {
    Err("WebView2 chỉ hỗ trợ trên Windows".to_string())
}

/// Ask the UI thread for one cookie probe. The returned receiver stays valid across polls because
/// callers reuse the sender through [`PendingCookieProbe`].
fn request_cookie_on_main_thread(
    app: &AppHandle,
    label: &str,
    tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<String>>>>,
) -> Result<(), String> {
    use tauri::Manager;
    let label = label.to_string();
    let app_handle = app.clone();

    app.run_on_main_thread(move || {
        if let Some(window) = app_handle.get_webview_window(&label) {
            let _ = extract_roblox_security_cookie_tx(&window, tx);
        }
        // Window gone: keep the sender so the caller's receiver is not spuriously disconnected.
    })
    .map_err(|error| error.to_string())
}

/// A cookie probe whose sender is retained between polls.
///
/// The old implementation created a fresh `oneshot` channel every 800 ms and abandoned the receiver
/// after 1 s. Because `Sender::send` fails silently once the receiver is dropped, a slow WebView2
/// callback lost the cookie outright and the value had to be re-fetched. Here the receiver is created
/// once and the sender is `take()`n only when a value is actually delivered, so a late callback still
/// lands.
pub struct PendingCookieProbe {
    rx: tokio::sync::oneshot::Receiver<String>,
    tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<String>>>>,
    answered: bool,
}

impl PendingCookieProbe {
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::oneshot::channel();
        Self {
            rx,
            tx: Arc::new(Mutex::new(Some(tx))),
            answered: false,
        }
    }

    fn sender(&self) -> Arc<Mutex<Option<tokio::sync::oneshot::Sender<String>>>> {
        Arc::clone(&self.tx)
    }

    /// Whether a cookie has already been delivered.
    #[must_use]
    pub fn is_answered(&self) -> bool {
        self.answered
    }

    /// Non-blocking check for a delivered cookie. Used by the polling loop so a value that arrives
    /// between ticks is not missed.
    pub fn try_take(&mut self) -> Option<String> {
        if self.answered {
            return None;
        }

        let value = match self.rx.try_recv() {
            Ok(value) => value,
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => return None,
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                self.answered = true;
                return None;
            }
        };

        self.answered = true;
        Some(value)
    }
}

impl Default for PendingCookieProbe {
    fn default() -> Self {
        Self::new()
    }
}

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
    pub store: Arc<Mutex<AccountStore>>,
    pub mutex_guard: Mutex<Option<SingletonMutex>>,
    pub data_file: PathBuf,
    pub data_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub limiter: Arc<RateLimiter>,
    /// Bumped every time `browser_open_login` starts. An older invocation whose token no longer
    /// matches has been superseded and must bail out instead of running two loops over the same
    /// window (which used to report a phantom "window closed" error on the stale one).
    pub login_generation: Arc<std::sync::atomic::AtomicU64>,
}

impl AppState {
    /// Start a new login generation, superseding any loop still in flight.
    fn next_login_generation(&self) -> u64 {
        self.login_generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1
    }

    fn is_current_login(&self, generation: u64) -> bool {
        self.login_generation
            .load(std::sync::atomic::Ordering::SeqCst)
            == generation
    }

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

/// The parts of [`AppState`] that account creation needs, owned so the whole (blocking) job can be
/// moved onto `spawn_blocking`. `AppState` itself cannot be moved: it is managed by Tauri and the
/// borrow of `State<'_, AppState>` cannot outlive the command.
#[derive(Clone)]
pub struct AccountWriter {
    pub store: Arc<Mutex<AccountStore>>,
    pub data_file: PathBuf,
    pub limiter: Arc<RateLimiter>,
}

impl AccountWriter {
    fn from_state(state: &AppState) -> Self {
        Self {
            store: Arc::clone(&state.store),
            data_file: state.data_file.clone(),
            limiter: Arc::clone(&state.limiter),
        }
    }

    fn http_transport(&self) -> Result<RetryTransport<ReqwestTransport>, String> {
        ReqwestTransport::new()
            .map(|transport| RetryTransport::new(transport).with_limiter(Arc::clone(&self.limiter)))
            .map_err(|error| error.to_string())
    }

    fn save_accounts(&self) -> Result<(), String> {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        vault::write_accounts(&self.data_file, store.accounts()).map_err(|e| e.to_string())
    }
}

/// Blocking: performs network calls and, through `reqwest::blocking`, owns a Tokio runtime of its
/// own. MUST NOT be called directly from an `async fn` — dropping the `ReqwestTransport` inside an
/// async context panics with "Cannot drop a runtime in a context where blocking is not allowed."
/// Callers on the async side must go through `spawn_blocking` (see `add_account_from_cookie`).
fn add_account_from_cookie_blocking(
    writer: &AccountWriter,
    cookie: &str,
    alias: Option<String>,
    password: Option<String>,
) -> Result<Account, String> {
    let cookie = roblox_auth::sanitize_cookie(cookie);
    if cookie.is_empty() {
        return Err("Cookie .ROBLOSECURITY rỗng".to_owned());
    }

    let transport = writer.http_transport()?;
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
        password: password.unwrap_or_default(),
        group: None,
        user_id: user_info.id,
        fields,
        last_attempted_refresh: now_local_json_net(),
        browser_tracker_id: Some(tracker_id),
        extra: BTreeMap::new(),
    };

    let index = {
        let mut store = writer.store.lock().map_err(|e| e.to_string())?;
        let (_, index) = store.add(account).map_err(|e| e.to_string())?;
        index
    };
    writer.save_accounts()?;

    let store = writer.store.lock().map_err(|e| e.to_string())?;
    store.get(index).cloned().map_err(|e| e.to_string())
}

/// Async entry point: hands the blocking work to a dedicated blocking thread so no Tokio runtime is
/// ever constructed or dropped on an async worker.
async fn add_account_from_cookie(
    writer: AccountWriter,
    cookie: String,
    alias: Option<String>,
    password: Option<String>,
) -> Result<Account, String> {
    tauri::async_runtime::spawn_blocking(move || {
        add_account_from_cookie_blocking(&writer, &cookie, alias, password)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
#[specta::specta]
fn account_add_cookie(
    cookie: String,
    alias: Option<String>,
    state: State<AppState>,
) -> Result<Account, String> {
    add_account_from_cookie_blocking(&AccountWriter::from_state(&state), &cookie, alias, None)
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

/// How often the login loop asks WebView2 for cookies.
pub const COOKIE_POLL_INTERVAL_MS: u64 = 800;

/// How many times the auto-fill script is re-run while the login page boots (1s apart).
pub const LOGIN_AUTOFILL_ATTEMPTS: usize = 15;

/// Build the credential-filling script.
///
/// Roblox's login form is React-controlled: assigning `input.value` updates the DOM but not React's
/// internal value tracker, so React re-renders and wipes the text, and the form submits empty. The
/// fix is the standard React workaround: call the *native* value setter through
/// `Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set` so React's tracker
/// observes the change, then dispatch an `input` event. React's synthetic `onChange` listens for
/// `input`, not `change`, so both are dispatched for good measure.
#[must_use]
fn autofill_credentials_script(user: &str, pass: &str) -> String {
    format!(
        r#"(() => {{
            const setNative = (el, value) => {{
                const setter = Object.getOwnPropertyDescriptor(
                    HTMLInputElement.prototype, 'value'
                ).set;
                setter.call(el, value);
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
            }};
            const u = document.querySelector('#login-username')
                || document.querySelector('input[name="username"]')
                || document.querySelector('input[autocomplete="username"]');
            const p = document.querySelector('#login-password')
                || document.querySelector('input[name="password"]')
                || document.querySelector('input[type="password"]');
            let filled = false;
            if (u && !u.value) {{ setNative(u, {user_json}); filled = true; }}
            if (p && !p.value) {{ setNative(p, {pass_json}); filled = true; }}
            return filled ? 'true' : 'false';
        }})()"#,
        user_json = serde_json::to_string(user).unwrap_or_default(),
        pass_json = serde_json::to_string(pass).unwrap_or_default(),
    )
}

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

pub const LOGIN_WINDOW_LABEL: &str = "roblox-login";

#[tauri::command]
#[specta::specta]
fn browser_status() -> BrowserStatus {
    BrowserStatus {
        ready: true,
        chrome_for_testing: None,
        chrome_for_testing_version: Some("Edge WebView2 (Native Windows)".to_string()),
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

#[tauri::command]
#[specta::specta]
async fn browser_download_chromium() -> Result<String, String> {
    Ok("WebView2 sẵn sàng".to_string())
}

#[tauri::command]
#[specta::specta]
async fn browser_open_login(
    app: AppHandle,
    state: State<'_, AppState>,
    username: Option<String>,
    password: Option<String>,
) -> Result<Account, String> {
    use tauri::Manager;

    let saved_password = password.clone();
    let url = tauri::Url::parse("https://www.roblox.com/login").map_err(|e| e.to_string())?;

    // Supersede any login loop still polling: it will notice the generation changed and exit
    // quietly instead of racing us for the window.
    let generation = state.next_login_generation();

    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        let _ = existing.close();
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    emit_browser_progress(&app, "launching", 0.0, 0.0);

    let window = tauri::WebviewWindowBuilder::new(
        &app,
        LOGIN_WINDOW_LABEL,
        tauri::WebviewUrl::External(url),
    )
    .title("Đăng nhập Roblox — Muster")
    .inner_size(860.0, 780.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    emit_browser_progress(&app, "waiting-login", 0.0, 0.0);

    if let (Some(user), Some(pass)) = (
        username.filter(|u| !u.trim().is_empty()),
        password.filter(|p| !p.is_empty()),
    ) {
        let w_clone = window.clone();
        tauri::async_runtime::spawn(async move {
            for _ in 0..LOGIN_AUTOFILL_ATTEMPTS {
                tokio::time::sleep(Duration::from_millis(1000)).await;

                // Only stop once the renderer actually reports success, so a page that is still
                // loading gets retried instead of silently giving up.
                // `eval_with_callback` takes a `Fn`, so the sender is shared and `take()`n once.
                let (tx, rx) = tokio::sync::oneshot::channel();
                let tx = Arc::new(Mutex::new(Some(tx)));
                let script = autofill_credentials_script(&user, &pass);
                if w_clone
                    .eval_with_callback(script, move |result| {
                        if let Ok(mut guard) = tx.lock() {
                            if let Some(tx) = guard.take() {
                                let _ = tx.send(result);
                            }
                        }
                    })
                    .is_err()
                {
                    // Window is gone; nothing left to fill.
                    break;
                }

                match rx.await {
                    // The script returns "true" only when it could fill at least one field.
                    Ok(result) if result.trim() == "true" => break,
                    Ok(_) => continue,
                    // Window closed while the script was running.
                    Err(_) => break,
                }
            }
        });
    }

    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(BROWSER_LOGIN_TIMEOUT_SECONDS);
    let mut probe = PendingCookieProbe::new();

    let cookie: String = loop {
        // Sleep briefly before each poll to avoid hammering the main thread.
        tokio::time::sleep(Duration::from_millis(COOKIE_POLL_INTERVAL_MS)).await;

        // A newer login superseded us: exit without touching the window it now owns.
        if !state.is_current_login(generation) {
            return Err("Lần đăng nhập này đã bị lần đăng nhập mới hơn thay thế.".to_string());
        }

        if start_time.elapsed() > timeout {
            let _ = window.close();
            return Err("Hết thời gian chờ đăng nhập (timeout).".to_string());
        }

        // Check if the window is still alive before requesting cookies.
        if app.get_webview_window(LOGIN_WINDOW_LABEL).is_none() {
            // Window was closed by user before we got the cookie.
            return Err("Cửa sổ đăng nhập đã bị đóng trước khi hoàn tất.".to_string());
        }

        // A cookie may have arrived since the previous tick.
        if let Some(cookie) = probe.try_take() {
            let _ = window.close();
            break cookie;
        }

        // Ask the main thread to probe both hosts. The sender is retained inside `probe`, so a slow
        // WebView2 callback still delivers on a later tick instead of being dropped.
        if let Err(error) = request_cookie_on_main_thread(&app, LOGIN_WINDOW_LABEL, probe.sender())
        {
            return Err(format!("không gửi được yêu cầu đọc cookie: {error}"));
        }
    };

    emit_browser_progress(&app, "ready", 1.0, 1.0);

    // `add_account_from_cookie` performs blocking HTTP via `reqwest::blocking`, which owns a Tokio
    // runtime. It must run on a blocking thread: dropping that runtime on an async worker panics.
    add_account_from_cookie(
        AccountWriter::from_state(&state),
        cookie,
        None,
        saved_password,
    )
    .await
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
        store: Arc::new(Mutex::new(AccountStore::from_accounts(initial_accounts))),
        mutex_guard: Mutex::new(None),
        data_file,
        data_dir,
        settings: Mutex::new(settings),
        limiter: Arc::new(RateLimiter::new(Duration::from_millis(
            HTTP_MIN_INTERVAL_MS,
        ))),
        login_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
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
