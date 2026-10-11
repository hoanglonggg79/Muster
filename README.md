# Muster

Ứng dụng desktop quản lý nhiều tài khoản Roblox và mở game, kế thừa **Roblox Account Manager**
(tiền thân: RBX Alt Manager, tác giả ic3w0lf22).

Windows. Rust + Tauri 2 + React + TypeScript.

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

## Giấy phép

GPL-3.0=
