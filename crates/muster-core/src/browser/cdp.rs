use serde::Deserialize;

use crate::roblox::auth::SECURITY_COOKIE;

pub const DEVTOOLS_HOST: &str = "127.0.0.1";
pub const DEVTOOLS_VERSION_PATH: &str = "/json/version";
pub const DEVTOOLS_PAGES_PATH: &str = "/json/list";

#[derive(Debug, Clone, Deserialize)]
pub struct CdpCookie {
    pub name: String,
    pub value: String,

    #[serde(default)]
    pub domain: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DevToolsVersion {
    #[serde(rename = "Browser", default)]
    pub browser: String,

    #[serde(rename = "webSocketDebuggerUrl", default)]
    pub web_socket_debugger_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DevToolsPage {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    url: String,
    #[serde(rename = "webSocketDebuggerUrl", default)]
    web_socket_debugger_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CdpEnvelope {
    #[serde(default)]
    id: u64,

    #[serde(default)]
    result: Option<CdpResult>,

    #[serde(default)]
    error: Option<CdpError>,
}

#[derive(Debug, Deserialize)]
struct CdpResult {
    #[serde(default)]
    cookies: Vec<CdpCookie>,
}

#[derive(Debug, Deserialize)]
struct CdpError {
    #[serde(default)]
    message: String,
}

#[must_use]
pub fn enable_network_request(id: u64) -> String {
    format!("{{\"id\":{id},\"method\":\"Network.enable\"}}")
}

#[must_use]
pub fn set_user_agent_override_request(id: u64, user_agent: &str) -> String {
    format!(
        "{{\"id\":{id},\"method\":\"Network.setUserAgentOverride\",\"params\":{{\"userAgent\":{}}}}}",
        serde_json::Value::String(user_agent.to_owned())
    )
}

#[must_use]
pub fn get_all_cookies_request(id: u64) -> String {
    format!("{{\"id\":{id},\"method\":\"Network.getAllCookies\"}}")
}

#[must_use]
pub fn navigate_request(id: u64, url: &str, referrer: Option<&str>) -> String {
    let url = serde_json::Value::String(url.to_owned());

    match referrer {
        Some(referrer) => format!(
            "{{\"id\":{id},\"method\":\"Page.navigate\",\"params\":{{\"url\":{url},\"referrer\":{}}}}}",
            serde_json::Value::String(referrer.to_owned())
        ),
        None => {
            format!("{{\"id\":{id},\"method\":\"Page.navigate\",\"params\":{{\"url\":{url}}}}}")
        }
    }
}

pub fn parse_version(body: &str) -> Option<DevToolsVersion> {
    serde_json::from_str(body).ok()
}

#[must_use]
pub fn page_socket_url(body: &str) -> Option<String> {
    let pages: Vec<DevToolsPage> = serde_json::from_str(body).ok()?;
    pages
        .iter()
        .filter(|page| page.kind == "page")
        .find(|page| page.url.starts_with("https://www.roblox.com/login"))
        .or_else(|| pages.iter().find(|page| page.kind == "page"))?
        .web_socket_debugger_url
        .clone()
}

pub fn parse_cookies(message: &str, expected_id: u64) -> Result<Vec<CdpCookie>, String> {
    let envelope: CdpEnvelope = serde_json::from_str(message)
        .map_err(|error| format!("phản hồi CDP không hợp lệ: {error}"))?;

    if let Some(error) = envelope.error {
        return Err(format!("CDP báo lỗi: {}", error.message));
    }

    if envelope.id != expected_id {
        return Err(format!(
            "CDP trả về id {} trong khi chờ {expected_id}",
            envelope.id
        ));
    }

    Ok(envelope
        .result
        .map_or_else(Vec::new, |result| result.cookies))
}

#[must_use]
pub fn find_security_cookie(cookies: &[CdpCookie]) -> Option<String> {
    cookies
        .iter()
        .filter(|cookie| cookie.name == SECURITY_COOKIE)
        .find(|cookie| {
            cookie
                .domain
                .trim_start_matches('.')
                .eq_ignore_ascii_case("roblox.com")
        })
        .map(|cookie| cookie.value.clone())
        .filter(|value| !value.is_empty())
}

#[must_use]
pub fn ws_path_from_url(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("ws://")
        .or_else(|| url.strip_prefix("wss://"))?;
    let slash = without_scheme.find('/')?;
    Some(without_scheme[slash..].to_owned())
}

#[must_use]
pub fn ws_port_from_url(url: &str) -> Option<u16> {
    let without_scheme = url
        .strip_prefix("ws://")
        .or_else(|| url.strip_prefix("wss://"))?;
    let authority = without_scheme.split('/').next()?;
    let port = authority.rsplit(':').next()?;
    port.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_devtools_requests_are_the_documented_json() {
        assert_eq!(
            enable_network_request(1),
            r#"{"id":1,"method":"Network.enable"}"#
        );
        assert_eq!(
            set_user_agent_override_request(2, "UA"),
            r#"{"id":2,"method":"Network.setUserAgentOverride","params":{"userAgent":"UA"}}"#
        );
        assert_eq!(
            get_all_cookies_request(3),
            r#"{"id":3,"method":"Network.getAllCookies"}"#
        );
    }

    #[test]
    fn navigation_carries_the_referrer_only_when_one_is_given() {
        assert_eq!(
            navigate_request(4, "https://www.roblox.com/login", None),
            r#"{"id":4,"method":"Page.navigate","params":{"url":"https://www.roblox.com/login"}}"#
        );
        assert_eq!(
            navigate_request(
                5,
                "https://www.roblox.com/login",
                Some("https://google.com/")
            ),
            r#"{"id":5,"method":"Page.navigate","params":{"url":"https://www.roblox.com/login","referrer":"https://google.com/"}}"#
        );
    }

    #[test]
    fn page_target_selection_prefers_the_login_tab_over_other_targets() {
        let body = r#"[
            {"type":"browser","url":"","webSocketDebuggerUrl":"ws://127.0.0.1:9222/devtools/browser/x"},
            {"type":"page","url":"about:blank","webSocketDebuggerUrl":"ws://127.0.0.1:9222/devtools/page/blank"},
            {"type":"page","url":"https://www.roblox.com/login","webSocketDebuggerUrl":"ws://127.0.0.1:9222/devtools/page/login"}
        ]"#;
        assert_eq!(
            page_socket_url(body).as_deref(),
            Some("ws://127.0.0.1:9222/devtools/page/login")
        );
        assert_eq!(page_socket_url("[]"), None);
    }

    #[test]
    fn the_version_document_yields_the_websocket_url() {
        let body = r#"{
            "Browser": "Chrome/131.0.6778.85",
            "Protocol-Version": "1.3",
            "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/browser/abc-123"
        }"#;

        let version = parse_version(body).expect("doc duoc");
        assert_eq!(version.browser, "Chrome/131.0.6778.85");
        assert_eq!(
            version.web_socket_debugger_url.as_deref(),
            Some("ws://127.0.0.1:9222/devtools/browser/abc-123")
        );
    }

    #[test]
    fn a_version_document_without_a_socket_url_still_parses() {
        let version = parse_version(r#"{"Browser":"Edge/131"}"#).expect("doc duoc");
        assert!(version.web_socket_debugger_url.is_none());
    }

    #[test]
    fn the_socket_url_is_split_into_port_and_path() {
        let url = "ws://127.0.0.1:9222/devtools/browser/abc-123";
        assert_eq!(ws_port_from_url(url), Some(9222));
        assert_eq!(
            ws_path_from_url(url).as_deref(),
            Some("/devtools/browser/abc-123")
        );

        assert_eq!(ws_port_from_url("khong-phai-url"), None);
        assert_eq!(ws_path_from_url("ws://127.0.0.1:9222"), None);
    }

    #[test]
    fn cookies_are_read_from_the_result_envelope() {
        let message = r#"{"id":7,"result":{"cookies":[
            {"name":"a","value":"1","domain":".roblox.com"},
            {"name":".ROBLOSECURITY","value":"TOKEN","domain":".roblox.com"}
        ]}}"#;

