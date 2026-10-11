//! # `patch` — ghi `ClientAppSettings.json` cho client Roblox
//!
//! Tái lập `Classes/ClientSettingsPatcher.cs`. Mục đích: đặt các khoá tinh chỉnh của client —
//! phổ biến nhất là mở khoá FPS (`DFIntTaskSchedulerTargetFps`) — bằng cách ghi một file JSON vào
//! thư mục `ClientSettings` của **version Roblox đang cài**.
//!
//! ## Vì sao việc này nằm ngoài "out-of-process"
//!
//! Ghi `ClientSettings` là đọc/ghi **file cấu hình trên đĩa**, không phải inject hay patch memory.
//! `ROADMAP.md` mục 2 liệt nó vào đúng nhóm được phép. Đây cũng là cách duy nhất để đổi FPS mà
//! không chạm vào tiến trình.
//!
//! ## Ba bước, và bước nào cũng có thể thất bại im lặng
//!
//! 1. **Tìm thư mục version.** Đọc `HKEY_CLASSES_ROOT\roblox\DefaultIcon`, lấy đường dẫn
//!    `RobloxPlayerBeta.exe`, rồi lên một cấp (`ClientSettingsPatcher.cs:13-16`).
//! 2. **Kiểm tra thư mục hợp lệ.** Tên phải bắt đầu bằng `version-` **và** phải có
//!    `RobloxPlayerLauncher.exe` (`ClientSettingsPatcher.cs:19-20`).
//! 3. **Ghi file.** Ưu tiên file tuỳ biến nếu người dùng đặt `CustomClientSettings`; nếu không thì
//!    chỉ ghi khi `UnlockFPS` bật (`ClientSettingsPatcher.cs:29-40`).
//!
//! Mỗi bước RAM đều chỉ ghi log rồi `return` — không có lỗi nào nổi lên UI. Ở đây mỗi bước trả một
//! lỗi có kiểu, để chỗ gọi quyết định có báo cho người dùng hay không.

use std::path::{Path, PathBuf};

/// Tên thư mục con chứa cấu hình client.
pub const CLIENT_SETTINGS_DIR: &str = "ClientSettings";

/// Tên file cấu hình client.
pub const CLIENT_APP_SETTINGS_FILE: &str = "ClientAppSettings.json";

/// File đánh dấu một thư mục là version Roblox hợp lệ.
///
/// ⚠️ `ClientSettingsPatcher.cs:20` kiểm tra `RobloxPlayerLauncher.exe` nhưng thông báo lỗi của nó
/// lại ghi "RobloxPlayerBeta.exe not found". Ta kiểm tra đúng **file mà nó thật sự kiểm tra**.
pub const VERSION_MARKER_EXE: &str = "RobloxPlayerLauncher.exe";

/// Tiền tố bắt buộc của tên thư mục version.
pub const VERSION_FOLDER_PREFIX: &str = "version-";

/// Khoá JSON điều khiển giới hạn FPS của client.
pub const FPS_KEY: &str = "DFIntTaskSchedulerTargetFps";

/// Giá trị FPS RAM dùng khi không đọc được `MaxFPSValue`.
///
/// `ClientSettingsPatcher.cs:35` và `:39` đều lấy `240` làm mặc định, khác với mặc định `120` của
/// chính setting `MaxFPSValue` (`AccountManager.cs:147`). Giữ đúng cả hai con số.
pub const DEFAULT_UNLOCK_FPS: u32 = 240;

