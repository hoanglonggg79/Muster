# Muster

Ứng dụng desktop quản lý nhiều tài khoản Roblox và mở game, kế thừa **Roblox Account Manager**
(tiền thân: RBX Alt Manager, tác giả ic3w0lf22 — dự án đã ngừng phát triển).

Windows. Rust + Tauri 2 + React + TypeScript. Giấy phép **GPL-3.0**.

> **Tên dự án đã chốt: `Muster`** (2026-10-12). Nếu sau này cần đổi tên, dùng một lệnh:
> `tools/rename.sh <TenMoi>` — xem `ROADMAP.md` §10.3.

## Trạng thái

**M0 xong hoàn toàn (0.1 → 0.8).** **M1 ĐÃ ĐÓNG (2026-10-12).** **GĐ 2A xong** (Local HTTP API 22
route). **GĐ 2C gần xong** (game/account + UI). **GĐ 2D** mới có nhánh browser — `core::watcher` chưa
viết. **41 command Tauri.** Chi tiết: `ROADMAP.md` §9.

Cổng đo ngày 2026-10-15: **398 test xanh** (349 core + 11 vault + 38 api), `tsc` sạch, lint Biome
**0 error / 0 warning** (21 file), `clippy -D warnings` sạch, bindings khớp codegen — chi tiết ở
`ROADMAP.md` §5.

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
| M1 | `roblox::auth` (CSRF/ticket/login API + retry/rate limit + phân loại lỗi) | ✅ xong |
| M1 | `roblox::api` (user/robux/avatar) + `roblox::presence` + `roblox::vip` | ✅ xong |
| M1 | `launch` (URL, legacy, mutex, auto-close) + `patch` + `datetime` giờ địa phương | ✅ xong |
| M1 | GUI Tauri 2 + React: launch đơn/hàng loạt, sửa account, import có mật khẩu | ✅ xong |
| M1 | Nghiệm thu chạy thật (5 account song song, không kick) | ⏳ chờ chạy thật |
| 2A | `muster-api` 22 route + `muster-cli serve` với `LiveBackend` + `docs/API.md` | ✅ xong |
| 2C | `roblox::groups/games/servers/batch` + `account_ext` + UI game/account | 🟡 gần xong |
| 2D | `core::browser` (Chrome for Testing + CDP) | 🟡 có code, chưa nghiệm thu chạy thật |
| 2D | `core::watcher` (quét process, tail log, auto-relaunch) | ⬜ chưa làm |
| 2B | Nexus (WS cổng 5242) + `assets/lua/Nexus.lua` | ⬜ chưa làm |

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
│     ├─ import/       quét bản cài RAM, chuyển dữ liệu, thư mục dữ liệu
│     ├─ launch/       URL/legacy launch, mutex, auto-close
│     ├─ patch/        ClientAppSettings.json + FPS unlocker
│     ├─ browser/      tải Chrome for Testing + CDP qua WebSocket tự viết
│     ├─ datetime.rs   chuỗi thời gian theo Json.NET
│     ├─ roblox/       account, account_ext, api, auth, batch, games, groups,
│     │                http, limiter, presence, servers, vip
│     └─ logging.rs    tracing + khử secret khỏi log
├─ muster-api/         local HTTP API — 22 route (/v1/ text thô + /v2/ JSON)
├─ muster-cli/         chạy headless, không cần GUI — inspect · import · serve
└─ muster-app/         binary Tauri 2 (src-tauri/) + frontend React (src/)
   └─ src/bindings.ts  SINH TỰ ĐỘNG — đừng sửa tay
assets/
├─ AccountData.json    vault thật để test import — KHÔNG commit
└─ lua/, icons/        RỖNG (chưa port Nexus.lua / icon thật)
tools/
├─ rename.sh           đổi tên dự án bằng một lệnh
├─ gen-icons.py        sinh icon bundle từ icon.ico / icon.jpg ở gốc
└─ fixtures-gen/       sinh test vector cho core::vault
docs/
├─ BROWSER.md          chiến lược đăng nhập: API trước, browser sau
└─ API.md              đặc tả 22 route Local HTTP API
RAM_Source/            nguồn tham chiếu GPL-3.0 — ĐỌC, KHÔNG BAO GIỜ SỬA
```

**Nguyên tắc:** mọi logic nằm ở `muster-core`. `muster-app/src-tauri` chỉ chứa command wrapper.
Nếu bạn thấy mình đang viết thuật toán trong `src-tauri` — chuyển nó sang core.

> ⚠️ **Ngoại lệ đang tồn tại (nợ, xem `ROADMAP.md` §9.6):** luồng đăng nhập bằng browser
> (`run_browser_login`, `fill_login_form`, `wait_for_security_cookie`) **vẫn nằm trong
> `src-tauri/src/lib.rs`** → không test được bằng `cargo test`. Cần chuyển xuống
> `muster-core::browser::login`.

## Yêu cầu

- Windows 10/11
- Rust — MSRV 1.85; kênh ghim ở `rust-toolchain.toml` (1.98)
- Node 20+
- WebView2 Runtime (Win11 và Win10 21H2+ có sẵn)

## Lệnh

```bash
cargo test                     # 3 crate nhẹ (default-members) — nhanh, dùng hằng ngày
cargo test --workspace         # toàn bộ, gồm cả crate Tauri — 398 test
cargo check --workspace        # gồm cả crate Tauri — chậm hơn
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings

cargo run -p muster-cli -- inspect <file>                  # đoán nhánh định dạng AccountData.json
cargo run -p muster-cli -- import --from <dir_RAM> --dry-run   # xem trước, không ghi gì
cargo run -p muster-cli -- import --from <dir_RAM>             # nhập thật (có sao lưu ZIP)
cargo run -p muster-cli -- serve                               # Local HTTP API (mặc định localhost)

cd crates/muster-app
npm install
npm run tauri dev              # chạy app desktop
npm run typecheck
npm run lint                   # Biome (KHÔNG phải ESLint — typescript-eslint xung đột peer với TS 7)
npm run build                  # tsc --noEmit + vite build
npm run codegen                # sinh lại src/bindings.ts từ code Rust
```

`cargo check` không tham số chỉ kiểm 3 crate nhẹ (xem `default-members` trong `Cargo.toml`).
Crate Tauri kéo theo rất nhiều phụ thuộc nên chỉ build khi cần.

`cargo test --workspace` chạy được cả `muster-app` — lỗi nạp test binary của crate đó (trước gọi là
rủi ro #17) đã kiểm chứng lại là **không còn tái hiện**. Nếu gặp `Access is denied (os error 5)`
trong `target/debug/build/...` thì đó là lỗi khoá/tranh chấp file (rủi ro #16), **không phải lỗi
code**: chạy lại đúng lệnh đó lần hai, hoặc `cargo clean -p muster-app` sau khi đóng mọi tiến trình
`Muster.exe` / dev server. Xem `ROADMAP.md` §6.1.

## Giấy phép

GPL-3.0. Đây là bản phái sinh từ Roblox Account Manager (GPL-3.0) nên **bắt buộc** giữ GPL-3.0
và công khai mã nguồn. Xem `LICENSE` và `NOTICE`.
