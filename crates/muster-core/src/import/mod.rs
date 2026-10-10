//! # `import` — nhập dữ liệu từ một bản cài Roblox Account Manager
//!
//! Bốn yêu cầu bắt buộc ở `ROADMAP.md` mục 4.1, và cách module này đáp ứng:
//!
//! | Yêu cầu | Cách làm |
//! | :--- | :--- |
//! | Nhập **không phá huỷ** | Chỉ đọc file của RAM. Mọi thứ ghi ra đều nằm trong `out_dir` |
//! | Sao lưu TRƯỚC khi ghi | [`migrate`] tạo ZIP trước khi ghi byte nào |
//! | Báo cáo nhập | [`MigrationReport`] và [`ImportedAccounts::warnings`] |
//! | Giữ nguyên `Fields` và các field khác | Do `models::Account` bảo toàn, xem module đó |
//!
//! ## Vì sao logic nằm ở core chứ không ở CLI
//!
//! Wizard trong UI sẽ cần đúng những bước này. Nếu để ở `muster-cli` thì UI phải viết lại, và
//! hai bản sẽ lệch nhau. Nguyên tắc bất biến #3 nói logic phải ở core.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::Account;
use crate::settings::{self, IniFile, Settings};
use crate::vault::{self, Format, VaultError};

/// File chính chứa danh sách tài khoản.
pub const ACCOUNT_FILE: &str = "AccountData.json";
/// Danh sách game gần đây, file riêng.
pub const RECENT_GAMES_FILE: &str = "RecentGames.json";
/// Toàn bộ cấu hình của RAM.
pub const SETTINGS_FILE: &str = "RAMSettings.ini";
/// File cờ: có nó thì RAM ghi plaintext, không có thì ghi DPAPI LocalMachine (`AccountManager.cs:377-380`).
pub const NO_ENCRYPTION_FLAG: &str = "NoEncryption.IUnderstandTheRisks.iautamor";

/// Các hậu tố của bản sao `AccountData.json` mà RAM có thể để lại.
pub const ACCOUNT_COPY_SUFFIXES: [&str; 3] = ["backup", "old", "bak"];

/// Tên thư mục dữ liệu của ứng dụng trong `%APPDATA%`.
///
/// Trùng với codename tạm trong `[workspace.metadata.project]`, nên `tools/rename.sh` sẽ đổi
/// theo khi chốt tên thật.
pub const APP_DIR_NAME: &str = "muster";

/// Lỗi của lớp nhập liệu.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// Thư mục người dùng trỏ tới không tồn tại.
    #[error("thư mục không tồn tại: {0}")]
    MissingRoot(PathBuf),

    /// Không tìm thấy `AccountData.json` trong thư mục đó.
    #[error("không tìm thấy {ACCOUNT_FILE} trong {0}")]
    NoAccountFile(PathBuf),

    /// File gốc là nhánh password-locked nhưng không có mật khẩu để ghi lại.
    #[error("cần mật khẩu để ghi lại file ở nhánh password-locked")]
    PasswordRequiredForWrite,

    /// Lỗi từ lớp vault.
    #[error(transparent)]
    Vault(#[from] VaultError),

    /// Lỗi từ lớp settings.
    #[error(transparent)]
    Settings(#[from] settings::SettingsError),

    /// Lỗi nén ZIP.
    #[error("lỗi ZIP: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Lỗi đọc/ghi file.
    #[error("lỗi vào/ra: {0}")]
    Io(#[from] std::io::Error),
}

/// Những file tìm thấy trong một bản cài RAM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RamInstall {
    /// Thư mục gốc đã quét.
    pub root: PathBuf,
    /// `AccountData.json` — file chính.
    pub account_file: Option<PathBuf>,
    /// `AccountData.json.backup` / `.old` / `.bak`, theo thứ tự đó.
    pub account_copies: Vec<PathBuf>,
    /// `RecentGames.json`.
    pub recent_games: Option<PathBuf>,
    /// `RAMSettings.ini`.
    pub settings: Option<PathBuf>,
    /// Có file cờ `NoEncryption...` hay không.
    pub no_encryption_flag: bool,
}

impl RamInstall {
    /// Mọi file cần đưa vào bản sao lưu ZIP.
    #[must_use]
    pub fn files_to_back_up(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        files.extend(self.account_file.iter().cloned());
        files.extend(self.account_copies.iter().cloned());
        files.extend(self.recent_games.iter().cloned());
        files.extend(self.settings.iter().cloned());

        if self.no_encryption_flag {
            files.push(self.root.join(NO_ENCRYPTION_FLAG));
        }

        files
    }

    /// `true` nếu không tìm thấy file nào của RAM.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.account_file.is_none()
            && self.account_copies.is_empty()
            && self.recent_games.is_none()
            && self.settings.is_none()
            && !self.no_encryption_flag
    }
}

