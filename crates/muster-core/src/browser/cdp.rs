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

#[must_use]
pub fn evaluate_request(id: u64, expression: &str) -> String {
    serde_json::json!({
        "id": id,
        "method": "Runtime.evaluate",
        "params": { "expression": expression, "returnByValue": true }
    })
    .to_string()
}

pub const LOGIN_USERNAME_SELECTOR: &str = "#login-username";
pub const LOGIN_PASSWORD_SELECTOR: &str = "#login-password";
pub const LOGIN_BUTTON_SELECTOR: &str = "#login-button";

pub const SHIFT_MODIFIER: u32 = 8;
pub const LEFT_BUTTON: &str = "left";
pub const NO_BUTTON: &str = "none";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyStroke {
    pub key: String,
    pub code: String,
    pub virtual_key_code: u32,
    pub shift: bool,
}

#[must_use]
pub fn key_stroke(character: char) -> Option<KeyStroke> {
    let (code, virtual_key_code, shift) = match character {
        'a'..='z' => (
            format!("Key{}", character.to_ascii_uppercase()),
            u32::from(character.to_ascii_uppercase()),
            false,
        ),
        'A'..='Z' => (format!("Key{character}"), u32::from(character), true),
        '0'..='9' => (format!("Digit{character}"), u32::from(character), false),
        ' ' => ("Space".to_owned(), 32, false),
        '!' => ("Digit1".to_owned(), 49, true),
        '"' => ("Quote".to_owned(), 222, true),
        '#' => ("Digit3".to_owned(), 51, true),
        '$' => ("Digit4".to_owned(), 52, true),
        '%' => ("Digit5".to_owned(), 53, true),
        '&' => ("Digit7".to_owned(), 55, true),
        '\'' => ("Quote".to_owned(), 222, false),
        '(' => ("Digit9".to_owned(), 57, true),
        ')' => ("Digit0".to_owned(), 48, true),
        '*' => ("Digit8".to_owned(), 56, true),
        '+' => ("Equal".to_owned(), 187, true),
        ',' => ("Comma".to_owned(), 188, false),
        '-' => ("Minus".to_owned(), 189, false),
        '.' => ("Period".to_owned(), 190, false),
        '/' => ("Slash".to_owned(), 191, false),
        ':' => ("Semicolon".to_owned(), 186, true),
        ';' => ("Semicolon".to_owned(), 186, false),
        '<' => ("Comma".to_owned(), 188, true),
        '=' => ("Equal".to_owned(), 187, false),
        '>' => ("Period".to_owned(), 190, true),
        '?' => ("Slash".to_owned(), 191, true),
        '@' => ("Digit2".to_owned(), 50, true),
        '[' => ("BracketLeft".to_owned(), 219, false),
        '\\' => ("Backslash".to_owned(), 220, false),
        ']' => ("BracketRight".to_owned(), 221, false),
        '^' => ("Digit6".to_owned(), 54, true),
        '_' => ("Minus".to_owned(), 189, true),
        '`' => ("Backquote".to_owned(), 192, false),
        '{' => ("BracketLeft".to_owned(), 219, true),
        '|' => ("Backslash".to_owned(), 220, true),
        '}' => ("BracketRight".to_owned(), 221, true),
        '~' => ("Backquote".to_owned(), 192, true),
        _ => return None,
    };

    Some(KeyStroke {
        key: character.to_string(),
        code,
        virtual_key_code,
        shift,
    })
}

#[must_use]
pub fn key_down_request(id: u64, stroke: &KeyStroke) -> String {
    serde_json::json!({
        "id": id,
        "method": "Input.dispatchKeyEvent",
        "params": {
            "type": "keyDown",
            "modifiers": if stroke.shift { SHIFT_MODIFIER } else { 0 },
            "text": stroke.key,
            "unmodifiedText": stroke.key,
            "key": stroke.key,
            "code": stroke.code,
            "windowsVirtualKeyCode": stroke.virtual_key_code,
            "nativeVirtualKeyCode": stroke.virtual_key_code
        }
    })
    .to_string()
}

