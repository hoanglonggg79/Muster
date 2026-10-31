use zeroize::Zeroizing;

use super::http::{HttpMethod, HttpRequest, HttpResponse, Transport, TransportError};

pub const BROWSER_REFERER: &str = "https://www.roblox.com/games/4924922222/Brookhaven-RP";

pub const SECURITY_COOKIE: &str = ".ROBLOSECURITY";
pub const TICKET_CONTENT_TYPE: &str = "application/x-www-form-urlencoded";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub auth: String,
    pub www: String,
    pub users: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            auth: "https://auth.roblox.com".to_owned(),
            www: "https://www.roblox.com".to_owned(),
            users: "https://users.roblox.com".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error("Roblox không trả về header x-csrf-token (HTTP {status})")]
    MissingCsrfToken { status: u16 },

    #[error("Cookie .ROBLOSECURITY không hợp lệ hoặc đã hết hạn (HTTP 401 Unauthorized)")]
    Unauthorized,

    #[error("Roblox không trả về header rbx-authentication-ticket (HTTP {status})")]
    MissingTicket { status: u16 },

    #[error("lấy CSRF token thất bại: HTTP {status} — {message}")]
    CsrfRequestFailed { status: u16, message: String },

    #[error("lấy authentication ticket thất bại: HTTP {status} — {message}")]
    TicketRequestFailed { status: u16, message: String },

    #[error("sai tên đăng nhập hoặc mật khẩu")]
    InvalidCredentials,

    #[error("Roblox yêu cầu giải captcha")]
    CaptchaRequired { challenge_id: Option<String> },

    #[error("Roblox yêu cầu xác thực hai bước")]
    TwoFactorRequired { challenge_id: Option<String> },

    #[error("bị giới hạn tần suất gọi API")]
    RateLimited { retry_after_seconds: Option<u64> },

    #[error("tài khoản bị khoá")]
    AccountLocked,

    #[error("đăng nhập bị từ chối: HTTP {status} — {message}")]
    LoginRejected { status: u16, message: String },
}

#[derive(Debug, Clone)]
pub struct AuthClient<T> {
    transport: T,
    endpoints: Endpoints,
}