/// Quét một thư mục để tìm các file của RAM.
///
/// Không đọc nội dung, chỉ kiểm tra sự tồn tại — nên gọi được trước khi biết mật khẩu.
#[must_use]
pub fn discover(root: &Path) -> RamInstall {
    let existing = |name: &str| {
        let path = root.join(name);
        path.is_file().then_some(path)
    };

    RamInstall {
        root: root.to_path_buf(),
        account_file: existing(ACCOUNT_FILE),
        account_copies: ACCOUNT_COPY_SUFFIXES
            .iter()
            .filter_map(|suffix| existing(&format!("{ACCOUNT_FILE}.{suffix}")))
            .collect(),
        recent_games: existing(RECENT_GAMES_FILE),
        settings: existing(SETTINGS_FILE),
        no_encryption_flag: root.join(NO_ENCRYPTION_FLAG).is_file(),
    }
}

/// Kết quả đọc danh sách tài khoản từ một bản cài RAM.
#[derive(Debug)]
pub struct ImportedAccounts {
    /// Tài khoản đọc được.
    pub accounts: Vec<Account>,
    /// Nhánh định dạng đã dùng để mở.
    pub format: Format,
    /// File thực sự đã đọc — khác `AccountData.json` khi phải dùng bản sao.
    pub source: PathBuf,
    /// Những điều người dùng nên biết: đọc từ bản sao, file plaintext, v.v.
    pub warnings: Vec<String>,
}

/// Đọc danh sách tài khoản, tự chuyển sang bản sao nếu file chính hỏng.
///
/// Thứ tự thử: `AccountData.json`, rồi `.backup`, `.old`, `.bak`. Dùng được bản sao thì ghi lại
/// một cảnh báo — người dùng cần biết mình đang khôi phục từ bản cũ chứ không phải file chính.
///
/// # Errors
///
/// Trả [`ImportError::NoAccountFile`] nếu không có file nào, hoặc lỗi của lần thử cuối cùng nếu
/// tất cả đều hỏng.
pub fn read_accounts(
    install: &RamInstall,
    password: Option<&str>,
) -> Result<ImportedAccounts, ImportError> {
    let mut candidates: Vec<&PathBuf> = Vec::new();
    candidates.extend(install.account_file.iter());
    candidates.extend(install.account_copies.iter());

    if candidates.is_empty() {
        return Err(ImportError::NoAccountFile(install.root.clone()));
    }

    let mut warnings = Vec::new();
    let mut last_error = None;

    for (index, candidate) in candidates.iter().enumerate() {
        match vault::read_accounts_from_path(candidate, password) {
            Ok((accounts, format)) => {
                if index > 0 {
                    warnings.push(format!(
                        "không đọc được {ACCOUNT_FILE}, đã dùng bản sao {}",
                        candidate
                            .file_name()
                            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
                    ));
                }
                if format == Format::PlainJson {
                    warnings.push(
                        "file đang ở nhánh plain JSON — cookie nằm trên đĩa dưới dạng không mã hoá. \
                         Nên chuyển sang nhánh password-locked."
                            .to_owned(),
                    );
                }
                if format.is_machine_bound() {
                    warnings.push(
                        "file đang ở nhánh DPAPI — chỉ mở được trên đúng máy và tài khoản Windows \
                         này. Đổi máy sẽ không đọc được nữa; nên chuyển sang nhánh password-locked."
                            .to_owned(),
                    );
                }

                return Ok(ImportedAccounts {
                    accounts,
                    format,
                    source: (*candidate).clone(),
                    warnings,
                });
            }
            Err(error) => {
                warnings.push(format!(
                    "bỏ qua {}: {error}",
                    candidate
                        .file_name()
                        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
                ));
                last_error = Some(error);
            }
        }
    }

    Err(last_error.map_or(
        ImportError::NoAccountFile(install.root.clone()),
        ImportError::Vault,
    ))
}

