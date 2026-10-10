//! # `vault::dpapi` — Windows Data Protection API
//!
//! Bọc đúng hai hàm `CryptProtectData` / `CryptUnprotectData` của `crypt32.dll`, dùng để đọc
//! nhánh 2 và nhánh 3 của `AccountData.json` (mục 4.1 trong `ROADMAP.md`).
//!
//! ## Vì sao khai báo FFI trực tiếp thay vì dùng crate `windows` / `windows-sys`
//!
//! Chỉ cần đúng hai hàm. ABI của DPAPI đã đóng băng từ Windows 2000 và sẽ không đổi nữa, nên
//! rủi ro của một khai báo thủ công ở đây thấp hơn cái lợi: `muster-core` giữ được cây phụ thuộc
//! gọn, và điều quan trọng hơn — toàn bộ chỗ chạm vào `unsafe` nằm trong một file 150 dòng đọc
//! hết được trong một lần, thay vì nằm rải trong một crate sinh tự động hàng trăm nghìn dòng.
//! Với một module mật mã thì khả năng kiểm tra bằng mắt có giá trị riêng.
//!
//! ## Vì sao file này không có `#[cfg]` bên trong
//!
//! Cả module được `#[cfg(windows)]` ở `vault/mod.rs`. Sản phẩm chỉ nhắm Windows (quyết định
//! 2026-10-10), nên ở đây không trừu tượng hoá hệ điều hành — đúng tinh thần mục 2 của roadmap.

use core::ffi::c_void;
use core::ptr;

use zeroize::Zeroizing;

/// Phạm vi bảo vệ, khớp `System.Security.Cryptography.DataProtectionScope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Chỉ tài khoản Windows hiện tại giải mã được.
    ///
    /// RAM dùng phạm vi này ở **chiều đọc** (`AccountManager.cs:294-298`).
    CurrentUser,
    /// Mọi tài khoản Windows trên cùng máy đều giải mã được.
    ///
    /// RAM dùng phạm vi này ở **chiều ghi** khi không có file cờ `NoEncryption...`
    /// (`AccountManager.cs:377-380`).
    LocalMachine,
}

impl Scope {
    /// Tên phạm vi để hiển thị trong báo cáo nhập và thông báo lỗi.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CurrentUser => "CurrentUser",
            Self::LocalMachine => "LocalMachine",
        }
    }
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lỗi từ lớp DPAPI. Không chứa dữ liệu người dùng — chỉ mã lỗi Win32.
#[derive(Debug, thiserror::Error)]
#[error("DPAPI thất bại (scope {scope}, mã lỗi Win32 {code})")]
pub struct DpapiError {
    /// Phạm vi đã dùng khi gọi.
    pub scope: Scope,
    /// Mã lỗi từ `GetLastError()`.
    pub code: u32,
}

/// `CRYPT_INTEGER_BLOB` — một mảng byte kèm độ dài.
#[repr(C)]
struct DataBlob {
    cb_data: u32,
    pb_data: *mut u8,
}

const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x0000_0001;
const CRYPTPROTECT_LOCAL_MACHINE: u32 = 0x0000_0004;

