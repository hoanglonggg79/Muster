#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl HttpRequest {
    #[must_use]
    pub fn new(method: HttpMethod, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    #[must_use]
    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }

    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        header_value(&self.headers, name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    #[must_use]
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    #[must_use]
    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        header_value(&self.headers, name)
    }

    #[must_use]
    pub fn header_values(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }

    #[must_use]
    pub fn body_as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.body).ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("lỗi tầng vận chuyển: {message}")]
pub struct TransportError {
    pub message: String,
}

impl TransportError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait Transport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError>;
}

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
