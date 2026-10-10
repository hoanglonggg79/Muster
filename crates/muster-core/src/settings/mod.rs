//! # `settings` — cấu hình của ứng dụng + migration từ `RAMSettings.ini`
//!
//! ## Vì sao giữ nguyên tên khoá và tên section của RAM
//!
//! Nguyên tắc bất biến #2: người dùng RAM phải chuyển sang được mà không phải cấu hình lại.
//! Một phần của việc đó là để người dùng **tự sửa file cấu hình như trước đây**. Nếu đổi
//! `WebServerPort` thành `web_server_port` thì mọi hướng dẫn, ghi chú và thói quen cũ đều hỏng.
//! Vì vậy file TOML giữ nguyên `[WebServer]` và `WebServerPort`.
//!
//! ## Default ở đây lấy từ đâu
//!
//! Mọi giá trị mặc định trong module này chép từ chính `RAM_Source/`, chủ yếu là
//! `AccountManager.cs:121-165` (khối `if (!X.Exists(...)) X.Set(...)`) và vài chỗ trong
//! `Forms/ServerList.cs` cho section `Watcher`. Không có giá trị nào tự nghĩ ra.
//!
//! ## Điểm đã phát hiện khi đối chiếu
//!
//! - `ROADMAP.md` mục 4.4 ghi khoá là `ShuffleJobID`, nhưng `AccountManager.cs:2079` ghi ra
//!   **`ShuffleJobId`** (chữ `d` thường). Khoá thật là `ShuffleJobId`; khi đọc vẫn chấp nhận cả
//!   hai cách viết.
//! - Danh sách khoá thật **nhiều hơn 16 khoá** mà mục 4.4 liệt kê. Migration ở đây đọc hết,
//!   và mọi khoá không mô hình hoá được vẫn được giữ nguyên trong [`Section::other`].
//! - `RobloxProcess.cs:81` đọc khoá `" ExitIfNoConnection"` với **dấu cách ở đầu** — lỗi đánh máy
//!   trong RAM. Vô hại vì parser INI `Trim()` khoá, và parser ở đây cũng vậy.

pub mod ini;

