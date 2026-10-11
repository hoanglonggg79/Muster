use std::time::Duration;

use super::http::{HttpMethod, HttpRequest, HttpResponse, Transport, TransportError};

pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub const USER_AGENT: &str = concat!("Muster/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new() -> Result<Self, TransportError> {
        Self::with_timeout(REQUEST_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, TransportError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(timeout)
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

        let status = response.status().as_u16();
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
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
