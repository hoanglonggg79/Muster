use std::fmt;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use tracing::Event;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::registry::LookupSpan;

pub const REDACTED: &str = "<đã ẩn>";

pub const LOG_FILE_PREFIX: &str = "muster.log";

static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)(\.ROBLOSECURITY\s*[=:]\s*)(\S+)",
        r"(?i)(rbx-authentication-ticket\s*[=:]\s*)(\S+)",
        r"(?i)(x-csrf-token\s*[=:]\s*)(\S+)",
        r#"(?i)((?:password|passwd|secret|cookie|token|ticket)[A-Za-z0-9_]*["']?\s*[=:]\s*)(?:"[^"]*"|'[^']*'|\S+)"#,
    ]
    .iter()
    .map(|pattern| Regex::new(pattern).expect("mẫu regex khử secret phải hợp lệ"))
    .collect()
});

#[must_use]
pub fn redact(text: &str) -> String {
    let mut result = text.to_owned();
    for pattern in PATTERNS.iter() {
        let replacement = format!("${{1}}{REDACTED}");
        if pattern.is_match(&result) {
            result = pattern
                .replace_all(&result, replacement.as_str())
                .into_owned();
        }
    }
    result
}

#[must_use]
pub fn looks_like_it_leaks(text: &str) -> bool {
    PATTERNS.iter().any(|pattern| pattern.is_match(text))
}

#[derive(Debug, Clone, Copy)]
pub struct RedactingFormat<F>(pub F);

impl<F> RedactingFormat<F> {
    pub const fn new(inner: F) -> Self {
        Self(inner)
    }
}

impl<S, N, F> FormatEvent<S, N> for RedactingFormat<F>
where
    S: tracing::Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
    F: FormatEvent<S, N>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut buffer = String::new();
        self.0.format_event(ctx, Writer::new(&mut buffer), event)?;
        write!(writer, "{}", redact(&buffer))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    #[error("không tạo được thư mục log {path}: {source}")]
    CreateDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("subscriber toàn cục đã được đặt trước đó")]
    AlreadyInitialized,
}

pub type LogGuard = tracing_appender::non_blocking::WorkerGuard;

pub fn rolling_writer(
    dir: &Path,
) -> Result<(tracing_appender::non_blocking::NonBlocking, LogGuard), LoggingError> {
    std::fs::create_dir_all(dir).map_err(|source| LoggingError::CreateDir {
        path: dir.to_path_buf(),
        source,
    })?;

    let appender = tracing_appender::rolling::daily(dir, LOG_FILE_PREFIX);
    Ok(tracing_appender::non_blocking(appender))
}