        let cookies = parse_cookies(message, 7).expect("doc duoc");
        assert_eq!(cookies.len(), 2);
        assert_eq!(find_security_cookie(&cookies).as_deref(), Some("TOKEN"));
    }

    #[test]
    fn a_cookie_from_another_domain_is_not_mistaken_for_the_security_cookie() {
        let cookies = vec![CdpCookie {
            name: ".ROBLOSECURITY".to_owned(),
            value: "WRONG".to_owned(),
            domain: ".evil.example".to_owned(),
        }];

        assert_eq!(find_security_cookie(&cookies), None);
    }

    #[test]
    fn a_lookalike_domain_does_not_leak_its_cookie() {
        let cookies = vec![CdpCookie {
            name: ".ROBLOSECURITY".to_owned(),
            value: "WRONG".to_owned(),
            domain: "notroblox.com".to_owned(),
        }];
        assert_eq!(find_security_cookie(&cookies), None);
    }

    #[test]
    fn an_empty_cookie_value_is_not_returned() {
        let cookies = vec![CdpCookie {
            name: ".ROBLOSECURITY".to_owned(),
            value: String::new(),
            domain: ".roblox.com".to_owned(),
        }];

        assert_eq!(find_security_cookie(&cookies), None);
    }

    #[test]
    fn a_cdp_error_envelope_is_reported_not_swallowed() {
        let message = r#"{"id":7,"error":{"code":-32601,"message":"wasn't found"}}"#;
        let error = parse_cookies(message, 7).expect_err("phai loi");
        assert!(error.contains("wasn't found"), "gap: {error}");
    }

    #[test]
    fn a_reply_for_another_request_is_rejected() {
        let message = r#"{"id":9,"result":{"cookies":[]}}"#;
        let error = parse_cookies(message, 7).expect_err("phai loi");
        assert!(error.contains("id 9"), "gap: {error}");
    }
}
