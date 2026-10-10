//! # Muster local API
//!
//! Tái lập local HTTP API của Roblox Account Manager: **22 route** cộng `/Running` và
//! `/favicon.ico`. Hợp đồng đầy đủ ở `ROADMAP.md` mục 4.2.
//!
//! Trạng thái: mới có khung. Hai route phụ đã xong, 22 route chính thuộc M2.
//!
//! ## Quy ước phải giữ
//!
//! - Cổng mặc định **7963** (setting `WebServerPort`).
//! - Mặc định chỉ nghe `localhost`. Chỉ bind rộng khi người dùng bật `AllowExternalConnections`
//!   (nguyên tắc bất biến #8).
//! - Nhánh `/v2/<Method>` trả JSON `{"Success": bool, "Message": string}`.
//! - Nhánh `/v1/<Method>` trả text thô, mã lỗi nằm ở header `ws-error`.
//! - HTTP status: thành công 200, lỗi 400, sai quyền 401.

use axum::Router;
use axum::routing::get;

/// Cổng mặc định, khớp setting `WebServerPort` của RAM.
pub const DEFAULT_PORT: u16 = 7963;

/// Dựng router của local API.
///
/// Chưa có 22 route chính — chúng cần `core::roblox::account` và lớp xác thực của M1/M2.
pub fn router() -> Router {
    Router::new()
        .route("/Running", get(running))
        .route("/favicon.ico", get(favicon))
}

/// `GET /Running` — RAM trả về **text thô** `true`, không phải JSON.
async fn running() -> &'static str {
    "true"
}

/// `GET /favicon.ico` — RAM trả về rỗng.
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
