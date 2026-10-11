use core::ffi::c_void;
use core::ptr;
use std::time::Duration;

use super::{ClientControl, LaunchError, SINGLETON_MUTEX_NAME};

type Hwnd = *mut c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *const c_void, initial_owner: i32, name: *const u16)
    -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn GetLastError() -> u32;
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
    fn TerminateProcess(process: *mut c_void, exit_code: u32) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
    fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        lp_enum_func: unsafe extern "system" fn(Hwnd, isize) -> i32,
        l_param: isize,
    ) -> i32;
    fn GetWindowThreadProcessId(h_wnd: Hwnd, lpdw_process_id: *mut u32) -> u32;
    fn PostMessageW(h_wnd: Hwnd, msg: u32, w_param: usize, l_param: isize) -> i32;
    fn IsWindowVisible(h_wnd: Hwnd) -> i32;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        h_wnd: Hwnd,
        lp_operation: *const u16,
        lp_file: *const u16,
        lp_parameters: *const u16,
        lp_directory: *const u16,
        n_show_cmd: i32,
    ) -> isize;
}

#[repr(C)]
struct ProcessEntry32W {
    dw_size: u32,
    cnt_usage: u32,
    th32_process_id: u32,
    th32_default_heap_id: usize,
    th32_module_id: u32,
    cnt_threads: u32,
    th32_parent_process_id: u32,
    pc_pri_class_base: i32,
    dw_flags: u32,
    sz_exe_file: [u16; 260],
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

pub fn launch_roblox_url(url: &str) -> Result<(), LaunchError> {
    let open_verb: Vec<u16> = "open\0".encode_utf16().collect();
    let url_w: Vec<u16> = url.encode_utf16().chain(core::iter::once(0)).collect();

    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            open_verb.as_ptr(),
            url_w.as_ptr(),
            ptr::null(),
            ptr::null(),
            1,
        )
    };

    if (result as isize) <= 32 {
        return Err(LaunchError::ShellExecute {
            code: result as usize,
        });
    }

    Ok(())
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClientControl;

impl ClientControl for SystemClientControl {
    fn close_main_window(&mut self, pid: u32) -> bool {
        struct SearchCtx {
            target_pid: u32,
            found: bool,
        }

        unsafe extern "system" fn enum_proc(hwnd: Hwnd, lparam: isize) -> i32 {
            unsafe {
                let ctx = &mut *(lparam as *mut SearchCtx);
                let mut window_pid = 0u32;
                GetWindowThreadProcessId(hwnd, &mut window_pid);

                if window_pid == ctx.target_pid && IsWindowVisible(hwnd) != 0 {
                    PostMessageW(hwnd, 0x0010, 0, 0);
                    ctx.found = true;
                }
            }
            1
        }

        let mut ctx = SearchCtx {
            target_pid: pid,
            found: false,
        };

        unsafe {
            EnumWindows(enum_proc, &mut ctx as *mut _ as isize);
        }

        ctx.found
    }

    fn kill(&mut self, pid: u32) -> bool {
        const PROCESS_TERMINATE: u32 = 0x0001;
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if handle.is_null() {
                return false;
            }
            let success = TerminateProcess(handle, 1) != 0;
            CloseHandle(handle);
            success
        }
    }

    fn wait(&mut self, millis: u64) {
        std::thread::sleep(Duration::from_millis(millis));
    }
}

#[must_use]
pub fn find_running_roblox_pids() -> Vec<u32> {
    const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
    let mut pids = Vec::new();

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() || snapshot as isize == -1 {
            return pids;
        }

        let mut entry = ProcessEntry32W {
            dw_size: size_of::<ProcessEntry32W>() as u32,
            cnt_usage: 0,
            th32_process_id: 0,
            th32_default_heap_id: 0,
            th32_module_id: 0,
            cnt_threads: 0,
            th32_parent_process_id: 0,
            pc_pri_class_base: 0,
            dw_flags: 0,
            sz_exe_file: [0u16; 260],
        };

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let name = String::from_utf16_lossy(&entry.sz_exe_file);
                let trimmed = name.trim_matches('\0');
                if trimmed.eq_ignore_ascii_case("RobloxPlayerBeta.exe") {
                    pids.push(entry.th32_process_id);
                }

                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
    }

    pids
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
