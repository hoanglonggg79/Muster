use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{cdp, detect, launch, ws};

pub const FORM_POLL_INTERVAL: Duration = Duration::from_millis(300);
pub const COOKIE_POLL_INTERVAL: Duration = Duration::from_millis(700);
pub const COOKIE_READ_TIMEOUT: Duration = Duration::from_secs(2);

pub const NO_BROWSER_MESSAGE: &str = "Muster chỉ dùng Chromium riêng của nó (Chrome for Testing), \
không dùng Chrome/Edge trên máy. Hãy bấm \"Tải Chromium về\" trước.";

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("lỗi vào/ra: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Launch(#[from] launch::BrowserLaunchError),

    #[error(transparent)]
    WebSocket(#[from] ws::WsError),

    #[error("không sinh được số ngẫu nhiên")]
    Random,

    #[error("CDP báo lỗi: {0}")]
    Cdp(String),

    #[error("không thấy {0} trong {1} giây")]
    FormTimeout(&'static str, u64),

    #[error("Hết thời gian chờ đăng nhập")]
    LoginTimeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginPhase {
    Launching,
    WaitingLogin,
    FillingForm,
    ManualLoginRequired(String),
}

pub struct LoginRequest<'a> {
    pub executable: &'a Path,
    pub data_dir: &'a Path,
    pub credentials: Option<(&'a str, &'a str)>,
}

pub struct LoginTimings {
    pub devtools: Duration,
    pub form: Duration,
    pub login: Duration,
}

#[derive(Debug, Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

struct RequestIds {
    next: u64,
}

impl RequestIds {
    fn new() -> Self {
        Self { next: 1 }
    }

    fn take(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }
}

pub fn run_login(
    request: &LoginRequest<'_>,
    timings: &LoginTimings,
    progress: &mut dyn FnMut(LoginPhase),
) -> Result<String, LoginError> {
    let user_agent =
        launch::chrome_user_agent(detect::installed_version(request.data_dir).as_deref());

    let mut session = [0u8; 16];
    getrandom::getrandom(&mut session).map_err(|_| LoginError::Random)?;
    let session_name: String = session.iter().map(|byte| format!("{byte:02x}")).collect();
    let profile = launch::profile_dir(request.data_dir, &session_name);
    std::fs::create_dir_all(&profile)?;

    let args = launch::launch_args(&profile);

    progress(LoginPhase::Launching);
    let mut child = match launch::spawn(request.executable, &args) {
        Ok(child) => child,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&profile);
            return Err(LoginError::Launch(error));
        }
    };

    let outcome = run_session(
        &profile,
        user_agent.as_deref(),
        request.credentials,
        timings,
        progress,
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&profile);

    outcome
}

fn run_session(
    profile: &Path,
    user_agent: Option<&str>,
    credentials: Option<(&str, &str)>,
    timings: &LoginTimings,
    progress: &mut dyn FnMut(LoginPhase),
) -> Result<String, LoginError> {
    let (port, path) =
        launch::wait_for_devtools_endpoint(profile, launch::DEVTOOLS_HOST, timings.devtools)?;

    let mut socket = ws::WebSocket::connect(launch::DEVTOOLS_HOST, port, &path, timings.devtools)?;
    let mut ids = RequestIds::new();

    let _ = send(&mut socket, &mut ids, cdp::enable_network_request)?;

    if let Some(user_agent) = user_agent {
        let _ = send(&mut socket, &mut ids, |id| {
            cdp::set_user_agent_override_request(id, user_agent)
        })?;
    }

    let _ = send(&mut socket, &mut ids, |id| {
        cdp::navigate_request(id, launch::ROBLOX_LOGIN_URL, Some(launch::LOGIN_REFERRER))
    })?;

    progress(LoginPhase::WaitingLogin);

    if let Some((username, password)) = credentials {
        progress(LoginPhase::FillingForm);

        match fill_login_form(&mut socket, &mut ids, username, password, timings.form) {
            Ok(()) => {}
            Err(error @ LoginError::FormTimeout(..)) => {
                progress(LoginPhase::ManualLoginRequired(error.to_string()));
            }
            Err(error) => return Err(error),
        }
    }

    let cookie = wait_for_security_cookie(&mut socket, &mut ids, timings.login)?;
    socket.close();
    Ok(cookie)
}

fn send(
    socket: &mut ws::WebSocket,
    ids: &mut RequestIds,
    build: impl FnOnce(u64) -> String,
) -> Result<Option<Value>, LoginError> {
    let id = ids.take();
    socket.send_text(&build(id))?;
    await_reply(socket, id)
}