#[must_use]
pub fn key_up_request(id: u64, stroke: &KeyStroke) -> String {
    serde_json::json!({
        "id": id,
        "method": "Input.dispatchKeyEvent",
        "params": {
            "type": "keyUp",
            "modifiers": if stroke.shift { SHIFT_MODIFIER } else { 0 },
            "key": stroke.key,
            "code": stroke.code,
            "windowsVirtualKeyCode": stroke.virtual_key_code,
            "nativeVirtualKeyCode": stroke.virtual_key_code
        }
    })
    .to_string()
}

#[must_use]
pub fn mouse_move_request(id: u64, x: f64, y: f64) -> String {
    serde_json::json!({
        "id": id,
        "method": "Input.dispatchMouseEvent",
        "params": { "type": "mouseMoved", "x": x, "y": y, "button": NO_BUTTON, "buttons": 0 }
    })
    .to_string()
}

#[must_use]
pub fn mouse_press_request(id: u64, x: f64, y: f64) -> String {
    serde_json::json!({
        "id": id,
        "method": "Input.dispatchMouseEvent",
        "params": {
            "type": "mousePressed", "x": x, "y": y,
            "button": LEFT_BUTTON, "buttons": 1, "clickCount": 1
        }
    })
    .to_string()
}

#[must_use]
pub fn mouse_release_request(id: u64, x: f64, y: f64) -> String {
    serde_json::json!({
        "id": id,
        "method": "Input.dispatchMouseEvent",
        "params": {
            "type": "mouseReleased", "x": x, "y": y,
            "button": LEFT_BUTTON, "buttons": 0, "clickCount": 1
        }
    })
    .to_string()
}

