//! # `launch::windows` — phần chỉ chạy trên Windows của `core::launch`
//!
//! ## Vì sao khai báo FFI trực tiếp
//!
//! Cùng lý do đã ghi ở `vault/dpapi.rs`: chỉ cần đúng ba hàm của `kernel32`, ABI của chúng đã
//! đóng băng, và với một chỗ chạm vào `unsafe` thì khả năng đọc hết bằng mắt có giá trị riêng so
//! với việc kéo vào crate `windows` hàng trăm nghìn dòng sinh tự động.
//!
//! ## Vì sao file này không có `#[cfg]` bên trong
//!
//! Cả module được `#[cfg(windows)]` ở `launch/mod.rs`. Sản phẩm chỉ nhắm Windows (quyết định
//! 2026-10-10), nên ở đây không trừu tượng hoá hệ điều hành.

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

/// Handle đang giữ tới named mutex `ROBLOX_singletonMutex`.
///
/// ## Cách nó phá chống nhiều client
///
/// Roblox tạo named mutex này lúc khởi động và kiểm tra `GetLastError() == ERROR_ALREADY_EXISTS`.
/// Nếu mutex **đã tồn tại** thì Roblox bỏ qua phần cưỡng chế một-client. Vì vậy chỉ cần app ta tạo
/// mutex đó trước và **giữ handle** trong suốt thời gian chạy là Roblox cho mở nhiều client. Ta
/// không cần sở hữu mutex, chỉ cần nó tồn tại — nên `bInitialOwner = FALSE`.
///
/// ## Vì sao `handle` là `isize` chứ không phải con trỏ
///
/// Handle của kernel là giá trị dùng được từ **mọi luồng** trong tiến trình, và đóng nó từ luồng
/// khác luồng tạo là hợp lệ. Giữ dưới dạng `isize` khiến kiểu này tự động `Send + Sync`, nhờ vậy
/// nhét được vào state của Tauri mà không cần `unsafe impl` — và không có `unsafe impl` nào để
/// viết sai.
pub struct SingletonMutex {
    handle: isize,
}

impl SingletonMutex {
    /// Tạo (hoặc mở lại) named mutex và giữ handle cho tới khi giá trị này bị drop.
    ///
    /// Gọi nhiều lần trong cùng tiến trình đều thành công — lần thứ hai nhận handle tới **cùng**
    /// một đối tượng mutex của kernel.
    ///
    /// # Errors
    ///
    /// Trả [`LaunchError::Mutex`] nếu Win32 từ chối tạo.
    pub fn acquire() -> Result<Self, LaunchError> {
        let name: Vec<u16> = SINGLETON_MUTEX_NAME
            .encode_utf16()
            .chain(core::iter::once(0))
            .collect();

        // SAFETY: `name` la chuoi wide ket thuc bang 0 va song trong suot loi goi; attributes = NULL
        // la hop le (dung security descriptor mac dinh).
        let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };

        if handle.is_null() {
            // SAFETY: GetLastError khong co tien dieu kien nao.
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

    /// `true` nếu đang giữ một handle hợp lệ.
    #[must_use]
    pub fn is_held(&self) -> bool {
        self.handle != 0
    }
}

impl Drop for SingletonMutex {
    fn drop(&mut self) {
        if self.handle != 0 {
            // SAFETY: handle nay do CreateMutexW cap va chua tung bi dong.
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
        // Lan thu hai mo lai cung mot doi tuong mutex cua kernel — khong duoc loi.
        let first = SingletonMutex::acquire().expect("lan 1");
        let second = SingletonMutex::acquire().expect("lan 2");
        assert!(first.is_held() && second.is_held());
    }

    #[test]
    fn dropping_releases_the_handle_and_is_idempotent_in_effect() {
        let mutex = SingletonMutex::acquire().expect("tao mutex");
        drop(mutex);

        // Sau khi drop, van phai tao lai duoc (neu handle ro ri thi khong anh huong, nhung phep
        // thu nay bat duoc truong hop dong hai lan lam hong handle khac).
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
