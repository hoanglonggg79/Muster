use core::ffi::c_void;
use core::ptr;
use std::time::Duration;

use super::{ClientControl, LaunchError, RunningClient, SINGLETON_MUTEX_NAME};

type Hwnd = *mut c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *const c_void, initial_owner: i32, name: *const u16)
    -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn ReleaseMutex(handle: *mut c_void) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn GetLastError() -> u32;
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
    fn TerminateProcess(process: *mut c_void, exit_code: u32) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
    fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
    fn ReadProcessMemory(
        process: *mut c_void,
        base_address: *const c_void,
        buffer: *mut c_void,
        size: usize,
        number_of_bytes_read: *mut usize,
    ) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryInformationProcess(
        process: *mut c_void,
        information_class: u32,
        buffer: *mut c_void,
        length: u32,
        return_length: *mut u32,
    ) -> i32;
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

#[cfg(target_pointer_width = "64")]
#[repr(C)]
struct ProcessBasicInformation {
    reserved1: *mut c_void,
    peb_base_address: *mut c_void,
    reserved2: [*mut c_void; 2],
    unique_process_id: *mut c_void,
    reserved3: *mut c_void,
}

const WAIT_OBJECT_0: u32 = 0x0000_0000;
const WAIT_ABANDONED: u32 = 0x0000_0080;
const WAIT_TIMEOUT: u32 = 0x0000_0102;

