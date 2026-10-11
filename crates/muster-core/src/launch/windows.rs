use core::ffi::c_void;
use core::ptr;

use super::{LaunchError, SINGLETON_MUTEX_NAME};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *const c_void, initial_owner: i32, name: *const u16)
    -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn GetLastError() -> u32;
}

pub struct SingletonMutex {
    handle: isize,
}

impl SingletonMutex {
    pub fn acquire() -> Result<Self, LaunchError> {
        let name: Vec<u16> = SINGLETON_MUTEX_NAME
            .encode_utf16()
            .chain(core::iter::once(0))
            .collect();

        let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };

        if handle.is_null() {
            let code = unsafe { GetLastError() };
            return Err(LaunchError::Mutex {
                name: SINGLETON_MUTEX_NAME,
                code,
            });
        }

        Ok(Self {
            handle: handle as isize,
        })
    }

    #[must_use]
    pub fn is_held(&self) -> bool {
        self.handle != 0
    }
}

impl Drop for SingletonMutex {
    fn drop(&mut self) {
        if self.handle != 0 {
            unsafe {
                CloseHandle(self.handle as *mut c_void);
            }
            self.handle = 0;
        }
    }
}

impl std::fmt::Debug for SingletonMutex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingletonMutex")
            .field("name", &SINGLETON_MUTEX_NAME)
            .field("held", &self.is_held())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquiring_gives_a_live_handle() {
        let mutex = SingletonMutex::acquire().expect("phai tao duoc mutex");
        assert!(mutex.is_held());
    }

    #[test]
    fn acquiring_twice_in_the_same_process_both_succeed() {
        let first = SingletonMutex::acquire().expect("lan 1");
        let second = SingletonMutex::acquire().expect("lan 2");
        assert!(first.is_held() && second.is_held());
    }

    #[test]
    fn dropping_releases_the_handle_and_is_idempotent_in_effect() {
        let mutex = SingletonMutex::acquire().expect("tao mutex");
        drop(mutex);

        let again = SingletonMutex::acquire().expect("tao lai");
        assert!(again.is_held());
    }

    #[test]
    fn the_debug_output_does_not_leak_a_raw_pointer() {
        let mutex = SingletonMutex::acquire().expect("tao mutex");
        let text = format!("{mutex:?}");
        assert!(text.contains("ROBLOX_singletonMutex"), "text: {text}");
        assert!(!text.contains("0x"), "khong nen in dia chi tho: {text}");
    }
}