pub use ini::IniFile;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Lỗi của lớp settings.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// Lỗi đọc/ghi file.
    #[error("lỗi vào/ra: {0}")]
    Io(#[from] std::io::Error),

    /// TOML không đọc được.
    #[error("TOML không hợp lệ: {0}")]
    TomlRead(#[from] toml::de::Error),

    /// Không tuần tự hoá được ra TOML.
    #[error("không tuần tự hoá được TOML: {0}")]
    TomlWrite(#[from] toml::ser::Error),
}

/// Các khoá chưa được mô hình hoá của một section.
///
/// Giữ nguyên dạng chuỗi để không mất dữ liệu của người dùng (nguyên tắc bất biến #2).
pub type Section = BTreeMap<String, String>;

/// Toàn bộ cấu hình, tương ứng một `RAMSettings.ini`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Section `[General]`.
    #[serde(rename = "General")]
    pub general: General,
    /// Section `[Developer]`.
    #[serde(rename = "Developer")]
    pub developer: Developer,
    /// Section `[WebServer]`.
    #[serde(rename = "WebServer")]
    pub web_server: WebServer,
    /// Section `[AccountControl]`.
    #[serde(rename = "AccountControl")]
    pub account_control: AccountControl,
    /// Section `[Watcher]`.
    #[serde(rename = "Watcher")]
    pub watcher: Watcher,
    /// Section `[Prompts]` — cờ "đừng hỏi lại", khoá là hash do RAM sinh.
    #[serde(rename = "Prompts")]
    pub prompts: Section,
    /// Mọi section không nằm trong danh sách trên.
    #[serde(rename = "OtherSections")]
    pub other_sections: BTreeMap<String, Section>,
}

/// Section `[General]` — `AccountManager.cs:121-148`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct General {
    /// `CheckForUpdates` — mặc định `true`.
    pub check_for_updates: bool,
    /// `AccountJoinDelay` — số giây chờ giữa hai lần join. Mặc định `8`.
    pub account_join_delay: i32,
    /// `AsyncJoin` — mặc định `false`.
    pub async_join: bool,
    /// `DisableAgingAlert` — mặc định `false`.
    pub disable_aging_alert: bool,
    /// `SavePasswords` — lưu mật khẩu vào vault. Mặc định `true`.
    pub save_passwords: bool,
    /// `ServerRegionFormat` — mặc định `"<city>, <countryCode>"`.
    pub server_region_format: String,
    /// `MaxRecentGames` — mặc định `8`.
    pub max_recent_games: i32,
    /// `ShuffleChoosesLowestServer` — mặc định `false`.
    pub shuffle_chooses_lowest_server: bool,
    /// `ShufflePageCount` — mặc định `5`.
    pub shuffle_page_count: i32,
    /// `IPApiLink` — mặc định `"http://ip-api.com/json/<ip>"`.
    ///
    /// RAM viết hoa cả `IP` nên không thể dùng `rename_all` mà phải khai tay.
    #[serde(rename = "IPApiLink", alias = "IpApiLink")]
    pub ip_api_link: String,
    /// `WindowScale` — mặc định `1.0`.
    pub window_scale: f32,
    /// `ScaleFonts` — mặc định `true`.
    pub scale_fonts: bool,
    /// `AutoCookieRefresh` — mặc định `true`.
    pub auto_cookie_refresh: bool,
    /// `AutoCloseLastProcess` — mặc định `true`.
    pub auto_close_last_process: bool,
    /// `ShowPresence` — mặc định `true`.
    pub show_presence: bool,
    /// `PresenceUpdateRate` — mặc định `5`.
    pub presence_update_rate: i32,
    /// `UnlockFPS` — mặc định `false`.
    #[serde(rename = "UnlockFPS", alias = "UnlockFps")]
    pub unlock_fps: bool,
    /// `MaxFPSValue` — mặc định `120`.
    #[serde(rename = "MaxFPSValue", alias = "MaxFpsValue")]
    pub max_fps_value: i32,
    /// `UseCefSharpBrowser` — mặc định `false`.
    pub use_cef_sharp_browser: bool,
    /// `CustomClientSettings` — đường dẫn file tuỳ biến, mặc định không có.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_client_settings: Option<String>,
    /// `EnableMultiRbx` — mặc định `false` (rủi ro #1 nên để TẮT).
    pub enable_multi_rbx: bool,
    /// `ShuffleJobId` — mặc định `false`. Xem ghi chú đầu module về cách viết hoa.
    #[serde(alias = "ShuffleJobID")]
    pub shuffle_job_id: bool,
    /// Các khoá `[General]` chưa mô hình hoá.
    #[serde(flatten)]
    pub other: Section,
}

impl Default for General {
    fn default() -> Self {
        Self {
            check_for_updates: true,
            account_join_delay: 8,
            async_join: false,
            disable_aging_alert: false,
            save_passwords: true,
            server_region_format: "<city>, <countryCode>".to_owned(),
            max_recent_games: 8,
            shuffle_chooses_lowest_server: false,
            shuffle_page_count: 5,
            ip_api_link: "http://ip-api.com/json/<ip>".to_owned(),
            window_scale: 1.0,
            scale_fonts: true,
            auto_cookie_refresh: true,
            auto_close_last_process: true,
            show_presence: true,
            presence_update_rate: 5,
            unlock_fps: false,
            max_fps_value: 120,
            use_cef_sharp_browser: false,
            custom_client_settings: None,
            enable_multi_rbx: false,
            shuffle_job_id: false,
            other: Section::new(),
        }
    }
}

/// Section `[Developer]` — `AccountManager.cs:150-151`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Developer {
    /// `DevMode` — mặc định `false`. Gate cho mọi tính năng nguy hiểm (nguyên tắc bất biến #7).
    pub dev_mode: bool,
    /// `EnableWebServer` — mặc định `false`.
    pub enable_web_server: bool,
    /// Các khoá `[Developer]` chưa mô hình hoá.
    #[serde(flatten)]
    pub other: Section,
}

