use serde::{Deserialize, Serialize};

use super::games::{GameDetails, multiget_place_details_url};
use super::http::{HttpMethod, HttpRequest, Transport, TransportError};

pub const BATCH_WINDOW_MS: u64 = 50;
pub const THUMBNAIL_BATCH_SIZE: usize = 100;
pub const PLACE_BATCH_SIZE: usize = 50;

pub const THUMBNAILS_HOST: &str = "https://thumbnails.roblox.com";

pub const UNKNOWN_IMAGE: &str = "UNK";

#[derive(Debug, thiserror::Error)]
pub enum BatchError {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error("yêu cầu batch thất bại: HTTP {status}")]
    Failed { status: u16 },

    #[error("phản hồi batch không đúng cấu trúc: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailRequest {
    pub asset_id: i64,
    pub kind: String,
    pub size: String,
    pub format: Option<String>,
}

impl ThumbnailRequest {
    #[must_use]
    pub fn new(asset_id: i64, kind: impl Into<String>) -> Self {
        Self {
            asset_id,
            kind: kind.into(),
            size: "150x150".to_owned(),
            format: None,
        }
    }

    #[must_use]
    pub fn with_size(mut self, size: impl Into<String>) -> Self {
        self.size = size.into();
        self
    }

    #[must_use]
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }

    #[must_use]
    pub fn request_id(&self) -> String {
        format!(
            "{}:undefined:{}:{}:{}:regular",
            self.asset_id,
            self.kind,
            self.size,
            self.format.as_deref().unwrap_or("")
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchRequestItem {
    #[serde(rename = "requestId")]
    request_id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "targetId")]
    target_id: i64,
    size: String,
    format: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BatchResponseItem {
    #[serde(rename = "targetId", default)]
    target_id: i64,
    #[serde(rename = "imageUrl", default)]
    image_url: Option<String>,
    #[serde(rename = "errorCode", default)]
    error_code: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct BatchResponse {
    #[serde(default)]
    data: Vec<BatchResponseItem>,
}

#[must_use]
pub fn thumbnail_batch_url() -> String {
    format!("{THUMBNAILS_HOST}/v1/batch")
}

#[must_use]
pub fn chunks<T>(items: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 {
        return Vec::new();
    }
    items.chunks(size).collect()
}

#[must_use]
pub fn thumbnail_batch_body(requests: &[ThumbnailRequest]) -> Vec<u8> {
    let items: Vec<BatchRequestItem> = requests
        .iter()
        .map(|request| BatchRequestItem {
            request_id: request.request_id(),
            kind: request.kind.clone(),
            target_id: request.asset_id,
            size: request.size.clone(),
            format: request.format.clone(),
        })
        .collect();

    serde_json::to_vec(&items).unwrap_or_else(|_| b"[]".to_vec())
}

#[must_use]
pub fn parse_thumbnail_response(body: &str, asset_id: i64) -> Option<String> {
    let parsed: BatchResponse = serde_json::from_str(body).ok()?;
    parsed
        .data
        .into_iter()
        .find(|item| item.error_code == 0 && item.target_id == asset_id)
        .and_then(|item| item.image_url)
}

#[must_use]
pub fn parse_place_details(body: &str) -> Vec<GameDetails> {
    serde_json::from_str(body).unwrap_or_default()
}

pub fn fetch_thumbnails<T: Transport>(
    transport: &T,
    requests: &[ThumbnailRequest],
) -> Result<Vec<(i64, String)>, BatchError> {
    let mut out = Vec::new();

    for chunk in chunks(requests, THUMBNAIL_BATCH_SIZE) {
        let request = HttpRequest::new(HttpMethod::Post, thumbnail_batch_url())
            .with_header("Content-Type", "application/json")
            .with_body(thumbnail_batch_body(chunk));

        let response = transport.execute(&request)?;
        if !(200..300).contains(&response.status) {
            return Err(BatchError::Failed {
                status: response.status,
            });
        }

        let body = response.body_as_str().unwrap_or("");
        for item in chunk {
            out.push((
                item.asset_id,
                parse_thumbnail_response(body, item.asset_id)
                    .unwrap_or_else(|| UNKNOWN_IMAGE.to_owned()),
            ));
        }
    }

    Ok(out)
}

pub fn fetch_place_details<T: Transport>(
    transport: &T,
    cookie: &str,
    place_ids: &[i64],
) -> Result<Vec<GameDetails>, BatchError> {
    let mut out = Vec::new();

    for chunk in chunks(place_ids, PLACE_BATCH_SIZE) {
        let request = HttpRequest::new(HttpMethod::Get, multiget_place_details_url(chunk))
            .with_header("Cookie", format!(".ROBLOSECURITY={cookie}"));

        let response = transport.execute(&request)?;
        if !(200..300).contains(&response.status) {
            return Err(BatchError::Failed {
                status: response.status,
            });
        }

        out.extend(parse_place_details(response.body_as_str().unwrap_or("")));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::http::HttpResponse;
    use super::*;

    struct Mock {
        response: HttpResponse,
        seen: std::cell::RefCell<Vec<HttpRequest>>,
    }

    impl Mock {
        fn new(response: HttpResponse) -> Self {
            Self {
                response,
                seen: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl Transport for Mock {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            self.seen.borrow_mut().push(request.clone());
            Ok(self.response.clone())
        }
    }

    #[test]
    fn the_request_id_matches_the_ram_format() {
        let request = ThumbnailRequest::new(123, "Avatar");
        assert_eq!(
            request.request_id(),
            "123:undefined:Avatar:150x150::regular"
        );

        let request = ThumbnailRequest::new(9, "GameIcon")
            .with_size("512x512")
            .with_format("png");
        assert_eq!(
            request.request_id(),
            "9:undefined:GameIcon:512x512:png:regular"
        );
    }

    #[test]
    fn the_batch_body_uses_the_keys_roblox_expects() {
        let body = thumbnail_batch_body(&[ThumbnailRequest::new(42, "Avatar")]);
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");

        assert_eq!(
            value[0]["requestId"],
            "42:undefined:Avatar:150x150::regular"
        );
        assert_eq!(value[0]["type"], "Avatar");
        assert_eq!(value[0]["targetId"], 42);
        assert_eq!(value[0]["size"], "150x150");
        assert!(value[0]["format"].is_null());
    }

    #[test]
    fn a_thumbnail_is_found_by_target_id_and_only_when_error_code_is_zero() {
        let body = r#"{"data":[
            {"targetId":1,"imageUrl":"https://x/1.png","errorCode":0},
            {"targetId":2,"imageUrl":"https://x/2.png","errorCode":3}
        ]}"#;

        assert_eq!(
            parse_thumbnail_response(body, 1),
            Some("https://x/1.png".to_owned())
        );
        assert_eq!(parse_thumbnail_response(body, 2), None);
        assert_eq!(parse_thumbnail_response(body, 3), None);
    }

    #[test]
    fn a_broken_thumbnail_body_reads_as_none_instead_of_panicking() {
        assert_eq!(parse_thumbnail_response("<html>", 1), None);
    }

    #[test]
    fn the_place_details_url_repeats_the_place_ids_key() {
        assert_eq!(
            multiget_place_details_url(&[1, 2, 3]),
            "https://games.roblox.com/v1/games/multiget-place-details?placeIds=1&placeIds=2&placeIds=3"
        );
    }

    #[test]
    fn requests_are_split_into_batches_of_one_hundred_and_fifty() {
        let thumbs: Vec<u32> = (0..250).collect();
        let sizes: Vec<usize> = chunks(&thumbs, THUMBNAIL_BATCH_SIZE)
            .iter()
            .map(|chunk| chunk.len())
            .collect();
        assert_eq!(sizes, [100, 100, 50]);

        let places: Vec<u32> = (0..120).collect();
        let sizes: Vec<usize> = chunks(&places, PLACE_BATCH_SIZE)
            .iter()
            .map(|chunk| chunk.len())
            .collect();
        assert_eq!(sizes, [50, 50, 20]);

        assert!(chunks(&places, 0).is_empty());
    }

    #[test]
    fn fetching_thumbnails_sends_one_request_per_batch() {
        let body = r#"{"data":[{"targetId":7,"imageUrl":"https://x/7.png","errorCode":0}]}"#;
        let mock = Mock::new(HttpResponse::new(200).with_body(body.as_bytes().to_vec()));
        let requests: Vec<ThumbnailRequest> = (0..150)
            .map(|id| ThumbnailRequest::new(id, "Avatar"))
            .collect();

        let out = fetch_thumbnails(&mock, &requests).expect("tai thumbnail");

        assert_eq!(out.len(), 150);
        assert_eq!(
            mock.seen.borrow().len(),
            2,
            "150 yeu cau chia thanh 2 batch"
        );
        assert_eq!(
            mock.seen.borrow()[0].url,
            "https://thumbnails.roblox.com/v1/batch"
        );
        assert_eq!(mock.seen.borrow()[0].method, HttpMethod::Post);
    }

    #[test]
    fn a_missing_thumbnail_falls_back_to_unk() {
        let mock = Mock::new(HttpResponse::new(200).with_body(b"{\"data\":[]}".to_vec()));
        let out = fetch_thumbnails(&mock, &[ThumbnailRequest::new(5, "Avatar")]).expect("tai");

        assert_eq!(out, [(5, UNKNOWN_IMAGE.to_owned())]);
    }

    #[test]
    fn fetching_place_details_sends_the_cookie_and_parses_the_array() {
        let body = r#"[{"placeId":1,"universeId":99,"name":"Cool Game","isPlayable":true}]"#;
        let mock = Mock::new(HttpResponse::new(200).with_body(body.as_bytes().to_vec()));

        let out = fetch_place_details(&mock, "COOKIE", &[1]).expect("tai chi tiet");

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "Cool Game");
        assert_eq!(out[0].universe_id, 99);
        assert!(out[0].is_playable);

        let seen = mock.seen.borrow();
        assert_eq!(seen[0].header("cookie"), Some(".ROBLOSECURITY=COOKIE"));
    }

    #[test]
    fn a_failed_batch_is_reported_with_its_status() {
        let mock = Mock::new(HttpResponse::new(429));
        let error =
            fetch_thumbnails(&mock, &[ThumbnailRequest::new(1, "Avatar")]).expect_err("phai loi");

        assert!(matches!(error, BatchError::Failed { status: 429 }));
    }
}