impl<T: Transport> AuthClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            endpoints: Endpoints::default(),
        }
    }

    pub fn with_endpoints(transport: T, endpoints: Endpoints) -> Self {
        Self {
            transport,
            endpoints,
        }
    }

    pub fn csrf_token(&self, cookie: &str) -> Result<String, AuthError> {
        let response = self.transport.execute(&self.ticket_request(cookie, None))?;
        if response.status == 401 {
            return Err(AuthError::Unauthorized);
        }
        if response.status == 429 {
            return Err(rate_limited(&response));
        }
        if response.status != 403 {
            return Err(AuthError::CsrfRequestFailed {
                status: response.status,
                message: failure_message(&response),
            });
        }
        extract_csrf_token(&response)
    }

    pub fn auth_ticket(&self, cookie: &str) -> Result<Zeroizing<String>, AuthError> {
        let csrf = self.csrf_token(cookie)?;
        self.auth_ticket_with_csrf(cookie, &csrf)
    }

    pub fn auth_ticket_with_csrf(
        &self,
        cookie: &str,
        csrf: &str,
    ) -> Result<Zeroizing<String>, AuthError> {
        let response = self
            .transport
            .execute(&self.ticket_request(cookie, Some(csrf)))?;

        if response.status == 401 {
            return Err(AuthError::Unauthorized);
        }
        if response.status == 429 {
            return Err(rate_limited(&response));
        }
        if !(200..300).contains(&response.status) {
            return Err(AuthError::TicketRequestFailed {
                status: response.status,
                message: failure_message(&response),
            });
        }

        response
            .header("rbx-authentication-ticket")
            .filter(|ticket| !ticket.is_empty())
            .map(|ticket| Zeroizing::new(ticket.to_owned()))
            .ok_or(AuthError::MissingTicket {
                status: response.status,
            })
    }

    pub fn login(&self, username: &str, password: &str) -> Result<Zeroizing<String>, AuthError> {
        let bootstrap_request =
            HttpRequest::new(HttpMethod::Post, join(&self.endpoints.auth, "v2/login"))
                .with_header("Referer", BROWSER_REFERER);
        let bootstrap = self.transport.execute(&bootstrap_request)?;
        if bootstrap.status == 401 {
            return Err(AuthError::Unauthorized);
        }
        let csrf = extract_csrf_token(&bootstrap)?;

        let body = login_body(username, password);
        let request = HttpRequest::new(HttpMethod::Post, join(&self.endpoints.auth, "v2/login"))
            .with_header("Content-Type", "application/json")
            .with_header("X-CSRF-TOKEN", csrf)
            .with_header("Referer", BROWSER_REFERER)
            .with_body(body);

        let response = self.transport.execute(&request)?;

        if !(200..300).contains(&response.status) {
            return Err(classify_login_failure(&response));
        }

        roblosecurity_from_set_cookie(&response.headers)
            .map(Zeroizing::new)
            .ok_or(AuthError::LoginRejected {
                status: response.status,
                message: "đăng nhập thành công nhưng không thấy cookie .ROBLOSECURITY".to_owned(),
            })
    }

    fn ticket_request(&self, cookie: &str, csrf: Option<&str>) -> HttpRequest {
        let mut request = HttpRequest::new(
            HttpMethod::Post,
            join(&self.endpoints.auth, "v1/authentication-ticket/"),
        )
        .with_header("Content-Type", TICKET_CONTENT_TYPE)
        .with_header("Referer", BROWSER_REFERER)
        .with_body(Vec::new());

        if !cookie.is_empty() {
            request = request.with_header("Cookie", format!("{SECURITY_COOKIE}={cookie}"));
        }
        if let Some(csrf) = csrf {
            request = request.with_header("X-CSRF-TOKEN", csrf);
        }

        request
    }
}

#[must_use]
pub fn sanitize_cookie(raw: &str) -> String {
    let trimmed = raw.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(trimmed);
    unquoted.trim().to_owned()
}