pub fn init_file_logging(dir: &Path, level: &str) -> Result<LogGuard, LoggingError> {
    let (writer, guard) = rolling_writer(dir)?;

    let filter = tracing_subscriber::EnvFilter::try_new(level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .event_format(RedactingFormat::new(
            tracing_subscriber::fmt::format::Format::default(),
        ))
        .try_init()
        .map_err(|_| LoggingError::AlreadyInitialized)?;

    Ok(guard)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::fmt::MakeWriter;

    use super::*;

    #[derive(Clone)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for CaptureWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if let Ok(mut buffer) = self.0.lock() {
                buffer.extend_from_slice(buf);
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'writer> MakeWriter<'writer> for Capture {
        type Writer = CaptureWriter;

        fn make_writer(&'writer self) -> Self::Writer {
            CaptureWriter(Arc::clone(&self.0))
        }
    }

    fn capture(body: impl FnOnce()) -> String {
        let buffer = Arc::new(Mutex::new(Vec::new()));

        let subscriber = tracing_subscriber::fmt()
            .with_writer(Capture(Arc::clone(&buffer)))
            .with_ansi(false)
            .event_format(RedactingFormat::new(
                tracing_subscriber::fmt::format::Format::default(),
            ))
            .finish();

        tracing::subscriber::with_default(subscriber, body);

        let bytes = buffer
            .lock()
            .map_or_else(|_| Vec::new(), |guard| guard.clone());
        String::from_utf8(bytes).expect("log phải là UTF-8")
    }

    #[test]
    fn redact_hides_a_roblosecurity_cookie() {
        let line = "gui request voi .ROBLOSECURITY=ABC123DEF456; done";
        let safe = redact(line);

        assert!(!safe.contains("ABC123DEF456"), "cookie con sot: {safe}");
        assert!(safe.contains(REDACTED));
        assert!(
            safe.contains(".ROBLOSECURITY"),
            "van phai biet do la cookie"
        );
    }

    #[test]
    fn redact_hides_an_authentication_ticket_header() {
        let safe = redact("rbx-authentication-ticket: VGhpcyBpcyBhIHRpY2tldA==");
        assert!(
            !safe.contains("VGhpcyBpcyBhIHRpY2tldA=="),
            "ticket con sot: {safe}"
        );
        assert!(safe.contains(REDACTED));
    }

    #[test]
    fn redact_hides_a_csrf_token_header() {
        let safe = redact("x-csrf-token=ABCDEF123456");
        assert!(!safe.contains("ABCDEF123456"), "csrf con sot: {safe}");
    }

    #[test]
    fn redact_hides_password_and_token_key_value_pairs() {
        for line in [
            "Password=matkhau123",
            "password: matkhau123",
            r#"password="mat khau co dau cach""#,
            "SecurityToken=ABC123",
            "Cookie=ABC123",
            "api_secret=ABC123",
        ] {
            let safe = redact(line);
            assert!(
                !safe.contains("matkhau123") && !safe.contains("ABC123"),
                "con sot secret trong {line:?} -> {safe:?}"
            );
        }
    }

    #[test]
    fn redact_keeps_the_key_name_so_the_log_stays_useful() {
        let safe = redact("Password=matkhau123");
        assert!(safe.starts_with("Password="), "phai giu ten khoa: {safe}");
    }

    #[test]
    fn redact_leaves_ordinary_text_alone() {
        for line in [
            "doc xong 2 tai khoan tu AccountData.json",
            "nhanh dinh dang: password-locked",
            "cong 7963 dang lang nghe",
        ] {
            assert_eq!(
                redact(line),
                line,
                "khong duoc sua dong binh thuong: {line}"
            );
        }
    }

    #[test]
    fn redact_does_not_touch_a_key_that_only_looks_similar() {
        let line = "nap tokenizer xong";
        assert_eq!(redact(line), line);
    }

    #[test]
    fn redact_handles_many_secrets_on_one_line() {
        let safe = redact("cookie=.ROBLOSECURITY=AAA; password=BBB; ticket=CCC");
        assert!(!safe.contains("AAA"));
        assert!(!safe.contains("BBB"));
        assert!(!safe.contains("CCC"));
    }

    #[test]
    fn looks_like_it_leaks_agrees_with_redact() {
        assert!(looks_like_it_leaks("Password=abc"));
        assert!(!looks_like_it_leaks("khong co gi bi mat"));
    }

    #[test]
    fn the_format_wrapper_scrubs_what_actually_gets_written() {
        let output = capture(|| {
            tracing::info!(cookie = ".ROBLOSECURITY=SECRET_COOKIE_VALUE", "thu ghi log");
        });

        assert!(
            !output.contains("SECRET_COOKIE_VALUE"),
            "log thuc te con lo cookie: {output}"
        );
        assert!(
            output.contains(REDACTED),
            "phai co dau hieu da khu: {output}"
        );
    }

    #[test]
    fn the_format_wrapper_scrubs_a_secret_hidden_in_the_message() {
        let output = capture(|| {
            tracing::warn!("dang nhap that bai voi Password=matkhau-that");
        });

        assert!(
            !output.contains("matkhau-that"),
            "log thuc te con lo mat khau: {output}"
        );
    }

    #[test]
    fn the_format_wrapper_leaves_a_clean_event_readable() {
        let output = capture(|| {
            tracing::info!(accounts = 2, "doc xong AccountData.json");
        });

        assert!(
            output.contains("doc xong AccountData.json"),
            "mat noi dung: {output}"
        );
        assert!(output.contains("accounts=2"), "mat field: {output}");
    }

    #[test]
    fn rolling_writer_creates_the_directory_and_writes_a_real_log_file() {
        let dir = std::env::temp_dir().join("muster-logging-test");
        let _ = std::fs::remove_dir_all(&dir);

        let (writer, guard) = rolling_writer(&dir).expect("tạo writer");
        assert!(dir.is_dir(), "phải tạo thư mục log");

        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer)
            .with_ansi(false)
            .event_format(RedactingFormat::new(
                tracing_subscriber::fmt::format::Format::default(),
            ))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("dong log thu nghiem");
        });

        drop(guard);

        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("đọc thư mục log")
            .filter_map(Result::ok)
            .collect();
        assert!(!entries.is_empty(), "phải có file log được tạo");

        let content = entries
            .iter()
            .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
            .collect::<String>();
        assert!(
            content.contains("dong log thu nghiem"),
            "file log phải chứa dòng vừa ghi, nội dung: {content:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_secret_written_to_a_real_log_file_is_scrubbed() {
        let dir = std::env::temp_dir().join("muster-logging-secret-test");
        let _ = std::fs::remove_dir_all(&dir);

        let (writer, guard) = rolling_writer(&dir).expect("tạo writer");
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer)
            .with_ansi(false)
            .event_format(RedactingFormat::new(
                tracing_subscriber::fmt::format::Format::default(),
            ))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::error!("khong dang nhap duoc, cookie=.ROBLOSECURITY=COOKIE_THAT");
        });
        drop(guard);

        let content = std::fs::read_dir(&dir)
            .expect("đọc thư mục log")
            .filter_map(Result::ok)
            .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
            .collect::<String>();

        assert!(
            !content.contains("COOKIE_THAT"),
            "file log tren dia con lo cookie: {content:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
