//! # `roblox::auth` — CSRF, auth ticket, và đăng nhập bằng API
//!
//! Ba việc, theo đúng thứ tự phụ thuộc:
//!
//! 1. **Xin CSRF token.** Roblox từ chối `POST` không có header `X-CSRF-TOKEN` bằng `403` kèm
//!    token trong header `x-csrf-token`. Đây là cách "xin" token: cố tình gọi một lần để bị từ chối.
//! 2. **Đổi cookie lấy auth ticket.** Gọi lại chính endpoint đó, lần này kèm token; Roblox trả
//!    ticket trong header `rbx-authentication-ticket`. Ticket này là thứ nhét vào URL mở game.
//! 3. **Đăng nhập bằng `user:pass`.** `POST auth.roblox.com/v2/login`, body
//!    `{"ctype":"Username","cvalue":"<user>","password":"<pass>"}`, kèm token CSRF; thành công thì
//!    Roblox đặt cookie `.ROBLOSECURITY`.
//!
//! ## Vì sao bước 3 là cải tiến, không phải port
//!
//! RAM **không có** đường đăng nhập bằng API. `user:pass` trong RAM chỉ là chữ được gõ sẵn vào
//! form của Chromium, và tính năng nhập hàng loạt mở **một cửa sổ trình duyệt cho mỗi dòng**
//! (`AccountManager.cs:1396-1417`). Nghĩa là muốn thêm 20 tài khoản bằng `user:pass` thì RAM bắt
//! tải ~198 MiB Chromium và mở 20 cửa sổ. Gọi thẳng API bỏ hẳn được cả hai. Xem `docs/BROWSER.md`
//! mục 3.
//!
//! ## Phân loại lỗi là điều kiện tiên quyết, không phải tính năng phụ
//!
//! Biết được "sai mật khẩu" khác với "cần captcha" là điều kiện để quyết định **có nên đề nghị
//! người dùng tải trình duyệt hay không**. Sai mật khẩu thì mở trình duyệt cũng vô ích; captcha
//! thì bắt buộc phải có trình duyệt. Vì vậy [`classify_login_failure`] là một hàm thuần, tách hẳn
//! khỏi lời gọi mạng, và có test cho từng nhánh.
//!
//! ⚠️ **Chưa đối chiếu với Roblox thật.** Hình dạng phản hồi (header `rblx-challenge-type`, cấu
//! trúc `errors[].message`) lấy từ tài liệu và quan sát cộng đồng, chưa chạy thật trong phiên làm
//! việc này. Vì vậy hàm phân loại đọc **nhiều dấu hiệu** cùng lúc và mọi nhánh đều có test để chỉnh
//! lại chỉ tốn một dòng. Cần một lần chạy thật để chốt.

use zeroize::Zeroizing;

use super::http::{HttpMethod, HttpRequest, HttpResponse, Transport, TransportError};

/// `Referer` mà RAM gửi kèm các lời gọi xác thực — `Account.cs:118`.
///
/// Trỏ vào một game cụ thể (Brookhaven) là lựa chọn của RAM; Roblox không đòi hỏi referer trỏ vào
/// game nào, chỉ cần nó hợp lệ. Giữ nguyên để không tự tạo khác biệt.
pub const BROWSER_REFERER: &str = "https://www.roblox.com/games/4924922222/Brookhaven-RP";

/// Tên cookie phiên đăng nhập.
pub const SECURITY_COOKIE: &str = ".ROBLOSECURITY";

/// Các địa chỉ gốc của Roblox API mà M1 cần.
///
/// Tách thành dữ liệu chứ không viết cứng trong hàm, để test đổi được sang một transport giả mà
/// không phải sửa logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// `https://auth.roblox.com` — CSRF, ticket, đăng nhập.
    pub auth: String,
    /// `https://www.roblox.com` — thông tin tài khoản.
    pub www: String,
    /// `https://users.roblox.com` — hồ sơ người dùng.
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

