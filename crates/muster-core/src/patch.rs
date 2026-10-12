use std::path::{Path, PathBuf};

pub const CLIENT_SETTINGS_DIR: &str = "ClientSettings";

pub const CLIENT_APP_SETTINGS_FILE: &str = "ClientAppSettings.json";

pub const VERSION_MARKER_EXE: &str = "RobloxPlayerLauncher.exe";

pub const VERSION_FOLDER_PREFIX: &str = "version-";

pub const FPS_KEY: &str = "DFIntTaskSchedulerTargetFps";

pub const DEFAULT_UNLOCK_FPS: u32 = 240;

#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    #[cfg(windows)]
    #[error("không đọc được registry {subkey} (mã lỗi Win32 {code})")]
    Registry { subkey: &'static str, code: u32 },

    #[error("giá trị DefaultIcon của giao thức roblox không phải đường dẫn hợp lệ: {0:?}")]
    BadDefaultIcon(String),

    #[error("không phải thư mục version Roblox hợp lệ: {0}")]
    NotAVersionFolder(PathBuf),

    #[error("lỗi vào/ra tại {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PatchSettings {
    pub custom_client_settings: Option<PathBuf>,

    pub unlock_fps: bool,

    pub max_fps_value: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientSettingsPlan {
    CopyCustom(PathBuf),

    Write(String),

    Leave,
}

#[must_use]
pub fn version_folder_from_default_icon(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_index = match trimmed.rfind(',') {
        Some(position) if trimmed[position + 1..].trim().parse::<i32>().is_ok() => {
            &trimmed[..position]
        }
        _ => trimmed,
    };

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

#[must_use]
pub fn is_version_folder(folder: &Path) -> bool {
    let name_ok = folder
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(VERSION_FOLDER_PREFIX));

    name_ok && folder.is_dir() && folder.join(VERSION_MARKER_EXE).is_file()
}

#[must_use]
pub fn client_app_settings_path(version_folder: &Path) -> PathBuf {
    version_folder
        .join(CLIENT_SETTINGS_DIR)
        .join(CLIENT_APP_SETTINGS_FILE)
}

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

    let fps = settings.max_fps_value.unwrap_or(DEFAULT_UNLOCK_FPS);

    if let Some(text) = existing {
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(text) {
            if let Some(object) = value.as_object_mut() {
                object.insert(FPS_KEY.to_owned(), serde_json::Value::from(fps));
                return ClientSettingsPlan::Write(value.to_string());
            }
        }
    }

    ClientSettingsPlan::Write(format!("{{\"{FPS_KEY}\":{fps}}}"))
}

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