#[must_use]
pub fn reply_string_value(reply: &serde_json::Value) -> Option<String> {
    reply
        .pointer("/result/result/value")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

#[must_use]
pub fn cookies_from_value(reply: &serde_json::Value) -> Vec<CdpCookie> {
    reply
        .pointer("/result/cookies")
        .and_then(|value| serde_json::from_value::<Vec<CdpCookie>>(value.clone()).ok())
        .unwrap_or_default()
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

    #[test]
    fn the_key_stroke_table_matches_the_us_layout() {
        let lower = key_stroke('a').expect("a");
        assert_eq!(lower.key, "a");
        assert_eq!(lower.code, "KeyA");
        assert_eq!(lower.virtual_key_code, 65);
        assert!(!lower.shift);

        let upper = key_stroke('A').expect("A");
        assert_eq!(upper.key, "A");
        assert_eq!(upper.code, "KeyA");
        assert_eq!(upper.virtual_key_code, 65);
        assert!(upper.shift, "chu hoa phai giu Shift");

        let digit = key_stroke('7').expect("7");
        assert_eq!(digit.code, "Digit7");
        assert_eq!(digit.virtual_key_code, 55);
        assert!(!digit.shift);

        let at = key_stroke('@').expect("@");
        assert_eq!(at.code, "Digit2");
        assert_eq!(at.virtual_key_code, 50);
        assert!(at.shift, "@ nam tren phim 2");

        let underscore = key_stroke('_').expect("_");
        assert_eq!(underscore.code, "Minus");
        assert!(underscore.shift);

        assert_eq!(
            key_stroke(' ').map(|stroke| stroke.code),
            Some("Space".to_owned())
        );
        assert_eq!(
            key_stroke('~').map(|stroke| stroke.code),
            Some("Backquote".to_owned())
        );
        assert_eq!(
            key_stroke('|').map(|stroke| stroke.code),
            Some("Backslash".to_owned())
        );
    }

    #[test]
    fn a_character_outside_the_layout_is_refused_instead_of_guessed() {
        assert_eq!(key_stroke('é'), None);
        assert_eq!(key_stroke('\n'), None);
        assert_eq!(key_stroke('中'), None);
    }

    #[test]
    fn every_printable_ascii_character_has_a_key_stroke() {
        for code in 0x21u8..=0x7E {
            let character = char::from(code);
            assert!(
                key_stroke(character).is_some(),
                "thieu anh xa cho {character:?} (0x{code:02X})"
            );
        }
    }

    #[test]
    fn a_key_down_frame_carries_the_text_and_a_key_up_does_not() {
        let stroke = key_stroke('@').expect("@");

        let down: serde_json::Value =
            serde_json::from_str(&key_down_request(11, &stroke)).expect("json");
        assert_eq!(down["method"], "Input.dispatchKeyEvent");
        assert_eq!(down["params"]["type"], "keyDown");
        assert_eq!(down["params"]["text"], "@");
        assert_eq!(down["params"]["unmodifiedText"], "@");
        assert_eq!(down["params"]["modifiers"], SHIFT_MODIFIER);
        assert_eq!(down["params"]["code"], "Digit2");

        let up: serde_json::Value =
            serde_json::from_str(&key_up_request(12, &stroke)).expect("json");
        assert_eq!(up["params"]["type"], "keyUp");
        assert!(up["params"].get("text").is_none(), "keyUp khong mang text");
    }

    #[test]
    fn a_plain_letter_needs_no_modifier() {
        let stroke = key_stroke('b').expect("b");
        let down: serde_json::Value =
            serde_json::from_str(&key_down_request(1, &stroke)).expect("json");
        assert_eq!(down["params"]["modifiers"], 0);
    }

    #[test]
    fn the_click_is_a_press_and_a_release_at_the_same_point() {
        let press: serde_json::Value =
            serde_json::from_str(&mouse_press_request(5, 120.5, 240.25)).expect("json");
        assert_eq!(press["method"], "Input.dispatchMouseEvent");
        assert_eq!(press["params"]["type"], "mousePressed");
        assert_eq!(press["params"]["button"], "left");
        assert_eq!(press["params"]["buttons"], 1);
        assert_eq!(press["params"]["clickCount"], 1);

        let release: serde_json::Value =
            serde_json::from_str(&mouse_release_request(6, 120.5, 240.25)).expect("json");
        assert_eq!(release["params"]["type"], "mouseReleased");
        assert_eq!(release["params"]["buttons"], 0);

        let moved: serde_json::Value =
            serde_json::from_str(&mouse_move_request(4, 120.5, 240.25)).expect("json");
        assert_eq!(moved["params"]["type"], "mouseMoved");
        assert_eq!(moved["params"]["button"], "none");
    }

    #[test]
    fn a_string_reply_is_read_from_the_evaluate_envelope() {
        let reply: serde_json::Value = serde_json::from_str(
            r#"{"id":4,"result":{"result":{"type":"string","value":"focused"}}}"#,
        )
        .expect("json");
        assert_eq!(reply_string_value(&reply).as_deref(), Some("focused"));

        let missing: serde_json::Value =
            serde_json::from_str(r#"{"id":4,"result":{}}"#).expect("json");
        assert_eq!(reply_string_value(&missing), None);
    }

    #[test]
    fn cookies_are_read_from_the_parsed_reply() {
        let reply: serde_json::Value = serde_json::from_str(
            r#"{"id":10,"result":{"cookies":[
                {"name":"a","value":"1","domain":".roblox.com"},
                {"name":".ROBLOSECURITY","value":"TOKEN","domain":".roblox.com"}
            ]}}"#,
        )
        .expect("json");

        let cookies = cookies_from_value(&reply);
        assert_eq!(cookies.len(), 2);
        assert_eq!(find_security_cookie(&cookies).as_deref(), Some("TOKEN"));

        let empty: serde_json::Value =
            serde_json::from_str(r#"{"id":10,"result":{}}"#).expect("json");
        assert!(cookies_from_value(&empty).is_empty());
    }
}