/// Section `[WebServer]` — `AccountManager.cs:153-160`.
///
/// `Debug` viết tay vì `password` là secret.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct WebServer {
    /// `WebServerPort` — mặc định `7963`.
    pub web_server_port: u16,
    /// `AllowGetCookie` — mặc định `false`.
    pub allow_get_cookie: bool,
    /// `AllowGetAccounts` — mặc định `false`.
    pub allow_get_accounts: bool,
    /// `AllowLaunchAccount` — mặc định `false`.
    pub allow_launch_account: bool,
    /// `AllowAccountEditing` — mặc định `false`.
    pub allow_account_editing: bool,
    /// `Password` — mật khẩu gate của local API. **Là secret.** RAM yêu cầu tối thiểu 6 ký tự.
    pub password: String,
    /// `EveryRequestRequiresPassword` — mặc định `false`.
    pub every_request_requires_password: bool,
    /// `AllowExternalConnections` — mặc định `false` (nguyên tắc bất biến #8).
    pub allow_external_connections: bool,
    /// Các khoá `[WebServer]` chưa mô hình hoá.
    #[serde(flatten)]
    pub other: Section,
}

impl Default for WebServer {
    fn default() -> Self {
        Self {
            web_server_port: 7963,
            allow_get_cookie: false,
            allow_get_accounts: false,
            allow_launch_account: false,
            allow_account_editing: false,
            password: String::new(),
            every_request_requires_password: false,
            allow_external_connections: false,
            other: Section::new(),
        }
    }
}

impl std::fmt::Debug for WebServer {
    /// Bản `Debug` đã khử mật khẩu của local API.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebServer")
            .field("web_server_port", &self.web_server_port)
            .field("allow_get_cookie", &self.allow_get_cookie)
            .field("allow_get_accounts", &self.allow_get_accounts)
            .field("allow_launch_account", &self.allow_launch_account)
            .field("allow_account_editing", &self.allow_account_editing)
            .field(
                "password",
                &if self.password.is_empty() {
                    "<rỗng>"
                } else {
                    "<đã ẩn>"
                },
            )
            .field(
                "every_request_requires_password",
                &self.every_request_requires_password,
            )
            .field(
                "allow_external_connections",
                &self.allow_external_connections,
            )
            .finish_non_exhaustive()
    }
}

/// Section `[AccountControl]` — `AccountManager.cs:162-165`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct AccountControl {
    /// `AllowExternalConnections` — mặc định `false`.
    pub allow_external_connections: bool,
    /// `RelaunchDelay` — mặc định `60`.
    pub relaunch_delay: i32,
    /// `LauncherDelayNumber` — mặc định `9`.
    pub launcher_delay_number: i32,
    /// `NexusPort` — mặc định `5242`.
    pub nexus_port: u16,
    /// Các khoá `[AccountControl]` chưa mô hình hoá.
    #[serde(flatten)]
    pub other: Section,
}

impl Default for AccountControl {
    fn default() -> Self {
        Self {
            allow_external_connections: false,
            relaunch_delay: 60,
            launcher_delay_number: 9,
            nexus_port: 5242,
            other: Section::new(),
        }
    }
}

/// Section `[Watcher]`.
///
/// Mặc định lấy từ `Forms/ServerList.cs:42-61`, nơi RAM đặt giá trị cho section này.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Watcher {
    /// `Enabled` — bật bộ theo dõi tiến trình. Mặc định `false`.
    pub enabled: bool,
    /// `ExitOnBeta` — mặc định `false`.
    pub exit_on_beta: bool,
    /// `ExitIfNoConnection` — mặc định `false`.
    pub exit_if_no_connection: bool,
    /// `SaveWindowPositions` — mặc định `false`.
    pub save_window_positions: bool,
    /// `VerifyDataModel` — mặc định `true` (`ServerList.cs:42`).
    pub verify_data_model: bool,
    /// `IgnoreExistingProcesses` — mặc định `true` (`ServerList.cs:43`).
    pub ignore_existing_processes: bool,
    /// `CloseRbxWindowTitle` — mặc định `false`.
    pub close_rbx_window_title: bool,
    /// `CloseRbxMemory` — mặc định `false`.
    pub close_rbx_memory: bool,
    /// `ExpectedWindowTitle` — mặc định `"Roblox"` (`ServerList.cs:44`).
    pub expected_window_title: String,
    /// `MemoryLowValue` — mặc định `200` (`ServerList.cs:57`).
    pub memory_low_value: i32,
    /// `NoConnectionTimeout` — mặc định `60` (`ServerList.cs:58`).
    pub no_connection_timeout: i32,
    /// `ScanInterval` — mặc định `6` (`ServerList.cs:60`).
    pub scan_interval: i32,
    /// `ReadInterval` — mặc định `250` (`ServerList.cs:61`).
    pub read_interval: i32,
    /// Các khoá `[Watcher]` chưa mô hình hoá.
    #[serde(flatten)]
    pub other: Section,
}

