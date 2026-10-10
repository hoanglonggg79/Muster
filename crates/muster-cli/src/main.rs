//! # `<project>` — giao diện dòng lệnh
//!
//! Chạy headless, không cần GUI. Đây là công cụ để kiểm chứng `muster-core` mà không phải
//! khởi động Tauri (nguyên tắc bất biến #3 trong `ROADMAP.md`).
//!
//! ```text
//! muster-cli inspect <file>        # doan nhanh dinh dang cua mot file AccountData.json
//! muster-cli import --from <dir>   # nhap du lieu tu mot ban cai RAM  (M0 buoc 0.7)
//! muster-cli serve                 # chay local HTTP API              (M2)
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use muster_core::vault::{self, Branch};

#[derive(Parser, Debug)]
#[command(
    name = "muster-cli",
    version,
    about = "Headless interface for <PROJECT>",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Đoán nhánh định dạng của một file `AccountData.json`.
    Inspect {
        /// Đường dẫn tới file cần kiểm tra.
        path: PathBuf,
    },

    /// Nhập tài khoản từ một bản cài Roblox Account Manager. (chưa làm — M0 bước 0.7)
    Import {
        /// Thư mục chứa `AccountData.json` của RAM.
        #[arg(long, value_name = "DIR")]
        from: PathBuf,
    },

    /// Chạy local HTTP API. (chưa làm — M2)
    Serve {
        /// Cổng lắng nghe. Mặc định khớp RAM.
        #[arg(long, default_value_t = muster_api::DEFAULT_PORT)]
        port: u16,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("loi: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Inspect { path } => inspect(&path),
        Command::Import { from } => {
            anyhow::bail!(
                "`import` chua lam — thuoc M0 buoc 0.7 (thu muc: {})",
                from.display()
            )
        }
        Command::Serve { port } => {
            anyhow::bail!("`serve` chua lam — thuoc M2 (cong: {port})")
        }
    }
}

fn inspect(path: &std::path::Path) -> anyhow::Result<()> {
    let data = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("khong doc duoc {}: {e}", path.display()))?;

    let branch = vault::detect_branch(&data);
    let label = match branch {
        Branch::PasswordLocked => "password-locked (Argon2i + secretbox)",
        Branch::Dpapi => "DPAPI (CurrentUser hoac LocalMachine)",
        Branch::PlainJson => "plain JSON",
    };

    println!("file   : {}", path.display());
    println!("size   : {} byte", data.len());
    println!("branch : {label}");

    if branch == Branch::PasswordLocked {
        if data.len() < vault::MIN_PASSWORD_LOCKED_LEN {
            println!(
                "canh bao: file ngan hon {MIN} byte — nhieu kha nang bi hong",
                MIN = vault::MIN_PASSWORD_LOCKED_LEN
            );
        } else {
            println!(
                "salt   : {} byte @ offset {}",
                vault::SALT_LEN,
                vault::SALT_OFFSET
            );
            println!(
                "nonce  : {} byte @ offset {}",
                vault::NONCE_LEN,
                vault::NONCE_OFFSET
            );
        }
    }

    Ok(())
}