fn wide_name() -> Vec<u16> {
    SINGLETON_MUTEX_NAME
        .encode_utf16()
        .chain(core::iter::once(0))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutexOutcome {
    Acquired,
    HeldByAnotherProcess,
    Failed(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutexProbe {
    pub name: &'static str,
    pub acquire_attempt: MutexOutcome,
}

#[must_use]
pub fn probe_singleton_mutex() -> MutexProbe {
    let name = wide_name();
    let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };

    if handle.is_null() {
        return MutexProbe {
            name: SINGLETON_MUTEX_NAME,
            acquire_attempt: MutexOutcome::Failed(unsafe { GetLastError() }),
        };
    }

    let wait = unsafe { WaitForSingleObject(handle, 0) };
    let outcome = match wait {
        WAIT_OBJECT_0 | WAIT_ABANDONED => {
            unsafe { ReleaseMutex(handle) };
            MutexOutcome::Acquired
        }
        WAIT_TIMEOUT => MutexOutcome::HeldByAnotherProcess,
        _ => MutexOutcome::Failed(unsafe { GetLastError() }),
    };

    unsafe { CloseHandle(handle) };

    MutexProbe {
        name: SINGLETON_MUTEX_NAME,
        acquire_attempt: outcome,
    }
}

pub struct SingletonMutex {
    handle: isize,
    owns: bool,
}

impl SingletonMutex {
    pub fn acquire() -> Result<Self, LaunchError> {
        let handle = unsafe { CreateMutexW(ptr::null(), 0, wide_name().as_ptr()) };

        if handle.is_null() {
            let code = unsafe { GetLastError() };
            return Err(LaunchError::Mutex {
                name: SINGLETON_MUTEX_NAME,
                code,
            });
        }

        let wait = unsafe { WaitForSingleObject(handle, 0) };
        let owns = match wait {
            WAIT_OBJECT_0 | WAIT_ABANDONED => true,
            WAIT_TIMEOUT => false,
            _ => {
                let code = unsafe { GetLastError() };
                unsafe { CloseHandle(handle) };
                return Err(LaunchError::Mutex {
                    name: SINGLETON_MUTEX_NAME,
                    code,
                });
            }
        };

        Ok(Self {
            handle: handle as isize,
            owns,
        })
    }

    #[must_use]
    pub fn is_held(&self) -> bool {
        self.handle != 0 && self.owns
    }
}

impl Drop for SingletonMutex {
    fn drop(&mut self) {
        if self.handle != 0 {
            unsafe {
                if self.owns {
                    ReleaseMutex(self.handle as *mut c_void);
                }
                CloseHandle(self.handle as *mut c_void);
            }
            self.handle = 0;
            self.owns = false;
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

#[cfg(target_pointer_width = "64")]
pub fn read_command_line(process_id: u32) -> Option<String> {
    const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
    const PROCESS_VM_READ: u32 = 0x0010;

    const PEB_PROCESS_PARAMETERS_OFFSET: usize = 0x20;
    const PROCESS_PARAMETERS_COMMAND_LINE_OFFSET: usize = 0x70;
    const UNICODE_STRING_BUFFER_OFFSET: usize = 0x08;
    const MAX_COMMAND_LINE_BYTES: usize = 32_768;

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, process_id);
        if handle.is_null() {
            return None;
        }

        let result = read_command_line_from_handle(
            handle,
            PEB_PROCESS_PARAMETERS_OFFSET,
            PROCESS_PARAMETERS_COMMAND_LINE_OFFSET,
            UNICODE_STRING_BUFFER_OFFSET,
            MAX_COMMAND_LINE_BYTES,
        );

        CloseHandle(handle);
        result
    }
}

#[cfg(not(target_pointer_width = "64"))]
pub fn read_command_line(_process_id: u32) -> Option<String> {
    None
}

#[cfg(target_pointer_width = "64")]
unsafe fn read_command_line_from_handle(
    handle: *mut c_void,
    peb_process_parameters_offset: usize,
    command_line_offset: usize,
    buffer_offset: usize,
    max_bytes: usize,
) -> Option<String> {
    unsafe {
        let mut info = ProcessBasicInformation {
            reserved1: ptr::null_mut(),
            peb_base_address: ptr::null_mut(),
            reserved2: [ptr::null_mut(), ptr::null_mut()],
            unique_process_id: ptr::null_mut(),
            reserved3: ptr::null_mut(),
        };

        let status = NtQueryInformationProcess(
            handle,
            0,
            (&raw mut info).cast(),
            size_of::<ProcessBasicInformation>() as u32,
            ptr::null_mut(),
        );
        if status != 0 || info.peb_base_address.is_null() {
            return None;
        }

        let parameters = read_pointer(
            handle,
            add(info.peb_base_address, peb_process_parameters_offset),
        )?;
        if parameters.is_null() {
            return None;
        }

        let command_line = add(parameters, command_line_offset);
        if command_line.is_null() {
            return None;
        }

        let mut length_bytes = [0u8; 2];
        let mut bytes_read = 0usize;
        if ReadProcessMemory(
            handle,
            command_line.cast_const(),
            length_bytes.as_mut_ptr().cast(),
            2,
            &raw mut bytes_read,
        ) == 0
        {
            return None;
        }

        let length = usize::from(u16::from_le_bytes(length_bytes));
        if length == 0 || length > max_bytes {
            return None;
        }

        let buffer = read_pointer(handle, add(command_line, buffer_offset))?;
        if buffer.is_null() {
            return None;
        }

        let mut bytes = vec![0u8; length];
        let mut read = 0usize;
        if ReadProcessMemory(
            handle,
            buffer,
            bytes.as_mut_ptr().cast(),
            length,
            &raw mut read,
        ) == 0
            || read != length
        {
            return None;
        }

        let wide: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();

        Some(String::from_utf16_lossy(&wide))
    }
}

#[cfg(target_pointer_width = "64")]
unsafe fn read_pointer(handle: *mut c_void, address: *mut c_void) -> Option<*mut c_void> {
    let mut buffer = [0u8; size_of::<*mut c_void>()];
    let mut read = 0usize;

    let ok = unsafe {
        ReadProcessMemory(
            handle,
            address.cast_const(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &raw mut read,
        )
    };

    if ok == 0 || read != buffer.len() {
        return None;
    }

    Some(usize::from_le_bytes(buffer) as *mut c_void)
}

#[cfg(target_pointer_width = "64")]
fn add(address: *mut c_void, offset: usize) -> *mut c_void {
    (address as usize + offset) as *mut c_void
}

#[must_use]
pub fn list_running_clients() -> Vec<RunningClient> {
    find_running_roblox_pids()
        .into_iter()
        .map(|pid| RunningClient {
            pid,
            command_line: read_command_line(pid).unwrap_or_default(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquiring_gives_a_live_handle() {
        let _guard = serialized();
        let mutex = SingletonMutex::acquire().expect("phai tao duoc mutex");
        assert!(mutex.is_held());
    }

    #[test]
    fn acquiring_twice_in_the_same_process_both_succeed() {
        let _guard = serialized();
        let first = SingletonMutex::acquire().expect("lan 1");
        assert!(first.is_held());
        let second = SingletonMutex::acquire().expect("lan 2");
        assert!(
            second.is_held(),
            "Win32 mutex la reentrant trong cung thread, nen lan hai van phai giu duoc"
        );
        drop(second);
        assert!(
            first.is_held(),
            "drop ban sao khong duoc lam mat quyen cua ban goc"
        );
    }

    static MUTEX_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serialized() -> std::sync::MutexGuard<'static, ()> {
        MUTEX_TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    #[test]
    fn the_probe_reports_the_mutex_free_when_nobody_holds_it() {
        let _guard = serialized();
        let probe = probe_singleton_mutex();
        assert_eq!(probe.name, SINGLETON_MUTEX_NAME);
        assert_eq!(
            probe.acquire_attempt,
            MutexOutcome::Acquired,
            "khong ai giu mutex thi probe phai lanh: {probe:?}"
        );
    }

    #[test]
    fn the_probe_sees_our_own_hold_and_reports_it_as_held() {
        let _guard = serialized();
        let mutex = SingletonMutex::acquire().expect("giu mutex");
        let probe = std::thread::spawn(probe_singleton_mutex)
            .join()
            .expect("thread probe");
        assert_eq!(
            probe.acquire_attempt,
            MutexOutcome::HeldByAnotherProcess,
            "win32 mutex giu theo THREAD, nen chi thread khac moi thay bi chiem: {probe:?}"
        );
        drop(mutex);
        let after = std::thread::spawn(probe_singleton_mutex)
            .join()
            .expect("thread probe");
        assert_eq!(after.acquire_attempt, MutexOutcome::Acquired);
    }

    #[test]
    fn dropping_releases_the_handle_and_is_idempotent_in_effect() {
        let _guard = serialized();
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

    #[test]
    fn the_command_line_of_our_own_process_can_be_read() {
        let command_line = read_command_line(std::process::id()).expect("doc duoc cmdline");

        assert!(
            !command_line.is_empty(),
            "cmdline khong duoc rong: {command_line}"
        );
        assert!(
            command_line.contains("muster"),
            "cmdline cua chinh test binary nay phai chua 'muster': {command_line}"
        );
    }

    #[test]
    fn a_process_that_is_gone_reads_as_nothing() {
        assert_eq!(read_command_line(u32::MAX), None);
    }
}