/// Lỗi của lớp xác thực.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// Không gọi được mạng.
    #[error(transparent)]
    Transport(#[from] TransportError),

    /// Phản hồi không kèm header `x-csrf-token`.
    #[error("Roblox không trả về header x-csrf-token (HTTP {status})")]
    MissingCsrfToken {
        /// Mã trạng thái của phản hồi.
        status: u16,
    },

    /// Phản hồi không kèm header `rbx-authentication-ticket`.
    #[error("Roblox không trả về header rbx-authentication-ticket (HTTP {status})")]
    MissingTicket {
        /// Mã trạng thái của phản hồi.
        status: u16,
    },

    /// Sai tên đăng nhập hoặc mật khẩu.
    #[error("sai tên đăng nhập hoặc mật khẩu")]
    InvalidCredentials,

    /// Roblox đòi giải captcha — chỉ trình duyệt mới qua được.
    #[error("Roblox yêu cầu giải captcha")]
    CaptchaRequired {
        /// Mã thách thức, nếu Roblox gửi kèm.
        challenge_id: Option<String>,
    },

    /// Roblox đòi xác thực hai bước.
    #[error("Roblox yêu cầu xác thực hai bước")]
    TwoFactorRequired {
        /// Mã thách thức, nếu Roblox gửi kèm.
        challenge_id: Option<String>,
    },

    /// Bị giới hạn tần suất.
    #[error("bị giới hạn tần suất gọi API")]
    RateLimited {
        /// Số giây nên chờ, nếu Roblox gửi header `Retry-After`.
        retry_after_seconds: Option<u64>,
    },

    /// Tài khoản bị khoá hoặc vô hiệu.
    #[error("tài khoản bị khoá")]
    AccountLocked,

    /// Bị từ chối vì lý do khác — kèm thông báo gốc để còn chẩn đoán.
    #[error("đăng nhập bị từ chối: HTTP {status} — {message}")]
    LoginRejected {
        /// Mã trạng thái.
        status: u16,
        /// Thông báo Roblox trả về; dùng câu thay thế khi không đọc được.
        message: String,
    },
}

/// Client gọi các endpoint xác thực của Roblox.
#[derive(Debug, Clone)]
pub struct AuthClient<T> {
    transport: T,
    endpoints: Endpoints,
}

impl<T: Transport> AuthClient<T> {
    /// Dùng địa chỉ mặc định của Roblox.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            endpoints: Endpoints::default(),
        }
    }

    /// Dùng địa chỉ tuỳ chỉnh.
    pub fn with_endpoints(transport: T, endpoints: Endpoints) -> Self {
        Self {
            transport,
            endpoints,
        }
    }

    /// Xin một CSRF token cho phiên gắn với `cookie`.
    ///
    /// Không kiểm tra mã trạng thái: Roblox trả `403` kèm token khi bị từ chối, nhưng một số
    /// endpoint trả token kèm `200`. Điều kiện thật sự cần là **có header**.
    ///
    /// # Errors
    ///
    /// Trả [`AuthError::MissingCsrfToken`] nếu phản hồi không kèm token,
    /// [`AuthError::Transport`] nếu không gọi được.
    pub fn csrf_token(&self, cookie: &str) -> Result<String, AuthError> {
        let response = self.transport.execute(&self.ticket_request(cookie, None))?;
        extract_csrf_token(&response)
    }

    /// Đổi `cookie` lấy auth ticket.
    ///
    /// Gồm hai lời gọi: xin CSRF rồi gọi lại kèm token.
    ///
    /// # Errors
    ///
    /// Trả [`AuthError::MissingTicket`] nếu Roblox không trả ticket.
    pub fn auth_ticket(&self, cookie: &str) -> Result<Zeroizing<String>, AuthError> {
        let csrf = self.csrf_token(cookie)?;
        let response = self
            .transport
            .execute(&self.ticket_request(cookie, Some(&csrf)))?;

        response
            .header("rbx-authentication-ticket")
            .filter(|ticket| !ticket.is_empty())
            .map(|ticket| Zeroizing::new(ticket.to_owned()))
            .ok_or(AuthError::MissingTicket {
                status: response.status,
            })
    }

    /// Đăng nhập bằng tên đăng nhập và mật khẩu, trả về cookie `.ROBLOSECURITY`.
    ///
    /// # Errors
    ///
    /// Trả một biến thể của [`AuthError`] đã phân loại — xem [`classify_login_failure`].
    pub fn login(&self, username: &str, password: &str) -> Result<Zeroizing<String>, AuthError> {
        // Bước 1: xin CSRF cho phiên ẩn danh. Dùng đúng endpoint mà RAM dùng để lấy token.
        let bootstrap = self.transport.execute(&self.ticket_request("", None))?;
        let csrf = extract_csrf_token(&bootstrap)?;

        // Bước 2: gửi thông tin đăng nhập.
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
        .with_header("Referer", BROWSER_REFERER);

        if !cookie.is_empty() {
            request = request.with_header("Cookie", format!("{SECURITY_COOKIE}={cookie}"));
        }
        if let Some(csrf) = csrf {
            request = request.with_header("X-CSRF-TOKEN", csrf);
        }

        request
    }
}