#[link(name = "crypt32")]
unsafe extern "system" {
    fn CryptProtectData(
        data_in: *const DataBlob,
        data_descr: *const u16,
        optional_entropy: *const DataBlob,
        reserved: *const c_void,
        prompt_struct: *const c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;

    fn CryptUnprotectData(
        data_in: *const DataBlob,
        data_descr: *mut *mut u16,
        optional_entropy: *const DataBlob,
        reserved: *const c_void,
        prompt_struct: *const c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LocalFree(mem: *mut c_void) -> *mut c_void;
    fn GetLastError() -> u32;
}

/// Dựng `DataBlob` trỏ vào một slice đang sống.
///
/// `len` bị cắt về `u32::MAX` thay vì panic: trên thực tế blob DPAPI không bao giờ lớn tới vậy,
/// và thà để Win32 trả lỗi còn hơn làm sập tiến trình vì một con số vô lý.
fn as_blob(data: &[u8]) -> DataBlob {
    DataBlob {
        cb_data: u32::try_from(data.len()).unwrap_or(u32::MAX),
        pb_data: data.as_ptr().cast_mut(),
    }
}

fn flags_for(scope: Scope) -> u32 {
    match scope {
        Scope::CurrentUser => CRYPTPROTECT_UI_FORBIDDEN,
        Scope::LocalMachine => CRYPTPROTECT_UI_FORBIDDEN | CRYPTPROTECT_LOCAL_MACHINE,
    }
}

/// Sao chép dữ liệu Win32 đã cấp phát ra `Zeroizing<Vec<u8>>` rồi trả bộ nhớ lại cho hệ thống.
///
/// Bản sao nằm trong `Zeroizing` nên sẽ được ghi đè khi rơi khỏi scope; buffer gốc của Win32 thì
/// `LocalFree` (không kiểm soát được nội dung của nó sau khi free, nhưng đó là giới hạn của DPAPI).
///
/// # Safety
///
/// `out` phải là blob do `CryptProtectData`/`CryptUnprotectData` ghi ra, chưa bị free.
unsafe fn take_output(out: DataBlob) -> Zeroizing<Vec<u8>> {
    if out.pb_data.is_null() {
        return Zeroizing::new(Vec::new());
    }

    let copied = if out.cb_data == 0 {
        Zeroizing::new(Vec::new())
    } else {
        // SAFETY: theo hợp đồng của DPAPI, khi hàm trả về thành công thì `pb_data` trỏ tới
        // `cb_data` byte hợp lệ do `LocalAlloc` cấp, và chỉ mình ta sở hữu vùng đó.
        let bytes = unsafe { core::slice::from_raw_parts(out.pb_data, out.cb_data as usize) };
        Zeroizing::new(bytes.to_vec())
    };

    // SAFETY: `pb_data` do DPAPI cấp bằng LocalAlloc nên phải trả lại bằng LocalFree.
    unsafe {
        LocalFree(out.pb_data.cast::<c_void>());
    }

    copied
}

/// Giải mã một blob DPAPI bằng entropy cho trước.
///
/// # Errors
///
/// Trả [`DpapiError`] khi Win32 từ chối — thường là sai entropy, sai phạm vi, hoặc blob được
/// tạo trên máy/tài khoản khác.
pub fn unprotect(
    data: &[u8],
    entropy: &[u8],
    scope: Scope,
) -> Result<Zeroizing<Vec<u8>>, DpapiError> {
    let input = as_blob(data);
    let entropy_blob = as_blob(entropy);
    let mut out = DataBlob {
        cb_data: 0,
        pb_data: ptr::null_mut(),
    };

    // SAFETY: hai blob trỏ vào slice đang sống trong suốt lời gọi; `out` là bộ nhớ hợp lệ để
    // Win32 ghi kết quả vào. `data_descr` để NULL nên DPAPI không cấp thêm chuỗi mô tả nào.
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            &entropy_blob,
            ptr::null(),
            ptr::null(),
            flags_for(scope),
            &mut out,
        )
    };

    if ok == 0 {
        // SAFETY: GetLastError không có tiền điều kiện nào.
        let code = unsafe { GetLastError() };
        return Err(DpapiError { scope, code });
    }

    // SAFETY: `ok != 0` nên `out` đã được DPAPI ghi ra hợp lệ.
    Ok(unsafe { take_output(out) })
}

/// Mã hoá một blob bằng DPAPI với entropy cho trước.
///
/// # Errors
///
/// Trả [`DpapiError`] khi Win32 từ chối.
pub fn protect(plaintext: &[u8], entropy: &[u8], scope: Scope) -> Result<Vec<u8>, DpapiError> {
    let input = as_blob(plaintext);
    let entropy_blob = as_blob(entropy);
    let mut out = DataBlob {
        cb_data: 0,
        pb_data: ptr::null_mut(),
    };

    // SAFETY: như `unprotect`.
    let ok = unsafe {
        CryptProtectData(
            &input,
            ptr::null(),
            &entropy_blob,
            ptr::null(),
            ptr::null(),
            flags_for(scope),
            &mut out,
        )
    };

    if ok == 0 {
        // SAFETY: GetLastError không có tiền điều kiện nào.
        let code = unsafe { GetLastError() };
        return Err(DpapiError { scope, code });
    }

    // SAFETY: `ok != 0` nên `out` đã được DPAPI ghi ra hợp lệ.
    Ok(unsafe { take_output(out) }.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTROPY: &[u8] = b"muster dpapi unit test entropy";

    #[test]
    fn round_trip_current_user() {
        let plaintext = b"hello DPAPI";
        let blob = protect(plaintext, ENTROPY, Scope::CurrentUser).expect("protect");
        assert_ne!(blob.as_slice(), plaintext, "blob phai khac plaintext");

        let back = unprotect(&blob, ENTROPY, Scope::CurrentUser).expect("unprotect");
        assert_eq!(back.as_slice(), plaintext);
    }

    #[test]
    fn round_trip_local_machine() {
        let plaintext = b"local machine scope";
        let blob = protect(plaintext, ENTROPY, Scope::LocalMachine).expect("protect");
        let back = unprotect(&blob, ENTROPY, Scope::LocalMachine).expect("unprotect");
        assert_eq!(back.as_slice(), plaintext);
    }

    #[test]
    fn wrong_entropy_is_rejected() {
        let blob = protect(b"secret", ENTROPY, Scope::CurrentUser).expect("protect");
        let err = unprotect(&blob, b"entropy khac hoan toan", Scope::CurrentUser)
            .expect_err("sai entropy phai bi tu choi");
        assert_eq!(err.scope, Scope::CurrentUser);
        assert_ne!(err.code, 0);
    }

    #[test]
    fn a_garbage_blob_is_rejected_instead_of_panicking() {
        assert!(unprotect(b"khong phai blob dpapi", ENTROPY, Scope::CurrentUser).is_err());
        assert!(unprotect(b"", ENTROPY, Scope::CurrentUser).is_err());
    }

    #[test]
    fn scope_names_are_stable() {
        assert_eq!(Scope::CurrentUser.as_str(), "CurrentUser");
        assert_eq!(Scope::LocalMachine.as_str(), "LocalMachine");
    }
}
