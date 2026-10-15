use super::auth::{SECURITY_COOKIE, roblosecurity_from_set_cookie};
use super::http::{HttpMethod, HttpRequest, HttpResponse, Transport, TransportError};
use crate::launch::url_encode;

pub const WWW_HOST: &str = "https://www.roblox.com";
pub const AUTH_HOST: &str = "https://auth.roblox.com";
pub const ACCOUNT_SETTINGS_HOST: &str = "https://accountsettings.roblox.com";
pub const USERS_HOST: &str = "https://users.roblox.com";
pub const AVATAR_HOST: &str = "https://avatar.roblox.com";
pub const FRIENDS_HOST: &str = "https://friends.roblox.com";

pub const ACCOUNT_REFERER: &str = "https://www.roblox.com/";
pub const MY_ACCOUNT_REFERER: &str = "https://www.roblox.com/my/account";

pub const FORM_CONTENT_TYPE: &str = "application/x-www-form-urlencoded";
pub const JSON_CONTENT_TYPE: &str = "application/json";

pub const PIN_LENGTH: usize = 4;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AccountActionError {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error("cookie .ROBLOSECURITY không hợp lệ hoặc đã hết hạn (HTTP 401)")]
    Unauthorized,

    #[error("thao tác thất bại: HTTP {status} — {message}")]
    Failed { status: u16, message: String },

    #[error("mã PIN phải gồm đúng 4 chữ số")]
    InvalidPin,

    #[error("dữ liệu avatar không hợp lệ: {0}")]
    InvalidAvatar(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PinState {
    pub enabled: bool,
    pub unlocked_until: i64,
}

impl PinState {
    #[must_use]
    pub fn is_unlocked(&self) -> bool {
        !self.enabled || self.unlocked_until > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowPrivacy {
    All,
    Followers,
    Following,
    Friends,
    NoOne,
}

impl FollowPrivacy {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Followers => "Followers",
            Self::Following => "Following",
            Self::Friends => "Friends",
            Self::NoOne => "NoOne",
        }
    }

    #[must_use]
    pub fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::All),
            1 => Some(Self::Followers),
            2 => Some(Self::Following),
            3 => Some(Self::Friends),
            4 => Some(Self::NoOne),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AvatarOutcome {
    pub invalid_asset_ids: Vec<i64>,
}

#[must_use]
pub fn form_body(params: &[(&str, &str)]) -> Vec<u8> {
    params
        .iter()
        .map(|(key, value)| format!("{}={}", url_encode(key), url_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
        .into_bytes()
}

fn authed(
    method: HttpMethod,
    url: String,
    cookie: &str,
    csrf: Option<&str>,
    referer: &str,
) -> HttpRequest {
    let mut request = HttpRequest::new(method, url).with_header("Referer", referer);

    if !cookie.is_empty() {
        request = request.with_header("Cookie", format!("{SECURITY_COOKIE}={cookie}"));
    }
    if let Some(csrf) = csrf {
        request = request.with_header("X-CSRF-TOKEN", csrf);
    }

    request
}

fn ensure_ok(response: &HttpResponse) -> Result<(), AccountActionError> {
    if response.status == 401 {
        return Err(AccountActionError::Unauthorized);
    }
    if !(200..300).contains(&response.status) {
        return Err(AccountActionError::Failed {
            status: response.status,
            message: response.body_as_str().unwrap_or("").trim().to_owned(),
        });
    }
    Ok(())
}

#[must_use]
pub fn parse_pin_state(body: &str) -> PinState {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return PinState::default();
    };

    PinState {
        enabled: value
            .get("isEnabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        unlocked_until: value
            .get("unlockedUntil")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
    }
}

#[must_use]
pub fn parse_blocked_user_ids(body: &str) -> Vec<i64> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };

    value
        .get("blockedUsers")
        .and_then(serde_json::Value::as_array)
        .map(|users| {
            users
                .iter()
                .filter_map(|user| {
                    user.get("userId").and_then(|id| {
                        id.as_i64()
                            .or_else(|| id.as_str().and_then(|text| text.parse().ok()))
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[must_use]
pub fn parse_invalid_asset_ids(body: &str) -> Vec<i64> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("invalidAssetIds")
                .and_then(serde_json::Value::as_array)
                .map(|ids| ids.iter().filter_map(serde_json::Value::as_i64).collect())
        })
        .unwrap_or_default()
}

pub fn check_pin<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: Option<&str>,
) -> Result<PinState, AccountActionError> {
    let request = authed(
        HttpMethod::Get,
        format!("{AUTH_HOST}/v1/account/pin/"),
        cookie,
        csrf,
        ACCOUNT_REFERER,
    );

    let response = transport.execute(&request)?;
    ensure_ok(&response)?;
    Ok(parse_pin_state(response.body_as_str().unwrap_or("")))
}

pub fn unlock_pin<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
    pin: &str,
) -> Result<PinState, AccountActionError> {
    if pin.len() != PIN_LENGTH || !pin.chars().all(|digit| digit.is_ascii_digit()) {
        return Err(AccountActionError::InvalidPin);
    }

    let request = authed(
        HttpMethod::Post,
        format!("{AUTH_HOST}/v1/account/pin/unlock"),
        cookie,
        Some(csrf),
        ACCOUNT_REFERER,
    )
    .with_header("Content-Type", FORM_CONTENT_TYPE)
    .with_body(form_body(&[("pin", pin)]));

    let response = transport.execute(&request)?;
    ensure_ok(&response)?;
    Ok(parse_pin_state(response.body_as_str().unwrap_or("")))
}

pub fn set_follow_privacy<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
    privacy: FollowPrivacy,
) -> Result<(), AccountActionError> {
    let request = authed(
        HttpMethod::Post,
        format!("{WWW_HOST}/account/settings/follow-me-privacy"),
        cookie,
        Some(csrf),
        MY_ACCOUNT_REFERER,
    )
    .with_header("Content-Type", FORM_CONTENT_TYPE)
    .with_body(form_body(&[("FollowMePrivacy", privacy.as_str())]));

    let response = transport.execute(&request)?;
    ensure_ok(&response)
}

pub fn change_password<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
    current: &str,
    new: &str,
) -> Result<Option<String>, AccountActionError> {
    let request = authed(
        HttpMethod::Post,
        format!("{AUTH_HOST}/v2/user/passwords/change"),
        cookie,
        Some(csrf),
        ACCOUNT_REFERER,
    )
    .with_header("Content-Type", FORM_CONTENT_TYPE)
    .with_body(form_body(&[
        ("currentPassword", current),
        ("newPassword", new),
    ]));

    let response = transport.execute(&request)?;
    ensure_ok(&response)?;
    Ok(roblosecurity_from_set_cookie(&response.headers))
}

pub fn change_email<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
    password: &str,
    email: &str,
) -> Result<(), AccountActionError> {
    let request = authed(
        HttpMethod::Post,
        format!("{ACCOUNT_SETTINGS_HOST}/v1/email"),
        cookie,
        Some(csrf),
        ACCOUNT_REFERER,
    )
    .with_header("Content-Type", FORM_CONTENT_TYPE)
    .with_body(form_body(&[
        ("password", password),
        ("emailAddress", email),
    ]));

    let response = transport.execute(&request)?;
    ensure_ok(&response)
}

