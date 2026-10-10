//! # `logging` — tracing, log xoay vòng, và khử secret
//!
//! Nguyên tắc bất biến #4: **log không bao giờ ghi cookie, auth ticket hay mật khẩu.** Module
//! này là chốt chặn cuối cùng cho điều đó.
//!
//! ## Hai lớp bảo vệ, không phải một
//!
//! 1. **Kiểu dữ liệu tự khử.** `models::Account` và `settings::WebServer` có `Debug` viết tay
//!    nên `tracing::debug!(?account)` đã an toàn. Đây là lớp chính, vì nó khử *theo ngữ nghĩa*.
//! 2. **Lọc theo mẫu ở đầu ra.** [`RedactingFormat`] quét chuỗi đã định dạng và thay mọi thứ
//!    trông giống secret bằng [`REDACTED`]. Đây là lớp dự phòng, vì nó bắt được cả những chỗ
//!    secret bị nhét vào một `String` thường rồi `{}` ra log.
//!
//! Lớp 2 **không thể** thay lớp 1: nó chỉ nhận ra được những mẫu đã biết, và một chuỗi bí mật
//! đứng trơ trọi giữa câu văn thì nó không nhận ra. Vì vậy đừng dựa vào nó rồi log ẩu.
//!
//! ## Vì sao lọc ở tầng định dạng chứ không phải ở tầng `Layer`
//!
//! `tracing` cho `Layer` can thiệp vào sự kiện, nhưng giá trị của field chỉ được biết khi
//! `Visitor` đi qua nó, và việc dựng lại một `Event` đã sửa là không làm được. Bọc
//! `FormatEvent` thì đơn giản, chạy được cho mọi subscriber của `fmt`, và chỉ có một chỗ để
//! kiểm thử.

use std::fmt;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use tracing::Event;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::registry::LookupSpan;

/// Chuỗi thay thế cho mọi secret bị khử.
pub const REDACTED: &str = "<đã ẩn>";

/// Tên file log trong thư mục dữ liệu.
pub const LOG_FILE_PREFIX: &str = "muster.log";

/// Các mẫu nhận diện secret.
///
/// Cố tình **hẹp**: thà bỏ sót một mẫu lạ còn hơn khử nhầm dữ liệu bình thường khiến log vô dụng.
/// Mọi mẫu đều có một nhóm bắt `key = ` hoặc `key: ` ở đầu để chỉ thay phần giá trị.
static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // Cookie phien dang nhap Roblox.
        r"(?i)(\.ROBLOSECURITY\s*[=:]\s*)(\S+)",
        // Header lay auth ticket.
        r"(?i)(rbx-authentication-ticket\s*[=:]\s*)(\S+)",
        // Header CSRF.
        r"(?i)(x-csrf-token\s*[=:]\s*)(\S+)",
        // Cap key=value ma ten khoa noi ro no la secret. Bat ca dang co ngoac kep.
        r#"(?i)((?:password|passwd|secret|cookie|token|ticket)[A-Za-z0-9_]*["']?\s*[=:]\s*)(?:"[^"]*"|'[^']*'|\S+)"#,
    ]
    .iter()
    .map(|pattern| Regex::new(pattern).expect("mẫu regex khử secret phải hợp lệ"))
    .collect()
});

/// Thay mọi secret nhận ra được trong `text` bằng [`REDACTED`].
///
/// Giữ lại phần tên khoá để log vẫn còn hữu ích: `Password=<đã ẩn>` vẫn cho biết có một mật khẩu
/// ở đó, khác hẳn với việc xoá cả dòng.
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

/// `true` nếu `text` còn chứa thứ gì đó trông giống secret.
///
/// Dùng cho test và cho các chốt chặn nội bộ.
#[must_use]
pub fn looks_like_it_leaks(text: &str) -> bool {
    PATTERNS.iter().any(|pattern| pattern.is_match(text))
}

/// Bọc một [`FormatEvent`] để khử secret trên chuỗi đã định dạng.
#[derive(Debug, Clone, Copy)]
pub struct RedactingFormat<F>(pub F);

impl<F> RedactingFormat<F> {
    /// Bọc `inner`.
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

/// Lỗi khi dựng lớp logging.
#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    /// Không tạo được thư mục log.
    #[error("không tạo được thư mục log {path}: {source}")]
    CreateDir {
        /// Thư mục định tạo.
        path: std::path::PathBuf,
        /// Lỗi gốc.
        source: std::io::Error,
    },

    /// Subscriber toàn cục đã được đặt trước đó.
    #[error("subscriber toàn cục đã được đặt trước đó")]
    AlreadyInitialized,
}

/// Bộ giữ cho luồng ghi log nền.
///
/// Phải giữ sống suốt thời gian chạy chương trình. Rơi khỏi scope là log ngừng được ghi — đây là
/// hành vi của `tracing-appender`, không phải lỗi.
pub type LogGuard = tracing_appender::non_blocking::WorkerGuard;

/// Tạo writer ghi log xoay vòng theo ngày trong `dir`.
///
/// Tách khỏi [`init_file_logging`] vì hàm này **không** đụng tới trạng thái toàn cục, nên dùng
/// được trong test và dùng được khi cần nhiều đích ghi log.
///
/// # Errors
///
/// Trả [`LoggingError::CreateDir`] nếu không tạo được thư mục log.
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

/// Dựng logging toàn cục ghi ra file xoay vòng theo ngày, có khử secret.
///
/// Trả về guard; caller phải giữ nó sống. Gọi hai lần trong cùng tiến trình sẽ trả
/// [`LoggingError::AlreadyInitialized`] chứ không panic.
///
/// # Errors
///
/// Trả [`LoggingError`] nếu không tạo được thư mục log hoặc subscriber đã được đặt trước đó.
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

    /// Ghi log vào một buffer trong bộ nhớ để kiểm tra đầu ra thật.
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

    /// Chạy `body` với một subscriber dùng [`RedactingFormat`], trả về nội dung đã ghi.
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
        // "tokenizer" chua "token" nhung khong phai cap key=value cua secret.
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

        // Guard phai con song thi log nen moi duoc ghi ra dia.
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