impl Default for Watcher {
    fn default() -> Self {
        Self {
            enabled: false,
            exit_on_beta: false,
            exit_if_no_connection: false,
            save_window_positions: false,
            verify_data_model: true,
            ignore_existing_processes: true,
            close_rbx_window_title: false,
            close_rbx_memory: false,
            expected_window_title: "Roblox".to_owned(),
            memory_low_value: 200,
            no_connection_timeout: 60,
            scan_interval: 6,
            read_interval: 250,
            other: Section::new(),
        }
    }
}

impl Settings {
    /// Dựng cấu hình từ một `RAMSettings.ini` đã phân tích.
    ///
    /// Cách làm giống hệt RAM: bắt đầu từ giá trị mặc định, rồi ghi đè bằng những gì có trong file.
    /// Khoá nào file không có thì giữ mặc định — **không hỏi lại người dùng**.
    #[must_use]
    pub fn from_ini(ini: &IniFile) -> Self {
        let defaults = Self::default();
        Self {
            general: General::from_ini(ini, &defaults.general),
            developer: Developer::from_ini(ini, &defaults.developer),
            web_server: WebServer::from_ini(ini, &defaults.web_server),
            account_control: AccountControl::from_ini(ini, &defaults.account_control),
            watcher: Watcher::from_ini(ini, &defaults.watcher),
            prompts: collect_unknown(ini, "Prompts"),
            other_sections: collect_other_sections(ini),
        }
    }

    /// Tuần tự hoá thành chuỗi TOML.
    ///
    /// # Errors
    ///
    /// Trả [`SettingsError::TomlWrite`] nếu không tuần tự hoá được.
    pub fn to_toml_string(&self) -> Result<String, SettingsError> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Đọc cấu hình từ chuỗi TOML.
    ///
    /// # Errors
    ///
    /// Trả [`SettingsError::TomlRead`] nếu TOML hỏng.
    pub fn from_toml_str(text: &str) -> Result<Self, SettingsError> {
        Ok(toml::from_str(text)?)
    }

    /// Đọc cấu hình từ file TOML.
    ///
    /// # Errors
    ///
    /// Trả lỗi I/O hoặc lỗi TOML.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        Self::from_toml_str(&std::fs::read_to_string(path)?)
    }

    /// Ghi cấu hình ra file TOML.
    ///
    /// # Errors
    ///
    /// Trả lỗi I/O hoặc lỗi tuần tự hoá.
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        std::fs::write(path, self.to_toml_string()?)?;
        Ok(())
    }
}

/// Cổng local API đang dùng, đã kiểm tra hợp lệ.
///
/// RAM không kiểm tra, nhưng một cổng bằng 0 sẽ khiến server bind vào cổng ngẫu nhiên và người
/// dùng mất khả năng kết nối — nên ở đây kẹp về giá trị mặc định thay vì truyền bừa.
#[must_use]
pub fn sanitize_port(port: u16, fallback: u16) -> u16 {
    if port == 0 { fallback } else { port }
}

// ------------------------------------------------------------------ chuyen tu INI sang struct

