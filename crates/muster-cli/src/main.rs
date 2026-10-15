use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use muster_core::import;
use muster_core::vault::{self, Branch};

mod backend;

#[derive(Parser, Debug)]
#[command(
    name = "muster-cli",
    version,
    about = "Headless interface for Muster",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Inspect {
        path: PathBuf,
    },

    Import {
        #[arg(long, value_name = "DIR")]
        from: PathBuf,

        #[arg(long, value_name = "PW", conflicts_with = "password_stdin")]
        password: Option<String>,

        #[arg(long)]
        password_stdin: bool,

        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,

        #[arg(long)]
        dry_run: bool,
    },

    Serve {
        #[arg(long, default_value_t = muster_api::DEFAULT_PORT)]
        port: u16,

        #[arg(long, value_name = "PW")]
        password: Option<String>,

        #[arg(long)]
        every_request_requires_password: bool,

        #[arg(long)]
        allow_get_accounts: bool,

        #[arg(long)]
        allow_get_cookie: bool,

        #[arg(long)]
        allow_launch_account: bool,

        #[arg(long)]
        allow_account_editing: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

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
        Command::Serve {
            port,
            password,
            every_request_requires_password,
            allow_get_accounts,
            allow_get_cookie,
            allow_launch_account,
            allow_account_editing,
        } => run_serve(ServeOptions {
            port,
            password,
            every_request_requires_password,
            allow_get_accounts,
            allow_get_cookie,
            allow_launch_account,
            allow_account_editing,
        }),
    }
}

struct ServeOptions {
    port: u16,
    password: Option<String>,
    every_request_requires_password: bool,
    allow_get_accounts: bool,
    allow_get_cookie: bool,
    allow_launch_account: bool,
    allow_account_editing: bool,
}

fn run_serve(options: ServeOptions) -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let data_file = import::portable_account_file();
        let accounts = if data_file.is_file() {
            vault::read_accounts_from_path(&data_file, None)
                .map(|(accs, _)| accs)
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let store = std::sync::Arc::new(std::sync::Mutex::new(
            muster_core::roblox::account::AccountStore::from_accounts(accounts),
        ));
        let backend = std::sync::Arc::new(backend::LiveBackend::new(std::sync::Arc::clone(&store)));

        let mut state = muster_api::ApiState::with_store(store).with_backend(backend);

        state.allow_get_accounts = options.allow_get_accounts;
        state.allow_get_cookie = options.allow_get_cookie;
        state.allow_launch_account = options.allow_launch_account;
        state.allow_account_editing = options.allow_account_editing;
        state.every_request_requires_password = options.every_request_requires_password;

        if let Some(password) = options.password {
            state = state.with_password(password);
        }

        let state = std::sync::Arc::new(state);

        println!(
            "Muster API server đang chạy tại http://127.0.0.1:{}",
            options.port
        );
        println!("(chỉ nhận kết nối từ localhost)");

        muster_api::start_server(options.port, state)
            .await
            .map_err(|e| anyhow::anyhow!("Lỗi server: {e}"))
    })
}

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
            report.out_dir.join(import::SETTINGS_JSON_FILE).display()
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