fn await_reply(socket: &mut ws::WebSocket, id: u64) -> Result<Option<Value>, LoginError> {
    loop {
        let message = match socket.recv_text() {
            Ok(message) => message,
            Err(ws::WsError::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(LoginError::WebSocket(error)),
        };

        let Ok(reply) = serde_json::from_str::<Value>(&message) else {
            continue;
        };

        if reply.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }

        if let Some(message) = reply.pointer("/error/message").and_then(Value::as_str) {
            return Err(LoginError::Cdp(message.to_owned()));
        }

        return Ok(Some(reply));
    }
}

fn evaluate(
    socket: &mut ws::WebSocket,
    ids: &mut RequestIds,
    expression: &str,
) -> Result<Option<String>, LoginError> {
    let Some(reply) = send(socket, ids, |id| cdp::evaluate_request(id, expression))? else {
        return Ok(None);
    };

    Ok(cdp::reply_string_value(&reply))
}

fn quoted(selector: &str) -> String {
    Value::String(selector.to_owned()).to_string()
}

fn focus_expression(selector: &str) -> String {
    format!(
        "(() => {{ const el = document.querySelector({}); if (!el) return 'missing'; \
         el.focus(); return document.activeElement === el ? 'focused' : 'waiting'; }})()",
        quoted(selector)
    )
}

fn button_center_expression(selector: &str) -> String {
    format!(
        "(() => {{ const el = document.querySelector({}); if (!el) return 'missing'; \
         const box = el.getBoundingClientRect(); \
         if (box.width <= 0 || box.height <= 0) return 'hidden'; \
         return JSON.stringify({{ x: box.left + box.width / 2, y: box.top + box.height / 2 }}); }})()",
        quoted(selector)
    )
}

fn wait_for_focus(
    socket: &mut ws::WebSocket,
    ids: &mut RequestIds,
    selector: &'static str,
    timeout: Duration,
) -> Result<(), LoginError> {
    let deadline = Instant::now() + timeout;
    let expression = focus_expression(selector);

    while Instant::now() < deadline {
        if evaluate(socket, ids, &expression)?.as_deref() == Some("focused") {
            return Ok(());
        }

        std::thread::sleep(FORM_POLL_INTERVAL);
    }

    Err(LoginError::FormTimeout(selector, timeout.as_secs()))
}

fn wait_for_center(
    socket: &mut ws::WebSocket,
    ids: &mut RequestIds,
    selector: &'static str,
    timeout: Duration,
) -> Result<Point, LoginError> {
    let deadline = Instant::now() + timeout;
    let expression = button_center_expression(selector);

    while Instant::now() < deadline {
        if let Some(raw) = evaluate(socket, ids, &expression)? {
            if let Some(point) = parse_point(&raw) {
                return Ok(point);
            }
        }

        std::thread::sleep(FORM_POLL_INTERVAL);
    }

    Err(LoginError::FormTimeout(selector, timeout.as_secs()))
}

fn parse_point(raw: &str) -> Option<Point> {
    let value: Value = serde_json::from_str(raw).ok()?;

    Some(Point {
        x: value.get("x")?.as_f64()?,
        y: value.get("y")?.as_f64()?,
    })
}

fn type_text(
    socket: &mut ws::WebSocket,
    ids: &mut RequestIds,
    text: &str,
) -> Result<(), LoginError> {
    for character in text.chars() {
        let Some(stroke) = cdp::key_stroke(character) else {
            continue;
        };

        let _ = send(socket, ids, |id| cdp::key_down_request(id, &stroke))?;
        let _ = send(socket, ids, |id| cdp::key_up_request(id, &stroke))?;
    }

    Ok(())
}

fn click_at(
    socket: &mut ws::WebSocket,
    ids: &mut RequestIds,
    point: Point,
) -> Result<(), LoginError> {
    let _ = send(socket, ids, |id| {
        cdp::mouse_move_request(id, point.x, point.y)
    })?;
    let _ = send(socket, ids, |id| {
        cdp::mouse_press_request(id, point.x, point.y)
    })?;
    let _ = send(socket, ids, |id| {
        cdp::mouse_release_request(id, point.x, point.y)
    })?;
    Ok(())
}