impl General {
    fn from_ini(ini: &IniFile, d: &Self) -> Self {
        Self {
            check_for_updates: ini.bool_or("General", "CheckForUpdates", d.check_for_updates),
            account_join_delay: ini.int_or("General", "AccountJoinDelay", d.account_join_delay),
            async_join: ini.bool_or("General", "AsyncJoin", d.async_join),
            disable_aging_alert: ini.bool_or("General", "DisableAgingAlert", d.disable_aging_alert),
            save_passwords: ini.bool_or("General", "SavePasswords", d.save_passwords),
            server_region_format: ini.str_or(
                "General",
                "ServerRegionFormat",
                &d.server_region_format,
            ),
            max_recent_games: ini.int_or("General", "MaxRecentGames", d.max_recent_games),
            shuffle_chooses_lowest_server: ini.bool_or(
                "General",
                "ShuffleChoosesLowestServer",
                d.shuffle_chooses_lowest_server,
            ),
            shuffle_page_count: ini.int_or("General", "ShufflePageCount", d.shuffle_page_count),
            ip_api_link: ini.str_or("General", "IPApiLink", &d.ip_api_link),
            window_scale: ini.float_or("General", "WindowScale", d.window_scale),
            scale_fonts: ini.bool_or("General", "ScaleFonts", d.scale_fonts),
            auto_cookie_refresh: ini.bool_or("General", "AutoCookieRefresh", d.auto_cookie_refresh),
            auto_close_last_process: ini.bool_or(
                "General",
                "AutoCloseLastProcess",
                d.auto_close_last_process,
            ),
            show_presence: ini.bool_or("General", "ShowPresence", d.show_presence),
            presence_update_rate: ini.int_or(
                "General",
                "PresenceUpdateRate",
                d.presence_update_rate,
            ),
            unlock_fps: ini.bool_or("General", "UnlockFPS", d.unlock_fps),
            max_fps_value: ini.int_or("General", "MaxFPSValue", d.max_fps_value),
            use_cef_sharp_browser: ini.bool_or(
                "General",
                "UseCefSharpBrowser",
                d.use_cef_sharp_browser,
            ),
            custom_client_settings: ini.opt_str("General", "CustomClientSettings"),
            enable_multi_rbx: ini.bool_or("General", "EnableMultiRbx", d.enable_multi_rbx),
            // RAM ghi "ShuffleJobId"; van doc duoc "ShuffleJobID" neu nguoi dung tu sua.
            shuffle_job_id: ini.bool_or("General", "ShuffleJobId", d.shuffle_job_id)
                || ini.bool_or("General", "ShuffleJobID", false),
            other: collect_unknown_excluding(ini, "General", GENERAL_KNOWN),
        }
    }
}

impl Developer {
    fn from_ini(ini: &IniFile, d: &Self) -> Self {
        Self {
            dev_mode: ini.bool_or("Developer", "DevMode", d.dev_mode),
            enable_web_server: ini.bool_or("Developer", "EnableWebServer", d.enable_web_server),
            other: collect_unknown_excluding(ini, "Developer", DEVELOPER_KNOWN),
        }
    }
}

impl WebServer {
    fn from_ini(ini: &IniFile, d: &Self) -> Self {
        Self {
            web_server_port: sanitize_port(
                ini.int_or("WebServer", "WebServerPort", d.web_server_port),
                d.web_server_port,
            ),
            allow_get_cookie: ini.bool_or("WebServer", "AllowGetCookie", d.allow_get_cookie),
            allow_get_accounts: ini.bool_or("WebServer", "AllowGetAccounts", d.allow_get_accounts),
            allow_launch_account: ini.bool_or(
                "WebServer",
                "AllowLaunchAccount",
                d.allow_launch_account,
            ),
            allow_account_editing: ini.bool_or(
                "WebServer",
                "AllowAccountEditing",
                d.allow_account_editing,
            ),
            password: ini.str_or("WebServer", "Password", &d.password),
            every_request_requires_password: ini.bool_or(
                "WebServer",
                "EveryRequestRequiresPassword",
                d.every_request_requires_password,
            ),
            allow_external_connections: ini.bool_or(
                "WebServer",
                "AllowExternalConnections",
                d.allow_external_connections,
            ),
            other: collect_unknown_excluding(ini, "WebServer", WEB_SERVER_KNOWN),
        }
    }
}

impl AccountControl {
    fn from_ini(ini: &IniFile, d: &Self) -> Self {
        Self {
            allow_external_connections: ini.bool_or(
                "AccountControl",
                "AllowExternalConnections",
                d.allow_external_connections,
            ),
            relaunch_delay: ini.int_or("AccountControl", "RelaunchDelay", d.relaunch_delay),
            launcher_delay_number: ini.int_or(
                "AccountControl",
                "LauncherDelayNumber",
                d.launcher_delay_number,
            ),
            nexus_port: sanitize_port(
                ini.int_or("AccountControl", "NexusPort", d.nexus_port),
                d.nexus_port,
            ),
            other: collect_unknown_excluding(ini, "AccountControl", ACCOUNT_CONTROL_KNOWN),
        }
    }
}

