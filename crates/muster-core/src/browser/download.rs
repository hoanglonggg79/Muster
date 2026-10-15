use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::detect::{WINDOWS_BINARY, WINDOWS_SUBDIR, chrome_for_testing_binary};

pub const CFT_MANIFEST_URL: &str = "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json";
pub const CFT_WINDOWS_PLATFORM: &str = "win64";

pub const APPCONTAINER_GRANT: &str = "*S-1-15-2-1:(OI)(CI)(RX)";
pub const APPCONTAINER_ACE_NAME: &str = "ALL APPLICATION PACKAGES";

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("lỗi HTTP: {0}")]
    Http(String),

    #[error("lỗi vào/ra: {0}")]
    Io(#[from] std::io::Error),

    #[error("manifest của Chrome for Testing không có bản win64 cho kênh Stable")]
    NoRelease,

    #[error("file tải về không khớp SHA-256 (chờ {expected}, nhận {actual})")]
    HashMismatch { expected: String, actual: String },

    #[error("giải nén thất bại: {0}")]
    Zip(String),

    #[error("cấp ACE AppContainer cho {0} thất bại: {1}")]
    Acl(String, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CftRelease {
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
}

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn sha256_file(path: &Path) -> Result<String, DownloadError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 128 * 1024];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[must_use]
pub fn verify_sha256(path: &Path, expected: &str) -> bool {
    match sha256_file(path) {
        Ok(actual) => actual.eq_ignore_ascii_case(expected.trim()),
        Err(_) => false,
    }
}

#[must_use]
pub fn parse_stable_release(manifest: &str) -> Option<CftRelease> {
    let value: serde_json::Value = serde_json::from_str(manifest).ok()?;
    let channel = value
        .get("channels")
        .and_then(|channels| channels.get("Stable"))
        .unwrap_or(&value);

    let version = channel.get("version")?.as_str()?.to_owned();
    let downloads = channel.get("downloads")?.get("chrome")?.as_array()?;

    let entry = downloads.iter().find(|item| {
        item.get("platform").and_then(|platform| platform.as_str()) == Some(CFT_WINDOWS_PLATFORM)
    })?;

    let url = entry.get("url")?.as_str()?.to_owned();
    let sha256 = entry
        .get("sha256")
        .and_then(|value| value.as_str())
        .map(str::to_owned);

    Some(CftRelease {
        version,
        url,
        sha256,
    })
}

pub fn fetch_stable_release() -> Result<CftRelease, DownloadError> {
    let body = reqwest::blocking::get(CFT_MANIFEST_URL)
        .map_err(|error| DownloadError::Http(error.to_string()))?;

    if !body.status().is_success() {
        return Err(DownloadError::Http(format!("HTTP {}", body.status())));
    }

    let text = body
        .text()
        .map_err(|error| DownloadError::Http(error.to_string()))?;

    parse_stable_release(&text).ok_or(DownloadError::NoRelease)
}

pub fn download_to_file(
    url: &str,
    destination: &Path,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), DownloadError> {
    let mut response =
        reqwest::blocking::get(url).map_err(|error| DownloadError::Http(error.to_string()))?;

    if !response.status().is_success() {
        return Err(DownloadError::Http(format!("HTTP {}", response.status())));
    }

    let total = response.content_length().unwrap_or(0);

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::File::create(destination)?;
    let mut buffer = vec![0u8; 128 * 1024];
    let mut downloaded = 0u64;

    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|error| DownloadError::Http(error.to_string()))?;
        if read == 0 {
            break;
        }

        file.write_all(&buffer[..read])?;
        downloaded += read as u64;
        progress(downloaded, total);
    }

    file.flush()?;
    Ok(())
}

pub fn extract_chrome(archive: &Path, destination: &Path) -> Result<PathBuf, DownloadError> {
    let file = std::fs::File::open(archive)?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|error| DownloadError::Zip(error.to_string()))?;

    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|error| DownloadError::Zip(error.to_string()))?;

        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let relative = relative.to_owned();

        if !relative.starts_with(WINDOWS_SUBDIR) {
            continue;
        }

        let target = destination.join(&relative);

        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut out = std::fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut out)?;
    }

    let binary = chrome_for_testing_binary(destination);
    if binary.is_file() {
        Ok(binary)
    } else {
        Err(DownloadError::Zip(format!(
            "gói tải về không có {WINDOWS_SUBDIR}/{WINDOWS_BINARY}"
        )))
    }
}

pub fn grant_app_container_access(directory: &Path) -> Result<(), DownloadError> {
    let output = std::process::Command::new("icacls")
        .arg(directory)
        .args(["/grant", APPCONTAINER_GRANT, "/t", "/c", "/q"])
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();

    Err(DownloadError::Acl(
        directory.display().to_string(),
        if detail.is_empty() {
            format!("mã thoát {:?}", output.status.code())
        } else {
            detail
        },
    ))
}