pub fn logout_other_sessions<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
) -> Result<Option<String>, AccountActionError> {
    let request = authed(
        HttpMethod::Post,
        format!("{WWW_HOST}/authentication/signoutfromallsessionsandreauthenticate"),
        cookie,
        Some(csrf),
        ACCOUNT_REFERER,
    );

    let response = transport.execute(&request)?;
    ensure_ok(&response)?;
    Ok(roblosecurity_from_set_cookie(&response.headers))
}

pub fn block_user<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
    user_id: i64,
    unblock: bool,
) -> Result<u16, AccountActionError> {
    let action = if unblock { "unblock" } else { "block" };
    let request = authed(
        HttpMethod::Post,
        format!("{ACCOUNT_SETTINGS_HOST}/v1/users/{user_id}/{action}"),
        cookie,
        Some(csrf),
        ACCOUNT_REFERER,
    );

    let response = transport.execute(&request)?;
    if response.status == 401 {
        return Err(AccountActionError::Unauthorized);
    }
    Ok(response.status)
}

pub fn blocked_list<T: Transport>(
    transport: &T,
    cookie: &str,
) -> Result<String, AccountActionError> {
    let request = authed(
        HttpMethod::Get,
        format!("{ACCOUNT_SETTINGS_HOST}/v1/users/get-detailed-blocked-users"),
        cookie,
        None,
        ACCOUNT_REFERER,
    );

    let response = transport.execute(&request)?;
    ensure_ok(&response)?;
    Ok(response.body_as_str().unwrap_or("").to_owned())
}