/// Đóng gói toàn bộ file của bản cài RAM vào một ZIP.
///
/// Trả về đường dẫn file ZIP đã tạo. Tên: `<codename>-backup-<unix_giây>.zip`.
///
/// # Errors
///
/// Trả lỗi I/O hoặc lỗi ZIP nếu không ghi được.
pub fn backup_zip(install: &RamInstall, destination_dir: &Path) -> Result<PathBuf, ImportError> {
    std::fs::create_dir_all(destination_dir)?;

    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |delta| delta.as_secs());
    let archive = destination_dir.join(format!("{APP_DIR_NAME}-backup-{seconds}.zip"));

    let file = std::fs::File::create(&archive)?;
    let mut zip = zip::ZipWriter::new(file);

    for path in install.files_to_back_up() {
        let Some(name) = path.file_name() else {
            continue;
        };
        let data = std::fs::read(&path)?;
        zip.start_file(
            name.to_string_lossy(),
            zip::write::SimpleFileOptions::default(),
        )?;
        zip.write_all(&data)?;
    }

    zip.finish()?;
    Ok(archive)
}

/// Báo cáo sau khi chuyển dữ liệu.
#[derive(Debug)]
pub struct MigrationReport {
    /// Thư mục đã ghi dữ liệu mới vào.
    pub out_dir: PathBuf,
    /// File ZIP sao lưu dữ liệu gốc của RAM.
    pub backup: PathBuf,
    /// Số tài khoản đã ghi.
    pub accounts_written: usize,
    /// Nhánh định dạng đã dùng khi ghi lại.
    pub format: Format,
    /// `true` nếu đã ghi `settings.toml` từ `RAMSettings.ini`.
    pub settings_written: bool,
    /// Cảnh báo cần hiển thị cho người dùng.
    pub warnings: Vec<String>,
}

/// Chuyển dữ liệu từ một bản cài RAM sang thư mục dữ liệu của `<PROJECT>`.
///
/// Thứ tự bắt buộc: **sao lưu ZIP trước**, rồi mới ghi. File gốc của RAM không bị sửa.
///
/// Nhánh mã hoá được **giữ nguyên**: file password-locked ghi lại thành password-locked, file
/// DPAPI ghi lại bằng DPAPI LocalMachine (đúng mặc định ghi của RAM), file plaintext ghi lại
/// plaintext kèm cảnh báo. Nhờ vậy việc nhập không tự ý đổi mức bảo vệ mà người dùng đang có.
///
/// # Errors
///
/// Trả [`ImportError::PasswordRequiredForWrite`] nếu file gốc là nhánh password-locked mà không
/// có mật khẩu, hoặc lỗi I/O khi ghi.
pub fn migrate(
    install: &RamInstall,
    out_dir: &Path,
    password: Option<&str>,
) -> Result<MigrationReport, ImportError> {
    std::fs::create_dir_all(out_dir)?;

    // Buoc 1 — sao luu TRUOC khi ghi bat cu thu gi.
    let backup = backup_zip(install, out_dir)?;

    // Buoc 2 — doc du lieu goc.
    let imported = read_accounts(install, password)?;
    let mut warnings = imported.warnings;

    // Buoc 3 — cau hinh.
    let mut settings_written = false;
    if let Some(ini_path) = install.settings.as_ref() {
        let ini = IniFile::from_path(ini_path)?;
        let migrated = Settings::from_ini(&ini);
        migrated.save(&out_dir.join("settings.toml"))?;
        settings_written = true;

        if ini
            .get("WebServer", "Password")
            .is_some_and(|pw| pw.len() < 6)
        {
            warnings.push(
                "RAMSettings.ini có [WebServer] Password ngắn hơn 6 ký tự — RAM sẽ từ chối dùng \
                 mật khẩu đó cho local API."
                    .to_owned(),
            );
        }
    }

    // Buoc 4 — ghi lai danh sach tai khoan, giu nguyen nhanh ma hoa.
    let plaintext = vault::accounts_to_bytes(&imported.accounts)?;
    let sealed = seal_like(&plaintext, imported.format, password, &mut warnings)?;
    std::fs::write(out_dir.join(ACCOUNT_FILE), sealed)?;

    if install.no_encryption_flag {
        warnings.push(
            "bản cài RAM có file cờ NoEncryption... nghĩa là RAM đang ghi plaintext. \
             <PROJECT> không tự tạo file cờ này."
                .to_owned(),
        );
    }

    Ok(MigrationReport {
        out_dir: out_dir.to_path_buf(),
        backup,
        accounts_written: imported.accounts.len(),
        format: imported.format,
        settings_written,
        warnings,
    })
}

