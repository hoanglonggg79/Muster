use serde::{Deserialize, Serialize};

use super::api::ApiError;
use super::http::{HttpMethod, HttpRequest, Transport};

pub const PRESENCE_URL: &str = "https://presence.roblox.com/v1/presence/users";

pub const PRESENCE_BATCH_SIZE: usize = 100;

#[derive(Debug, thiserror::Error)]
#[error("giá trị userPresenceType không hợp lệ: {0}")]
pub struct UnknownPresenceType(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "u8")]
#[serde(try_from = "u8")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum PresenceType {
    Offline,
    Online,
    InGame,
    InStudio,
}

impl From<PresenceType> for u8 {
    fn from(value: PresenceType) -> Self {
        match value {
            PresenceType::Offline => 0,
            PresenceType::Online => 1,
            PresenceType::InGame => 2,
            PresenceType::InStudio => 3,
        }
    }
}

impl TryFrom<u8> for PresenceType {
    type Error = UnknownPresenceType;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Offline),
            1 => Ok(Self::Online),
            2 => Ok(Self::InGame),
            3 => Ok(Self::InStudio),
            other => Err(UnknownPresenceType(other)),
        }
    }
}

impl PresenceType {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Offline => "Offline",
            Self::Online => "Online",
            Self::InGame => "InGame",
            Self::InStudio => "InStudio",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UserPresence {
    pub user_id: i64,

    pub user_presence_type: PresenceType,

    #[serde(default)]
    pub last_location: Option<String>,

    #[serde(default)]
    pub place_id: Option<i64>,

    #[serde(default)]
    pub root_place_id: Option<i64>,

    #[serde(default)]
    pub game_id: Option<String>,

    #[serde(default)]
    pub universe_id: Option<i64>,

    #[serde(default)]
    pub last_online: Option<String>,
}

#[must_use]
pub fn presence_request_body(user_ids: &[i64]) -> Vec<u8> {
    serde_json::json!({ "userIds": user_ids })
        .to_string()
        .into_bytes()
}

pub fn parse_presences(body: &[u8]) -> Result<Vec<UserPresence>, ApiError> {
    #[derive(Deserialize)]
    struct Payload {
        #[serde(rename = "userPresences", default)]
        user_presences: Vec<UserPresence>,
    }

    let payload: Payload = serde_json::from_slice(body)?;
    Ok(payload.user_presences)
}

pub fn fetch_presences<T: Transport>(
    transport: &T,
    user_ids: &[i64],
) -> Result<Vec<UserPresence>, ApiError> {
    let mut collected = Vec::new();

    for chunk in user_ids.chunks(PRESENCE_BATCH_SIZE) {
        let request = HttpRequest::new(HttpMethod::Post, PRESENCE_URL)
            .with_header("Content-Type", "application/json")
            .with_body(presence_request_body(chunk));

        let response = transport.execute(&request)?;
        if !(200..300).contains(&response.status) {
            return Err(ApiError::RequestFailed {
                status: response.status,
                message: response.body_as_str().unwrap_or("").to_owned(),
            });
        }

        collected.extend(parse_presences(&response.body)?);
    }

    Ok(collected)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::super::http::{HttpResponse, TransportError};
    use super::*;

    struct MockTransport {
        responses: RefCell<VecDeque<Result<HttpResponse, TransportError>>>,
    }

    impl MockTransport {
        fn single(status: u16, body: &str) -> Self {
            Self {
                responses: RefCell::new(
                    vec![Ok(
                        HttpResponse::new(status).with_body(body.as_bytes().to_vec())
                    )]
                    .into(),
                ),
            }
        }
    }

    impl Transport for MockTransport {
        fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            self.responses
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Err(TransportError::new("hết kịch bản")))
        }
    }

    #[test]
    fn the_request_body_lists_every_user_id() {
        let body = presence_request_body(&[1, 2, 3]);
        let value: serde_json::Value = serde_json::from_slice(&body).expect("JSON hợp lệ");
        assert_eq!(value["userIds"], serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn presences_parse_with_numeric_types() {
        let body = br#"{"userPresences":[{"userId":42,"userPresenceType":2,"lastLocation":"Blox Fruits","placeId":4483381587,"universeId":13058,"lastOnline":"2026-10-12T07:00:00Z"},{"userId":43,"userPresenceType":0}]}"#;

        let presences = parse_presences(body).expect("phân tích");

        assert_eq!(presences.len(), 2);
        assert_eq!(presences[0].user_id, 42);
        assert_eq!(presences[0].user_presence_type, PresenceType::InGame);
        assert_eq!(presences[0].place_id, Some(4483381587));
        assert_eq!(presences[1].user_presence_type, PresenceType::Offline);
        assert_eq!(presences[1].last_location, None);
    }

    #[test]
    fn an_empty_list_means_no_requests_at_all() {
        let transport = MockTransport::single(200, r#"{"userPresences":[]}"#);
        let presences = fetch_presences(&transport, &[]).expect("không lỗi");
        assert!(presences.is_empty());
    }

    #[test]
    fn a_broken_body_is_reported_as_a_json_error() {
        let transport = MockTransport::single(200, "khong phai json");
        let error = fetch_presences(&transport, &[1]).expect_err("phải lỗi");
        assert!(matches!(error, ApiError::Json(_)), "gặp: {error:?}");
    }

    #[test]
    fn an_http_error_becomes_a_request_failure() {
        let transport = MockTransport::single(500, " Server ");
        let error = fetch_presences(&transport, &[1]).expect_err("phải lỗi");
        assert!(
            matches!(error, ApiError::RequestFailed { status: 500, .. }),
            "gặp: {error:?}"
        );
    }

    #[test]
    fn presence_type_labels_are_stable() {
        assert_eq!(PresenceType::InGame.label(), "InGame");
        assert_eq!(PresenceType::InStudio.label(), "InStudio");
    }
}
