//! # `vault` — đọc/ghi `AccountData.json` tương thích Roblox Account Manager
//!
//! Module này là hợp đồng tương thích ngược quan trọng nhất của dự án: người dùng RAM phải
//! chuyển sang được mà không mất dữ liệu (nguyên tắc bất biến #2). Mọi con số ở đây đã được
//! xác minh trực tiếp từ `RAM_Source/`, không có chỗ nào là phỏng đoán.
//!
//! ## Bốn nhánh định dạng phải đọc được
//!
//! | # | Nhánh | Nhận diện | Cách giải mã |
//! | :--- | :--- | :--- | :--- |
//! | 1 | Password-locked | 64 byte đầu khớp [`RAM_HEADER`] | Argon2i → `secretbox` |
//! | 2 | DPAPI CurrentUser | không khớp header, `CryptUnprotectData` ra UTF-8 | scope CurrentUser |
//! | 3 | DPAPI LocalMachine | như trên | scope LocalMachine |
//! | 4 | Plain JSON | byte đầu là `[` | đọc thẳng UTF-8 |
//!
//! [`detect_branch`] chỉ nhìn phần mở đầu nên phân biệt được **3** khả năng. Vì vậy có hai enum:
//! [`Branch`] cho *phỏng đoán rẻ tiền*, [`Format`] cho *đã mở được bằng cách nào*.
//!
//! ⚠️ **Nhánh 2 và 3 không tách được.** Đã đo thật: một blob DPAPI phạm vi máy vẫn mở được bằng
//! đường CurrentUser, nên không thể suy ra phạm vi từ việc thử giải mã. [`Format`] vì vậy chỉ có
//! một biến thể `Dpapi` — xem ghi chú đầy đủ ở đó. Chiến lược thử lần lượt CurrentUser rồi
//! LocalMachine vẫn được giữ, vì nó tối đa hoá khả năng mở được file.
//!
//! ## Layout nhị phân của nhánh 1
//!
//! ```text
//! offset 0    : 64 byte  RAM_HEADER
//! offset 64   : 16 byte  salt
//! offset 80   : 24 byte  nonce
//! offset 104  : phần còn lại  secretbox
//! ```
//!
//! ⚠️ **Vị trí 16 byte MAC.** `ROADMAP.md` mục 4.1 ghi "gồm 16 byte MAC ở cuối" — **sai**.
//! `crypto_secretbox_easy` của libsodium (mà `Sodium.SecretBox.Encrypt` gọi tới) đặt MAC ở
//! **đầu**: 16 byte đầu của phần sau nonce là MAC, phần còn lại mới là ciphertext. Xem [`MAC_LEN`].
//!
//! ## Tham số Argon2 — ĐÃ XÁC MINH, không được đoán
//!
//! Nguồn: dịch ngược IL `Sodium.PasswordHash` của `libsodium-net 0.10.0` và đối chiếu với
//! native `libsodium 1.0.11`. Bằng chứng tái lập được ở `tools/fixtures-gen/provenance/`.
//!
//! - Thuật toán: **Argon2i**, **không phải Argon2id**. Dùng Argon2id là giải mã sai 100%.
//! - `t_cost = 6`, `m_cost = 131072` KiB (128 MiB), `p_cost = 1`, khoá ra 32 byte.
//! - Đầu vào của Argon2 là **SHA-512(password)** (64 byte), không phải password thô.

#[cfg(windows)]
pub mod dpapi;

use std::path::Path;

use crypto_secretbox::XSalsa20Poly1305;
use crypto_secretbox::aead::{AeadInPlace, KeyInit, generic_array::GenericArray};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::models::Account;

/// 64 byte magic mở đầu file password-locked.
///
/// `Classes/Cryptography.cs:10` — giữ nguyên từng byte, kể cả 7 dấu chấm ở cuối.
pub const RAM_HEADER: &[u8] = b"Roblox Account Manager created by ic3w0lf22 @ github.com .......";

/// Entropy 56 byte dùng cho cả hai nhánh DPAPI.
///
/// `AccountManager.cs:93`. Giữ nguyên lỗi chính tả gốc **"BUY"** (không phải "BY") —
/// sai một byte là không giải mã được.
pub const DPAPI_ENTROPY: &[u8] = b"ROBLOX ACCOUNT MANAGER | :) | BROUGHT TO YOU BUY ic3w0lf";