#[must_use]
pub fn join(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[must_use]
pub fn login_body(username: &str, password: &str) -> Vec<u8> {
    let body = serde_json::json!({
        "ctype": "Username",
        "cvalue": username,
        "password": password,
    });
    body.to_string().into_bytes()
}

pub fn extract_csrf_token(response: &HttpResponse) -> Result<String, AuthError> {
    response
        .header("x-csrf-token")
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .ok_or(AuthError::MissingCsrfToken {
            status: response.status,
        })
}

#[must_use]
pub fn roblosecurity_from_set_cookie(headers: &[(String, String)]) -> Option<String> {
    for (name, value) in headers {
        if !name.eq_ignore_ascii_case("set-cookie") {
            continue;
        }

        for part in value.split(';') {
            let part = part.trim();
            if let Some(rest) = part.strip_prefix(SECURITY_COOKIE) {
                if let Some(value) = rest.strip_prefix('=') {
                    if !value.is_empty() {
                        return Some(value.to_owned());
                    }
                }
            }
        }
    }
    None
}

fn rate_limited(response: &HttpResponse) -> AuthError {
    AuthError::RateLimited {
        retry_after_seconds: response
            .header("retry-after")
            .and_then(|value| value.trim().parse().ok()),
    }
}

fn failure_message(response: &HttpResponse) -> String {
    let raw = first_error_message(response.body_as_str().unwrap_or(""))
        .unwrap_or_else(|| response.body_as_str().unwrap_or("").trim().to_owned());
    let message = raw.trim();
    if message.is_empty() {
        return NO_MESSAGE.to_owned();
    }

    let mut shortened: String = message.chars().take(512).collect();
    if message.chars().count() > 512 {
        shortened.push('…');
    }
    shortened
}

#[must_use]
pub fn classify_login_failure(response: &HttpResponse) -> AuthError {
    if response.status == 429 {
        return rate_limited(response);
    }

    let challenge_id = response.header("rblx-challenge-id").map(str::to_owned);

    if let Some(kind) = response.header("rblx-challenge-type") {
        match kind.to_ascii_lowercase().as_str() {
            "twostepverification" => return AuthError::TwoFactorRequired { challenge_id },
            "captcha" => return AuthError::CaptchaRequired { challenge_id },
            _ => {}
        }
    }

    let message = first_error_message(response.body_as_str().unwrap_or(""))
        .unwrap_or_else(|| NO_MESSAGE.to_owned());

    let lowered = message.to_ascii_lowercase();

    if lowered.contains("incorrect username or password") {
        return AuthError::InvalidCredentials;
    }
    if lowered.contains("captcha") {
        return AuthError::CaptchaRequired { challenge_id };
    }
    if lowered.contains("two step")
        || lowered.contains("two-step")
        || lowered.contains("two-factor")
        || lowered.contains("verification")
    {
        return AuthError::TwoFactorRequired { challenge_id };
    }
    if lowered.contains("locked") || lowered.contains("banned") || lowered.contains("disabled") {
        return AuthError::AccountLocked;
    }

    AuthError::LoginRejected {
        status: response.status,
        message,
    }
}

pub const NO_MESSAGE: &str = "(Roblox không kèm thông báo)";

#[must_use]
pub fn first_error_message(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value
        .get("errors")?
        .as_array()?
        .first()?
        .get("message")?
        .as_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::*;

    #[derive(Default)]
    struct Scripted {
        responses: RefCell<VecDeque<HttpResponse>>,
        requests: RefCell<Vec<HttpRequest>>,
    }

    impl Scripted {
        fn new(responses: Vec<HttpResponse>) -> Self {
            Self {
                responses: RefCell::new(responses.into()),
                requests: RefCell::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<HttpRequest> {
            self.requests.borrow().clone()
        }
    }

    impl Transport for &Scripted {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            self.requests.borrow_mut().push(request.clone());
            self.responses
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| TransportError::new("hết kịch bản"))
        }
    }

    fn client(script: &Scripted) -> AuthClient<&Scripted> {
        AuthClient::new(script)
    }

    #[test]
    fn joining_a_path_never_doubles_or_drops_the_slash() {
        assert_eq!(
            join("https://auth.roblox.com", "v2/login"),
            "https://auth.roblox.com/v2/login"
        );
        assert_eq!(
            join("https://auth.roblox.com/", "/v2/login"),
            "https://auth.roblox.com/v2/login"
        );
    }

    #[test]
    fn the_login_body_uses_the_keys_from_rams_browser_flow() {
        let body = login_body("alice", "s3cret");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("JSON hợp lệ");

        assert_eq!(value["ctype"], "Username");
        assert_eq!(value["cvalue"], "alice");
        assert_eq!(value["password"], "s3cret");
    }

    #[test]
    fn a_csrf_token_is_read_from_the_forbidden_response() {
        let script = Scripted::new(vec![
            HttpResponse::new(403).with_header("x-csrf-token", "TOKEN-1"),
        ]);

        let token = client(&script).csrf_token("COOKIE").expect("lấy token");
        assert_eq!(token, "TOKEN-1");
    }

    #[test]
    fn the_ticket_request_carries_the_cookie_and_the_referer() {
        let script = Scripted::new(vec![
            HttpResponse::new(403).with_header("x-csrf-token", "T"),
        ]);
        let _ = client(&script).csrf_token("COOKIE-X");

        let requests = script.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, HttpMethod::Post);
        assert_eq!(
            requests[0].url,
            "https://auth.roblox.com/v1/authentication-ticket/"
        );
        assert_eq!(
            requests[0].header("cookie"),
            Some(".ROBLOSECURITY=COOKIE-X")
        );
        assert_eq!(requests[0].header("referer"), Some(BROWSER_REFERER));
        assert_eq!(
            requests[0].header("content-type"),
            Some(TICKET_CONTENT_TYPE)
        );
        assert_eq!(requests[0].body.as_deref(), Some([].as_slice()));
    }

    #[test]
    fn a_missing_csrf_token_is_reported_with_its_status() {
        let script = Scripted::new(vec![HttpResponse::new(403)]);
        let error = client(&script).csrf_token("COOKIE").expect_err("phải lỗi");

        assert_eq!(error, AuthError::MissingCsrfToken { status: 403 });
    }

    #[test]
    fn an_unexpected_csrf_status_is_not_misreported_as_a_missing_header() {
        let script = Scripted::new(vec![
            HttpResponse::new(415).with_body("Unsupported Media Type"),
        ]);
        let error = client(&script).csrf_token("COOKIE").expect_err("phải lỗi");

        assert_eq!(
            error,
            AuthError::CsrfRequestFailed {
                status: 415,
                message: "Unsupported Media Type".to_owned()
            }
        );
    }

    #[test]
    fn a_401_on_the_csrf_fetch_means_the_cookie_is_dead_not_a_missing_header() {
        let script = Scripted::new(vec![HttpResponse::new(401)]);
        let error = client(&script).csrf_token("COOKIE").expect_err("phải lỗi");

        assert_eq!(error, AuthError::Unauthorized);
    }

    #[test]
    fn a_401_on_the_login_bootstrap_is_reported_as_unauthorized() {
        let script = Scripted::new(vec![HttpResponse::new(401)]);
        let error = client(&script).login("alice", "pw").expect_err("phải lỗi");

        assert_eq!(error, AuthError::Unauthorized);
    }

    #[test]
    fn cookies_are_trimmed_and_unquoted_before_use() {
        assert_eq!(sanitize_cookie("  abc  "), "abc");
        assert_eq!(sanitize_cookie("\"abc\""), "abc");
        assert_eq!(sanitize_cookie(" \"abc\"\n"), "abc");
        assert_eq!(sanitize_cookie(""), "");
        assert_eq!(
            sanitize_cookie("_|WARNING:-DO-NOT-SHARE-THIS.|XYZ"),
            "_|WARNING:-DO-NOT-SHARE-THIS.|XYZ",
            "tiền tố cảnh báo là một phần giá trị cookie thật, không được cắt"
        );
    }

    #[test]
    fn an_auth_ticket_takes_two_calls_and_uses_the_token() {
        let script = Scripted::new(vec![
            HttpResponse::new(403).with_header("x-csrf-token", "TOKEN-1"),
            HttpResponse::new(200).with_header("rbx-authentication-ticket", "TICKET-9"),
        ]);

        let ticket = client(&script).auth_ticket("COOKIE").expect("lấy ticket");
        assert_eq!(ticket.as_str(), "TICKET-9");

        let requests = script.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].header("x-csrf-token"), Some("TOKEN-1"));
        for request in &requests {
            assert_eq!(request.header("content-type"), Some(TICKET_CONTENT_TYPE));
            assert_eq!(request.body.as_deref(), Some([].as_slice()));
        }
    }

    #[test]
    fn a_missing_ticket_is_reported() {
        let script = Scripted::new(vec![
            HttpResponse::new(403).with_header("x-csrf-token", "TOKEN-1"),
            HttpResponse::new(200),
        ]);

        let error = client(&script).auth_ticket("COOKIE").expect_err("phải lỗi");
        assert_eq!(error, AuthError::MissingTicket { status: 200 });
    }

    #[test]
    fn a_ticket_http_failure_is_not_misreported_as_a_missing_ticket() {
        let script = Scripted::new(vec![
            HttpResponse::new(403).with_header("x-csrf-token", "TOKEN-1"),
            HttpResponse::new(415).with_body("Unsupported Media Type"),
        ]);

        let error = client(&script).auth_ticket("COOKIE").expect_err("phải lỗi");
        assert_eq!(
            error,
            AuthError::TicketRequestFailed {
                status: 415,
                message: "Unsupported Media Type".to_owned()
            }
        );
        assert!(!error.to_string().contains("không trả về header"));
    }

    #[test]
    fn a_401_on_the_ticket_fetch_means_the_cookie_is_dead() {
        let script = Scripted::new(vec![HttpResponse::new(401)]);
        let error = client(&script)
            .auth_ticket_with_csrf("COOKIE", "TOKEN")
            .expect_err("phải lỗi");
        assert_eq!(error, AuthError::Unauthorized);
    }

    #[test]
    fn a_successful_login_returns_the_security_cookie() {
        let script = Scripted::new(vec![
            HttpResponse::new(403).with_header("x-csrf-token", "TOKEN-1"),
            HttpResponse::new(200).with_header(
                "Set-Cookie",
                ".ROBLOSECURITY=COOKIE-ABC; Path=/; Secure; HttpOnly",
            ),
        ]);

        let cookie = client(&script).login("alice", "pw").expect("đăng nhập");
        assert_eq!(cookie.as_str(), "COOKIE-ABC");

        let requests = script.requests();
        assert_eq!(requests[1].url, "https://auth.roblox.com/v2/login");
        assert_eq!(requests[1].header("x-csrf-token"), Some("TOKEN-1"));
        assert!(requests[1].header("content-type").is_some());
    }

    #[test]
    fn the_login_body_is_json_with_the_password() {
        let script = Scripted::new(vec![
            HttpResponse::new(403).with_header("x-csrf-token", "T"),
            HttpResponse::new(200).with_header("Set-Cookie", ".ROBLOSECURITY=C"),
        ]);
        let _ = client(&script).login("alice", "pw").expect("đăng nhập");

        let requests = script.requests();
        let body = requests[1].body.as_deref().expect("có thân");
        let value: serde_json::Value = serde_json::from_slice(body).expect("JSON hợp lệ");
        assert_eq!(value["cvalue"], "alice");
        assert_eq!(value["password"], "pw");
    }

    #[test]
    fn a_login_without_a_cookie_in_the_response_is_reported() {
        let script = Scripted::new(vec![
            HttpResponse::new(403).with_header("x-csrf-token", "T"),
            HttpResponse::new(200),
        ]);

        let error = client(&script).login("alice", "pw").expect_err("phải lỗi");
        assert!(matches!(
            error,
            AuthError::LoginRejected { status: 200, .. }
        ));
    }

    #[test]
    fn a_security_cookie_is_found_among_several_set_cookie_headers() {
        let headers = vec![
            ("Set-Cookie".to_owned(), "other=1; Path=/".to_owned()),
            (
                "Set-Cookie".to_owned(),
                ".ROBLOSECURITY=THE-COOKIE; Path=/; Secure".to_owned(),
            ),
        ];
        assert_eq!(
            roblosecurity_from_set_cookie(&headers),
            Some("THE-COOKIE".to_owned())
        );
    }

    #[test]
    fn a_cookie_with_no_value_is_not_returned() {
        let headers = vec![(
            "Set-Cookie".to_owned(),
            ".ROBLOSECURITY=; Path=/".to_owned(),
        )];
        assert_eq!(roblosecurity_from_set_cookie(&headers), None);
    }

    #[test]
    fn a_similarly_named_cookie_is_not_confused_for_the_security_cookie() {
        let headers = vec![(
            "Set-Cookie".to_owned(),
            "ROBLOSECURITY=WRONG; Path=/".to_owned(),
        )];
        assert_eq!(roblosecurity_from_set_cookie(&headers), None);
    }

    fn json_body(text: &str) -> Vec<u8> {
        text.as_bytes().to_vec()
    }

    #[test]
    fn a_wrong_password_is_recognised() {
        let response = HttpResponse::new(400).with_body(json_body(
            r#"{"errors":[{"code":0,"message":"Incorrect username or password. Please try again."}]}"#,
        ));
        assert_eq!(
            classify_login_failure(&response),
            AuthError::InvalidCredentials
        );
    }

    #[test]
    fn a_rate_limit_carries_the_retry_after_header() {
        let response = HttpResponse::new(429).with_header("Retry-After", "30");
        assert_eq!(
            classify_login_failure(&response),
            AuthError::RateLimited {
                retry_after_seconds: Some(30)
            }
        );
    }

    #[test]
    fn a_rate_limit_without_the_header_still_classifies() {
        let response = HttpResponse::new(429);
        assert_eq!(
            classify_login_failure(&response),
            AuthError::RateLimited {
                retry_after_seconds: None
            }
        );
    }

    #[test]
    fn a_captcha_challenge_header_is_recognised() {
        let response = HttpResponse::new(403)
            .with_header("rblx-challenge-type", "captcha")
            .with_header("rblx-challenge-id", "CHALLENGE-1");

        assert_eq!(
            classify_login_failure(&response),
            AuthError::CaptchaRequired {
                challenge_id: Some("CHALLENGE-1".to_owned())
            }
        );
    }

    #[test]
    fn a_two_step_challenge_header_is_recognised() {
        let response = HttpResponse::new(403)
            .with_header("rblx-challenge-type", "twostepverification")
            .with_header("rblx-challenge-id", "CHALLENGE-2");

        assert_eq!(
            classify_login_failure(&response),
            AuthError::TwoFactorRequired {
                challenge_id: Some("CHALLENGE-2".to_owned())
            }
        );
    }

    #[test]
    fn a_captcha_message_without_a_header_is_still_recognised() {
        let response = HttpResponse::new(403).with_body(json_body(
            r#"{"errors":[{"message":"Please complete the captcha challenge."}]}"#,
        ));
        assert!(matches!(
            classify_login_failure(&response),
            AuthError::CaptchaRequired { .. }
        ));
    }

    #[test]
    fn a_locked_account_is_recognised() {
        let response = HttpResponse::new(403).with_body(json_body(
            r#"{"errors":[{"message":"This account has been locked."}]}"#,
        ));
        assert_eq!(classify_login_failure(&response), AuthError::AccountLocked);
    }

    #[test]
    fn an_unrecognised_failure_keeps_the_original_message() {
        let response = HttpResponse::new(500).with_body(json_body(
            r#"{"errors":[{"message":"Something exploded"}]}"#,
        ));

        assert_eq!(
            classify_login_failure(&response),
            AuthError::LoginRejected {
                status: 500,
                message: "Something exploded".to_owned()
            }
        );
    }

    #[test]
    fn a_body_that_is_not_json_does_not_panic() {
        let response = HttpResponse::new(503).with_body(json_body("<html>down</html>"));
        assert_eq!(
            classify_login_failure(&response),
            AuthError::LoginRejected {
                status: 503,
                message: NO_MESSAGE.to_owned()
            }
        );
    }

    #[test]
    fn a_transport_failure_propagates_instead_of_being_misread_as_a_rejection() {
        let script = Scripted::new(vec![]);
        let error = client(&script).login("alice", "pw").expect_err("phải lỗi");
        assert!(matches!(error, AuthError::Transport(_)), "gặp: {error:?}");
    }

    #[test]
    fn first_error_message_reads_the_documented_shape() {
        assert_eq!(
            first_error_message(r#"{"errors":[{"message":"hello"}]}"#),
            Some("hello".to_owned())
        );
        assert_eq!(first_error_message(r#"{"errors":[]}"#), None);
        assert_eq!(first_error_message("khong phai json"), None);
    }
}
