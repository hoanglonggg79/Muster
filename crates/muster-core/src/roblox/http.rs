//! # `roblox::http` — lớp vận chuyển trung lập
//!
//! Mọi lời gọi mạng của `roblox::*` đi qua trait [`Transport`] chứ không gọi thẳng thư viện HTTP.
//! Lý do không phải là "cho đẹp kiến trúc":
//!
//! - **Test được.** Toàn bộ luồng xác thực (xin CSRF, đổi lấy ticket, phân loại lỗi đăng nhập) là
//!   logic thuần trên các con số và chuỗi — nhưng nếu gọi thẳng mạng thì không kiểm thử được gì
//!   cả, và CI sẽ phụ thuộc vào Roblox đang sống. Với một transport giả, mọi nhánh đều test được.
//! - **Không khoá cứng vào một thư viện.** `reqwest` là lựa chọn ở `ROADMAP.md` mục 3.3, nhưng nó
//!   nặng và thay đổi theo thời gian. Phần logic không nên biết tới nó.
//!
//! Phần cài đặt thật nằm ở module riêng, sau feature flag, để `muster-core` mặc định vẫn nhẹ.

/// Phương thức HTTP mà lớp này cần. Cố tình chỉ có hai giá trị — Roblox API dùng `GET`/`POST` cho
/// các route ở M1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// `GET`
    Get,
    /// `POST`
    Post,
}

/// Một yêu cầu HTTP ở dạng trung lập.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// Phương thức.
    pub method: HttpMethod,
    /// URL đầy đủ.
    pub url: String,
    /// Header, theo thứ tự thêm vào.
    pub headers: Vec<(String, String)>,
    /// Phần thân, `None` nếu không có.
    pub body: Option<Vec<u8>>,
}

impl HttpRequest {
    /// Tạo một yêu cầu không header, không thân.
    #[must_use]
    pub fn new(method: HttpMethod, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    /// Thêm một header, trả về chính nó để nối chuỗi.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Gắn phần thân.
    #[must_use]
    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Giá trị của một header, không phân biệt hoa thường.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        header_value(&self.headers, name)
    }
}

/// Một phản hồi HTTP ở dạng trung lập.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// Mã trạng thái.
    pub status: u16,
    /// Header phản hồi. Các header lặp lại (như `Set-Cookie`) giữ nguyên thành nhiều mục.
    pub headers: Vec<(String, String)>,
    /// Phần thân.
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Tạo phản hồi chỉ có mã trạng thái.
    #[must_use]
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Thêm một header, trả về chính nó để nối chuỗi.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Gắn phần thân.
    #[must_use]
    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    /// Giá trị của một header, không phân biệt hoa thường.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        header_value(&self.headers, name)
    }

    /// Toàn bộ giá trị của một header (dùng cho `Set-Cookie`).
    #[must_use]
    pub fn header_values(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }

    /// Phần thân dạng chuỗi, `None` nếu không phải UTF-8 hợp lệ.
    #[must_use]
    pub fn body_as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.body).ok()
    }
}

/// Lỗi của tầng vận chuyển — mạng hỏng, DNS, TLS, timeout.
///
/// Cố tình chỉ giữ một thông báo: phần logic không cần biết vì sao mạng hỏng, chỉ cần biết là
/// **không gọi được**, khác hẳn với "gọi được nhưng Roblox từ chối".
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("lỗi tầng vận chuyển: {message}")]
pub struct TransportError {
    /// Mô tả lỗi, do lớp cài đặt đặt.
    pub message: String,
}

impl TransportError {
    /// Bọc một thông báo.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Thứ thực hiện được một [`HttpRequest`].
///
/// Bản cài đặt thật dùng `reqwest`; bản dùng cho test trả về kịch bản đã dựng sẵn.
pub trait Transport {
    /// Gửi yêu cầu và nhận phản hồi.
    ///
    /// # Errors
    ///
    /// Trả [`TransportError`] khi **không gọi được** — mạng, DNS, TLS, timeout. Một phản hồi
    /// `4xx`/`5xx` vẫn là `Ok`.
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError>;
}

/// Tìm giá trị của một header, không phân biệt hoa thường.
///
/// HTTP coi tên header không phân biệt hoa thường, nhưng các thư viện giữ nguyên chữ người gửi
/// viết. `x-csrf-token` và `X-CSRF-Token` là **cùng một header**, nên tra thẳng bằng `==` là bug.
#[must_use]
pub fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_can_be_built_up_in_steps() {
        let request = HttpRequest::new(HttpMethod::Post, "https://auth.roblox.com/v2/login")
            .with_header("Referer", "https://www.roblox.com/")
            .with_body("{}");

        assert_eq!(request.method, HttpMethod::Post);
        assert_eq!(request.header("referer"), Some("https://www.roblox.com/"));
        assert_eq!(request.body.as_deref(), Some(b"{}".as_slice()));
    }

    #[test]
    fn header_lookup_ignores_case() {
        let headers = vec![("X-CSRF-TOKEN".to_owned(), "abc".to_owned())];
        assert_eq!(header_value(&headers, "x-csrf-token"), Some("abc"));
        assert_eq!(header_value(&headers, "X-Csrf-Token"), Some("abc"));
        assert_eq!(header_value(&headers, "missing"), None);
    }

    #[test]
    fn repeated_headers_are_all_returned() {
        let response = HttpResponse::new(200)
            .with_header("Set-Cookie", "a=1")
            .with_header("Set-Cookie", "b=2");

        assert_eq!(response.header_values("set-cookie"), ["a=1", "b=2"]);
        assert_eq!(response.header("set-cookie"), Some("a=1"));
    }

    #[test]
    fn a_body_that_is_not_utf8_reads_as_none_instead_of_panicking() {
        let response = HttpResponse::new(200).with_body(vec![0xFF, 0xFE]);
        assert_eq!(response.body_as_str(), None);
    }
}