/// Độ dài salt của Argon2i (`crypto_pwhash_argon2i_saltbytes`).
pub const SALT_LEN: usize = 16;
/// Độ dài nonce của `secretbox` (`crypto_secretbox_noncebytes`).
pub const NONCE_LEN: usize = 24;
/// Độ dài khoá dẫn xuất.
pub const ARGON2_OUT_LEN: usize = 32;
/// Số vòng (time cost).
pub const ARGON2_T_COST: u32 = 6;
/// Bộ nhớ tính bằng KiB (128 MiB).
pub const ARGON2_M_COST_KIB: u32 = 131_072;
/// Số làn song song.
pub const ARGON2_P_COST: u32 = 1;

/// Độ dài MAC của secretbox (`crypto_secretbox_MACBYTES`).
///
/// ⚠️ MAC nằm ở **đầu** phần dữ liệu sau nonce, không phải ở cuối.
pub const MAC_LEN: usize = 16;

/// Offset của salt trong file nhánh 1.
pub const SALT_OFFSET: usize = RAM_HEADER.len();
/// Offset của nonce trong file nhánh 1.
pub const NONCE_OFFSET: usize = SALT_OFFSET + SALT_LEN;
/// Offset của phần secretbox trong file nhánh 1.
pub const CIPHERTEXT_OFFSET: usize = NONCE_OFFSET + NONCE_LEN;
/// Kích thước tối thiểu để một file có thể là nhánh 1 (đã tính cả MAC).
pub const MIN_PASSWORD_LOCKED_LEN: usize = CIPHERTEXT_OFFSET + MAC_LEN;

const _: () = assert!(RAM_HEADER.len() == 64);
const _: () = assert!(DPAPI_ENTROPY.len() == 56);
const _: () = assert!(CIPHERTEXT_OFFSET == 104);

/// Nhánh định dạng **đoán được** chỉ từ phần mở đầu của file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Branch {
    /// 64 byte đầu khớp [`RAM_HEADER`].
    PasswordLocked,
    /// Không khớp header, và byte đầu không phải `[`. Có thể là CurrentUser hoặc LocalMachine.
    Dpapi,
    /// Byte đầu tiên là `[` — JSON thô.
    PlainJson,
}

/// Nhánh định dạng **thực sự đã dùng** để mở được nội dung.
///
/// Khác [`Branch`] ở chỗ [`Branch`] là phỏng đoán rẻ tiền từ phần mở đầu (một file hỏng cũng cho
/// ra [`Branch::Dpapi`]), còn `Format` chỉ được sinh ra khi nội dung **đã giải mã thành công**.
/// Đây cũng là kiểu được trả cho `muster-cli` và cho UI, nên nó là hợp đồng công khai.
///
/// ## Vì sao DPAPI chỉ có một biến thể, không tách CurrentUser / LocalMachine
///
/// `ROADMAP.md` mục 4.1 mô tả nhánh 2 và 3 là hai phạm vi khác nhau, và trình nhập nên thử
/// CurrentUser trước rồi LocalMachine. Chiến lược thử đó vẫn đúng và vẫn được dùng ở [`open`] —
/// nhưng **kết quả thử không cho biết phạm vi thật**.
///
/// Đo thật trên Windows (2026-10-10, xem `tests/vault_fixtures.rs`): cả hai fixture DPAPI đều
/// giải mã được **với và cả không có** cờ `CRYPTPROTECT_LOCAL_MACHINE`. Tức là một blob phạm vi
/// máy vẫn mở được bằng đường CurrentUser. Vì vậy nếu báo cáo nhập ghi "dpapi-currentuser" thì
/// đó là khẳng định sai sự thật trong trường hợp phổ biến nhất.
///
/// Hệ quả: nhánh DPAPI được báo cáo chung là `dpapi`. Việc ghi thì vẫn chọn được phạm vi —
/// xem [`seal_dpapi`]. Trường hợp ngoại lệ duy nhất đáng ghi nhận: nếu đường không cờ thất bại
/// mà đường có cờ thành công thì chắc chắn là blob phạm vi máy, nhưng trường hợp đó hiếm và
/// không đủ để đổi hình dạng của kiểu công khai.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Format {
    /// Nhánh 1 — Argon2i + secretbox, cần mật khẩu.
    PasswordLocked,
    /// Nhánh 2 hoặc 3 — DPAPI, đã mở được (không phân biệt được phạm vi, xem ghi chú trên).
    Dpapi,
    /// Nhánh 4 — JSON thô.
    PlainJson,
}