/// Lỗi của lớp `patch`.
#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    /// Không đọc được giá trị trong registry.
    #[cfg(windows)]
    #[error("không đọc được registry {subkey} (mã lỗi Win32 {code})")]
    Registry {
        /// Đường dẫn khoá con đã đọc.
        subkey: &'static str,
        /// Mã lỗi Win32.
        code: u32,
    },

    /// Giá trị `DefaultIcon` không suy ra được thư mục nào.
    #[error("giá trị DefaultIcon của giao thức roblox không phải đường dẫn hợp lệ: {0:?}")]
    BadDefaultIcon(String),

    /// Thư mục version không tồn tại, không bắt đầu bằng `version-`, hoặc thiếu file đánh dấu.
    #[error("không phải thư mục version Roblox hợp lệ: {0}")]
    NotAVersionFolder(PathBuf),

    /// Lỗi đọc/ghi file.
    #[error("lỗi vào/ra tại {path}: {source}")]
    Io {
        /// Đường dẫn đang thao tác.
        path: PathBuf,
        /// Lỗi gốc.
        source: std::io::Error,
    },
}

/// Tham số quyết định nội dung `ClientAppSettings.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PatchSettings {
    /// Đường dẫn file tuỳ biến, từ setting `CustomClientSettings`.
    pub custom_client_settings: Option<PathBuf>,

    /// Setting `UnlockFPS`.
    pub unlock_fps: bool,

    /// Setting `MaxFPSValue`. Chỉ dùng khi [`PatchSettings::unlock_fps`] bật.
    pub max_fps_value: u32,
}

/// Việc sẽ làm với `ClientAppSettings.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientSettingsPlan {
    /// Chép nguyên file tuỳ biến đè lên.
    CopyCustom(PathBuf),

    /// Ghi chuỗi JSON này.
    Write(String),

    /// Không đổi gì.
    Leave,
}

/// Suy ra thư mục version từ giá trị `DefaultIcon` của giao thức `roblox`.
///
/// Giá trị thật trong registry thường ở một trong hai dạng:
///
/// - `C:\Program Files (x86)\Roblox\Versions\version-abc\RobloxPlayerBeta.exe`
/// - `"C:\...\RobloxPlayerBeta.exe",0`
///
/// RAM dùng thẳng giá trị làm đường dẫn nên **dạng thứ hai làm nó hỏng** — `Directory.GetParent`
/// trả về thư mục cha của chuỗi có `,0`, không tồn tại, và `ClientSettingsPatcher.cs:18` chỉ ghi
/// log rồi thoát. Ta cắt cặp ngoặc kép và hậu tố `,<index>` trước. Đây là cải tiến không phá tương
/// thích: nó chỉ thành công ở đúng chỗ RAM thất bại im lặng.
#[must_use]
pub fn version_folder_from_default_icon(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Bỏ hậu tố ",<index>" TRƯỚC, vì nó nằm ngoài cặp ngoặc kép:
    //   "C:\...\RobloxPlayerBeta.exe",0
    // Cắt ở dấu phẩy CUỐI và chỉ khi phần sau nó là một số, để không đụng vào dấu phẩy trong tên
    // thư mục (ví dụ `C:\Roblox, Inc\...`).
    let without_index = match trimmed.rfind(',') {
        Some(position) if trimmed[position + 1..].trim().parse::<i32>().is_ok() => {
            &trimmed[..position]
        }
        _ => trimmed,
    };

    // Rồi mới bỏ cặp ngoặc kép bao quanh đường dẫn.
    let path_part = without_index.trim();
    let path_part =
        if path_part.len() >= 2 && path_part.starts_with('"') && path_part.ends_with('"') {
            &path_part[1..path_part.len() - 1]
        } else {
            path_part
        };

    let folder = PathBuf::from(path_part.trim()).parent()?.to_path_buf();

    if folder.as_os_str().is_empty() {
        return None;
    }
    Some(folder)
}

/// `true` nếu `folder` là một thư mục version Roblox hợp lệ.
#[must_use]
pub fn is_version_folder(folder: &Path) -> bool {
    let name_ok = folder
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(VERSION_FOLDER_PREFIX));

    name_ok && folder.is_dir() && folder.join(VERSION_MARKER_EXE).is_file()
}

