use std::path::{Path, PathBuf};

pub const CHROME_FOR_TESTING_DIR: &str = "browser/chrome-for-testing";
pub const WINDOWS_SUBDIR: &str = "chrome-win64";
pub const WINDOWS_BINARY: &str = "chrome.exe";

#[must_use]
pub fn chrome_for_testing_root(data_dir: &Path) -> PathBuf {
    data_dir.join(CHROME_FOR_TESTING_DIR)
}

#[must_use]
pub fn chrome_for_testing_binary(version_dir: &Path) -> PathBuf {
    version_dir.join(WINDOWS_SUBDIR).join(WINDOWS_BINARY)
}

#[must_use]
pub fn installed_chrome_for_testing(data_dir: &Path) -> Option<PathBuf> {
    let root = chrome_for_testing_root(data_dir);
    let entries = std::fs::read_dir(&root).ok()?;

    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| chrome_for_testing_binary(&entry.path()))
        .filter(|candidate| candidate.is_file())
        .collect();

    found.sort();
    found.pop()
}

#[must_use]
pub fn installed_version(data_dir: &Path) -> Option<String> {
    let binary = installed_chrome_for_testing(data_dir)?;
    let version_dir = binary.parent()?.parent()?;
    Some(version_dir.file_name()?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chrome_for_testing_layout_matches_the_zip_we_unpack() {
        let data_dir = Path::new("C:/data");
        let root = chrome_for_testing_root(data_dir);

        assert_eq!(root, Path::new("C:/data/browser/chrome-for-testing"));
        assert_eq!(
            chrome_for_testing_binary(&root.join("131.0.6778.85")),
            root.join("131.0.6778.85")
                .join("chrome-win64")
                .join("chrome.exe")
        );
    }

    #[test]
    fn a_missing_install_directory_reads_as_not_installed() {
        let dir = std::env::temp_dir().join("muster-browser-detect-empty");
        let _ = std::fs::remove_dir_all(&dir);

        assert!(installed_chrome_for_testing(&dir).is_none());
        assert!(installed_version(&dir).is_none());
    }

    #[test]
    fn a_planted_install_is_found_and_its_version_reported() {
        let data_dir = std::env::temp_dir().join("muster-browser-detect-planted");
        let version_dir = chrome_for_testing_root(&data_dir).join("131.0.6778.85");
        std::fs::create_dir_all(version_dir.join(WINDOWS_SUBDIR)).expect("tao thu muc");
        let binary = version_dir.join(WINDOWS_SUBDIR).join(WINDOWS_BINARY);
        std::fs::write(&binary, b"stub").expect("ghi file");

        assert_eq!(installed_chrome_for_testing(&data_dir), Some(binary));
        assert_eq!(
            installed_version(&data_dir).as_deref(),
            Some("131.0.6778.85")
        );

        let _ = std::fs::remove_dir_all(&data_dir);
    }
}