pub fn unblock_everyone<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
) -> Result<usize, AccountActionError> {
    let body = blocked_list(transport, cookie)?;
    let ids = parse_blocked_user_ids(&body);

    let mut done = 0;
    for user_id in ids {
        if block_user(transport, cookie, csrf, user_id, true)
            .is_ok_and(|status| (200..300).contains(&status))
        {
            done += 1;
        }
    }

    Ok(done)
}

pub fn set_display_name<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
    user_id: i64,
    name: &str,
) -> Result<(), AccountActionError> {
    let body = serde_json::json!({ "newDisplayName": name }).to_string();
    let request = authed(
        HttpMethod::Patch,
        format!("{USERS_HOST}/v1/users/{user_id}/display-names"),
        cookie,
        Some(csrf),
        ACCOUNT_REFERER,
    )
    .with_header("Content-Type", JSON_CONTENT_TYPE)
    .with_body(body);

    let response = transport.execute(&request)?;
    ensure_ok(&response)
}

fn post_json<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
    url: &str,
    body: String,
) -> Result<HttpResponse, AccountActionError> {
    let request = authed(
        HttpMethod::Post,
        url.to_owned(),
        cookie,
        Some(csrf),
        ACCOUNT_REFERER,
    )
    .with_header("Content-Type", JSON_CONTENT_TYPE)
    .with_body(body);

    let response = transport.execute(&request)?;
    ensure_ok(&response)?;
    Ok(response)
}

pub fn set_avatar<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
    avatar_json: &str,
) -> Result<AvatarOutcome, AccountActionError> {
    let value: serde_json::Value = serde_json::from_str(avatar_json)
        .map_err(|error| AccountActionError::InvalidAvatar(error.to_string()))?;

    let object = value
        .as_object()
        .ok_or_else(|| AccountActionError::InvalidAvatar("gốc phải là object JSON".to_owned()))?;

    let mut outcome = AvatarOutcome::default();

    if let Some(kind) = object.get("playerAvatarType") {
        let body = serde_json::json!({ "playerAvatarType": kind }).to_string();
        post_json(
            transport,
            cookie,
            csrf,
            &format!("{AVATAR_HOST}/v1/avatar/set-player-avatar-type"),
            body,
        )?;
    }

    if let Some(scales) = object.get("scales").or_else(|| object.get("scale")) {
        post_json(
            transport,
            cookie,
            csrf,
            &format!("{AVATAR_HOST}/v1/avatar/set-scales"),
            scales.to_string(),
        )?;
    }

    if let Some(colors) = object.get("bodyColors") {
        post_json(
            transport,
            cookie,
            csrf,
            &format!("{AVATAR_HOST}/v1/avatar/set-body-colors"),
            colors.to_string(),
        )?;
    }

    if let Some(assets) = object.get("assets") {
        let body = serde_json::json!({ "assets": assets }).to_string();
        let response = post_json(
            transport,
            cookie,
            csrf,
            &format!("{AVATAR_HOST}/v2/avatar/set-wearing-assets"),
            body,
        )?;
        outcome.invalid_asset_ids = parse_invalid_asset_ids(response.body_as_str().unwrap_or(""));
    }

    Ok(outcome)
}