/// Đường dẫn file `ClientAppSettings.json` bên trong một thư mục version.
#[must_use]
pub fn client_app_settings_path(version_folder: &Path) -> PathBuf {
    version_folder
        .join(CLIENT_SETTINGS_DIR)
        .join(CLIENT_APP_SETTINGS_FILE)
}

/// Quyết định nội dung `ClientAppSettings.json` — thuần tuý, không chạm đĩa.
///
/// `existing` là nội dung file hiện có, `None` nếu file chưa tồn tại.
///
/// Thứ tự ưu tiên đúng như RAM (`ClientSettingsPatcher.cs:29-40`): file tuỳ biến thắng, rồi mới
/// tới `UnlockFPS`.
#[must_use]
pub fn plan_client_settings(
    existing: Option<&str>,
    settings: &PatchSettings,
) -> ClientSettingsPlan {
    if let Some(custom) = &settings.custom_client_settings {
        if custom.is_file() {
            return ClientSettingsPlan::CopyCustom(custom.clone());
        }
    }

    if !settings.unlock_fps {
        return ClientSettingsPlan::Leave;
    }

    // Merge vào JSON sẵn có để không xoá mất các khoá tinh chỉnh khác người dùng đã đặt.
    if let Some(text) = existing {
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(text) {
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    FPS_KEY.to_owned(),
                    serde_json::Value::from(settings.max_fps_value),
                );
                // `Value::to_string` là JSON nén — tương đương `Formatting.None` của RAM.
                return ClientSettingsPlan::Write(value.to_string());
            }
        }
    }

    ClientSettingsPlan::Write(format!("{{\"{FPS_KEY}\":{}}}", settings.max_fps_value))
}

/// Áp [`plan_client_settings`] lên đĩa.
///
/// Tạo thư mục `ClientSettings` nếu chưa có, đúng như `ClientSettingsPatcher.cs:24`.
///
/// # Errors
///
/// Trả [`PatchError::NotAVersionFolder`] nếu thư mục không hợp lệ, hoặc [`PatchError::Io`] nếu
/// không đọc/ghi được.
pub fn apply_client_settings(
    version_folder: &Path,
    settings: &PatchSettings,
) -> Result<ClientSettingsPlan, PatchError> {
    if !is_version_folder(version_folder) {
        return Err(PatchError::NotAVersionFolder(version_folder.to_path_buf()));
    }

    let settings_dir = version_folder.join(CLIENT_SETTINGS_DIR);
    let target = client_app_settings_path(version_folder);

    let existing = std::fs::read_to_string(&target).ok();
    let plan = plan_client_settings(existing.as_deref(), settings);

    match &plan {
        ClientSettingsPlan::Leave => {}
        ClientSettingsPlan::CopyCustom(source) => {
            std::fs::create_dir_all(&settings_dir).map_err(|source| PatchError::Io {
                path: settings_dir.clone(),
                source,
            })?;
            std::fs::copy(source, &target).map_err(|source| PatchError::Io {
                path: target.clone(),
                source,
            })?;
        }
        ClientSettingsPlan::Write(text) => {
            std::fs::create_dir_all(&settings_dir).map_err(|source| PatchError::Io {
                path: settings_dir.clone(),
                source,
            })?;
            std::fs::write(&target, text).map_err(|source| PatchError::Io {
                path: target.clone(),
                source,
            })?;
        }
    }

    Ok(plan)
}

// ------------------------------------------------------------------ đọc registry (chỉ Windows)

