#[cfg(windows)]
pub mod dpapi;

use std::path::{Path, PathBuf};

use crypto_secretbox::XSalsa20Poly1305;
use crypto_secretbox::aead::{AeadInPlace, KeyInit, generic_array::GenericArray};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::models::Account;

pub const RAM_HEADER: &[u8] = b"Roblox Account Manager created by ic3w0lf22 @ github.com .......";

pub const DPAPI_ENTROPY: &[u8] = b"ROBLOX ACCOUNT MANAGER | :) | BROUGHT TO YOU BUY ic3w0lf";

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
pub const ARGON2_OUT_LEN: usize = 32;
pub const ARGON2_T_COST: u32 = 6;
pub const ARGON2_M_COST_KIB: u32 = 131_072;
pub const ARGON2_P_COST: u32 = 1;

pub const MAC_LEN: usize = 16;

pub const SALT_OFFSET: usize = RAM_HEADER.len();
pub const NONCE_OFFSET: usize = SALT_OFFSET + SALT_LEN;
pub const CIPHERTEXT_OFFSET: usize = NONCE_OFFSET + NONCE_LEN;
pub const MIN_PASSWORD_LOCKED_LEN: usize = CIPHERTEXT_OFFSET + MAC_LEN;

const _: () = assert!(RAM_HEADER.len() == 64);
const _: () = assert!(DPAPI_ENTROPY.len() == 56);
const _: () = assert!(CIPHERTEXT_OFFSET == 104);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Branch {
    PasswordLocked,
    Dpapi,
    PlainJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Format {
    PasswordLocked,
    Dpapi,
    PlainJson,
}

impl Format {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PasswordLocked => "password-locked",
            Self::Dpapi => "dpapi",
            Self::PlainJson => "plain-json",
        }
    }

    #[must_use]
    pub const fn is_machine_bound(self) -> bool {
        matches!(self, Self::Dpapi)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("file không khớp magic RAMHeader nên không phải nhánh password-locked")]
    BadHeader,

    #[error("file quá ngắn: {len} byte, cần tối thiểu {min} byte cho nhánh password-locked")]
    TooShort { len: usize, min: usize },

    #[error("file được khoá bằng mật khẩu, cần cung cấp mật khẩu để mở")]
    PasswordRequired,

    #[error("dẫn xuất khoá Argon2 thất bại")]
    Argon2,

    #[error("giải mã thất bại: sai mật khẩu hoặc file bị hỏng")]
    Decrypt,

    #[error("không lấy được số ngẫu nhiên từ hệ điều hành")]
    Random,

    #[cfg(windows)]
    #[error(transparent)]
    Dpapi(#[from] dpapi::DpapiError),

    #[error(
        "không nhận ra định dạng file: không phải plain JSON, không phải password-locked, và DPAPI từ chối giải mã"
    )]
    UnknownFormat,

    #[error("JSON không hợp lệ: {0}")]
    Json(#[from] serde_json::Error),

    #[error("lỗi vào/ra: {0}")]
    Io(#[from] std::io::Error),
}

#[must_use]
pub fn detect_branch(data: &[u8]) -> Branch {
    if data.starts_with(RAM_HEADER) {
        Branch::PasswordLocked
    } else if data.first() == Some(&b'[') {
        Branch::PlainJson
    } else {
        Branch::Dpapi
    }
}

pub fn derive_key(
    password: &str,
    salt: &[u8; SALT_LEN],
) -> Result<Zeroizing<[u8; ARGON2_OUT_LEN]>, VaultError> {
    use argon2::{Algorithm, Argon2, Params, Version};
    use sha2::{Digest, Sha512};

    let digest = Sha512::digest(password.as_bytes());

    let params = Params::new(
        ARGON2_M_COST_KIB,
        ARGON2_T_COST,
        ARGON2_P_COST,
        Some(ARGON2_OUT_LEN),
    )
    .map_err(|_| VaultError::Argon2)?;

    let argon2 = Argon2::new(Algorithm::Argon2i, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; ARGON2_OUT_LEN]);
    argon2
        .hash_password_into(&digest, salt, key.as_mut())
        .map_err(|_| VaultError::Argon2)?;

    Ok(key)
}

fn secretbox(key: &[u8; ARGON2_OUT_LEN]) -> Result<XSalsa20Poly1305, VaultError> {
    XSalsa20Poly1305::new_from_slice(key).map_err(|_| VaultError::Argon2)
}

fn random_bytes(buffer: &mut [u8]) -> Result<(), VaultError> {
    getrandom::getrandom(buffer).map_err(|_| VaultError::Random)
}

