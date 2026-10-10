//! # `muster-app` — lớp vỏ Tauri 2
//!
//! Nguyên tắc bất biến #3 (`ROADMAP.md` mục 2): crate này **chỉ** được chứa command wrapper và
//! phần setup. Mọi logic phải nằm ở `muster-core` để `muster-cli` dùng lại được và để kiểm thử
//! bằng `cargo test` mà không cần khởi động Tauri.
//!
//! Nếu bạn thấy mình đang viết một thuật toán trong file này — dừng lại, chuyển nó sang core.
//!
//! ## Bindings TypeScript
//!
//! Các command ở đây được `tauri-specta` sinh sang `crates/muster-app/src/bindings.ts`. Đây là
//! **hợp đồng** giữa nhánh Rust và nhánh UI (mục 6.0-UI của roadmap): UI không được tự định nghĩa
//! type cho dữ liệu, mà phải dùng type sinh ra từ đây.
//!
//! File bindings được ghi lại mỗi lần chạy bản debug (`npm run tauri dev`), và test
//! `bindings_are_in_sync` kiểm tra file đã commit còn khớp với code hay không.

use std::path::{Path, PathBuf};

use muster_core::models::Account;
use muster_core::vault::{self, Branch, Format};
use serde::Serialize;
use specta::Type;
use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

/// Kết quả của [`vault_inspect`].
#[derive(Debug, Serialize, Type)]
pub struct VaultInspection {
    /// Đường dẫn file đã kiểm tra.
    pub path: String,
    /// Kích thước file, tính bằng byte.
    pub size_bytes: u32,
    /// Nhánh định dạng đoán được từ phần mở đầu file.
    pub branch: Branch,
}

/// Kết quả của [`vault_open`].
///
/// Cố tình **không** trả về cookie hay mật khẩu: cửa sổ WebView không cần tới chúng, và mọi thứ
/// đi qua IPC đều nằm ngoài tầm kiểm soát của lớp zeroize trong core.
#[derive(Debug, Serialize, Type)]
pub struct VaultOpenSummary {
    /// Đường dẫn file đã mở.
    pub path: String,
    /// Nhánh định dạng đã dùng để mở được nội dung.
    pub format: Format,
    /// Số tài khoản đọc được.
    pub account_count: u32,
    /// Tên hiển thị của từng tài khoản, theo đúng thứ tự trong file.
    pub usernames: Vec<String>,
}

/// Đoán nhánh định dạng của một file `AccountData.json` mà không giải mã.
///
/// # Errors
///
/// Trả về thông báo lỗi nếu không đọc được file.
#[tauri::command]
#[specta::specta]
fn vault_inspect(path: String) -> Result<VaultInspection, String> {
    let data = std::fs::read(&path).map_err(|error| format!("không đọc được {path}: {error}"))?;

    Ok(VaultInspection {
        path,
        size_bytes: u32::try_from(data.len()).unwrap_or(u32::MAX),
        branch: vault::detect_branch(&data),
    })
}

/// Mở một file `AccountData.json` và trả về bản tóm tắt danh sách tài khoản.
///
/// # Errors
///
/// Trả về thông báo lỗi nếu không đọc được file, thiếu mật khẩu, hoặc sai mật khẩu.
#[tauri::command]
#[specta::specta]
fn vault_open(path: String, password: Option<String>) -> Result<VaultOpenSummary, String> {
    let (accounts, format) = vault::read_accounts_from_path(Path::new(&path), password.as_deref())
        .map_err(|error| error.to_string())?;

    Ok(VaultOpenSummary {
        path,
        format,
        account_count: u32::try_from(accounts.len()).unwrap_or(u32::MAX),
        usernames: accounts
            .iter()
            .map(|account| account.display_name().to_owned())
            .collect(),
    })
}

/// Đường dẫn file bindings TypeScript được sinh ra.
///
/// Tính từ `CARGO_MANIFEST_DIR` chứ không phải thư mục làm việc: chạy `cargo run -p muster-app`
/// từ gốc workspace sẽ có thư mục làm việc khác với chạy `npm run tauri dev`, và đường dẫn tương
/// đối sẽ ghi nhầm chỗ.
#[must_use]
pub fn bindings_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bindings.ts")
}

/// Dựng danh sách command và type cho `tauri-specta`.
///
/// [`Account`] được đăng ký bằng [`Builder::typ`] dù không command nào trả về nó: UI cần hình dạng
/// của `Account` để dựng danh sách tài khoản ở M1, và mục 6.0-UI nói rõ UI **không được tự định
/// nghĩa type cho dữ liệu**. Đăng ký ở đây để type đó có mặt trong bindings ngay từ bây giờ.
#[must_use]
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![vault_inspect, vault_open])
        .typ::<Account>()
}

/// Cấu hình bộ sinh TypeScript.
///
/// `Account::user_id` là `i64` nhưng đã được khai đè sang `number` ngay tại field
/// (`#[specta(type = f64)]` trong `muster-core::models`) — chỗ đúng để nói rằng field đó đi qua
/// đường truyền dưới dạng số. Nhờ vậy không cần bật `enable_lossless_bigints()` ở đây, vốn sẽ
/// sinh ra `bigint` trong TypeScript — sai với thực tế `serde_json` gửi số JSON.
#[must_use]
pub fn typescript_exporter() -> Typescript {
    Typescript::default()
}

/// Ghi bindings TypeScript vào [`bindings_path`].
///
/// # Errors
///
/// Trả về thông báo lỗi nếu không ghi được file.
pub fn export_bindings() -> Result<(), String> {
    specta_builder()
        .export(typescript_exporter(), bindings_path())
        .map_err(|error| error.to_string())
}

/// Khởi động ứng dụng desktop.
///
/// # Panics
///
/// Panic nếu cửa sổ chính không khởi tạo được — đây là lỗi không thể phục hồi khi khởi động.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    // Chỉ ghi bindings ở bản debug: bản phát hành không được ghi vào cây nguồn.
    #[cfg(debug_assertions)]
    export_bindings().expect("không ghi được bindings TypeScript");

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("không khởi động được ứng dụng");
}
