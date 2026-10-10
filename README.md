# Muster

Ứng dụng desktop quản lý nhiều tài khoản Roblox và mở game, kế thừa **Roblox Account Manager**
(tiền thân: RBX Alt Manager, tác giả ic3w0lf22 — dự án đã ngừng phát triển).

Windows. Rust + Tauri 2 + React + TypeScript. Giấy phép **GPL-3.0**.

> **Tên dự án chưa chốt.** Codename tạm là `Muster`. Đổi tên bằng một lệnh:
> `tools/rename.sh <TenMoi>` — xem `ROADMAP.md` mục 12.

## Trạng thái

**M0 đã xong hoàn toàn (0.1 → 0.8).** Việc tiếp theo là **M1** — xem `ROADMAP.md` mục 6.

| Bước | Việc | Trạng thái |
| :--- | :--- | :--- |
| 0.1 | Test vector vault (4 nhánh định dạng) | ✅ xong |
| 0.2 | Chốt tham số Argon2 | ✅ xong |
| 0.3 | Scaffold workspace + `tools/rename.sh` | ✅ xong |
| 0.4 | `core::vault` + test xanh | ✅ xong |
| 0.5 | IPC có kiểu bằng `tauri-specta` | ✅ xong |
| 0.6 | `core::settings` + migration `RAMSettings.ini` | ✅ xong |
| 0.7 | Wizard nhập dữ liệu từ RAM | ✅ xong (CLI) |
| 0.8 | `tracing` + redaction + CI | ✅ xong |

Nghiệm thu M0 đã đạt: `muster-cli import` đọc đúng số tài khoản, sao lưu ZIP trước khi ghi, và
**không sửa file gốc của RAM** (đã kiểm bằng so hash trước/sau).

## Cấu trúc

```
crates/
├─ muster-core/        logic thuần Rust — KHÔNG phụ thuộc Tauri
│  └─ src/
│     ├─ models.rs     Account — hợp đồng dữ liệu dùng chung
│     ├─ vault/        4 nhánh AccountData.json + DPAPI
│     ├─ settings/     config + parser RAMSettings.ini
│     ├─ import/       quét bản cài RAM, sao lưu ZIP, chuyển dữ liệu
│     └─ logging.rs    tracing + khử secret khỏi log
├─ muster-api/         local HTTP API (22 route của RAM) + WebSocket
├─ muster-cli/         chạy headless, không cần GUI
└─ muster-app/         binary Tauri 2 (src-tauri/) + frontend React (src/)
   └─ src/bindings.ts  SINH TỰ ĐỘNG — đừng sửa tay
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
- Rust — kênh ghim ở `rust-toolchain.toml` (MSRV 1.85)
- Node 20+
- WebView2 Runtime (Win11 và Win10 21H2+ có sẵn)

## Lệnh

```bash
cargo test                     # 3 crate nhẹ — nhanh, dùng hằng ngày
cargo check --workspace        # gồm cả crate Tauri — chậm hơn
cargo clippy --all-targets -- -D warnings

cargo run -p muster-cli -- inspect <file>                  # đoán nhánh định dạng AccountData.json
cargo run -p muster-cli -- import --from <dir_RAM> --dry-run   # xem trước, không ghi gì
cargo run -p muster-cli -- import --from <dir_RAM>             # nhập thật (có sao lưu ZIP)

cd crates/muster-app
npm install
npm run tauri dev              # chạy app desktop
npm run typecheck
npm run codegen                # sinh lại src/bindings.ts từ code Rust
```

`cargo check` không tham số chỉ kiểm 3 crate nhẹ (xem `default-members` trong `Cargo.toml`).
Crate Tauri kéo theo rất nhiều phụ thuộc nên chỉ build khi cần.

⚠️ `cargo test --workspace` **phải loại trừ `muster-app`** — test binary của crate đó không nạp
được trên Windows (rủi ro #17 trong `ROADMAP.md`). CI cũng làm vậy.

## Giấy phép

GPL-3.0. Đây là bản phái sinh từ Roblox Account Manager (GPL-3.0) nên **bắt buộc** giữ GPL-3.0
và công khai mã nguồn. Xem `LICENSE` và `NOTICE`.
