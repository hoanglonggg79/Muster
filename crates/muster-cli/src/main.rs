//! # `<project>` — giao diện dòng lệnh
//!
//! Chạy headless, không cần GUI. Đây là công cụ để kiểm chứng `muster-core` mà không phải
//! khởi động Tauri (nguyên tắc bất biến #3 trong `ROADMAP.md`).
//!
//! ```text
//! muster-cli inspect <file>              # đoán nhánh định dạng của một file AccountData.json
//! muster-cli import --from <dir>         # nhập dữ liệu từ một bản cài RAM   (M0 bước 0.7)
//! muster-cli serve                       # chạy local HTTP API              (M2)
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use muster_core::import;
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

    /// Nhập tài khoản từ một bản cài Roblox Account Manager.
    ///
    /// Không sửa file gốc. Luôn tạo một bản sao lưu ZIP trước khi ghi bất cứ thứ gì.
    Import {
        /// Thư mục chứa `AccountData.json` của RAM.
        #[arg(long, value_name = "DIR")]
        from: PathBuf,

        /// Mật khẩu, chỉ cần khi file ở nhánh password-locked.
        ///
        /// Lưu ý: giá trị truyền qua dòng lệnh có thể bị nhìn thấy trong danh sách tiến trình
        /// của hệ điều hành. Với mật khẩu thật, cân nhắc dùng `--password-stdin`.
        #[arg(long, value_name = "PW", conflicts_with = "password_stdin")]
        password: Option<String>,

        /// Đọc mật khẩu từ stdin, không truyền qua dòng lệnh.
        #[arg(long)]
        password_stdin: bool,

        /// Thư mục ghi dữ liệu mới. Mặc định là thư mục dữ liệu của ứng dụng.
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,

        /// Chỉ xem và báo cáo, không ghi gì cả.
        #[arg(long)]
        dry_run: bool,
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

    // Giu guard song suot chuong trinh — roi khoi scope la log ngung duoc ghi.
    // Loi o day khong dang de lam hong ca lan chay: chi bao ra stderr roi di tiep.
    let _log_guard = muster_core::logging::init_file_logging(
        &import::default_data_dir().join("logs"),
        &std::env::var("MUSTER_LOG").unwrap_or_else(|_| "info".to_owned()),
    )
    .map_err(|error| eprintln!("canh bao: khong dung duoc logging ra file: {error}"))
    .ok();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("lenh that bai: {err:#}");
            eprintln!("loi: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Inspect { path } => inspect(&path),
        Command::Import {
            from,
            password,
            password_stdin,
            out,
            dry_run,
        } => {
            let password = resolve_password(password, password_stdin)?;
            run_import(&from, password.as_deref(), out, dry_run)
        }
        Command::Serve { port } => {
            anyhow::bail!("`serve` chua lam — thuoc M2 (cong: {port})")
        }
    }
}

/// Lấy mật khẩu từ `--password` hoặc từ stdin.
///
/// Đọc từ stdin sẽ cắt ký tự xuống dòng ở cuối — người dùng gõ tay hay dùng `echo` đều ra cùng
/// một kết quả.
fn resolve_password(from_flag: Option<String>, from_stdin: bool) -> anyhow::Result<Option<String>> {
    if !from_stdin {
        return Ok(from_flag);
    }

    let mut buffer = String::new();
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut buffer)
        .map_err(|error| anyhow::anyhow!("khong doc duoc mat khau tu stdin: {error}"))?;

    let password = buffer.trim_end_matches(['\r', '\n']).to_owned();
    Ok(Some(password))
}

fn inspect(path: &Path) -> anyhow::Result<()> {
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

fn run_import(
    from: &Path,
    password: Option<&str>,
    out: Option<PathBuf>,
    dry_run: bool,
) -> anyhow::Result<()> {
    if !from.is_dir() {
        anyhow::bail!("khong phai thu muc: {}", from.display());
    }

    let install = import::discover(from);
    if install.is_empty() {
        anyhow::bail!(
            "khong tim thay file nao cua Roblox Account Manager trong {}",
            from.display()
        );
    }

    println!("thu muc RAM : {}", install.root.display());
    println!("tim thay    :");
    for path in install.files_to_back_up() {
        println!("              {}", file_name_of(&path));
    }

    let imported = import::read_accounts(&install, password)?;

    println!();
    println!("tai khoan   : {}", imported.accounts.len());
    println!("nhanh       : {}", imported.format.as_str());
    println!("doc tu      : {}", file_name_of(&imported.source));

    for warning in &imported.warnings {
        println!("canh bao    : {warning}");
    }

    if imported.accounts.is_empty() {
        println!();
        println!("khong co tai khoan nao trong file.");
        return Ok(());
    }

    println!();
    println!("danh sach   :");
    for account in &imported.accounts {
        let group = account.group_or_default();
        let cookie = if account.cookie().is_some() {
            ""
        } else {
            "  (khong co cookie)"
        };
        println!(
            "              {:<24} [{group}]{cookie}",
            account.display_name()
        );
    }

    if dry_run {
        println!();
        println!("--dry-run: khong ghi gi ca.");
        return Ok(());
    }

    let out_dir = out.unwrap_or_else(import::default_data_dir);
    let report = import::migrate(&install, &out_dir, password)?;

    println!();
    println!("sao luu     : {}", report.backup.display());
    println!(
        "da ghi      : {} tai khoan -> {}",
        report.accounts_written,
        report.out_dir.join(import::ACCOUNT_FILE).display()
    );
    if report.settings_written {
        println!(
            "              cau hinh -> {}",
            report.out_dir.join("settings.toml").display()
        );
    }
    for warning in &report.warnings {
        println!("canh bao    : {warning}");
    }
    println!();
    println!("file goc cua RAM khong bi sua.");

    Ok(())
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}
