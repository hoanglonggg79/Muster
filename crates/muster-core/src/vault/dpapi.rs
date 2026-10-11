use core::ffi::c_void;
use core::ptr;

use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    CurrentUser,
    LocalMachine,
}

impl Scope {
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

#[derive(Debug, thiserror::Error)]
#[error("DPAPI thất bại (scope {scope}, mã lỗi Win32 {code})")]
pub struct DpapiError {
    pub scope: Scope,
    pub code: u32,
}

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

unsafe fn take_output(out: DataBlob) -> Zeroizing<Vec<u8>> {
    if out.pb_data.is_null() {
        return Zeroizing::new(Vec::new());
    }

    let copied = if out.cb_data == 0 {
        Zeroizing::new(Vec::new())
    } else {
        let bytes = unsafe { core::slice::from_raw_parts(out.pb_data, out.cb_data as usize) };
        Zeroizing::new(bytes.to_vec())
    };

    unsafe {
        LocalFree(out.pb_data.cast::<c_void>());
    }

    copied
}

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
        let code = unsafe { GetLastError() };
        return Err(DpapiError { scope, code });
    }

    Ok(unsafe { take_output(out) })
}

pub fn protect(plaintext: &[u8], entropy: &[u8], scope: Scope) -> Result<Vec<u8>, DpapiError> {
    let input = as_blob(plaintext);
    let entropy_blob = as_blob(entropy);
    let mut out = DataBlob {
        cb_data: 0,
        pb_data: ptr::null_mut(),
    };

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
        let code = unsafe { GetLastError() };
        return Err(DpapiError { scope, code });
    }

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
