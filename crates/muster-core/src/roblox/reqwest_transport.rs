//! # `roblox::reqwest_transport` — bản cài đặt thật của trait `Transport`
//!
//! Bọc `reqwest` (bản `blocking`) thành [`Transport`]. Cố tình **mỏng**: mọi quyết định nghiệp vụ
//! nằm ở `roblox::auth` và đã được kiểm thử bằng transport giả; ở đây chỉ có chuyển đổi qua lại
//! giữa kiểu của ta và kiểu của `reqwest`, cộng thêm cấu hình client.
//!
//! ## Vì sao `blocking`
//!
//! `Transport::execute` là hàm đồng bộ. Đổi nó thành `async` sẽ buộc mọi chỗ gọi phải có runtime,
//! trong khi phần lớn logic không cần bất đồng bộ. Chỗ nào cần (ví dụ command Tauri) thì tự bọc
//! lời gọi vào một luồng nền — quyết định đó thuộc về chỗ gọi, không thuộc về lớp này.
//!
//! ## Vì sao nằm sau feature flag
//!
//! `reqwest` kéo theo cả cây TLS và một runtime. `muster-core` mặc định phải nhẹ và
//! `cargo test -p muster-core` phải chạy nhanh, nên adapter chỉ được biên dịch khi feature
//! `http-reqwest` được bật — `muster-cli` bật nó.

use std::time::Duration;

use super::http::{HttpMethod, HttpRequest, HttpResponse, Transport, TransportError};

/// Thời gian chờ tối đa cho một lời gọi.
///
/// Roblox API bình thường trả trong vài trăm mili giây. Để dài hơn nữa thì một lần treo mạng sẽ
/// giữ luồng quá lâu mà không thêm khả năng thành công nào.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// User-Agent gửi kèm. Không chứa chữ "Roblox" — quy tắc đặt tên ở `ROADMAP.md` mục 1.
pub const USER_AGENT: &str = concat!("Muster/", env!("CARGO_PKG_VERSION"));

/// [`Transport`] chạy trên `reqwest`.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    /// Dựng transport với cấu hình mặc định.
    ///
    /// # Errors
    ///
    /// Trả [`TransportError`] nếu `reqwest` không dựng được client (hiếm — thường là do TLS
    /// backend không khởi tạo được).
    pub fn new() -> Result<Self, TransportError> {
        Self::with_timeout(REQUEST_TIMEOUT)
    }

    /// Dựng transport với thời gian chờ tuỳ chỉnh.
    ///
    /// # Errors
    ///
    /// Như [`ReqwestTransport::new`].
    pub fn with_timeout(timeout: Duration) -> Result<Self, TransportError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(timeout)
            // Không theo redirect: các endpoint xác thực trả dữ liệu cần đọc nằm ở chính phản hồi
            // đầu tiên (header CSRF, header ticket, Set-Cookie). Đi theo redirect có thể làm mất
            // chúng và biến một `403` có ý nghĩa thành một `200` vô nghĩa.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| {
                TransportError::new(format!("không dựng được HTTP client: {error}"))
            })?;

        Ok(Self { client })
    }
}

impl Transport for ReqwestTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let method = match request.method {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Post => reqwest::Method::POST,
        };

        let mut builder = self.client.request(method, &request.url);

        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }

        let response = builder.send().map_err(|error| {
            TransportError::new(format!("gọi {} thất bại: {error}", request.url))
        })?;

        // Đọc trạng thái và header TRƯỚC khi lấy phần thân, vì lấy thân sẽ tiêu thụ phản hồi.
        let status = response.status().as_u16();
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    // Header không phải UTF-8 là chuyện hiếm; thay bằng chuỗi rỗng còn hơn làm hỏng
                    // cả lời gọi vì một header ta không dùng tới.
                    value.to_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();

        let body = response
            .bytes()
            .map_err(|error| TransportError::new(format!("đọc thân phản hồi thất bại: {error}")))?
            .to_vec();

        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_client_can_be_built() {
        ReqwestTransport::new().expect("dựng được transport");
    }

    #[test]
    fn a_custom_timeout_is_accepted() {
        ReqwestTransport::with_timeout(Duration::from_millis(500)).expect("dựng được transport");
    }

    #[test]
    fn the_user_agent_does_not_contain_the_roblox_brand() {
        assert!(
            !USER_AGENT.to_ascii_lowercase().contains("roblox"),
            "ten san pham khong duoc chua 'Roblox': {USER_AGENT}"
        );
    }
}
