//! # `vault` — đọc/ghi `AccountData.json` tương thích Roblox Account Manager
//!
//! Trạng thái: **mới có hằng số đã xác minh.** Logic mã hoá/giải mã thuộc bước M0 0.4.
//!
//! ## Bốn nhánh định dạng phải đọc được
//!
//! | # | Nhánh | Nhận diện | Cách giải mã |
//! | :--- | :--- | :--- | :--- |
//! | 1 | Password-locked | 64 byte đầu khớp [`RAM_HEADER`] | Argon2i → `secretbox_open` |
//! | 2 | DPAPI CurrentUser | không khớp header, `CryptUnprotectData` ra UTF-8 | scope CurrentUser |
//! | 3 | DPAPI LocalMachine | như trên | scope LocalMachine |
//! | 4 | Plain JSON | byte đầu là `[` | đọc thẳng UTF-8 |
//!
//! ## Layout nhị phân của nhánh 1
//!
//! ```text
//! offset 0    : 64 byte  RAM_HEADER
//! offset 64   : 16 byte  salt
//! offset 80   : 24 byte  nonce
//! offset 104  : phần còn lại  ciphertext (secretbox, 16 byte MAC ở cuối)
//! ```
//!
//! ## Tham số Argon2 — ĐÃ XÁC MINH, không được đoán
//!
//! Nguồn: dịch ngược IL `Sodium.PasswordHash` của `libsodium-net 0.10.0` và đối chiếu với
//! native `libsodium 1.0.11`. Bằng chứng tái lập được ở `tools/fixtures-gen/provenance/`.
//!
//! - Thuật toán: **Argon2i**, **không phải Argon2id**. Dùng Argon2id là giải mã sai 100%.
//! - `t_cost = 6`, `m_cost = 131072` KiB (128 MiB), `p_cost = 1`, khoá ra 32 byte.
//! - Đầu vào của Argon2 là **SHA-512(password)** (64 byte), không phải password thô.
//!
//! ```ignore
//! let params = argon2::Params::new(ARGON2_M_COST_KIB, ARGON2_T_COST, ARGON2_P_COST, Some(ARGON2_OUT_LEN))?;
//! let a = argon2::Argon2::new(argon2::Algorithm::Argon2i, argon2::Version::V0x13, params);
//! ```

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

/// Offset của salt trong file nhánh 1.
pub const SALT_OFFSET: usize = RAM_HEADER.len();
/// Offset của nonce trong file nhánh 1.
pub const NONCE_OFFSET: usize = SALT_OFFSET + SALT_LEN;
/// Offset của ciphertext trong file nhánh 1.
pub const CIPHERTEXT_OFFSET: usize = NONCE_OFFSET + NONCE_LEN;
/// Kích thước tối thiểu để một file có thể là nhánh 1 (chưa tính ciphertext).
pub const MIN_PASSWORD_LOCKED_LEN: usize = CIPHERTEXT_OFFSET;

const _: () = assert!(RAM_HEADER.len() == 64);
const _: () = assert!(DPAPI_ENTROPY.len() == 56);
const _: () = assert!(CIPHERTEXT_OFFSET == 104);

/// Nhánh định dạng của một file `AccountData.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    /// 64 byte đầu khớp [`RAM_HEADER`].
    PasswordLocked,
    /// Không khớp header, và byte đầu không phải `[`.
    Dpapi,
    /// Byte đầu tiên là `[` — JSON thô.
    PlainJson,
}

/// Đoán nhánh định dạng từ nội dung file.
///
/// Chỉ nhìn vào phần mở đầu. Việc thử giải mã DPAPI theo từng scope là việc của bước 0.4 —
/// `Dpapi` ở đây chỉ nghĩa là "không phải hai nhánh còn lại".
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
}
