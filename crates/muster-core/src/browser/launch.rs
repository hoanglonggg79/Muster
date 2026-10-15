use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use super::cdp;

pub const ROBLOX_LOGIN_URL: &str = "https://www.roblox.com/login";
pub const LOGIN_REFERRER: &str = "https://google.com/";
pub const BLANK_PAGE: &str = "about:blank";
pub const DEVTOOLS_ACTIVE_PORT_FILE: &str = "DevToolsActivePort";
pub const DEVTOOLS_HOST: &str = "127.0.0.1";

#[derive(Debug, thiserror::Error)]
pub enum BrowserLaunchError {
    #[error("không mở được tiến trình trình duyệt: {0}")]
    Spawn(#[from] std::io::Error),

    #[error("trình duyệt không ghi file {DEVTOOLS_ACTIVE_PORT_FILE} trong {0} giây")]
    DevtoolsTimeout(u64),
}

#[must_use]
pub fn launch_args(user_data_dir: &Path) -> Vec<String> {
    vec![
        format!("--user-data-dir={}", user_data_dir.display()),
        "--remote-debugging-port=0".to_owned(),
        format!("--remote-debugging-address={DEVTOOLS_HOST}"),
        "--no-first-run".to_owned(),
        "--no-default-browser-check".to_owned(),
        "--disable-background-networking".to_owned(),
        "--disable-component-update".to_owned(),
        "--remote-allow-origins=*".to_owned(),
        BLANK_PAGE.to_owned(),
    ]
}

#[must_use]
pub fn chrome_user_agent(version: Option<&str>) -> Option<String> {
    let major = version?.split('.').next()?;

    if major.is_empty() || !major.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    Some(format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
         (KHTML, like Gecko) Chrome/{major}.0.0.0 Safari/537.36"
    ))
}

pub fn spawn(executable: &Path, args: &[String]) -> Result<Child, BrowserLaunchError> {
    Ok(Command::new(executable).args(args).spawn()?)
}

pub fn read_devtools_active_port(user_data_dir: &Path) -> Option<(u16, String)> {
    let text = std::fs::read_to_string(user_data_dir.join(DEVTOOLS_ACTIVE_PORT_FILE)).ok()?;
    let mut lines = text.lines();

    let port: u16 = lines.next()?.trim().parse().ok()?;
    let path = lines.next()?.trim().to_owned();

    if path.is_empty() {
        return None;
    }

    Some((port, path))
}

pub fn wait_for_devtools(
    user_data_dir: &Path,
    timeout: Duration,
) -> Result<(u16, String), BrowserLaunchError> {
    wait_for_devtools_endpoint(user_data_dir, DEVTOOLS_HOST, timeout)
}

fn http_get(host: &str, port: u16, path: &str) -> Result<String, std::io::Error> {
    let address: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(500))?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let mut response = String::new();
    stream.take(1024 * 1024).read_to_string(&mut response)?;
    Ok(response)
}

fn json_body_from_http(response: &str) -> Option<&str> {
    response
        .find("\r\n\r\n")
        .map(|index| response[index + 4..].trim())
}

fn page_endpoint_from_document(body: &str, active_port: u16) -> Option<(u16, String)> {
    let url = cdp::page_socket_url(body)?;
    let port = cdp::ws_port_from_url(&url)?;
    if port != active_port || !url.starts_with(&format!("ws://{DEVTOOLS_HOST}:{active_port}/")) {
        return None;
    }
    Some((port, cdp::ws_path_from_url(&url)?))
}

pub fn wait_for_devtools_endpoint(
    user_data_dir: &Path,
    host: &str,
    timeout: Duration,
) -> Result<(u16, String), BrowserLaunchError> {
    let start = Instant::now();
    let step = Duration::from_millis(100);

    loop {
        if let Some((port, _)) = read_devtools_active_port(user_data_dir) {
            if let Ok(response) = http_get(host, port, cdp::DEVTOOLS_PAGES_PATH) {
                if response.starts_with("HTTP/1.1 200") {
                    if let Some(body) = json_body_from_http(&response) {
                        if let Some(endpoint) = page_endpoint_from_document(body, port) {
                            return Ok(endpoint);
                        }
                    }
                }
            }
        }

        if start.elapsed() >= timeout {
            return Err(BrowserLaunchError::DevtoolsTimeout(timeout.as_secs()));
        }

        std::thread::sleep(step);
    }
}

