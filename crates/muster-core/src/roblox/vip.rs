use std::sync::OnceLock;

use regex::Regex;

use super::api::ApiError;
use super::auth::{BROWSER_REFERER, SECURITY_COOKIE};
use super::http::{HttpMethod, HttpRequest, Transport};

pub const PRIVATE_SERVER_LINK_PREFIX: &str = "privateServerLinkCode=";

pub const VIP_JOB_PREFIX: &str = "VIP:";

fn access_code_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"Roblox\.GameLauncher\.joinPrivateGame\(\d+,\s*'(\w+-\w+-\w+-\w+-\w+)'")
            .expect("regex accessCode hop le")
    })
}

#[must_use]
pub fn strip_vip_prefix(job_id: &str) -> &str {
    job_id.strip_prefix(VIP_JOB_PREFIX).unwrap_or(job_id)
}

#[must_use]
pub fn extract_link_code(job_id: &str) -> Option<&str> {
    let position = job_id.find(PRIVATE_SERVER_LINK_PREFIX)?;
    let rest = &job_id[position + PRIVATE_SERVER_LINK_PREFIX.len()..];
    let code = rest.split(['&', ' ', '"', '\'']).next().unwrap_or("");

    (!code.is_empty()).then_some(code)
}

#[must_use]
pub fn extract_access_code(page: &str) -> Option<String> {
    access_code_regex()
        .captures(page)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_owned()))
}

pub fn fetch_access_code<T: Transport>(
    transport: &T,
    place_id: i64,
    link_code: &str,
    csrf: &str,
    cookie: &str,
) -> Result<String, ApiError> {
    let url =
        format!("https://www.roblox.com/games/{place_id}?{PRIVATE_SERVER_LINK_PREFIX}{link_code}");

    let request = HttpRequest::new(HttpMethod::Get, url)
        .with_header("X-CSRF-TOKEN", csrf)
        .with_header("Referer", BROWSER_REFERER)
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

    extract_access_code(response.body_as_str().unwrap_or("")).ok_or(ApiError::RequestFailed {
        status: response.status,
        message: "không rút được accessCode từ trang game".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::http::HttpResponse;
    use super::*;

    struct MockTransport {
        response: HttpResponse,
    }

    impl Transport for MockTransport {
        fn execute(
            &self,
            _request: &HttpRequest,
        ) -> Result<HttpResponse, super::super::http::TransportError> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn the_vip_prefix_is_stripped_before_looking_for_a_link_code() {
        assert_eq!(
            strip_vip_prefix("VIP:https://x.test/games/1"),
            "https://x.test/games/1"
        );
        assert_eq!(strip_vip_prefix("12345"), "12345");
        assert_eq!(strip_vip_prefix("VIP:12345"), "12345");
    }

    #[test]
    fn a_link_code_is_found_inside_a_pasted_url() {
        assert_eq!(
            extract_link_code(
                "https://www.roblox.com/games/4483381587?privateServerLinkCode=abcdef123456"
            ),
            Some("abcdef123456")
        );
        assert_eq!(
            extract_link_code(
                "VIP:https://www.roblox.com/games/4483381587?privateServerLinkCode=abcdef123456&other=1"
            ),
            Some("abcdef123456")
        );
        assert_eq!(extract_link_code("privateServerLinkCode="), None);
        assert_eq!(extract_link_code("12345"), None);
    }

    #[test]
    fn the_access_code_is_read_from_the_join_private_game_call() {
        let page = "var x = 1; Roblox.GameLauncher.joinPrivateGame(4483381587, '3f2504e0-4f89-11d3-9a0c-0305e82c3301');";
        assert_eq!(
            extract_access_code(page).as_deref(),
            Some("3f2504e0-4f89-11d3-9a0c-0305e82c3301")
        );
    }

    #[test]
    fn whitespace_between_the_arguments_is_allowed() {
        let page =
            "Roblox.GameLauncher.joinPrivateGame(123,\n    '3f2504e0-4f89-11d3-9a0c-0305e82c3301')";
        assert_eq!(
            extract_access_code(page).as_deref(),
            Some("3f2504e0-4f89-11d3-9a0c-0305e82c3301")
        );
    }

    #[test]
    fn a_page_without_the_call_yields_nothing() {
        assert_eq!(extract_access_code("<html>khong co gi</html>"), None);
        assert_eq!(
            extract_access_code("Roblox.GameLauncher.joinPrivateGame(1, 'too-few-groups')"),
            None
        );
        assert_eq!(
            extract_access_code("Roblox.GameLauncher.joinPrivateGame(1, 'has space not uuid')"),
            None
        );
    }

    #[test]
    fn fetching_uses_the_link_code_and_the_csrf_header() {
        struct Recording {
            response: HttpResponse,
            request: std::cell::RefCell<Option<HttpRequest>>,
        }

        impl Transport for Recording {
            fn execute(
                &self,
                request: &HttpRequest,
            ) -> Result<HttpResponse, super::super::http::TransportError> {
                *self.request.borrow_mut() = Some(request.clone());
                Ok(self.response.clone())
            }
        }

        let transport = Recording {
            response: HttpResponse::new(200).with_body(
                b"Roblox.GameLauncher.joinPrivateGame(1, '3f2504e0-4f89-11d3-9a0c-0305e82c3301')"
                    .to_vec(),
            ),
            request: std::cell::RefCell::new(None),
        };

        let code = fetch_access_code(&transport, 4483381587, "link-abc", "CSRF-1", "COOKIE-1")
            .expect("rút accessCode");

        assert_eq!(code, "3f2504e0-4f89-11d3-9a0c-0305e82c3301");

        let request = transport.request.borrow().clone().expect("có request");
        assert!(
            request
                .url
                .contains("games/4483381587?privateServerLinkCode=link-abc")
        );
        assert_eq!(request.header("x-csrf-token"), Some("CSRF-1"));
        assert_eq!(request.header("cookie"), Some(".ROBLOSECURITY=COOKIE-1"));
    }

    #[test]
    fn a_login_expired_page_is_reported_as_unauthorized() {
        let transport = MockTransport {
            response: HttpResponse::new(401),
        };

        let error =
            fetch_access_code(&transport, 1, "link", "CSRF", "COOKIE").expect_err("phải lỗi");
        assert!(matches!(error, ApiError::Unauthorized));
    }
}