pub fn send_friend_request<T: Transport>(
    transport: &T,
    cookie: &str,
    csrf: &str,
    user_id: i64,
) -> Result<bool, AccountActionError> {
    let request = authed(
        HttpMethod::Post,
        format!("{FRIENDS_HOST}/v1/users/{user_id}/request-friendship"),
        cookie,
        Some(csrf),
        ACCOUNT_REFERER,
    )
    .with_header("Content-Type", JSON_CONTENT_TYPE);

    let response = transport.execute(&request)?;
    Ok((200..300).contains(&response.status))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::*;

    #[derive(Default)]
    struct Scripted {
        responses: RefCell<VecDeque<HttpResponse>>,
        requests: RefCell<Vec<HttpRequest>>,
    }

    impl Scripted {
        fn new(responses: Vec<HttpResponse>) -> Self {
            Self {
                responses: RefCell::new(responses.into()),
                requests: RefCell::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<HttpRequest> {
            self.requests.borrow().clone()
        }
    }

    impl Transport for &Scripted {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            self.requests.borrow_mut().push(request.clone());
            self.responses
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| TransportError::new("hết kịch bản"))
        }
    }

    fn ok(body: &str) -> HttpResponse {
        HttpResponse::new(200).with_body(body.as_bytes().to_vec())
    }

    #[test]
    fn a_form_body_url_encodes_like_dotnet() {
        assert_eq!(
            form_body(&[("currentPassword", "a b&c"), ("newPassword", "x=y")]),
            b"currentPassword=a+b%26c&newPassword=x%3dy".to_vec()
        );
    }

    #[test]
    fn the_pin_state_is_read_from_the_documented_shape() {
        assert_eq!(
            parse_pin_state(r#"{"isEnabled":true,"unlockedUntil":300}"#),
            PinState {
                enabled: true,
                unlocked_until: 300
            }
        );
        assert!(parse_pin_state(r#"{"isEnabled":false,"unlockedUntil":0}"#).is_unlocked());
        assert!(!parse_pin_state("<html>").enabled);
    }

    #[test]
    fn checking_the_pin_uses_the_auth_host_and_carries_the_cookie() {
        let script = Scripted::new(vec![ok(r#"{"isEnabled":true,"unlockedUntil":0}"#)]);
        let state = check_pin(&&script, "COOKIE", Some("T")).expect("đọc pin");

        assert!(state.enabled);
        let requests = script.requests();
        assert_eq!(requests[0].url, "https://auth.roblox.com/v1/account/pin/");
        assert_eq!(requests[0].method, HttpMethod::Get);
        assert_eq!(requests[0].header("cookie"), Some(".ROBLOSECURITY=COOKIE"));
        assert_eq!(requests[0].header("referer"), Some(ACCOUNT_REFERER));
    }

    #[test]
    fn a_pin_that_is_not_four_digits_is_refused_without_a_request() {
        let script = Scripted::new(vec![]);
        assert_eq!(
            unlock_pin(&&script, "C", "T", "12"),
            Err(AccountActionError::InvalidPin)
        );
        assert_eq!(
            unlock_pin(&&script, "C", "T", "abcd"),
            Err(AccountActionError::InvalidPin)
        );
        assert!(script.requests().is_empty());
    }

    #[test]
    fn unlocking_the_pin_posts_the_pin_as_a_form() {
        let script = Scripted::new(vec![ok(r#"{"isEnabled":true,"unlockedUntil":300}"#)]);
        let state = unlock_pin(&&script, "COOKIE", "T", "1234").expect("mo khoa");

        assert_eq!(state.unlocked_until, 300);
        let requests = script.requests();
        assert_eq!(
            requests[0].url,
            "https://auth.roblox.com/v1/account/pin/unlock"
        );
        assert_eq!(requests[0].method, HttpMethod::Post);
        assert_eq!(requests[0].header("x-csrf-token"), Some("T"));
        assert_eq!(requests[0].header("content-type"), Some(FORM_CONTENT_TYPE));
        assert_eq!(requests[0].body.as_deref(), Some(b"pin=1234".as_slice()));
    }

    #[test]
    fn follow_privacy_maps_every_index_to_its_ram_string() {
        assert_eq!(FollowPrivacy::from_index(0), Some(FollowPrivacy::All));
        assert_eq!(FollowPrivacy::from_index(4), Some(FollowPrivacy::NoOne));
        assert_eq!(FollowPrivacy::from_index(9), None);

        let script = Scripted::new(vec![ok("")]);
        set_follow_privacy(&&script, "C", "T", FollowPrivacy::Friends).expect("dat privacy");

        let requests = script.requests();
        assert_eq!(
            requests[0].url,
            "https://www.roblox.com/account/settings/follow-me-privacy"
        );
        assert_eq!(requests[0].header("referer"), Some(MY_ACCOUNT_REFERER));
        assert_eq!(
            requests[0].body.as_deref(),
            Some(b"FollowMePrivacy=Friends".as_slice())
        );
    }

    #[test]
    fn changing_the_password_returns_the_rotated_cookie() {
        let script = Scripted::new(vec![
            HttpResponse::new(200).with_header("Set-Cookie", ".ROBLOSECURITY=NEW; Path=/"),
        ]);

        let cookie = change_password(&&script, "OLD", "T", "old", "new").expect("doi mat khau");
        assert_eq!(cookie.as_deref(), Some("NEW"));

        let requests = script.requests();
        assert_eq!(
            requests[0].url,
            "https://auth.roblox.com/v2/user/passwords/change"
        );
        let body = requests[0].body.as_deref().expect("co than");
        assert_eq!(
            std::str::from_utf8(body).expect("utf8"),
            "currentPassword=old&newPassword=new"
        );
    }

    #[test]
    fn changing_the_email_posts_to_account_settings() {
        let script = Scripted::new(vec![ok("")]);
        change_email(&&script, "C", "T", "pw", "a@b.c").expect("doi email");

        let requests = script.requests();
        assert_eq!(
            requests[0].url,
            "https://accountsettings.roblox.com/v1/email"
        );
        assert_eq!(
            requests[0].body.as_deref(),
            Some(b"password=pw&emailAddress=a%40b.c".as_slice())
        );
    }

    #[test]
    fn logging_out_other_sessions_returns_the_new_cookie() {
        let script = Scripted::new(vec![
            HttpResponse::new(200).with_header("Set-Cookie", ".ROBLOSECURITY=FRESH"),
        ]);

        let cookie = logout_other_sessions(&&script, "C", "T").expect("dang xuat");
        assert_eq!(cookie.as_deref(), Some("FRESH"));
        assert_eq!(
            script.requests()[0].url,
            "https://www.roblox.com/authentication/signoutfromallsessionsandreauthenticate"
        );
    }

    #[test]
    fn blocking_and_unblocking_report_the_status_without_failing_on_400() {
        let script = Scripted::new(vec![HttpResponse::new(400), HttpResponse::new(200)]);

        let blocked = block_user(&&script, "C", "T", 7, false).expect("chan");
        let unblocked = block_user(&&script, "C", "T", 7, true).expect("bo chan");

        assert_eq!(blocked, 400);
        assert_eq!(unblocked, 200);

        let requests = script.requests();
        assert_eq!(
            requests[0].url,
            "https://accountsettings.roblox.com/v1/users/7/block"
        );
        assert_eq!(
            requests[1].url,
            "https://accountsettings.roblox.com/v1/users/7/unblock"
        );
        assert_eq!(requests[0].header("x-csrf-token"), Some("T"));
    }

    #[test]
    fn the_blocked_list_reads_user_ids_whether_numbers_or_strings() {
        let body = r#"{"blockedUsers":[{"userId":11},{"userId":"22"}]}"#;
        assert_eq!(parse_blocked_user_ids(body), [11, 22]);
        assert!(parse_blocked_user_ids("<html>").is_empty());
    }

    #[test]
    fn unblocking_everyone_walks_the_list() {
        let script = Scripted::new(vec![
            ok(r#"{"blockedUsers":[{"userId":1},{"userId":2}]}"#),
            HttpResponse::new(200),
            HttpResponse::new(200),
        ]);

        let count = unblock_everyone(&&script, "C", "T").expect("bo chan tat ca");
        assert_eq!(count, 2);

        let requests = script.requests();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests[1].url,
            "https://accountsettings.roblox.com/v1/users/1/unblock"
        );
        assert_eq!(
            requests[2].url,
            "https://accountsettings.roblox.com/v1/users/2/unblock"
        );
    }

    #[test]
    fn setting_the_display_name_uses_patch_and_a_json_body() {
        let script = Scripted::new(vec![ok("{}")]);
        set_display_name(&&script, "C", "T", 42, "New Name").expect("dat ten");

        let requests = script.requests();
        assert_eq!(requests[0].method, HttpMethod::Patch);
        assert_eq!(
            requests[0].url,
            "https://users.roblox.com/v1/users/42/display-names"
        );
        let body = requests[0].body.as_deref().expect("co than");
        let value: serde_json::Value = serde_json::from_slice(body).expect("json");
        assert_eq!(value["newDisplayName"], "New Name");
    }

    #[test]
    fn setting_the_avatar_posts_only_the_pieces_that_are_present() {
        let script = Scripted::new(vec![ok("{}"), ok("{}")]);
        let avatar = r#"{"playerAvatarType":"R15","scales":{"height":1.0}}"#;

        let outcome = set_avatar(&&script, "C", "T", avatar).expect("dat avatar");

        assert!(outcome.invalid_asset_ids.is_empty());
        let requests = script.requests();
        assert_eq!(requests.len(), 2, "chi gui 2 manh co mat");
        assert_eq!(
            requests[0].url,
            "https://avatar.roblox.com/v1/avatar/set-player-avatar-type"
        );
        assert_eq!(
            requests[1].url,
            "https://avatar.roblox.com/v1/avatar/set-scales"
        );
    }

    #[test]
    fn setting_the_avatar_reports_invalid_assets() {
        let script = Scripted::new(vec![ok(r#"{"invalidAssetIds":[99,100]}"#)]);
        let outcome =
            set_avatar(&&script, "C", "T", r#"{"assets":[{"id":99}]}"#).expect("dat avatar");

        assert_eq!(outcome.invalid_asset_ids, [99, 100]);
        assert_eq!(
            script.requests()[0].url,
            "https://avatar.roblox.com/v2/avatar/set-wearing-assets"
        );
    }

    #[test]
    fn a_broken_avatar_payload_is_refused_without_a_request() {
        let script = Scripted::new(vec![]);
        assert!(matches!(
            set_avatar(&&script, "C", "T", "khong-phai-json"),
            Err(AccountActionError::InvalidAvatar(_))
        ));
        assert!(matches!(
            set_avatar(&&script, "C", "T", "[1,2,3]"),
            Err(AccountActionError::InvalidAvatar(_))
        ));
        assert!(script.requests().is_empty());
    }

    #[test]
    fn a_friend_request_reports_success_from_the_status() {
        let script = Scripted::new(vec![HttpResponse::new(200), HttpResponse::new(403)]);
        assert!(send_friend_request(&&script, "C", "T", 5).expect("ket ban"));
        assert!(!send_friend_request(&&script, "C", "T", 5).expect("ket ban"));

        assert_eq!(
            script.requests()[0].url,
            "https://friends.roblox.com/v1/users/5/request-friendship"
        );
    }

    #[test]
    fn a_401_is_reported_as_unauthorized_not_as_a_generic_failure() {
        let script = Scripted::new(vec![HttpResponse::new(401)]);
        let error = check_pin(&&script, "C", Some("T")).expect_err("phải lỗi");
        assert!(matches!(error, AccountActionError::Unauthorized));
    }

    #[test]
    fn a_non_success_status_keeps_the_body_as_the_message() {
        let script = Scripted::new(vec![
            HttpResponse::new(400).with_body(b"bad request".to_vec()),
        ]);
        let error = set_follow_privacy(&&script, "C", "T", FollowPrivacy::All).expect_err("lỗi");
        assert!(matches!(
            error,
            AccountActionError::Failed { status: 400, .. }
        ));
    }
}