/// Đọc giá trị chuỗi `DefaultIcon` của giao thức `roblox` từ `HKEY_CLASSES_ROOT`.
///
/// # Errors
///
/// Trả [`PatchError::Registry`] nếu Win32 từ chối.
#[cfg(windows)]
pub fn read_default_icon() -> Result<String, PatchError> {
    use core::ffi::c_void;

    /// Đường dẫn khoá con, cũng dùng làm nhãn trong thông báo lỗi.
    const SUBKEY: &str = r"roblox\DefaultIcon";

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(
            key: *mut c_void,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            value_type: *mut u32,
            data: *mut c_void,
            data_size: *mut u32,
        ) -> i32;
    }

    /// `HKEY_CLASSES_ROOT`. Trên 64 bit giá trị này là `0xFFFF_FFFF_8000_0000`, nên phải đi qua
    /// `i32` để sign-extend đúng — viết thẳng `0x8000_0000` sẽ ra một handle sai.
    const HKEY_CLASSES_ROOT: isize = 0x8000_0000u32 as i32 as isize;

    /// Chỉ nhận `REG_SZ` (và `REG_EXPAND_SZ` sẽ bị từ chối để không phải tự expand).
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;

    const ERROR_SUCCESS: i32 = 0;

    let subkey: Vec<u16> = SUBKEY.encode_utf16().chain(core::iter::once(0)).collect();
    // Chuỗi rỗng = giá trị mặc định (không tên) của khoá.
    let value_name: Vec<u16> = core::iter::once(0).collect();

    // 1024 ký tự là quá đủ cho một đường dẫn; nếu Win32 cần hơn nó sẽ báo lỗi thay vì tràn.
    let mut buffer = vec![0u16; 1024];
    let mut size_bytes = (buffer.len() * size_of::<u16>()) as u32;
    let mut value_type = 0u32;

    // SAFETY: hai chuoi wide ket thuc bang 0 song trong suot loi goi; `buffer` du lon va `size_bytes`
    // mo ta dung kich thuoc cua no; cac con tro con lai tro toi bien cuc bo hop le.
    let status = unsafe {
        RegGetValueW(
            HKEY_CLASSES_ROOT as *mut c_void,
            subkey.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_SZ,
            &mut value_type,
            buffer.as_mut_ptr().cast(),
            &mut size_bytes,
        )
    };

    if status != ERROR_SUCCESS {
        return Err(PatchError::Registry {
            subkey: SUBKEY,
            code: status as u32,
        });
    }

    // `size_bytes` tính cả ký tự kết thúc null.
    let length = (size_bytes as usize / size_of::<u16>()).saturating_sub(1);
    let length = length.min(buffer.len());

    Ok(String::from_utf16_lossy(&buffer[..length]))
}