pub fn open_password_locked(data: &[u8], password: &str) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    if !data.starts_with(RAM_HEADER) {
        return Err(VaultError::BadHeader);
    }
    if data.len() < MIN_PASSWORD_LOCKED_LEN {
        return Err(VaultError::TooShort {
            len: data.len(),
            min: MIN_PASSWORD_LOCKED_LEN,
        });
    }

    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&data[SALT_OFFSET..NONCE_OFFSET]);
    let nonce = &data[NONCE_OFFSET..CIPHERTEXT_OFFSET];

    let (tag, ciphertext) = data[CIPHERTEXT_OFFSET..].split_at(MAC_LEN);

    let key = derive_key(password, &salt)?;
    let cipher = secretbox(&key)?;

    let mut buffer = Zeroizing::new(ciphertext.to_vec());
    cipher
        .decrypt_in_place_detached(
            GenericArray::from_slice(nonce),
            b"",
            &mut buffer,
            GenericArray::from_slice(tag),
        )
        .map_err(|_| VaultError::Decrypt)?;

    Ok(buffer)
}

pub fn seal_password_locked(plaintext: &[u8], password: &str) -> Result<Vec<u8>, VaultError> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    random_bytes(&mut salt)?;
    random_bytes(&mut nonce)?;

    let key = derive_key(password, &salt)?;
    let cipher = secretbox(&key)?;

    let mut buffer = Zeroizing::new(plaintext.to_vec());
    let tag = cipher
        .encrypt_in_place_detached(GenericArray::from_slice(&nonce), b"", &mut buffer)
        .map_err(|_| VaultError::Decrypt)?;

    let mut out = Vec::with_capacity(MIN_PASSWORD_LOCKED_LEN + buffer.len());
    out.extend_from_slice(RAM_HEADER);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&buffer);
    Ok(out)
}

pub struct Opened {
    pub plaintext: Zeroizing<Vec<u8>>,
    pub format: Format,
}

impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened")
            .field("format", &self.format)
            .field(
                "plaintext",
                &format_args!("<{} byte, đã ẩn>", self.plaintext.len()),
            )
            .finish()
    }
}

pub fn open(data: &[u8], password: Option<&str>) -> Result<Opened, VaultError> {
    if data.first() == Some(&b'[') {
        return Ok(Opened {
            plaintext: Zeroizing::new(data.to_vec()),
            format: Format::PlainJson,
        });
    }

    if data.starts_with(RAM_HEADER) {
        let password = password.ok_or(VaultError::PasswordRequired)?;
        return Ok(Opened {
            plaintext: open_password_locked(data, password)?,
            format: Format::PasswordLocked,
        });
    }

    #[cfg(windows)]
    {
        use dpapi::Scope;
        let attempt = dpapi::unprotect(data, DPAPI_ENTROPY, Scope::CurrentUser)
            .or_else(|_| dpapi::unprotect(data, DPAPI_ENTROPY, Scope::LocalMachine));

        if let Ok(plaintext) = attempt {
            return Ok(Opened {
                plaintext,
                format: Format::Dpapi,
            });
        }
    }

    Err(VaultError::UnknownFormat)
}

pub fn read_accounts(
    data: &[u8],
    password: Option<&str>,
) -> Result<(Vec<Account>, Format), VaultError> {
    let opened = open(data, password)?;
    let accounts: Vec<Account> = serde_json::from_slice(&opened.plaintext)?;
    Ok((accounts, opened.format))
}

pub fn read_accounts_from_path(
    path: &Path,
    password: Option<&str>,
) -> Result<(Vec<Account>, Format), VaultError> {
    let data = std::fs::read(path)?;
    read_accounts(&data, password)
}

pub fn accounts_to_bytes(accounts: &[Account]) -> Result<Vec<u8>, VaultError> {
    Ok(serde_json::to_vec(accounts)?)
}

#[cfg(windows)]
pub fn seal_dpapi(plaintext: &[u8], scope: dpapi::Scope) -> Result<Vec<u8>, VaultError> {
    Ok(dpapi::protect(plaintext, DPAPI_ENTROPY, scope)?)
}

pub fn seal_default(plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
    #[cfg(windows)]
    {
        seal_dpapi(plaintext, dpapi::Scope::LocalMachine)
    }
    #[cfg(not(windows))]
    {
        let _ = plaintext;
        Err(VaultError::UnknownFormat)
    }
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);

    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)?;
    Ok(())
}