#[must_use]
pub fn profile_dir(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join("browser").join("profiles").join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arguments_pin_the_profile_and_the_localhost_debug_channel() {
        let args = launch_args(Path::new("C:/p"));

        assert!(args.contains(&"--user-data-dir=C:/p".to_owned()));
        assert!(args.contains(&"--remote-debugging-port=0".to_owned()));
        assert!(args.contains(&"--remote-debugging-address=127.0.0.1".to_owned()));
        assert!(args.contains(&"--remote-allow-origins=*".to_owned()));
        assert_eq!(args.last().map(String::as_str), Some(BLANK_PAGE));
    }

    #[test]
    fn the_login_url_never_rides_on_the_command_line() {
        let args = launch_args(Path::new("C:/p"));

        assert!(!args.contains(&ROBLOX_LOGIN_URL.to_owned()));
        assert!(
            !args.iter().any(|arg| arg.contains("roblox.com")),
            "gap: {args:?}"
        );
        assert!(
            !args
                .iter()
                .any(|arg| arg.contains("--disable-web-security"))
        );
    }

    #[test]
    fn the_user_agent_follows_the_installed_chrome_major_version() {
        assert_eq!(
            chrome_user_agent(Some("155.0.8059.39")).as_deref(),
            Some(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36"
            )
        );
        assert_eq!(
            chrome_user_agent(Some("131.0.6778.85")).as_deref(),
            Some(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
            )
        );
    }

    #[test]
    fn an_unusable_version_yields_no_user_agent_override() {
        assert_eq!(chrome_user_agent(None), None);
        assert_eq!(chrome_user_agent(Some("")), None);
        assert_eq!(chrome_user_agent(Some("stable")), None);
        assert_eq!(chrome_user_agent(Some("v131")), None);
    }

    #[test]
    fn a_written_active_port_file_is_read_back() {
        let dir = std::env::temp_dir().join("muster-browser-launch-port");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tao thu muc");

        std::fs::write(
            dir.join(DEVTOOLS_ACTIVE_PORT_FILE),
            "9222\n/devtools/browser/abc-123\n",
        )
        .expect("ghi file");

        assert_eq!(
            read_devtools_active_port(&dir),
            Some((9222, "/devtools/browser/abc-123".to_owned()))
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_or_broken_port_file_reads_as_none() {
        let dir = std::env::temp_dir().join("muster-browser-launch-broken");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tao thu muc");

        assert_eq!(read_devtools_active_port(&dir), None);

        std::fs::write(dir.join(DEVTOOLS_ACTIVE_PORT_FILE), "khong-phai-so\n").expect("ghi");
        assert_eq!(read_devtools_active_port(&dir), None);

        std::fs::write(dir.join(DEVTOOLS_ACTIVE_PORT_FILE), "9222\n").expect("ghi");
        assert_eq!(read_devtools_active_port(&dir), None, "thieu dong thu hai");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn waiting_gives_up_after_the_timeout_instead_of_hanging() {
        let dir = std::env::temp_dir().join("muster-browser-launch-wait");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tao thu muc");

        let start = Instant::now();
        let error = wait_for_devtools(&dir, Duration::from_millis(150)).expect_err("phai het gio");

        assert!(matches!(error, BrowserLaunchError::DevtoolsTimeout(_)));
        assert!(start.elapsed() >= Duration::from_millis(140));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_live_devtools_page_is_chosen_instead_of_the_browser_socket() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("local listener");
        let port = listener.local_addr().expect("local address").port();
        let dir = std::env::temp_dir().join(format!("muster-browser-target-{port}"));
        std::fs::create_dir_all(&dir).expect("create profile");
        std::fs::write(
            dir.join(DEVTOOLS_ACTIVE_PORT_FILE),
            format!("{port}\n/devtools/browser/wrong-target\n"),
        )
        .expect("write active port");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let mut request = [0u8; 1024];
            let read = stream.read(&mut request).expect("read request");
            assert!(String::from_utf8_lossy(&request[..read]).contains("GET /json/list"));
            let body = format!(
                "[{{\"type\":\"page\",\"url\":\"https://www.roblox.com/login\",\"webSocketDebuggerUrl\":\"ws://127.0.0.1:{port}/devtools/page/login\"}}]"
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .expect("write response");
        });
        let endpoint = wait_for_devtools(&dir, Duration::from_secs(2)).expect("page endpoint");
        assert_eq!(endpoint, (port, "/devtools/page/login".to_owned()));
        server.join().expect("server finished");
        std::fs::remove_dir_all(&dir).expect("remove test profile");
    }

    #[test]
    fn the_profile_directory_is_derived_from_the_data_directory() {
        assert_eq!(
            profile_dir(Path::new("C:/data"), "login"),
            Path::new("C:/data/browser/profiles/login")
        );
    }
}