impl Format {
    /// Tên ngắn để hiển thị trong báo cáo nhập.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PasswordLocked => "password-locked",
            Self::Dpapi => "dpapi",
            Self::PlainJson => "plain-json",
        }
    }

    /// `true` nếu nhánh này gắn với máy hoặc tài khoản Windows đã tạo ra file.
    ///
    /// Wizard nhập dùng cờ này để khuyến nghị người dùng chuyển sang nhánh password-locked
    /// (rủi ro #5 trong `ROADMAP.md`).
    #[must_use]
    pub const fn is_machine_bound(self) -> bool {
        matches!(self, Self::Dpapi)
    }
}

/// Lỗi của lớp vault.
///
/// **Không** biến thể nào chứa mật khẩu, cookie hay plaintext — đây là điều kiện để log lỗi
/// không làm rò rỉ secret (nguyên tắc bất biến #4).
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    /// File không bắt đầu bằng [`RAM_HEADER`] nên không phải nhánh 1.
    #[error("file không khớp magic RAMHeader nên không phải nhánh password-locked")]
    BadHeader,

    /// File ngắn hơn mức tối thiểu của nhánh 1.
    #[error("file quá ngắn: {len} byte, cần tối thiểu {min} byte cho nhánh password-locked")]
    TooShort {
        /// Độ dài thực tế.
        len: usize,
        /// Độ dài tối thiểu cần có.
        min: usize,
    },

    /// File là nhánh 1 nhưng người dùng chưa cung cấp mật khẩu.
    #[error("file được khoá bằng mật khẩu, cần cung cấp mật khẩu để mở")]
    PasswordRequired,

    /// Dẫn xuất khoá Argon2 thất bại (tham số không hợp lệ).
    #[error("dẫn xuất khoá Argon2 thất bại")]
    Argon2,

    /// Giải mã secretbox thất bại — hầu như luôn là do sai mật khẩu.
    #[error("giải mã thất bại: sai mật khẩu hoặc file bị hỏng")]
    Decrypt,

    /// Không lấy được số ngẫu nhiên từ hệ điều hành.
    #[error("không lấy được số ngẫu nhiên từ hệ điều hành")]
    Random,

    /// Lớp DPAPI từ chối.
    #[cfg(windows)]
    #[error(transparent)]
    Dpapi(#[from] dpapi::DpapiError),

    /// Không nhận ra file thuộc nhánh nào.
    #[error(
        "không nhận ra định dạng file: không phải plain JSON, không phải password-locked, và DPAPI từ chối giải mã"
    )]
    UnknownFormat,

    /// JSON không hợp lệ.
    #[error("JSON không hợp lệ: {0}")]
    Json(#[from] serde_json::Error),

    /// Lỗi đọc/ghi file.
    #[error("lỗi vào/ra: {0}")]
    Io(#[from] std::io::Error),
}

/// Đoán nhánh định dạng từ nội dung file.
///
/// Chỉ nhìn vào phần mở đầu. Việc thử giải mã DPAPI theo từng scope là việc của [`open`] —
/// [`Branch::Dpapi`] ở đây chỉ nghĩa là "không phải hai nhánh còn lại".
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

/// Dẫn xuất khoá 32 byte từ mật khẩu và salt.
///
/// ⚠️ Đầu vào của Argon2 **không phải** mật khẩu thô mà là `SHA-512(mật khẩu)` — 64 byte.
/// Bỏ qua bước băm này sẽ cho ra khoá khác hoàn toàn.
///
/// # Errors
///
/// Trả [`VaultError::Argon2`] nếu bộ tham số bị từ chối.
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

/// Giải mã một file nhánh 1 (password-locked).
///
/// # Errors
///
/// - [`VaultError::BadHeader`] nếu file không bắt đầu bằng [`RAM_HEADER`].
/// - [`VaultError::TooShort`] nếu file ngắn hơn [`MIN_PASSWORD_LOCKED_LEN`].
/// - [`VaultError::Decrypt`] nếu sai mật khẩu hoặc file hỏng.
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

    // MAC nam o DAU, khong phai o cuoi — xem ghi chu o MAC_LEN.
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

/// Mã hoá `plaintext` thành file nhánh 1 với mật khẩu cho trước.
///
/// Salt và nonce được sinh mới mỗi lần gọi, nên hai lần gọi trên cùng dữ liệu cho ra hai file khác nhau.
///
/// # Errors
///
/// Trả [`VaultError::Random`] nếu hệ điều hành không cấp được số ngẫu nhiên,
/// [`VaultError::Argon2`] nếu dẫn xuất khoá thất bại.
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

/// Nội dung đã giải mã kèm thông tin đã mở bằng nhánh nào.
///
/// Cố tình **không** derive `Debug`: `plaintext` chứa cookie và mật khẩu, in ra log là rò rỉ secret.
/// Bản `Debug` viết tay dưới đây chỉ in ra độ dài.
pub struct Opened {
    /// Nội dung gốc (JSON). Tự động được ghi đè khi rơi khỏi scope.
    pub plaintext: Zeroizing<Vec<u8>>,
    /// Nhánh định dạng đã dùng để mở.
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

/// Mở nội dung của một `AccountData.json` bất kỳ, tự dò nhánh.
///
/// Thứ tự thử:
/// 1. Plain JSON — rẻ nhất, và blob DPAPI không bao giờ bắt đầu bằng `[` nên không thể nhầm.
/// 2. Password-locked — nếu file khớp [`RAM_HEADER`]. Không có mật khẩu thì báo
///    [`VaultError::PasswordRequired`] chứ không âm thầm thử tiếp.
/// 3. DPAPI CurrentUser rồi DPAPI LocalMachine — theo đúng thứ tự ở `ROADMAP.md` mục 4.1.
///
/// # Errors
///
/// Trả [`VaultError::UnknownFormat`] nếu không nhánh nào mở được.
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
        // Thu CurrentUser truoc roi LocalMachine: nguyen tac bat bien #2 — toi da hoa kha nang mo
        // duoc file. Ca hai duong deu bao ve Format::Dpapi vi khong phan biet duoc pham vi that.
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

/// Đọc danh sách tài khoản từ nội dung một `AccountData.json`.
///
/// # Errors
///
/// Trả lỗi từ [`open`], hoặc [`VaultError::Json`] nếu nội dung không phải mảng `Account`.
pub fn read_accounts(
    data: &[u8],
    password: Option<&str>,
) -> Result<(Vec<Account>, Format), VaultError> {
    let opened = open(data, password)?;
    let accounts: Vec<Account> = serde_json::from_slice(&opened.plaintext)?;
    Ok((accounts, opened.format))
}

/// Như [`read_accounts`] nhưng đọc thẳng từ đường dẫn.
///
/// # Errors
///
/// Trả [`VaultError::Io`] nếu không đọc được file, cộng các lỗi của [`read_accounts`].
pub fn read_accounts_from_path(
    path: &Path,
    password: Option<&str>,
) -> Result<(Vec<Account>, Format), VaultError> {
    let data = std::fs::read(path)?;
    read_accounts(&data, password)
}

/// Tuần tự hoá danh sách tài khoản thành JSON đúng định dạng RAM ghi.
///
/// Dùng JSON nén (không thụt lề) vì `JsonConvert.SerializeObject` mặc định cũng vậy.
///
/// # Errors
///
/// Trả [`VaultError::Json`] nếu không tuần tự hoá được.
pub fn accounts_to_bytes(accounts: &[Account]) -> Result<Vec<u8>, VaultError> {
    Ok(serde_json::to_vec(accounts)?)
}

/// Mã hoá nội dung để ghi xuống đĩa theo đúng quy tắc của RAM.
///
/// RAM quyết định ở `AccountManager.cs:377-380`: có file cờ `NoEncryption...` thì ghi plain UTF-8,
/// không có thì ghi **DPAPI LocalMachine** kèm entropy. Ở M0 hàm này chỉ phục vụ test round-trip;
/// chính sách chọn nhánh khi ghi thật thuộc M1.
///
/// # Errors
///
/// Trả [`VaultError::Dpapi`] nếu Win32 từ chối.
#[cfg(windows)]
pub fn seal_dpapi(plaintext: &[u8], scope: dpapi::Scope) -> Result<Vec<u8>, VaultError> {
    Ok(dpapi::protect(plaintext, DPAPI_ENTROPY, scope)?)
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

        // blob DPAPI: khong khop header, khong bat dau bang '['
        assert_eq!(
            detect_branch(&[0x01, 0x00, 0x00, 0x00, 0xD0, 0x8C]),
            Branch::Dpapi
        );

        assert_eq!(detect_branch(b""), Branch::Dpapi);
    }

    #[test]
    fn a_short_truncated_password_locked_file_is_still_detected() {
        // detect_branch khong duoc panic voi input cut ngan
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
}