#[cfg(windows)]
pub fn read_default_icon() -> Result<String, PatchError> {
    use core::ffi::c_void;

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

    const HKEY_CLASSES_ROOT: isize = 0x8000_0000u32 as i32 as isize;

    const RRF_RT_REG_SZ: u32 = 0x0000_0002;

    const ERROR_SUCCESS: i32 = 0;

    let subkey: Vec<u16> = SUBKEY.encode_utf16().chain(core::iter::once(0)).collect();
    let value_name: Vec<u16> = core::iter::once(0).collect();

    let mut buffer = vec![0u16; 1024];
    let mut size_bytes = (buffer.len() * size_of::<u16>()) as u32;
    let mut value_type = 0u32;

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

    let length = (size_bytes as usize / size_of::<u16>()).saturating_sub(1);
    let length = length.min(buffer.len());

    Ok(String::from_utf16_lossy(&buffer[..length]))
}

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

    fn make_version_folder(tag: &str, with_marker: bool) -> PathBuf {
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

    fn patch_settings(custom: Option<&str>, unlock_fps: bool, fps: Option<u32>) -> PatchSettings {
        PatchSettings {
            custom_client_settings: custom.map(PathBuf::from),
            unlock_fps,
            max_fps_value: fps,
        }
    }

    #[test]
    fn nothing_is_written_when_both_switches_are_off() {
        let plan = plan_client_settings(None, &patch_settings(None, false, Some(120)));
        assert_eq!(plan, ClientSettingsPlan::Leave);
    }

    #[test]
    fn a_missing_file_with_unlock_fps_on_gets_the_minimal_json() {
        let plan = plan_client_settings(None, &patch_settings(None, true, Some(240)));
        assert_eq!(
            plan,
            ClientSettingsPlan::Write("{\"DFIntTaskSchedulerTargetFps\":240}".to_owned())
        );
    }

    #[test]
    fn no_fps_value_falls_back_to_240_like_ram_does() {
        let broken = plan_client_settings(
            Some("day khong phai JSON"),
            &patch_settings(None, true, None),
        );
        assert_eq!(
            broken,
            ClientSettingsPlan::Write("{\"DFIntTaskSchedulerTargetFps\":240}".to_owned()),
            "JSON hong phai ghi 240, khong phai 0"
        );

        let existing = plan_client_settings(
            Some(r#"{"SomeOtherFlag":true}"#),
            &patch_settings(None, true, None),
        );
        let ClientSettingsPlan::Write(text) = existing else {
            panic!("phai la Write");
        };
        let value: serde_json::Value = serde_json::from_str(&text).expect("JSON hợp lệ");
        assert_eq!(
            value["DFIntTaskSchedulerTargetFps"],
            serde_json::Value::from(240u32)
        );
        assert_eq!(value["SomeOtherFlag"], serde_json::Value::Bool(true));
    }

    #[test]
    fn an_existing_json_keeps_its_other_keys() {
        let existing = r#"{"SomeOtherFlag":true,"DFIntTaskSchedulerTargetFps":60}"#;
        let plan = plan_client_settings(Some(existing), &patch_settings(None, true, Some(144)));

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
        let plan = plan_client_settings(
            Some("day khong phai JSON"),
            &patch_settings(None, true, Some(90)),
        );
        assert_eq!(
            plan,
            ClientSettingsPlan::Write("{\"DFIntTaskSchedulerTargetFps\":90}".to_owned())
        );
    }

    #[test]
    fn a_json_document_that_is_not_an_object_falls_back_too() {
        let plan = plan_client_settings(Some("[1,2,3]"), &patch_settings(None, true, Some(30)));
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

        let plan = plan_client_settings(None, &patch_settings(custom.to_str(), true, Some(240)));
        assert_eq!(plan, ClientSettingsPlan::CopyCustom(custom.clone()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_custom_path_that_does_not_exist_falls_through_to_the_fps_unlock() {
        let plan = plan_client_settings(
            None,
            &patch_settings(Some(r"C:\khong-ton-tai\settings.json"), true, Some(240)),
        );
        assert_eq!(
            plan,
            ClientSettingsPlan::Write("{\"DFIntTaskSchedulerTargetFps\":240}".to_owned())
        );
    }

    #[test]
    fn applying_writes_the_file_and_creates_the_subfolder() {
        let dir = make_version_folder("apply", true);
        let plan =
            apply_client_settings(&dir, &patch_settings(None, true, Some(144))).expect("áp dụng");

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
        let plan =
            apply_client_settings(&dir, &patch_settings(None, false, Some(120))).expect("áp dụng");

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
        let error = apply_client_settings(&dir, &patch_settings(None, true, Some(240)))
            .expect_err("phải từ chối");

        assert!(matches!(error, PatchError::NotAVersionFolder(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn applying_a_custom_file_overwrites_the_target() {
        let dir = make_version_folder("copycustom", true);
        let custom = dir.join("custom.json");
        std::fs::write(&custom, r#"{"Custom":true}"#).expect("ghi file tuỳ biến");

        apply_client_settings(&dir, &patch_settings(custom.to_str(), true, Some(240)))
            .expect("áp dụng");

        let written = std::fs::read_to_string(client_app_settings_path(&dir)).expect("đọc lại");
        assert_eq!(written, r#"{"Custom":true}"#);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