/// Mã hoá `plaintext` theo đúng nhánh của file gốc.
fn seal_like(
    plaintext: &[u8],
    format: Format,
    password: Option<&str>,
    warnings: &mut Vec<String>,
) -> Result<Vec<u8>, ImportError> {
    match format {
        Format::PasswordLocked => {
            let password = password.ok_or(ImportError::PasswordRequiredForWrite)?;
            Ok(vault::seal_password_locked(plaintext, password)?)
        }
        Format::PlainJson => {
            warnings.push(
                "ghi lại ở nhánh plain JSON vì file gốc cũng vậy — dữ liệu không được mã hoá."
                    .to_owned(),
            );
            Ok(plaintext.to_vec())
        }
        Format::Dpapi => {
            #[cfg(windows)]
            {
                Ok(vault::seal_dpapi(
                    plaintext,
                    vault::dpapi::Scope::LocalMachine,
                )?)
            }
            #[cfg(not(windows))]
            {
                let _ = warnings;
                Err(ImportError::Vault(VaultError::UnknownFormat))
            }
        }
    }
}

/// Thư mục dữ liệu mặc định của ứng dụng: `%APPDATA%\<codename>`.
///
/// Trả về thư mục hiện tại khi không đọc được `%APPDATA%` (ví dụ chạy trên nền khác Windows)
/// thay vì panic.
#[must_use]
pub fn default_data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(APP_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    /// Dựng một thư mục giả lập bản cài RAM, dùng chính fixture có sẵn của repo.
    fn fake_install(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("muster-import-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tạo thư mục");

        std::fs::copy(fixture("accountdata.plain.json"), dir.join(ACCOUNT_FILE))
            .expect("chép AccountData.json");
        std::fs::copy(
            fixture("accountdata.password-locked.bin"),
            dir.join(format!("{ACCOUNT_FILE}.backup")),
        )
        .expect("chép bản sao");
        std::fs::write(
            dir.join(SETTINGS_FILE),
            "[General]\nSavePasswords=false\n[WebServer]\nWebServerPort=8080\n",
        )
        .expect("ghi RAMSettings.ini");
        std::fs::write(dir.join(RECENT_GAMES_FILE), "[]").expect("ghi RecentGames.json");

        dir
    }

    fn fixture_password() -> String {
        let raw = std::fs::read(fixture("manifest.json")).expect("đọc manifest");
        let manifest: serde_json::Value = serde_json::from_slice(&raw).expect("JSON");
        manifest["fixtures"]
            .as_array()
            .expect("mảng")
            .iter()
            .find(|entry| entry["kind"] == "password-locked")
            .and_then(|entry| entry["password"].as_str())
            .expect("mật khẩu fixture")
            .to_owned()
    }

    #[test]
    fn discover_finds_every_file_ram_may_leave_behind() {
        let dir = fake_install("discover");
        let install = discover(&dir);

        assert_eq!(
            install.account_file.as_deref(),
            Some(dir.join(ACCOUNT_FILE).as_path())
        );
        assert_eq!(install.account_copies.len(), 1);
        assert!(install.recent_games.is_some());
        assert!(install.settings.is_some());
        assert!(!install.no_encryption_flag);
        assert!(!install.is_empty());
        assert_eq!(install.files_to_back_up().len(), 4);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_on_an_empty_directory_is_empty() {
        let dir = std::env::temp_dir().join("muster-import-empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tạo thư mục");

        let install = discover(&dir);
        assert!(install.is_empty());
        assert!(install.files_to_back_up().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_notices_the_no_encryption_flag() {
        let dir = fake_install("flag");
        std::fs::write(dir.join(NO_ENCRYPTION_FLAG), "").expect("ghi cờ");

        let install = discover(&dir);
        assert!(install.no_encryption_flag);
        assert_eq!(install.files_to_back_up().len(), 5);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_accounts_opens_the_main_file() {
        let dir = fake_install("read");
        let install = discover(&dir);
        let imported = read_accounts(&install, None).expect("đọc");

        assert_eq!(imported.accounts.len(), 2);
        assert_eq!(imported.format, Format::PlainJson);
        assert_eq!(imported.source, dir.join(ACCOUNT_FILE));
        assert!(
            imported.warnings.iter().any(|w| w.contains("plain JSON")),
            "phải cảnh báo về plaintext: {:?}",
            imported.warnings
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_accounts_falls_back_to_a_copy_when_the_main_file_is_broken() {
        let dir = fake_install("fallback");
        // Lam hong file chinh: khong con la plain JSON, khong phai header RAM, khong phai DPAPI.
        std::fs::write(dir.join(ACCOUNT_FILE), [0x01, 0x02, 0x03, 0x04]).expect("ghi file hong");
        // Ban sao .backup cung doi thanh plain JSON hop le de kiem nhanh du phong.
        std::fs::copy(
            fixture("accountdata.plain.json"),
            dir.join(format!("{ACCOUNT_FILE}.backup")),
        )
        .expect("chép bản sao");

        let install = discover(&dir);
        let imported = read_accounts(&install, None).expect("phải đọc được bản sao");

        assert_eq!(imported.accounts.len(), 2);
        assert!(imported.source.ends_with(format!("{ACCOUNT_FILE}.backup")));
        assert!(
            imported.warnings.iter().any(|w| w.contains("bản sao")),
            "phải cảnh báo đã dùng bản sao: {:?}",
            imported.warnings
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_accounts_reports_a_directory_without_any_account_file() {
        let dir = std::env::temp_dir().join("muster-import-noaccount");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tạo thư mục");

        let install = discover(&dir);
        let error = read_accounts(&install, None).expect_err("phải từ chối");
        assert!(matches!(error, ImportError::NoAccountFile(_)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_accounts_warns_about_the_dpapi_branch() {
        let dir = fake_install("dpapi");
        std::fs::copy(
            fixture("accountdata.dpapi-currentuser.bin"),
            dir.join(ACCOUNT_FILE),
        )
        .expect("chép fixture DPAPI");

        let install = discover(&dir);
        let Ok(imported) = read_accounts(&install, None) else {
            // Blob DPAPI gắn với máy đã tạo ra nó. Máy khác không mở được là chuyện bình thường,
            // không phải lỗi của code — xem ghi chú ở tests/vault_fixtures.rs.
            eprintln!("bỏ qua: máy này không giải mã được fixture DPAPI");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };

        assert_eq!(imported.format, Format::Dpapi);
        assert!(
            imported.warnings.iter().any(|w| w.contains("DPAPI")),
            "phải cảnh báo DPAPI gắn với máy: {:?}",
            imported.warnings
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_zip_contains_every_discovered_file() {
        let dir = fake_install("zip");
        let out = dir.join("out");
        let install = discover(&dir);

        let archive = backup_zip(&install, &out).expect("nén ZIP");
        assert!(archive.exists(), "file ZIP phải tồn tại");

        let file = std::fs::File::open(&archive).expect("mở ZIP");
        let mut zip = zip::ZipArchive::new(file).expect("đọc ZIP");
        let mut names: Vec<String> = (0..zip.len())
            .filter_map(|index| {
                zip.by_index(index)
                    .ok()
                    .map(|entry| entry.name().to_owned())
            })
            .collect();
        names.sort();

        assert_eq!(
            names,
            vec![
                ACCOUNT_FILE.to_owned(),
                format!("{ACCOUNT_FILE}.backup"),
                // Thu tu theo byte: 'A' (0x41) < 'e' (0x65) nen "RAMSettings.ini" dung truoc
                // "RecentGames.json".
                SETTINGS_FILE.to_owned(),
                RECENT_GAMES_FILE.to_owned(),
            ]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_writes_a_backup_a_settings_file_and_the_accounts() {
        let dir = fake_install("migrate");
        let out = dir.join("out");
        let install = discover(&dir);

        let report = migrate(&install, &out, None).expect("nhập");

        assert_eq!(report.accounts_written, 2);
        assert!(report.settings_written);
        assert!(report.backup.exists());
        assert!(out.join(ACCOUNT_FILE).exists());
        assert!(out.join("settings.toml").exists());

        let settings = Settings::load(&out.join("settings.toml")).expect("đọc settings");
        assert_eq!(settings.web_server.web_server_port, 8080);
        assert!(!settings.general.save_passwords);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_does_not_touch_the_original_files() {
        let dir = fake_install("nondestructive");
        let out = dir.join("out");
        let install = discover(&dir);

        let before: Vec<(PathBuf, Vec<u8>)> = install
            .files_to_back_up()
            .into_iter()
            .map(|path| {
                let data = std::fs::read(&path).expect("đọc file gốc");
                (path, data)
            })
            .collect();

        migrate(&install, &out, None).expect("nhập");

        for (path, data) in before {
            assert_eq!(
                std::fs::read(&path).expect("đọc lại file gốc"),
                data,
                "file gốc bị sửa: {}",
                path.display()
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_preserves_the_password_locked_branch() {
        let dir = fake_install("preserve");
        let password = fixture_password();
        std::fs::copy(
            fixture("accountdata.password-locked.bin"),
            dir.join(ACCOUNT_FILE),
        )
        .expect("chép fixture password-locked");

        let out = dir.join("out");
        let install = discover(&dir);
        let report = migrate(&install, &out, Some(&password)).expect("nhập");

        assert_eq!(report.format, Format::PasswordLocked);

        // File ghi ra phải mở được bằng đúng mật khẩu đó, và cho ra đúng dữ liệu.
        let (accounts, format) =
            vault::read_accounts_from_path(&out.join(ACCOUNT_FILE), Some(&password))
                .expect("đọc lại file đã ghi");
        assert_eq!(format, Format::PasswordLocked);
        assert_eq!(accounts.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_refuses_to_write_password_locked_without_a_password() {
        let dir = fake_install("nopassword");
        std::fs::copy(
            fixture("accountdata.password-locked.bin"),
            dir.join(ACCOUNT_FILE),
        )
        .expect("chép fixture password-locked");

        let out = dir.join("out");
        let install = discover(&dir);
        let error = migrate(&install, &out, None).expect_err("phải từ chối");

        assert!(matches!(
            error,
            ImportError::Vault(VaultError::PasswordRequired)
        ));
        // Sao luu van phai duoc tao truoc khi that bai.
        assert!(out.exists(), "thư mục ra phải được tạo trước khi ghi");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_survives_an_install_without_a_settings_file() {
        let dir = fake_install("nosettings");
        std::fs::remove_file(dir.join(SETTINGS_FILE)).expect("xoá RAMSettings.ini");

        let out = dir.join("out");
        let install = discover(&dir);
        let report = migrate(&install, &out, None).expect("nhập");

        assert!(!report.settings_written);
        assert!(!out.join("settings.toml").exists());
        assert_eq!(report.accounts_written, 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_warns_when_ram_had_the_no_encryption_flag() {
        let dir = fake_install("flagmigrate");
        std::fs::write(dir.join(NO_ENCRYPTION_FLAG), "").expect("ghi cờ");

        let out = dir.join("out");
        let install = discover(&dir);
        let report = migrate(&install, &out, None).expect("nhập");

        assert!(
            report.warnings.iter().any(|w| w.contains("NoEncryption")),
            "phải cảnh báo về file cờ: {:?}",
            report.warnings
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_data_dir_ends_with_the_app_directory_name() {
        assert!(default_data_dir().ends_with(APP_DIR_NAME));
    }
}