impl Watcher {
    fn from_ini(ini: &IniFile, d: &Self) -> Self {
        Self {
            enabled: ini.bool_or("Watcher", "Enabled", d.enabled),
            exit_on_beta: ini.bool_or("Watcher", "ExitOnBeta", d.exit_on_beta),
            exit_if_no_connection: ini.bool_or(
                "Watcher",
                "ExitIfNoConnection",
                d.exit_if_no_connection,
            ),
            save_window_positions: ini.bool_or(
                "Watcher",
                "SaveWindowPositions",
                d.save_window_positions,
            ),
            verify_data_model: ini.bool_or("Watcher", "VerifyDataModel", d.verify_data_model),
            ignore_existing_processes: ini.bool_or(
                "Watcher",
                "IgnoreExistingProcesses",
                d.ignore_existing_processes,
            ),
            close_rbx_window_title: ini.bool_or(
                "Watcher",
                "CloseRbxWindowTitle",
                d.close_rbx_window_title,
            ),
            close_rbx_memory: ini.bool_or("Watcher", "CloseRbxMemory", d.close_rbx_memory),
            expected_window_title: ini.str_or(
                "Watcher",
                "ExpectedWindowTitle",
                &d.expected_window_title,
            ),
            memory_low_value: ini.int_or("Watcher", "MemoryLowValue", d.memory_low_value),
            no_connection_timeout: ini.int_or(
                "Watcher",
                "NoConnectionTimeout",
                d.no_connection_timeout,
            ),
            scan_interval: ini.int_or("Watcher", "ScanInterval", d.scan_interval),
            read_interval: ini.int_or("Watcher", "ReadInterval", d.read_interval),
            other: collect_unknown_excluding(ini, "Watcher", WATCHER_KNOWN),
        }
    }
}

/// Tên các section đã được mô hình hoá — mọi section khác rơi vào [`Settings::other_sections`].
const KNOWN_SECTIONS: [&str; 6] = [
    "General",
    "Developer",
    "WebServer",
    "AccountControl",
    "Watcher",
    "Prompts",
];

const GENERAL_KNOWN: &[&str] = &[
    "CheckForUpdates",
    "AccountJoinDelay",
    "AsyncJoin",
    "DisableAgingAlert",
    "SavePasswords",
    "ServerRegionFormat",
    "MaxRecentGames",
    "ShuffleChoosesLowestServer",
    "ShufflePageCount",
    "IPApiLink",
    "IpApiLink",
    "WindowScale",
    "ScaleFonts",
    "AutoCookieRefresh",
    "AutoCloseLastProcess",
    "ShowPresence",
    "PresenceUpdateRate",
    "UnlockFPS",
    "UnlockFps",
    "MaxFPSValue",
    "MaxFpsValue",
    "UseCefSharpBrowser",
    "CustomClientSettings",
    "EnableMultiRbx",
    "ShuffleJobId",
    "ShuffleJobID",
];

const DEVELOPER_KNOWN: &[&str] = &["DevMode", "EnableWebServer"];

const WEB_SERVER_KNOWN: &[&str] = &[
    "WebServerPort",
    "AllowGetCookie",
    "AllowGetAccounts",
    "AllowLaunchAccount",
    "AllowAccountEditing",
    "Password",
    "EveryRequestRequiresPassword",
    "AllowExternalConnections",
];

const ACCOUNT_CONTROL_KNOWN: &[&str] = &[
    "AllowExternalConnections",
    "RelaunchDelay",
    "LauncherDelayNumber",
    "NexusPort",
];

const WATCHER_KNOWN: &[&str] = &[
    "Enabled",
    "ExitOnBeta",
    "ExitIfNoConnection",
    "SaveWindowPositions",
    "VerifyDataModel",
    "IgnoreExistingProcesses",
    "CloseRbxWindowTitle",
    "CloseRbxMemory",
    "ExpectedWindowTitle",
    "MemoryLowValue",
    "NoConnectionTimeout",
    "ScanInterval",
    "ReadInterval",
];

