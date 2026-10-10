# <PROJECT>

Ứng dụng desktop quản lý nhiều tài khoản Roblox và mở game, kế thừa **Roblox Account Manager**
(tiền thân: RBX Alt Manager, tác giả ic3w0lf22 — dự án đã ngừng phát triển).

Windows. Rust + Tauri 2 + React + TypeScript. Giấy phép **GPL-3.0**.

> **Tên dự án chưa chốt.** Codename tạm là `muster`. Đổi tên bằng một lệnh:
> `tools/rename.sh <TenMoi>` — xem `ROADMAP.md` mục 12.

## Trạng thái

Đang ở **M0 — Nền móng**. Xem `ROADMAP.md` mục 6.0 để biết thứ tự thực thi.

| Bước | Việc | Trạng thái |
| :--- | :--- | :--- |
| 0.1 | Test vector vault (4 nhánh định dạng) | ✅ xong |
| 0.2 | Chốt tham số Argon2 | ✅ xong |
| 0.3 | Scaffold workspace + `tools/rename.sh` | ✅ xong |
| 0.4 | `core::vault` + test xanh | 🔶 đang làm (mới có phần nhận diện định dạng) |
| 0.5 | IPC có kiểu bằng `tauri-specta` | ⬜ |
| 0.6 | `core::settings` + migration `RAMSettings.ini` | ⬜ |
| 0.7 | Wizard nhập dữ liệu từ RAM | ⬜ |
| 0.8 | `tracing` + redaction + CI | ⬜ |

## Cấu trúc

```
crates/
├─ muster-core/        logic thuần Rust — KHÔNG phụ thuộc Tauri
├─ muster-api/         local HTTP API (22 route của RAM) + WebSocket
├─ muster-cli/         chạy headless, không cần GUI
└─ muster-app/         binary Tauri 2 (src-tauri/) + frontend React (src/)
tools/
├─ rename.sh           đổi tên dự án bằng một lệnh
└─ fixtures-gen/       sinh test vector cho core::vault
docs/
├─ BROWSER.md          chiến lược đăng nhập: API trước, browser sau
└─ …
RAM_Source/            nguồn tham chiếu GPL-3.0 — ĐỌC, KHÔNG BAO GIỜ SỬA
```

**Nguyên tắc:** mọi logic nằm ở `muster-core`. `muster-app/src-tauri` chỉ chứa command wrapper.
Nếu bạn thấy mình đang viết thuật toán trong `src-tauri` — chuyển nó sang core.

## Yêu cầu

- Windows 10/11
- Rust 1.85+ (đã ghim ở `rust-toolchain.toml`)
- Node 20+
- WebView2 Runtime (Win11 và Win10 21H2+ có sẵn)

## Lệnh

```bash
cargo test                     # 3 crate nhẹ — nhanh, dùng hằng ngày
cargo check --workspace        # gồm cả crate Tauri — chậm hơn
cargo clippy --all-targets -- -D warnings

cargo run -p muster-cli -- inspect <file>   # đoán nhánh định dạng AccountData.json

cd crates/muster-app
npm install
npm run tauri dev              # chạy app desktop
npm run typecheck
```

`cargo check` không tham số chỉ kiểm 3 crate nhẹ (xem `default-members` trong `Cargo.toml`).
Crate Tauri kéo theo rất nhiều phụ thuộc nên chỉ build khi cần.

## Giấy phép

GPL-3.0. Đây là bản phái sinh từ Roblox Account Manager (GPL-3.0) nên **bắt buộc** giữ GPL-3.0
và công khai mã nguồn. Xem `LICENSE` và `NOTICE`.