#[must_use]
pub fn has_app_container_access(directory: &Path) -> bool {
    let Ok(output) = std::process::Command::new("icacls").arg(directory).output() else {
        return false;
    };

    String::from_utf8_lossy(&output.stdout).contains(APPCONTAINER_ACE_NAME)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("muster-browser-download-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tao thu muc");
        dir
    }

    #[test]
    fn the_sha256_helper_matches_the_published_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn a_file_hash_is_verified_case_insensitively() {
        let dir = temp_dir("hash");
        let file = dir.join("blob.bin");
        std::fs::write(&file, b"abc").expect("ghi file");

        assert!(verify_sha256(
            &file,
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        ));
        assert!(!verify_sha256(&file, "00"));
        assert!(!verify_sha256(&dir.join("khong-ton-tai"), "00"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_manifest_yields_the_stable_win64_release() {
        let manifest = r#"{
            "channels": {
                "Stable": {
                    "channel": "Stable",
                    "version": "131.0.6778.85",
                    "downloads": {
                        "chrome": [
                            {"platform": "linux64", "url": "https://x/linux.zip"},
                            {"platform": "win64", "url": "https://x/win64.zip", "sha256": "ABCDEF"}
                        ]
                    }
                },
                "Beta": {"version": "132.0.0.0"}
            }
        }"#;

        let release = parse_stable_release(manifest).expect("doc duoc");
        assert_eq!(release.version, "131.0.6778.85");
        assert_eq!(release.url, "https://x/win64.zip");
        assert_eq!(release.sha256.as_deref(), Some("ABCDEF"));
    }

    #[test]
    fn a_manifest_without_a_hash_still_yields_a_release() {
        let manifest = r#"{"channels":{"Stable":{"version":"1","downloads":{"chrome":[
            {"platform":"win64","url":"https://x/win64.zip"}
        ]}}}}"#;

        let release = parse_stable_release(manifest).expect("doc duoc");
        assert!(release.sha256.is_none());
    }

    #[test]
    fn a_manifest_without_a_windows_build_is_refused() {
        let manifest = r#"{"channels":{"Stable":{"version":"1","downloads":{"chrome":[
            {"platform":"mac-arm64","url":"https://x/mac.zip"}
        ]}}}}"#;

        assert_eq!(parse_stable_release(manifest), None);
    }

    #[test]
    fn garbage_is_refused_instead_of_panicking() {
        assert_eq!(parse_stable_release("<html>"), None);
        assert_eq!(parse_stable_release("{}"), None);
    }

    #[test]
    fn extracting_a_zip_keeps_only_the_windows_folder_and_returns_the_binary() {
        let dir = temp_dir("extract");
        let archive_path = dir.join("chrome-win64.zip");

        {
            let file = std::fs::File::create(&archive_path).expect("tao zip");
            let mut writer = zip::ZipWriter::new(file);
            let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            writer
                .start_file(format!("{WINDOWS_SUBDIR}/{WINDOWS_BINARY}"), options)
                .expect("them chrome.exe");
            writer.write_all(b"stub-binary").expect("ghi noi dung");

            writer
                .start_file("chrome-win64/icudtl.dat", options)
                .expect("them icudtl");
            writer.write_all(b"data").expect("ghi noi dung");

            writer
                .start_file("chrome-linux64/chrome", options)
                .expect("them ban linux");
            writer.write_all(b"nope").expect("ghi noi dung");

            writer.finish().expect("dong zip");
        }

        let destination = dir.join("131.0.6778.85");
        let binary = extract_chrome(&archive_path, &destination).expect("giai nen");

        assert_eq!(
            binary,
            destination.join(WINDOWS_SUBDIR).join(WINDOWS_BINARY)
        );
        assert_eq!(std::fs::read(&binary).expect("doc"), b"stub-binary");
        assert!(
            destination
                .join(WINDOWS_SUBDIR)
                .join("icudtl.dat")
                .is_file()
        );
        assert!(
            !destination.join("chrome-linux64").exists(),
            "khong lay ban linux"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_zip_without_the_windows_binary_is_reported() {
        let dir = temp_dir("extract-missing");
        let archive_path = dir.join("bad.zip");

        {
            let file = std::fs::File::create(&archive_path).expect("tao zip");
            let mut writer = zip::ZipWriter::new(file);
            let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            writer.start_file("doc.txt", options).expect("them");
            writer.write_all(b"khong phai chrome").expect("ghi");
            writer.finish().expect("dong zip");
        }

        let error = extract_chrome(&archive_path, &dir.join("out")).expect_err("phai loi");
        assert!(matches!(error, DownloadError::Zip(_)), "gap: {error:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unpacked_directory_gets_the_app_container_ace() {
        let dir = temp_dir("acl");
        let binary_dir = dir.join(WINDOWS_SUBDIR);
        std::fs::create_dir_all(&binary_dir).expect("tao thu muc");
        let binary = binary_dir.join(WINDOWS_BINARY);
        std::fs::write(&binary, b"stub").expect("ghi file");

        grant_app_container_access(&dir).expect("cap ACE");

        assert!(
            has_app_container_access(&dir),
            "thu muc cai phai co ACE cho {APPCONTAINER_ACE_NAME}"
        );
        assert!(
            has_app_container_access(&binary),
            "chrome.exe phai huong ACE tu thu muc cha"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_directory_reports_no_app_container_access() {
        let dir = temp_dir("acl-missing");
        let absent = dir.join("khong-ton-tai");

        assert!(!has_app_container_access(&absent));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