fn collect_unknown(ini: &IniFile, section: &str) -> Section {
    ini.section(section).cloned().unwrap_or_default()
}

fn collect_unknown_excluding(ini: &IniFile, section: &str, known: &[&str]) -> Section {
    ini.section(section)
        .map(|entries| {
            entries
                .iter()
                .filter(|(key, _)| !known.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn collect_other_sections(ini: &IniFile) -> BTreeMap<String, Section> {
    ini.sections()
        .filter(|(name, _)| !KNOWN_SECTIONS.contains(name))
        .map(|(name, entries)| (name.to_owned(), entries.clone()))
        .collect()
}

// ------------------------------------------------------------------ sao luu file goc

/// Sao lưu `RAMSettings.ini` gốc **trước khi** ghi bất cứ thứ gì.
///
/// Nguyên tắc bất biến #2: không phá huỷ dữ liệu người dùng. RAM cũng không xoá file INI, nhưng
/// một bản sao có mốc thời gian thì khôi phục được cả khi người dùng tự sửa hỏng file.
///
/// Tên bản sao: `<tên_file>.backup-<unix_giây>`. Nếu bản sao của giây hiện tại đã tồn tại thì
/// trả về đường dẫn đó thay vì ghi đè.
///
/// # Errors
///
/// Trả lỗi I/O nếu không đọc được file gốc hoặc không ghi được bản sao.
pub fn backup_file(path: &Path) -> Result<PathBuf, SettingsError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |delta| delta.as_secs());

    let file_name = path.file_name().map_or_else(
        || "settings".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );

    let backup = path.with_file_name(format!("{file_name}.backup-{seconds}"));
    if !backup.exists() {
        std::fs::copy(path, &backup)?;
    }
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ini() -> IniFile {
        IniFile::parse(
            "\
[General]
SavePasswords=false
EnableMultiRbx=true
ShuffleJobID=true
WindowScale=1.25
SomeFutureKey=giu-lai
[Developer]
DevMode=true
[WebServer]
WebServerPort=8080
Password=mat-khau-api
AllowExternalConnections=true
[AccountControl]
NexusPort=9999
[Watcher]
Enabled=true
[Prompts]
abc123=true
[Theme]
Name=dark
",
        )
    }

    #[test]
    fn defaults_match_the_values_ram_writes() {
        let d = Settings::default();
        assert!(d.general.check_for_updates);
        assert_eq!(d.general.account_join_delay, 8);
        assert_eq!(d.general.server_region_format, "<city>, <countryCode>");
        assert_eq!(d.general.ip_api_link, "http://ip-api.com/json/<ip>");
        assert_eq!(d.general.max_fps_value, 120);
        assert!(!d.general.unlock_fps);
        assert!(
            !d.general.enable_multi_rbx,
            "multi-instance phai mac dinh TAT"
        );

        assert!(!d.developer.dev_mode, "DevMode phai mac dinh TAT");

        assert_eq!(d.web_server.web_server_port, 7963);
        assert!(!d.web_server.allow_external_connections);
        assert_eq!(d.account_control.nexus_port, 5242);

        assert!(d.watcher.verify_data_model);
        assert!(d.watcher.ignore_existing_processes);
        assert_eq!(d.watcher.expected_window_title, "Roblox");
        assert_eq!(d.watcher.read_interval, 250);
    }

    #[test]
    fn migration_takes_values_from_the_ini() {
        let settings = Settings::from_ini(&sample_ini());

        assert!(!settings.general.save_passwords);
        assert!(settings.general.enable_multi_rbx);
        assert!(settings.general.shuffle_job_id);
        assert!((settings.general.window_scale - 1.25).abs() < f32::EPSILON);

        assert!(settings.developer.dev_mode);
        assert_eq!(settings.web_server.web_server_port, 8080);
        assert_eq!(settings.web_server.password, "mat-khau-api");
        assert!(settings.web_server.allow_external_connections);
        assert_eq!(settings.account_control.nexus_port, 9999);
        assert!(settings.watcher.enabled);
    }

    #[test]
    fn migration_fills_defaults_for_keys_the_ini_does_not_have() {
        let settings = Settings::from_ini(&IniFile::parse("[General]\nSavePasswords=false\n"));
        assert_eq!(
            settings.general.account_join_delay,
            Settings::default().general.account_join_delay,
            "khoa thieu phai lay mac dinh, khong hoi lai nguoi dung"
        );
        assert_eq!(settings.web_server.web_server_port, 7963);
    }

    #[test]
    fn unknown_keys_and_sections_are_preserved() {
        let settings = Settings::from_ini(&sample_ini());

        assert_eq!(
            settings
                .general
                .other
                .get("SomeFutureKey")
                .map(String::as_str),
            Some("giu-lai")
        );
        assert_eq!(
            settings.prompts.get("abc123").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            settings
                .other_sections
                .get("Theme")
                .and_then(|section| section.get("Name"))
                .map(String::as_str),
            Some("dark")
        );
    }

    #[test]
    fn the_roadmap_spelling_shuffle_job_id_also_works() {
        let upper = Settings::from_ini(&IniFile::parse("[General]\nShuffleJobID=true\n"));
        assert!(upper.general.shuffle_job_id);

        let ram = Settings::from_ini(&IniFile::parse("[General]\nShuffleJobId=true\n"));
        assert!(ram.general.shuffle_job_id);
    }

    #[test]
    fn a_zero_port_falls_back_to_the_default() {
        assert_eq!(sanitize_port(0, 7963), 7963);
        assert_eq!(sanitize_port(8080, 7963), 8080);
    }

    #[test]
    fn toml_round_trip_is_lossless() {
        let settings = Settings::from_ini(&sample_ini());
        let text = settings.to_toml_string().expect("ghi TOML");
        let back = Settings::from_toml_str(&text).expect("doc TOML");
        assert_eq!(back, settings);
    }

    #[test]
    fn toml_keeps_the_ram_key_names() {
        let text = Settings::default().to_toml_string().expect("ghi TOML");
        for key in [
            "[General]",
            "[Developer]",
            "[WebServer]",
            "[AccountControl]",
            "[Watcher]",
            "SavePasswords",
            "WebServerPort",
            "NexusPort",
            "EnableMultiRbx",
            "MaxFPSValue",
            "UnlockFPS",
            "IPApiLink",
            "DevMode",
            "AutoCloseLastProcess",
            "CustomClientSettings",
        ] {
            if key == "CustomClientSettings" {
                continue;
            }
            assert!(text.contains(key), "thieu {key} trong TOML:\n{text}");
        }
    }

    #[test]
    fn a_none_custom_client_settings_is_not_written_out() {
        let text = Settings::default().to_toml_string().expect("ghi TOML");
        assert!(
            !text.contains("CustomClientSettings"),
            "None khong duoc ghi ra TOML:\n{text}"
        );
    }

    #[test]
    fn settings_can_be_read_from_a_partial_toml_file() {
        let settings = Settings::from_toml_str("[WebServer]\nWebServerPort = 9000\n").expect("doc");
        assert_eq!(settings.web_server.web_server_port, 9000);
        assert_eq!(
            settings.general.max_fps_value, 120,
            "phan thieu lay mac dinh"
        );
    }

    #[test]
    fn broken_toml_is_reported_as_an_error() {
        assert!(Settings::from_toml_str("day khong phai TOML [[[").is_err());
    }

    #[test]
    fn web_server_debug_redacts_the_api_password() {
        let mut settings = Settings::default();
        settings.web_server.password = "mat-khau-bi-mat".to_owned();

        let text = format!("{settings:?}");
        assert!(
            !text.contains("mat-khau-bi-mat"),
            "lo mat khau API trong Debug: {text}"
        );
    }

    #[test]
    fn backup_file_copies_the_original_without_touching_it() {
        let dir = std::env::temp_dir().join("muster-settings-backup-test");
        std::fs::create_dir_all(&dir).expect("tao thu muc tam");
        let original = dir.join("RAMSettings.ini");
        std::fs::write(&original, "[General]\nSavePasswords=true\n").expect("ghi file goc");

        let backup = backup_file(&original).expect("sao luu");

        assert!(backup.exists(), "ban sao phai ton tai");
        assert!(original.exists(), "file goc phai con nguyen");
        assert_eq!(
            std::fs::read_to_string(&backup).expect("doc ban sao"),
            std::fs::read_to_string(&original).expect("doc file goc")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
