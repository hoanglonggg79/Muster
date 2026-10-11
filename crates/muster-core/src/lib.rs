//! # Muster core
//!
//! Toàn bộ logic của Muster nằm ở đây. Crate này **không** phụ thuộc Tauri, không phụ thuộc
//! WinForms, không phụ thuộc bất cứ thứ gì chỉ chạy được trong GUI. Mọi thứ trong đây phải
//! dùng được từ `muster-cli` và kiểm thử được bằng `cargo test` thuần.
//!
//! Xem `ROADMAP.md` mục 2 (nguyên tắc bất biến #3) và mục 3.2 (cây thư mục mục tiêu).
//!
//! ## Các module sẽ có
//!
//! | Module | Nội dung | Milestone |
//! | :--- | :--- | :--- |
//! | [`models`] | [`models::Account`] — hình dạng dữ liệu dùng chung, hợp đồng với nhánh UI | M0 |
//! | [`datetime`] | định dạng thời gian khớp Json.NET (`LastUse`, `LastAttemptedRefresh`) | M1 |
//! | [`vault`] | 4 nhánh định dạng `AccountData.json`, mã hoá/giải mã | M0 |
//! | [`settings`] | config + migration `RAMSettings.ini` → TOML | M0 |
//! | [`import`] | quét bản cài RAM, sao lưu ZIP, báo cáo, chuyển dữ liệu | M0 |
//! | [`logging`] | tracing, log xoay vòng, khử secret khỏi log | M0 |
//! | [`roblox`] | [`roblox::account`] (CRUD + `Fields`), `auth`, `games`, `servers`, … | M1–M2 |
//! | `launch` | ticket → join script → spawn, multi-instance | M1 |
//! | `watcher` | quét process, tail log, regex | M2 |
//! | `patch` | `ClientAppSettings.json`, FPS unlocker | M1 |
//! | `nexus` | protocol WebSocket điều khiển client | M2 |
//!
//! Các module chưa làm sẽ được thêm khi tới milestone tương ứng — cố tình không tạo file rỗng.

pub mod datetime;
pub mod import;
pub mod launch;
pub mod logging;
pub mod models;
pub mod patch;
pub mod roblox;
pub mod settings;
pub mod vault;
