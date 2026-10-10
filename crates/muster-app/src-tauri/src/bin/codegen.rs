//! # `muster-codegen` — sinh bindings TypeScript
//!
//! Chạy: `npm run codegen` (trong `crates/muster-app`), hoặc
//! `cargo run -p muster-app --bin muster-codegen`.
//!
//! ## Vì sao là binary riêng chứ không phải một test
//!
//! Cách thường làm là để việc sinh bindings trong một `#[test]`. Ở crate này không dùng được:
//! test binary do `cargo test -p muster-app` sinh ra **không nạp được trên Windows** — chạy trực
//! tiếp trả về mã thoát 127, còn cargo báo `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)`. Đã thử
//! bỏ `staticlib`/`cdylib` khỏi `crate-type` và kiểm tra `dbghelp.dll` trên `PATH`, cả hai đều
//! không phải nguyên nhân. Binary thường thì chạy được, nên việc sinh bindings nằm ở đây.
//!
//! Xem rủi ro #17 trong `ROADMAP.md`.
//!
//! ## Kiểm tra lệch bindings
//!
//! Không có test nào lo việc này — test binary của crate không nạp được (xem trên). Thay vào đó,
//! CI chạy chính binary này rồi `git diff --exit-code -- crates/muster-app/src/bindings.ts`.
//! `npm run codegen` trong `crates/muster-app` cũng gọi đúng binary này.

use std::process::ExitCode;

fn main() -> ExitCode {
    let path = muster_app_lib::bindings_path();

    match muster_app_lib::export_bindings() {
        Ok(()) => {
            println!("đã ghi bindings: {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("lỗi: {error}");
            ExitCode::FAILURE
        }
    }
}
