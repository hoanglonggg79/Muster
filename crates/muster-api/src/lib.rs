use axum::Router;
use axum::routing::get;

pub const DEFAULT_PORT: u16 = 7963;

pub fn router() -> Router {
    Router::new()
        .route("/Running", get(running))
        .route("/favicon.ico", get(favicon))
}

async fn running() -> &'static str {
    "true"
}

async fn favicon() -> &'static str {
    ""
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn body_of(uri: &str) -> (StatusCode, Vec<u8>) {
        let res = router()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("router khong duoc loi");
        let status = res.status();
        let bytes = res
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes()
            .to_vec();
        (status, bytes)
    }

    #[tokio::test]
    async fn running_returns_plain_true_not_json() {
        let (status, body) = body_of("/Running").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"true", "RAM tra text tho 'true', khong phai JSON");
    }

    #[tokio::test]
    async fn favicon_is_empty() {
        let (status, body) = body_of("/favicon.ico").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_empty());
    }

    #[test]
    fn default_port_matches_ram() {
        assert_eq!(DEFAULT_PORT, 7963);
    }
}