/// Tìm thư mục version Roblox đang cài.
///
/// # Errors
///
/// Trả [`PatchError::Registry`] nếu không đọc được registry,
/// [`PatchError::BadDefaultIcon`] nếu giá trị không suy ra được đường dẫn, hoặc
/// [`PatchError::NotAVersionFolder`] nếu thư mục không hợp lệ.
#[cfg(windows)]
pub fn find_version_folder() -> Result<PathBuf, PatchError> {
    let raw = read_default_icon()?;
    let folder =
        version_folder_from_default_icon(&raw).ok_or(PatchError::BadDefaultIcon(raw.clone()))?;

    if !is_version_folder(&folder) {
        return Err(PatchError::NotAVersionFolder(folder));
    }

    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------- DefaultIcon

    #[test]
    fn a_bare_executable_path_yields_its_parent_folder() {
        let value = r"C:\Program Files (x86)\Roblox\Versions\version-abc\RobloxPlayerBeta.exe";
        assert_eq!(
            version_folder_from_default_icon(value),
            Some(PathBuf::from(
                r"C:\Program Files (x86)\Roblox\Versions\version-abc"
            ))
        );
    }

    #[test]
    fn a_quoted_path_with_an_icon_index_is_handled() {
        // Dang thuong gap trong registry, va la dang lam RAM hong.
        let value =
            r#""C:\Program Files (x86)\Roblox\Versions\version-abc\RobloxPlayerBeta.exe",0"#;
        assert_eq!(
            version_folder_from_default_icon(value),
            Some(PathBuf::from(
                r"C:\Program Files (x86)\Roblox\Versions\version-abc"
            ))
        );
    }

    #[test]
    fn a_quoted_path_without_an_index_is_handled() {
        let value = r#""C:\Roblox\Versions\version-1\RobloxPlayerBeta.exe""#;
        assert_eq!(
            version_folder_from_default_icon(value),
            Some(PathBuf::from(r"C:\Roblox\Versions\version-1"))
        );
    }

    #[test]
    fn an_empty_or_useless_value_yields_nothing() {
        assert_eq!(version_folder_from_default_icon(""), None);
        assert_eq!(version_folder_from_default_icon("   "), None);
        assert_eq!(
            version_folder_from_default_icon("RobloxPlayerBeta.exe"),
            None
        );
    }

    #[test]
    fn a_comma_that_is_not_an_icon_index_is_kept() {
        let value = r"C:\Roblox, Inc\Versions\version-abc\RobloxPlayerBeta.exe";
        assert_eq!(
            version_folder_from_default_icon(value),
            Some(PathBuf::from(r"C:\Roblox, Inc\Versions\version-abc"))
        );
    }

    // ---------------------------------------------------------------- version folder

    fn make_version_folder(tag: &str, with_marker: bool) -> PathBuf {
        // Ten thu muc PHAI bat dau bang "version-" thi moi la thu muc version hop le.
        let dir = std::env::temp_dir().join(format!("version-muster-patch-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tạo thư mục");

        if with_marker {
            std::fs::write(dir.join(VERSION_MARKER_EXE), b"").expect("ghi file đánh dấu");
        }
        dir
    }

    #[test]
    fn a_folder_with_the_marker_and_the_right_prefix_is_valid() {
        let dir = make_version_folder("valid", true);
        assert!(is_version_folder(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_without_the_marker_is_not_a_version_folder() {
        let dir = make_version_folder("nomarker", false);
        assert!(!is_version_folder(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_with_the_marker_but_the_wrong_name_is_rejected() {
        // ClientSettingsPatcher.cs:19 doi ten phai bat dau bang "version-".
        let dir = std::env::temp_dir().join("muster-patch-wrongname");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tạo thư mục");
        std::fs::write(dir.join(VERSION_MARKER_EXE), b"").expect("ghi file đánh dấu");

        assert!(
            !is_version_folder(&dir),
            "ten 'muster-patch-wrongname' khong hop le"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_settings_path_sits_inside_a_client_settings_subfolder() {
        let path = client_app_settings_path(Path::new(r"C:\Roblox\Versions\version-1"));
        assert!(path.ends_with(Path::new(CLIENT_SETTINGS_DIR).join(CLIENT_APP_SETTINGS_FILE)));
    }

    // ---------------------------------------------------------------- plan

    fn patch_settings(custom: Option<&str>, unlock_fps: bool, fps: u32) -> PatchSettings {
        PatchSettings {
            custom_client_settings: custom.map(PathBuf::from),
            unlock_fps,
            max_fps_value: fps,
        }
    }

    #[test]
    fn nothing_is_written_when_both_switches_are_off() {
        let plan = plan_client_settings(None, &patch_settings(None, false, 120));
        assert_eq!(plan, ClientSettingsPlan::Leave);
    }

    #[test]
    fn a_missing_file_with_unlock_fps_on_gets_the_minimal_json() {
        let plan = plan_client_settings(None, &patch_settings(None, true, 240));
        assert_eq!(
            plan,
            ClientSettingsPlan::Write("{\"DFIntTaskSchedulerTargetFps\":240}".to_owned())
        );
    }

    #[test]
    fn an_existing_json_keeps_its_other_keys() {
        let existing = r#"{"SomeOtherFlag":true,"DFIntTaskSchedulerTargetFps":60}"#;
        let plan = plan_client_settings(Some(existing), &patch_settings(None, true, 144));

        let ClientSettingsPlan::Write(text) = plan else {
            panic!("phai la Write");
        };
        let value: serde_json::Value = serde_json::from_str(&text).expect("JSON hợp lệ");

        assert_eq!(value["SomeOtherFlag"], serde_json::Value::Bool(true));
        assert_eq!(
            value["DFIntTaskSchedulerTargetFps"],
            serde_json::Value::from(144u32)
        );
    }

    #[test]
    fn broken_json_falls_back_to_the_minimal_document() {
        let plan =
            plan_client_settings(Some("day khong phai JSON"), &patch_settings(None, true, 90));
        assert_eq!(
            plan,
            ClientSettingsPlan::Write("{\"DFIntTaskSchedulerTargetFps\":90}".to_owned())
        );
    }

    #[test]
    fn a_json_document_that_is_not_an_object_falls_back_too() {
        let plan = plan_client_settings(Some("[1,2,3]"), &patch_settings(None, true, 30));
        assert_eq!(
            plan,
            ClientSettingsPlan::Write("{\"DFIntTaskSchedulerTargetFps\":30}".to_owned())
        );
    }

    #[test]
    fn a_custom_file_wins_over_the_fps_unlock() {
        let dir = make_version_folder("custom", true);
        let custom = dir.join("my-settings.json");
        std::fs::write(&custom, "{}").expect("ghi file tuỳ biến");

        let plan = plan_client_settings(None, &patch_settings(custom.to_str(), true, 240));
        assert_eq!(plan, ClientSettingsPlan::CopyCustom(custom.clone()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_custom_path_that_does_not_exist_falls_through_to_the_fps_unlock() {
        let plan = plan_client_settings(
            None,
            &patch_settings(Some(r"C:\khong-ton-tai\settings.json"), true, 240),
        );
        assert_eq!(
            plan,
            ClientSettingsPlan::Write("{\"DFIntTaskSchedulerTargetFps\":240}".to_owned())
        );
    }

    // ---------------------------------------------------------------- apply

    #[test]
    fn applying_writes_the_file_and_creates_the_subfolder() {
        let dir = make_version_folder("apply", true);
        let plan = apply_client_settings(&dir, &patch_settings(None, true, 144)).expect("áp dụng");

        assert_eq!(
            plan,
            ClientSettingsPlan::Write("{\"DFIntTaskSchedulerTargetFps\":144}".to_owned())
        );

        let written = std::fs::read_to_string(client_app_settings_path(&dir)).expect("đọc lại");
        let value: serde_json::Value = serde_json::from_str(&written).expect("JSON hợp lệ");
        assert_eq!(
            value["DFIntTaskSchedulerTargetFps"],
            serde_json::Value::from(144u32)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn applying_leaves_the_disk_alone_when_the_plan_says_leave() {
        let dir = make_version_folder("leave", true);
        let plan = apply_client_settings(&dir, &patch_settings(None, false, 120)).expect("áp dụng");

        assert_eq!(plan, ClientSettingsPlan::Leave);
        assert!(
            !client_app_settings_path(&dir).exists(),
            "khong duoc tao file khi ke hoach la Leave"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn applying_refuses_a_folder_that_is_not_a_version_folder() {
        let dir = make_version_folder("refuse", false);
        let error = apply_client_settings(&dir, &patch_settings(None, true, 240))
            .expect_err("phải từ chối");

        assert!(matches!(error, PatchError::NotAVersionFolder(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn applying_a_custom_file_overwrites_the_target() {
        let dir = make_version_folder("copycustom", true);
        let custom = dir.join("custom.json");
        std::fs::write(&custom, r#"{"Custom":true}"#).expect("ghi file tuỳ biến");

        apply_client_settings(&dir, &patch_settings(custom.to_str(), true, 240)).expect("áp dụng");

        let written = std::fs::read_to_string(client_app_settings_path(&dir)).expect("đọc lại");
        assert_eq!(written, r#"{"Custom":true}"#);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