/// Nối một đường dẫn vào địa chỉ gốc, đảm bảo đúng một dấu `/`.
#[must_use]
pub fn join(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// Thân yêu cầu đăng nhập, đúng hình dạng `docs/BROWSER.md` mục 4 ghi lại.
///
/// Hai khoá `ctype` (giá trị `Username`) và `password` đã xác nhận từ `AccountBrowser.cs:309-312`;
/// tên khoá chứa tên đăng nhập là `cvalue`.
#[must_use]
pub fn login_body(username: &str, password: &str) -> Vec<u8> {
    let body = serde_json::json!({
        "ctype": "Username",
        "cvalue": username,
        "password": password,
    });
    body.to_string().into_bytes()
}

/// Rút token CSRF khỏi header phản hồi.
///
/// # Errors
///
/// Trả [`AuthError::MissingCsrfToken`] nếu không có hoặc rỗng.
pub fn extract_csrf_token(response: &HttpResponse) -> Result<String, AuthError> {
    response
        .header("x-csrf-token")
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .ok_or(AuthError::MissingCsrfToken {
            status: response.status,
        })
}

/// Rút cookie `.ROBLOSECURITY` khỏi các header `Set-Cookie`.
///
/// Roblox có thể gửi nhiều `Set-Cookie` trong một phản hồi, và giá trị cookie có kèm thuộc tính
/// (`Path`, `Secure`, …) sau dấu `;`. Hàm này chỉ lấy đúng cặp tên=giá trị của cookie cần.
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

/// Phân loại một lần đăng nhập thất bại thành lỗi có nghĩa.
///
/// Đọc nhiều dấu hiệu cùng lúc vì Roblox không có một quy ước duy nhất:
///
/// 1. `429` → giới hạn tần suất (kèm `Retry-After` nếu có).
/// 2. Header `rblx-challenge-type` → `twostepverification` hoặc `captcha`.
/// 3. Thông báo trong `errors[0].message` → sai thông tin, captcha, hai bước, khoá tài khoản.
/// 4. Không nhận ra → giữ nguyên thông báo gốc để người dùng còn thấy lý do thật.
#[must_use]
pub fn classify_login_failure(response: &HttpResponse) -> AuthError {
    if response.status == 429 {
        return AuthError::RateLimited {
            retry_after_seconds: response
                .header("retry-after")
                .and_then(|value| value.trim().parse().ok()),
        };
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

/// Câu thay thế khi Roblox từ chối mà không kèm thông báo đọc được.
pub const NO_MESSAGE: &str = "(Roblox không kèm thông báo)";

/// Lấy thông báo lỗi đầu tiên từ thân phản hồi dạng `{"errors":[{"message":"..."}]}`.
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

    /// Transport trả về kịch bản dựng sẵn và ghi lại yêu cầu đã nhận.
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

    // ---------------------------------------------------------------- url join

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

    // ---------------------------------------------------------------- login body

    #[test]
    fn the_login_body_uses_the_keys_from_rams_browser_flow() {
        let body = login_body("alice", "s3cret");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("JSON hợp lệ");

        assert_eq!(value["ctype"], "Username");
        assert_eq!(value["cvalue"], "alice");
        assert_eq!(value["password"], "s3cret");
    }

    // ---------------------------------------------------------------- csrf + ticket

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
    }

    #[test]
    fn a_missing_csrf_token_is_reported_with_its_status() {
        let script = Scripted::new(vec![HttpResponse::new(200)]);
        let error = client(&script).csrf_token("COOKIE").expect_err("phải lỗi");

        assert_eq!(error, AuthError::MissingCsrfToken { status: 200 });
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

    // ---------------------------------------------------------------- login success

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

    // ---------------------------------------------------------------- cookie parsing

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

    // ---------------------------------------------------------------- classification

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
        // Hết kịch bản -> TransportError, KHONG duoc thanh LoginRejected.
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