fn fill_login_form(
    socket: &mut ws::WebSocket,
    ids: &mut RequestIds,
    username: &str,
    password: &str,
    timeout: Duration,
) -> Result<(), LoginError> {
    wait_for_focus(socket, ids, cdp::LOGIN_USERNAME_SELECTOR, timeout)?;
    type_text(socket, ids, username)?;

    wait_for_focus(socket, ids, cdp::LOGIN_PASSWORD_SELECTOR, timeout)?;
    type_text(socket, ids, password)?;

    let point = wait_for_center(socket, ids, cdp::LOGIN_BUTTON_SELECTOR, timeout)?;
    click_at(socket, ids, point)
}

fn wait_for_security_cookie(
    socket: &mut ws::WebSocket,
    ids: &mut RequestIds,
    timeout: Duration,
) -> Result<String, LoginError> {
    socket.set_read_timeout(COOKIE_READ_TIMEOUT)?;

    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        let Some(reply) = send(socket, ids, cdp::get_all_cookies_request)? else {
            continue;
        };

        if let Some(cookie) = cdp::find_security_cookie(&cdp::cookies_from_value(&reply)) {
            return Ok(cookie);
        }

        std::thread::sleep(COOKIE_POLL_INTERVAL);
    }

    Err(LoginError::LoginTimeout)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    fn text_frame(text: &str) -> Vec<u8> {
        let payload = text.as_bytes();
        let mut out = vec![0x81];

        if payload.len() < 126 {
            out.push(payload.len() as u8);
        } else {
            out.push(126);
            out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        }

        out.extend_from_slice(payload);
        out
    }

    fn id_of(request: &str) -> u64 {
        serde_json::from_str::<Value>(request)
            .ok()
            .and_then(|value| value.get("id").and_then(Value::as_u64))
            .expect("moi lenh CDP phai co id")
    }

    fn response_for(id: u64, request: &str, focus_reply: &str) -> String {
        if request.contains("Runtime.evaluate") {
            let value = if request.contains("getBoundingClientRect") {
                r#"{"x":120.5,"y":240.25}"#
            } else {
                focus_reply
            };

            return serde_json::json!({
                "id": id,
                "result": { "result": { "type": "string", "value": value } }
            })
            .to_string();
        }

        if request.contains("Network.getAllCookies") {
            return serde_json::json!({
                "id": id,
                "result": {
                    "cookies": [
                        { "name": ".ROBLOSECURITY", "value": "TOKEN", "domain": ".roblox.com" }
                    ]
                }
            })
            .to_string();
        }

        serde_json::json!({ "id": id, "result": {} }).to_string()
    }

    fn spawn_fake_devtools(
        focus_reply: &'static str,
    ) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");

            let mut handshake = Vec::new();
            let mut chunk = [0u8; 1024];

            while !handshake.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).expect("doc handshake");
                if read == 0 {
                    break;
                }
                handshake.extend_from_slice(&chunk[..read]);
            }

            stream
                .write_all(
                    b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\
                      Connection: Upgrade\r\n\r\n",
                )
                .expect("tra loi handshake");

            let mut requests: Vec<String> = Vec::new();
            let mut pending: Vec<u8> = Vec::new();

            loop {
                if let Some((opcode, payload, consumed)) = ws::decode_frame(&pending) {
                    pending.drain(..consumed);

                    match opcode {
                        ws::OpCode::Close => break,
                        ws::OpCode::Text => {}
                        _ => continue,
                    }

                    let request = String::from_utf8(payload).expect("utf8");
                    let response = response_for(id_of(&request), &request, focus_reply);
                    stream
                        .write_all(&text_frame(&response))
                        .expect("tra loi lenh");
                    requests.push(request);
                    continue;
                }

                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => pending.extend_from_slice(&chunk[..read]),
                }
            }

            requests
        });

        (port, handle)
    }

    fn connect(port: u16) -> ws::WebSocket {
        ws::WebSocket::connect(
            "127.0.0.1",
            port,
            "/devtools/page/fake",
            Duration::from_secs(2),
        )
        .expect("ket noi gia lap")
    }

    fn typed_text(requests: &[String]) -> String {
        requests
            .iter()
            .filter(|request| request.contains("\"type\":\"keyDown\""))
            .filter_map(|request| {
                serde_json::from_str::<Value>(request)
                    .ok()
                    .and_then(|value| {
                        value
                            .pointer("/params/text")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    })
            })
            .collect()
    }

    fn fill(socket: &mut ws::WebSocket, ids: &mut RequestIds) -> Result<(), LoginError> {
        fill_login_form(socket, ids, "alice", "p@ss1", Duration::from_secs(2))
    }

    #[test]
    fn the_form_is_filled_with_real_key_events_not_a_value_assignment() {
        let (port, server) = spawn_fake_devtools("focused");
        let mut socket = connect(port);
        let mut ids = RequestIds::new();

        fill(&mut socket, &mut ids).expect("dien duoc form");

        socket.close();
        let requests = server.join().expect("server xong");

        assert!(
            !requests.iter().any(|request| request.contains(".value")),
            "khong duoc gan .value bang JavaScript: {requests:?}"
        );
        assert_eq!(typed_text(&requests), "alicep@ss1");
    }

    #[test]
    fn the_submit_is_a_real_mouse_press_on_the_button() {
        let (port, server) = spawn_fake_devtools("focused");
        let mut socket = connect(port);
        let mut ids = RequestIds::new();

        fill(&mut socket, &mut ids).expect("dien duoc form");

        socket.close();
        let requests = server.join().expect("server xong");

        let pressed: Vec<Value> = requests
            .iter()
            .filter(|request| request.contains("mousePressed"))
            .filter_map(|request| serde_json::from_str::<Value>(request).ok())
            .collect();

        assert_eq!(pressed.len(), 1, "phai co dung mot cu bam: {requests:?}");
        assert_eq!(pressed[0]["params"]["x"], 120.5);
        assert_eq!(pressed[0]["params"]["y"], 240.25);
        assert_eq!(pressed[0]["params"]["button"], "left");
    }

    #[test]
    fn the_focus_targets_the_documented_selectors_in_order() {
        let (port, server) = spawn_fake_devtools("focused");
        let mut socket = connect(port);
        let mut ids = RequestIds::new();

        fill(&mut socket, &mut ids).expect("dien duoc form");

        socket.close();
        let requests = server.join().expect("server xong");

        let username = requests
            .iter()
            .position(|request| request.contains(cdp::LOGIN_USERNAME_SELECTOR))
            .expect("phai focus o ten dang nhap");
        let password = requests
            .iter()
            .position(|request| request.contains(cdp::LOGIN_PASSWORD_SELECTOR))
            .expect("phai focus o mat khau");
        let button = requests
            .iter()
            .position(|request| request.contains(cdp::LOGIN_BUTTON_SELECTOR))
            .expect("phai bam nut dang nhap");

        assert!(username < password, "ten dang nhap truoc mat khau");
        assert!(password < button, "mat khau truoc nut bam");
    }

    #[test]
    fn a_form_that_never_appears_is_reported_instead_of_silently_ignored() {
        let (port, server) = spawn_fake_devtools("missing");
        let mut socket = connect(port);
        let mut ids = RequestIds::new();

        let error = fill_login_form(
            &mut socket,
            &mut ids,
            "alice",
            "p@ss1",
            Duration::from_millis(200),
        )
        .expect_err("phai het gio");

        assert!(
            matches!(error, LoginError::FormTimeout(..)),
            "gap: {error:?}"
        );

        socket.close();
        let _ = server.join();
    }

    #[test]
    fn the_security_cookie_is_taken_from_the_cookie_list() {
        let (port, server) = spawn_fake_devtools("focused");
        let mut socket = connect(port);
        let mut ids = RequestIds::new();

        let cookie = wait_for_security_cookie(&mut socket, &mut ids, Duration::from_secs(2))
            .expect("bat duoc cookie");

        assert_eq!(cookie, "TOKEN");

        socket.close();
        let _ = server.join();
    }

    #[test]
    fn every_request_gets_a_fresh_id() {
        let mut ids = RequestIds::new();

        assert_eq!(ids.take(), 1);
        assert_eq!(ids.take(), 2);
        assert_eq!(ids.take(), 3);
    }

    #[test]
    fn a_button_center_reply_is_parsed() {
        let point = parse_point(r#"{"x":120.5,"y":240.25}"#).expect("doc duoc");

        assert_eq!(point.x, 120.5);
        assert_eq!(point.y, 240.25);

        assert!(parse_point("hidden").is_none());
        assert!(parse_point("{}").is_none());
        assert!(parse_point("").is_none());
    }

    #[test]
    fn the_expressions_address_the_documented_selectors() {
        assert!(
            focus_expression(cdp::LOGIN_USERNAME_SELECTOR).contains(cdp::LOGIN_USERNAME_SELECTOR)
        );
        assert!(
            button_center_expression(cdp::LOGIN_BUTTON_SELECTOR)
                .contains(cdp::LOGIN_BUTTON_SELECTOR)
        );
        assert!(
            button_center_expression(cdp::LOGIN_BUTTON_SELECTOR).contains("getBoundingClientRect")
        );
    }
}
