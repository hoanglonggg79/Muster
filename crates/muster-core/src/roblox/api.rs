use serde::{Deserialize, Serialize};

use super::auth::SECURITY_COOKIE;
use super::http::{HttpMethod, HttpRequest, Transport, TransportError};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error("cookie không hợp lệ hoặc đã hết hạn (HTTP 401)")]
    Unauthorized,

    #[error("gọi API Roblox thất bại: HTTP {status} — {message}")]
    RequestFailed { status: u16, message: String },

    #[error("phản hồi JSON không đúng cấu trúc: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AuthenticatedUser {
    pub id: i64,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
struct CurrencyResponse {
    pub robux: u64,
}

#[derive(Debug, Deserialize)]
struct ThumbnailItem {
    #[serde(rename = "targetId")]
    pub target_id: i64,
    #[serde(rename = "imageUrl")]
    pub image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ThumbnailsResponse {
    pub data: Vec<ThumbnailItem>,
}

pub fn fetch_authenticated_user<T: Transport>(
    transport: &T,
    cookie: &str,
) -> Result<AuthenticatedUser, ApiError> {
    let url = "https://users.roblox.com/v1/users/authenticated";
    let request = HttpRequest::new(HttpMethod::Get, url)
        .with_header("Cookie", format!("{SECURITY_COOKIE}={cookie}"));

    let response = transport.execute(&request)?;
    if response.status == 401 {
        return Err(ApiError::Unauthorized);
    }
    if !(200..300).contains(&response.status) {
        return Err(ApiError::RequestFailed {
            status: response.status,
            message: response.body_as_str().unwrap_or("").to_owned(),
        });
    }

    let user: AuthenticatedUser = serde_json::from_slice(&response.body)?;
    Ok(user)
}

pub fn fetch_user_robux<T: Transport>(
    transport: &T,
    user_id: i64,
    cookie: &str,
) -> Result<u64, ApiError> {
    let url = format!("https://economy.roblox.com/v1/users/{user_id}/currency");
    let request = HttpRequest::new(HttpMethod::Get, url)
        .with_header("Cookie", format!("{SECURITY_COOKIE}={cookie}"));

    let response = transport.execute(&request)?;
    if response.status == 401 {
        return Err(ApiError::Unauthorized);
    }
    if !(200..300).contains(&response.status) {
        return Err(ApiError::RequestFailed {
            status: response.status,
            message: response.body_as_str().unwrap_or("").to_owned(),
        });
    }

    let currency: CurrencyResponse = serde_json::from_slice(&response.body)?;
    Ok(currency.robux)
}

pub fn fetch_user_avatar_headshot<T: Transport>(
    transport: &T,
    user_id: i64,
) -> Result<Option<String>, ApiError> {
    let url = format!(
        "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={user_id}&size=150x150&format=Png&isCircular=true"
    );
    let request = HttpRequest::new(HttpMethod::Get, url);

    let response = transport.execute(&request)?;
    if !(200..300).contains(&response.status) {
        return Err(ApiError::RequestFailed {
            status: response.status,
            message: response.body_as_str().unwrap_or("").to_owned(),
        });
    }

    let payload: ThumbnailsResponse = serde_json::from_slice(&response.body)?;
    let url = payload
        .data
        .into_iter()
        .find(|item| item.target_id == user_id)
        .and_then(|item| item.image_url);

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::super::http::HttpResponse;
    use super::*;

    struct MockTransport {
        response: HttpResponse,
    }

    impl Transport for MockTransport {
        fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn authenticated_user_parses_json() {
        let body = r#"{"id":123456,"name":"RobloxFan","displayName":"ProGamer"}"#;
        let transport = MockTransport {
            response: HttpResponse {
                status: 200,
                headers: vec![],
                body: body.as_bytes().to_vec(),
            },
        };

        let user = fetch_authenticated_user(&transport, "cookie").unwrap();
        assert_eq!(user.id, 123456);
        assert_eq!(user.name, "RobloxFan");
        assert_eq!(user.display_name, "ProGamer");
    }

    #[test]
    fn unauthenticated_gives_unauthorized_error() {
        let transport = MockTransport {
            response: HttpResponse {
                status: 401,
                headers: vec![],
                body: vec![],
            },
        };

        let err = fetch_authenticated_user(&transport, "bad_cookie").unwrap_err();
        assert!(matches!(err, ApiError::Unauthorized));
    }

    #[test]
    fn robux_reads_currency() {
        let body = r#"{"robux":9999}"#;
        let transport = MockTransport {
            response: HttpResponse {
                status: 200,
                headers: vec![],
                body: body.as_bytes().to_vec(),
            },
        };

        let robux = fetch_user_robux(&transport, 123, "cookie").unwrap();
        assert_eq!(robux, 9999);
    }
}