pub fn write_accounts(path: &Path, accounts: &[Account]) -> Result<(), VaultError> {
    let plaintext = accounts_to_bytes(accounts)?;
    let sealed = seal_default(&plaintext)?;
    write_atomically(path, &sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_header_is_64_bytes_and_keeps_original_dots() {
        assert_eq!(RAM_HEADER.len(), 64);
        assert!(RAM_HEADER.starts_with(b"Roblox Account Manager created by ic3w0lf22"));
        assert!(RAM_HEADER.ends_with(b"......."));
    }

    #[test]
    fn dpapi_entropy_keeps_the_original_typo() {
        assert_eq!(DPAPI_ENTROPY.len(), 56);
        let text = core::str::from_utf8(DPAPI_ENTROPY).expect("entropy phai la ASCII");
        assert!(
            text.contains("BROUGHT TO YOU BUY ic3w0lf"),
            "phai giu nguyen loi chinh ta BUY"
        );
    }

    #[test]
    fn layout_offsets_match_cryptography_cs() {
        assert_eq!(SALT_OFFSET, 64);
        assert_eq!(NONCE_OFFSET, 80);
        assert_eq!(CIPHERTEXT_OFFSET, 104);
        assert_eq!(MIN_PASSWORD_LOCKED_LEN, 120);
    }

    #[test]
    fn detect_branch_recognises_the_four_formats() {
        let mut locked = RAM_HEADER.to_vec();
        locked.extend_from_slice(&[0u8; 64]);
        assert_eq!(detect_branch(&locked), Branch::PasswordLocked);

        assert_eq!(detect_branch(b"[{\"Username\":\"a\"}]"), Branch::PlainJson);

        assert_eq!(
            detect_branch(&[0x01, 0x00, 0x00, 0x00, 0xD0, 0x8C]),
            Branch::Dpapi
        );

        assert_eq!(detect_branch(b""), Branch::Dpapi);
    }

    #[test]
    fn a_short_truncated_password_locked_file_is_still_detected() {
        assert_eq!(detect_branch(&RAM_HEADER[..10]), Branch::Dpapi);
        assert_eq!(detect_branch(RAM_HEADER), Branch::PasswordLocked);
    }

    #[test]
    fn format_labels_and_machine_binding_are_stable() {
        assert_eq!(Format::PasswordLocked.as_str(), "password-locked");
        assert_eq!(Format::Dpapi.as_str(), "dpapi");
        assert_eq!(Format::PlainJson.as_str(), "plain-json");

        assert!(Format::Dpapi.is_machine_bound());
        assert!(!Format::PasswordLocked.is_machine_bound());
        assert!(!Format::PlainJson.is_machine_bound());
    }

    #[test]
    fn key_derivation_is_deterministic_for_the_same_salt() {
        let salt = [7u8; SALT_LEN];
        let first = derive_key("mat khau", &salt).expect("derive");
        let second = derive_key("mat khau", &salt).expect("derive");
        assert_eq!(first.as_ref(), second.as_ref());

        let other = derive_key("mat khau khac", &salt).expect("derive");
        assert_ne!(first.as_ref(), other.as_ref());
    }

    #[test]
    fn a_different_salt_gives_a_different_key() {
        let first = derive_key("mat khau", &[1u8; SALT_LEN]).expect("derive");
        let second = derive_key("mat khau", &[2u8; SALT_LEN]).expect("derive");
        assert_ne!(first.as_ref(), second.as_ref());
    }

    #[test]
    fn password_locked_round_trip() {
        let payload = br#"[{"Username":"alice"}]"#;
        let blob = seal_password_locked(payload, "mat-khau-test").expect("seal");
        assert!(blob.starts_with(RAM_HEADER));
        assert_eq!(&blob[..RAM_HEADER.len()], RAM_HEADER);

        let back = open_password_locked(&blob, "mat-khau-test").expect("open");
        assert_eq!(back.as_slice(), payload.as_slice());
    }

    #[test]
    fn password_locked_open_rejects_a_wrong_password() {
        let blob = seal_password_locked(b"[]", "dung").expect("seal");
        let err = open_password_locked(&blob, "sai").expect_err("phai tu choi");
        assert!(matches!(err, VaultError::Decrypt));
    }

    #[test]
    fn two_seals_of_the_same_payload_differ_because_salt_and_nonce_are_fresh() {
        let first = seal_password_locked(b"[]", "pw").expect("seal");
        let second = seal_password_locked(b"[]", "pw").expect("seal");
        assert_ne!(first, second);
    }

    #[test]
    fn open_password_locked_rejects_a_foreign_header() {
        let err = open_password_locked(b"khong phai header", "pw").expect_err("phai tu choi");
        assert!(matches!(err, VaultError::BadHeader));
    }

    #[test]
    fn open_password_locked_rejects_a_truncated_file() {
        let err = open_password_locked(RAM_HEADER, "pw").expect_err("phai tu choi");
        assert!(matches!(err, VaultError::TooShort { .. }));
    }

    #[test]
    fn open_requires_a_password_for_password_locked_files() {
        let blob = seal_password_locked(b"[]", "pw").expect("seal");
        let err = open(&blob, None).expect_err("phai doi mat khau");
        assert!(matches!(err, VaultError::PasswordRequired));
    }

    #[test]
    fn open_reads_plain_json_without_a_password() {
        let opened = open(b"[{\"Username\":\"alice\"}]", None).expect("open");
        assert_eq!(opened.format, Format::PlainJson);
        assert_eq!(opened.plaintext.as_slice(), b"[{\"Username\":\"alice\"}]");
    }

    #[test]
    fn open_rejects_a_file_from_no_known_branch() {
        let err = open(&[0x01, 0x02, 0x03, 0x04], None).expect_err("phai tu choi");
        assert!(matches!(err, VaultError::UnknownFormat));
    }

    #[test]
    fn opened_debug_does_not_leak_the_plaintext() {
        let opened = open(b"[{\"SecurityToken\":\"BI_MAT\"}]", None).expect("open");
        let text = format!("{opened:?}");
        assert!(
            !text.contains("BI_MAT"),
            "Debug khong duoc lo plaintext: {text}"
        );
        assert!(
            text.contains("PlainJson"),
            "van phai cho biet nhanh nao: {text}"
        );
    }

    #[test]
    fn read_accounts_parses_a_plain_file() {
        let raw = br#"[{"Username":"alice","UserID":1},{"Username":"bob","UserID":2}]"#;
        let (accounts, format) = read_accounts(raw, None).expect("read");
        assert_eq!(format, Format::PlainJson);
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].username.as_deref(), Some("alice"));
        assert_eq!(accounts[1].user_id, 2);
    }

    #[test]
    fn accounts_survive_a_full_write_read_cycle() {
        let raw = br#"[{"Username":"alice","Group":"007 Bank","UserID":42,"Fields":{"Window_Position_X":"120"}}]"#;
        let (accounts, _) = read_accounts(raw, None).expect("read");
        let bytes = accounts_to_bytes(&accounts).expect("write");

        let (again, format) = read_accounts(&bytes, None).expect("read back");
        assert_eq!(format, Format::PlainJson);
        assert_eq!(again, accounts);
    }

    #[test]
    fn accounts_survive_a_password_locked_write_read_cycle() {
        let raw = br#"[{"Username":"alice","UserID":42,"Fields":{"Window_Width":"880"}}]"#;
        let (accounts, _) = read_accounts(raw, None).expect("read");

        let plaintext = accounts_to_bytes(&accounts).expect("write");
        let blob = seal_password_locked(&plaintext, "mat-khau").expect("seal");

        let (again, format) = read_accounts(&blob, Some("mat-khau")).expect("read back");
        assert_eq!(format, Format::PasswordLocked);
        assert_eq!(again, accounts);
    }

    #[test]
    fn error_messages_never_contain_the_password() {
        let blob = seal_password_locked(b"[]", "mat-khau-bi-mat").expect("seal");
        let err = open_password_locked(&blob, "sai-nhung-cung-bi-mat").expect_err("phai tu choi");
        let text = format!("{err} {err:?}");
        assert!(
            !text.contains("bi-mat"),
            "lo mat khau trong thong bao loi: {text}"
        );
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("muster-vault-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn sample_account() -> Account {
        Account {
            valid: true,
            security_token: Some("COOKIE_BI_MAT_123".to_owned()),
            username: Some("alice".to_owned()),
            last_use: "2026-10-12T13:00:00".to_owned(),
            alias: "alice".to_owned(),
            description: String::new(),
            password: String::new(),
            group: Some("007 Bank".to_owned()),
            user_id: 42,
            fields: std::collections::BTreeMap::new(),
            last_attempted_refresh: String::new(),
            browser_tracker_id: Some("100000000001".to_owned()),
            extra: std::collections::BTreeMap::new(),
        }
    }

    #[cfg(windows)]
    #[test]
    fn write_accounts_seals_the_file_so_the_cookie_is_never_readable() {
        let dir = temp_dir("seal");
        let path = dir.join("AccountData.json");

        write_accounts(&path, &[sample_account()]).expect("ghi vault");

        let raw = std::fs::read(&path).expect("doc lai file");
        assert_ne!(
            raw.first(),
            Some(&b'['),
            "file ghi ra khong duoc la plain JSON"
        );

        let needle = b"COOKIE_BI_MAT_123";
        assert!(
            !raw.windows(needle.len()).any(|window| window == needle),
            "cookie lo ra dang plaintext trong file ghi ra"
        );

        let (accounts, format) = read_accounts_from_path(&path, None).expect("doc lai vault");
        assert_eq!(format, Format::Dpapi);
        assert_eq!(accounts, vec![sample_account()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn write_accounts_creates_the_parent_directory_and_leaves_no_temp_file() {
        let dir = temp_dir("mkdir");
        let path = dir.join("nested").join("AccountData.json");

        write_accounts(&path, &[]).expect("ghi vault");

        assert!(path.is_file());

        let leftover = dir.join("nested").join("AccountData.json.tmp");
        assert!(
            !leftover.exists(),
            "con sot file tam: {}",
            leftover.display()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
