//! # `roblox` — lớp giao tiếp với Roblox
//!
//! Các module con ở đây tái lập hành vi của `RAM_Source/RBX Alt Manager/`, nhưng chia theo **miền
//! nghiệp vụ** chứ không theo form WinForms:
//!
//! | Module | Nội dung | Milestone |
//! | :--- | :--- | :--- |
//! | [`account`] | danh sách tài khoản, CRUD, túi `Fields` | M1 |
//! | [`http`] | lớp vận chuyển trung lập — mọi lời gọi mạng đi qua trait `Transport` | M1 |
//! | [`auth`] | CSRF → auth ticket, đăng nhập bằng API, phân loại lỗi | M1 |
//! | `games`, `servers`, `presence`, `batch` | server list, presence, thumbnail | M2 |
//!
//! Nguyên tắc bất biến #3: mọi logic nằm ở `muster-core`, dùng được từ `muster-cli`, không phụ
//! thuộc Tauri.

pub mod account;
pub mod auth;
pub mod http;
