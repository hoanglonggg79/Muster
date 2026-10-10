//! # `muster-app` — lớp vỏ Tauri 2
//!
//! Nguyên tắc bất biến #3 (`ROADMAP.md` mục 2): crate này **chỉ** được chứa command wrapper và
//! phần setup. Mọi logic phải nằm ở `muster-core` để `muster-cli` dùng lại được và để kiểm thử
//! bằng `cargo test` mà không cần khởi động Tauri.
//!
//! Nếu bạn thấy mình đang viết một thuật toán trong file này — dừng lại, chuyển nó sang core.

use muster_core::vault::{self, Branch};
use serde::Serialize;

/// Kết quả của [`vault_inspect`], trả về cho frontend.
#[derive(Debug, Serialize)]
pub struct VaultInfo {
    /// Đường dẫn file đã kiểm tra.
    pub path: String,
    /// Kích thước file, tính bằng byte.
    pub size_bytes: usize,
    /// Tên nhánh định dạng: `password-locked`, `dpapi`, hoặc `plain-json`.
    pub branch: String,
}

/// Đoán nhánh định dạng của một file `AccountData.json`.
///
/// Đây là command thử nghiệm để chứng minh đường IPC có kiểu thông suốt từ Rust sang React.
/// Bước M0 0.5 sẽ thay bằng `tauri-specta` để sinh type TypeScript tự động.
///
/// # Errors
///
/// Trả về thông báo lỗi nếu không đọc được file.
#[tauri::command]
fn vault_inspect(path: String) -> Result<VaultInfo, String> {
    let data = std::fs::read(&path).map_err(|e| format!("khong doc duoc {path}: {e}"))?;

    let branch = match vault::detect_branch(&data) {
        Branch::PasswordLocked => "password-locked",
        Branch::Dpapi => "dpapi",
        Branch::PlainJson => "plain-json",
    };

    Ok(VaultInfo {
        path,
        size_bytes: data.len(),
        branch: branch.to_owned(),
    })
}

/// Khởi động ứng dụng desktop.
///
/// # Panics
///
/// Panic nếu cửa sổ chính không khởi tạo được — đây là lỗi không thể phục hồi khi khởi động.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![vault_inspect])
        .run(tauri::generate_context!())
        .expect("khong khoi dong duoc ung dung");
}
