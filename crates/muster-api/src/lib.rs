use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use muster_core::datetime::roblox_tick_from_json_net;
use muster_core::models::Account;
use muster_core::roblox::account::AccountStore;

pub const DEFAULT_PORT: u16 = 7963;
pub const MIN_PASSWORD_LEN: usize = 6;

pub const RUNNING_MESSAGE: &str = "Roblox Account Manager is running";
pub const INVALID_PASSWORD_MESSAGE: &str =
    "Invalid Password, make sure your password contains 6 or more characters";
pub const INVALID_PASSWORD_RAW: &str = "Invalid Password";
pub const METHOD_NOT_ALLOWED_RAW: &str = "Method not allowed";
pub const INVALID_ACCOUNT_MESSAGE: &str = "Invalid Account, the account's cookie may have expired and resulted in the account being logged out";
pub const INVALID_ACCOUNT_RAW: &str = "Invalid Account";
pub const EMPTY_ACCOUNT: &str = "Empty Account";
pub const EMPTY_USER_ID: &str = "Empty UserId";
pub const EMPTY_FIELD: &str = "Empty Field";
pub const INVALID_PLACE_ID: &str = "Invalid PlaceId provided";
pub const INVALID_PLACE_ID_RAW: &str = "Invalid PlaceId";
pub const INVALID_USERNAME: &str = "Invalid Username Parameter";
pub const INVALID_BODY: &str = "Empty body";
pub const NOT_FOUND: &str = "404 not found";
pub const NOT_CONFIGURED: &str = "Muster: thao tác này chưa được cấu hình cho tiến trình API";

pub const SENSITIVE_METHODS: [&str; 4] =
    ["GetCookie", "GetAccounts", "LaunchAccount", "FollowUser"];

pub const METHODS: [&str; 22] = [
    "GetAccounts",
    "GetAccountsJson",
    "GetCookie",
    "GetCSRFToken",
    "GetAlias",
    "GetDescription",
    "GetField",
    "GetBlockedList",
    "SetAlias",
    "SetDescription",
    "AppendDescription",
    "SetField",
    "RemoveField",
    "SetAvatar",
    "SetServer",
    "SetRecommendedServer",
    "LaunchAccount",
    "FollowUser",
    "ImportCookie",
    "BlockUser",
    "UnblockUser",
    "UnblockEveryone",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendReply {
    pub body: String,
    pub success: bool,
    pub code: Option<u16>,
}

impl BackendReply {
    #[must_use]
    pub fn ok(body: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            success: true,
            code: None,
        }
    }

    #[must_use]
    pub fn failed(body: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            success: false,
            code: None,
        }
    }

    #[must_use]
    pub fn with_code(mut self, code: u16) -> Self {
        self.code = Some(code);
        self
    }
}

pub trait ApiBackend: Send + Sync {
    fn import_cookie(&self, _cookie: &str) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }

    fn csrf_token(&self, _account: &str) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }

    fn join_server(
        &self,
        _account: &str,
        _place_id: i64,
        _job_id: &str,
        _follow_user: bool,
        _join_vip: bool,
    ) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }

    fn follow_user(&self, _account: &str, _username: &str) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }

    fn set_server(&self, _account: &str, _place_id: i64, _job_id: &str) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }

    fn recommended_server(&self, _account: &str, _place_id: Option<i64>) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }

    fn block_user(&self, _account: &str, _user_id: &str) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }

    fn unblock_user(&self, _account: &str, _user_id: &str) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }

    fn blocked_list(&self, _account: &str) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }

    fn unblock_everyone(&self, _account: &str) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }

    fn set_avatar(&self, _account: &str, _body: &str) -> BackendReply {
        BackendReply::failed(NOT_CONFIGURED)
    }
}

pub struct NoBackend;

impl ApiBackend for NoBackend {}

pub struct ApiState {
    pub store: Arc<Mutex<AccountStore>>,
    pub backend: Arc<dyn ApiBackend>,
    pub password: Option<String>,
    pub every_request_requires_password: bool,
    pub allow_external_connections: bool,
    pub allow_get_accounts: bool,
    pub allow_get_cookie: bool,
    pub allow_launch_account: bool,
    pub allow_account_editing: bool,
}

impl Default for ApiState {
    fn default() -> Self {
        Self {
            store: Arc::new(Mutex::new(AccountStore::new())),
            backend: Arc::new(NoBackend),
            password: None,
            every_request_requires_password: false,
            allow_external_connections: false,
            allow_get_accounts: false,
            allow_get_cookie: false,
            allow_launch_account: false,
            allow_account_editing: false,
        }
    }
}

impl ApiState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_store(store: Arc<Mutex<AccountStore>>) -> Self {
        Self {
            store,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn open() -> Self {
        Self {
            allow_get_accounts: true,
            allow_get_cookie: true,
            allow_launch_account: true,
            allow_account_editing: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_backend(mut self, backend: Arc<dyn ApiBackend>) -> Self {
        self.backend = backend;
        self
    }

    #[must_use]
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        let password = password.into();
        self.password = (!password.is_empty()).then_some(password);
        self
    }
}

struct Reply {
    status: StatusCode,
    body: String,
}

impl Reply {
    fn text(is_v2: bool, message: &str, success: bool, code: i32, raw: Option<&str>) -> Self {
        let body = if is_v2 {
            serde_json::json!({ "Success": success, "Message": message }).to_string()
        } else {
            raw.unwrap_or(message).to_owned()
        };

        Self {
            status: status_of(success, code),
            body,
        }
    }

    fn json(is_v2: bool, value: serde_json::Value, success: bool, code: i32) -> Self {
        let body = if is_v2 {
            serde_json::json!({ "Success": success, "Message": value }).to_string()
        } else {
            value.to_string()
        };

        Self {
            status: status_of(success, code),
            body,
        }
    }

    fn raw(status: StatusCode, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }

    fn from_backend(is_v2: bool, reply: BackendReply) -> Self {
        Self::text(
            is_v2,
            &reply.body,
            reply.success,
            reply.code.map_or(-1, i32::from),
            Some(&reply.body),
        )
    }
}

fn status_of(success: bool, code: i32) -> StatusCode {
    let raw = if code > 0 {
        u16::try_from(code).unwrap_or(400)
    } else if success {
        200
    } else {
        400
    };

    StatusCode::from_u16(raw).unwrap_or(StatusCode::BAD_REQUEST)
}

impl IntoResponse for Reply {
    fn into_response(self) -> Response {
        let Self { status, body } = self;
        let mut response = (status, body.clone()).into_response();

        if status.as_u16() > 299 {
            let sanitized = body.replace(['\r', '\n'], " ");
            if let Ok(value) = HeaderValue::from_str(&sanitized) {
                response.headers_mut().insert("ws-error", value);
            }
        }

        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );

        response
    }
}

pub fn router() -> Router {
    router_with_state(Arc::new(ApiState::new()))
}

pub fn router_with_state(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/Running", get(handle_running))
        .route("/v2/Running", get(handle_running_v2))
        .route("/favicon.ico", get(handle_favicon))
        .route("/v2/favicon.ico", get(handle_favicon))
        .route("/{method}", get(handle_v1).post(handle_v1))
        .route("/v2/{method}", get(handle_v2).post(handle_v2))
        .with_state(state)
}

pub async fn start_server(port: u16, state: Arc<ApiState>) -> Result<(), std::io::Error> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router_with_state(state))
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))
}

async fn handle_running() -> Reply {
    Reply::raw(StatusCode::OK, "true")
}

async fn handle_running_v2() -> Reply {
    Reply::text(true, RUNNING_MESSAGE, true, -1, None)
}

async fn handle_favicon() -> Reply {
    Reply::raw(StatusCode::OK, "")
}

async fn handle_v1(
    State(state): State<Arc<ApiState>>,
    Path(method): Path<String>,
    Query(params): Query<BTreeMap<String, String>>,
    body: String,
) -> Reply {
    dispatch(&state, &method, &params, &body, false)
}

async fn handle_v2(
    State(state): State<Arc<ApiState>>,
    Path(method): Path<String>,
    Query(params): Query<BTreeMap<String, String>>,
    body: String,
) -> Reply {
    dispatch(&state, &method, &params, &body, true)
}

fn param<'a>(params: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    params
        .get(name)
        .or_else(|| {
            params
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value)
        })
        .map(String::as_str)
}

fn password_matches(state: &ApiState, provided: Option<&str>) -> bool {
    match state.password.as_deref() {
        Some(expected) => expected.len() >= MIN_PASSWORD_LEN && provided == Some(expected),
        None => !state.every_request_requires_password,
    }
}

fn find_index(store: &AccountStore, identifier: &str) -> Option<usize> {
    if let Ok(user_id) = identifier.parse::<i64>() {
        if let Some(index) = store.find_by_user_id(user_id) {
            return Some(index);
        }
    }
    store.find_by_username(identifier)
}

fn account_snapshot(state: &ApiState, identifier: &str) -> Option<Account> {
    let store = state.store.lock().ok()?;
    let index = find_index(&store, identifier)?;
    store.get(index).ok().cloned()
}

fn edit_account(state: &ApiState, identifier: &str, edit: impl FnOnce(&mut Account)) -> bool {
    let Ok(mut store) = state.store.lock() else {
        return false;
    };
    let Some(index) = find_index(&store, identifier) else {
        return false;
    };
    match store.get_mut(index) {
        Ok(account) => {
            edit(account);
            true
        }
        Err(_) => false,
    }
}

fn dispatch(
    state: &ApiState,
    method: &str,
    params: &BTreeMap<String, String>,
    body: &str,
    is_v2: bool,
) -> Reply {
    if method == "favicon.ico" {
        return Reply::raw(StatusCode::OK, "");
    }

    if method == "Running" {
        return Reply::text(is_v2, RUNNING_MESSAGE, true, -1, Some("true"));
    }

    let provided = param(params, "Password");

    if state.every_request_requires_password && !password_matches(state, provided) {
        return Reply::text(
            is_v2,
            INVALID_PASSWORD_MESSAGE,
            false,
            401,
            Some(INVALID_PASSWORD_RAW),
        );
    }

    if SENSITIVE_METHODS.contains(&method) && !password_matches(state, provided) {
        return Reply::text(
            is_v2,
            INVALID_PASSWORD_MESSAGE,
            false,
            401,
            Some(INVALID_PASSWORD_RAW),
        );
    }

    if method == "GetAccounts" {
        if !state.allow_get_accounts {
            return not_allowed(is_v2, "GetAccounts");
        }

        let Ok(store) = state.store.lock() else {
            return Reply::text(is_v2, "Store error", false, 500, None);
        };

        let group_filter = param(params, "Group");
        let names: Vec<String> = store
            .accounts()
            .iter()
            .filter(|account| {
                group_filter.is_none_or(|group| account.group.as_deref() == Some(group))
            })
            .filter_map(|account| account.username.clone())
            .collect();
        let joined = names.join(",");

        return Reply::text(is_v2, &joined, true, -1, Some(&joined));
    }

    if method == "GetAccountsJson" {
        if !state.allow_get_accounts {
            return not_allowed(is_v2, "GetAccountsJson");
        }

        let Ok(store) = state.store.lock() else {
            return Reply::text(is_v2, "Store error", false, 500, None);
        };

        let group_filter = param(params, "Group");
        let show_cookies =
            param(params, "IncludeCookies") == Some("true") && state.allow_get_cookie;

        let list: Vec<serde_json::Value> = store
            .accounts()
            .iter()
            .filter(|account| {
                group_filter.is_none_or(|group| account.group.as_deref() == Some(group))
            })
            .map(|account| {
                serde_json::json!({
                    "Username": account.username,
                    "UserID": account.user_id,
                    "Alias": account.alias,
                    "Description": account.description,
                    "Group": account.group,
                    "CSRFToken": serde_json::Value::Null,
                    "LastUsed": roblox_tick_from_json_net(&account.last_use).unwrap_or(0.0),
                    "Cookie": if show_cookies { account.security_token.clone() } else { None },
                    "Fields": account.fields,
                })
            })
            .collect();

        return Reply::json(is_v2, serde_json::Value::Array(list), true, -1);
    }

    if method == "ImportCookie" {
        let cookie = param(params, "Cookie").unwrap_or("");
        let reply = state.backend.import_cookie(cookie);
        let raw = if reply.success { "true" } else { "false" };
        return Reply::text(
            is_v2,
            &reply.body,
            reply.success,
            reply.code.map_or(-1, i32::from),
            Some(raw),
        );
    }

    let account_name = match param(params, "Account") {
        Some(account) if !account.is_empty() => account,
        _ => return Reply::text(is_v2, EMPTY_ACCOUNT, false, -1, None),
    };

    let Some(snapshot) = account_snapshot(state, account_name) else {
        return Reply::text(
            is_v2,
            INVALID_ACCOUNT_MESSAGE,
            false,
            -1,
            Some(INVALID_ACCOUNT_RAW),
        );
    };

    match method {
        "GetCookie" => {
            if !state.allow_get_cookie {
                return not_allowed(is_v2, "GetCookie");
            }
            match snapshot.security_token {
                Some(token) => Reply::text(is_v2, &token, true, -1, None),
                None => Reply::text(is_v2, "Account has no cookie", false, -1, None),
            }
        }

        "GetCSRFToken" => Reply::from_backend(is_v2, state.backend.csrf_token(account_name)),

        "GetAlias" => Reply::text(is_v2, &snapshot.alias, true, -1, None),

        "GetDescription" => Reply::text(is_v2, &snapshot.description, true, -1, None),

        "LaunchAccount" => {
            if !state.allow_launch_account {
                return not_allowed(is_v2, "LaunchAccount");
            }

            let Some(place_id) = param(params, "PlaceId").and_then(|raw| raw.parse::<i64>().ok())
            else {
                return Reply::text(
                    is_v2,
                    INVALID_PLACE_ID,
                    false,
                    -1,
                    Some(INVALID_PLACE_ID_RAW),
                );
            };

            let job_id = param(params, "JobId").unwrap_or("");
            let follow_user = param(params, "FollowUser") == Some("true");
            let join_vip = param(params, "JoinVIP") == Some("true");

            let reply =
                state
                    .backend
                    .join_server(account_name, place_id, job_id, follow_user, join_vip);

            if reply.code.is_none() && reply.success {
                Reply::text(
                    is_v2,
                    &format!("Launched {account_name} to {place_id}"),
                    true,
                    -1,
                    None,
                )
            } else {
                Reply::from_backend(is_v2, reply)
            }
        }

        "FollowUser" => {
            if !state.allow_launch_account {
                return not_allowed(is_v2, "FollowUser");
            }

            let Some(username) = param(params, "Username").filter(|value| !value.is_empty()) else {
                return Reply::text(is_v2, INVALID_USERNAME, false, -1, None);
            };

            let reply = state.backend.follow_user(account_name, username);
            if reply.code.is_none() && reply.success {
                Reply::text(
                    is_v2,
                    &format!("Joining {username}'s game on {account_name}"),
                    true,
                    -1,
                    None,
                )
            } else {
                Reply::from_backend(is_v2, reply)
            }
        }

        "BlockUser" => {
            let Some(user_id) = param(params, "UserId").filter(|value| !value.is_empty()) else {
                return Reply::text(is_v2, EMPTY_USER_ID, false, -1, None);
            };
            Reply::from_backend(is_v2, state.backend.block_user(account_name, user_id))
        }

        "UnblockUser" => {
            let Some(user_id) = param(params, "UserId").filter(|value| !value.is_empty()) else {
                return Reply::text(is_v2, EMPTY_USER_ID, false, -1, None);
            };
            Reply::from_backend(is_v2, state.backend.unblock_user(account_name, user_id))
        }

        "GetBlockedList" => Reply::from_backend(is_v2, state.backend.blocked_list(account_name)),

        "UnblockEveryone" => {
            Reply::from_backend(is_v2, state.backend.unblock_everyone(account_name))
        }

        "SetServer" => {
            let place_id = param(params, "PlaceId").filter(|value| !value.is_empty());
            let job_id = param(params, "JobId").filter(|value| !value.is_empty());

            match (place_id, job_id) {
                (Some(place_id), Some(job_id)) => {
                    let Ok(place_id) = place_id.parse::<i64>() else {
                        return Reply::text(
                            is_v2,
                            INVALID_PLACE_ID,
                            false,
                            -1,
                            Some(INVALID_PLACE_ID_RAW),
                        );
                    };
                    Reply::from_backend(
                        is_v2,
                        state.backend.set_server(account_name, place_id, job_id),
                    )
                }
                _ => Reply::text(is_v2, NOT_FOUND, false, 404, None),
            }
        }

        "SetRecommendedServer" => {
            let place_id = param(params, "PlaceId")
                .filter(|value| !value.is_empty())
                .and_then(|value| value.parse::<i64>().ok());
            Reply::from_backend(
                is_v2,
                state.backend.recommended_server(account_name, place_id),
            )
        }

        "GetField" => {
            let Some(field) = param(params, "Field").filter(|value| !value.is_empty()) else {
                return Reply::text(is_v2, EMPTY_FIELD, false, -1, None);
            };
            let value = snapshot.get_field(field).to_owned();
            Reply::text(is_v2, &value, true, -1, None)
        }

        "SetField" => {
            if !state.allow_account_editing {
                return not_allowed(is_v2, "SetField");
            }
            let field = param(params, "Field").filter(|value| !value.is_empty());
            let value = param(params, "Value").filter(|value| !value.is_empty());

            match (field, value) {
                (Some(field), Some(value)) => {
                    let field = field.to_owned();
                    let value = value.to_owned();
                    let ok = edit_account(state, account_name, |account| {
                        account.set_field(field.clone(), value.clone());
                    });
                    if ok {
                        Reply::text(
                            is_v2,
                            &format!(
                                "Set Field {field} to {value} for {}",
                                snapshot.username.as_deref().unwrap_or_default()
                            ),
                            true,
                            -1,
                            None,
                        )
                    } else {
                        Reply::text(is_v2, "Error updating", false, -1, None)
                    }
                }
                _ => Reply::text(is_v2, EMPTY_FIELD, false, -1, None),
            }
        }

        "RemoveField" => {
            if !state.allow_account_editing {
                return not_allowed(is_v2, "RemoveField");
            }
            let Some(field) = param(params, "Field").filter(|value| !value.is_empty()) else {
                return Reply::text(is_v2, EMPTY_FIELD, false, -1, None);
            };

            let field = field.to_owned();
            let ok = edit_account(state, account_name, |account| {
                account.remove_field(&field);
            });

            if ok {
                Reply::text(
                    is_v2,
                    &format!(
                        "Removed Field {field} from {}",
                        snapshot.username.as_deref().unwrap_or_default()
                    ),
                    true,
                    -1,
                    None,
                )
            } else {
                Reply::text(is_v2, "Error updating", false, -1, None)
            }
        }

        "SetAvatar" => {
            if serde_json::from_str::<serde_json::Value>(body).is_err() {
                return Reply::text(is_v2, NOT_FOUND, false, 404, None);
            }
            Reply::from_backend(is_v2, state.backend.set_avatar(account_name, body))
        }

        "SetAlias" => {
            if !state.allow_account_editing {
                return not_allowed(is_v2, "SetAlias");
            }
            if body.is_empty() {
                return Reply::text(is_v2, NOT_FOUND, false, 404, None);
            }

            let alias = body.to_owned();
            let ok = edit_account(state, account_name, |account| {
                account.alias = alias.clone();
            });

            if ok {
                Reply::text(
                    is_v2,
                    &format!(
                        "Set Alias of {} to {body}",
                        snapshot.username.as_deref().unwrap_or_default()
                    ),
                    true,
                    -1,
                    None,
                )
            } else {
                Reply::text(is_v2, "Error updating", false, -1, None)
            }
        }

        "SetDescription" => {
            if !state.allow_account_editing {
                return not_allowed(is_v2, "SetDescription");
            }
            if body.is_empty() {
                return Reply::text(is_v2, NOT_FOUND, false, 404, None);
            }

            let description = body.to_owned();
            let ok = edit_account(state, account_name, |account| {
                account.description = description.clone();
            });

            if ok {
                Reply::text(
                    is_v2,
                    &format!(
                        "Set Description of {} to {body}",
                        snapshot.username.as_deref().unwrap_or_default()
                    ),
                    true,
                    -1,
                    None,
                )
            } else {
                Reply::text(is_v2, "Error updating", false, -1, None)
            }
        }

        "AppendDescription" => {
            if !state.allow_account_editing {
                return not_allowed(is_v2, "AppendDescription");
            }
            if body.is_empty() {
                return Reply::text(is_v2, NOT_FOUND, false, 404, None);
            }

            let suffix = body.to_owned();
            let ok = edit_account(state, account_name, |account| {
                account.description.push_str(&suffix);
            });

            if ok {
                Reply::text(
                    is_v2,
                    &format!(
                        "Appended Description of {} with {body}",
                        snapshot.username.as_deref().unwrap_or_default()
                    ),
                    true,
                    -1,
                    None,
                )
            } else {
                Reply::text(is_v2, "Error updating", false, -1, None)
            }
        }

        _ => Reply::text(is_v2, NOT_FOUND, false, 404, None),
    }
}

fn not_allowed(is_v2: bool, method: &str) -> Reply {
    Reply::text(
        is_v2,
        &format!("Method `{method}` not allowed"),
        false,
        401,
        Some(METHOD_NOT_ALLOWED_RAW),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    #[derive(Default)]
    struct Recording {
        calls: Mutex<Vec<String>>,
        reply: Option<BackendReply>,
    }

    impl Recording {
        fn answering(reply: BackendReply) -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                reply: Some(reply),
            })
        }

        fn note(&self, call: impl Into<String>) {
            if let Ok(mut calls) = self.calls.lock() {
                calls.push(call.into());
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().map(|c| c.clone()).unwrap_or_default()
        }

        fn answer(&self) -> BackendReply {
            self.reply.clone().unwrap_or_else(|| BackendReply::ok("ok"))
        }
    }

    impl ApiBackend for Recording {
        fn import_cookie(&self, cookie: &str) -> BackendReply {
            self.note(format!("import:{cookie}"));
            self.answer()
        }

        fn csrf_token(&self, account: &str) -> BackendReply {
            self.note(format!("csrf:{account}"));
            self.answer()
        }

        fn join_server(
            &self,
            account: &str,
            place_id: i64,
            job_id: &str,
            follow_user: bool,
            join_vip: bool,
        ) -> BackendReply {
            self.note(format!(
                "join:{account}:{place_id}:{job_id}:{follow_user}:{join_vip}"
            ));
            self.answer()
        }

        fn follow_user(&self, account: &str, username: &str) -> BackendReply {
            self.note(format!("follow:{account}:{username}"));
            self.answer()
        }

        fn set_server(&self, account: &str, place_id: i64, job_id: &str) -> BackendReply {
            self.note(format!("server:{account}:{place_id}:{job_id}"));
            self.answer()
        }

        fn recommended_server(&self, account: &str, place_id: Option<i64>) -> BackendReply {
            self.note(format!("recommended:{account}:{place_id:?}"));
            self.answer()
        }

        fn block_user(&self, account: &str, user_id: &str) -> BackendReply {
            self.note(format!("block:{account}:{user_id}"));
            self.answer()
        }

        fn unblock_user(&self, account: &str, user_id: &str) -> BackendReply {
            self.note(format!("unblock:{account}:{user_id}"));
            self.answer()
        }

        fn blocked_list(&self, account: &str) -> BackendReply {
            self.note(format!("blocked:{account}"));
            self.answer()
        }

        fn unblock_everyone(&self, account: &str) -> BackendReply {
            self.note(format!("unblock-all:{account}"));
            self.answer()
        }

        fn set_avatar(&self, account: &str, body: &str) -> BackendReply {
            self.note(format!("avatar:{account}:{body}"));
            self.answer()
        }
    }

    fn account(username: &str, user_id: i64, group: Option<&str>) -> Account {
        Account {
            valid: true,
            security_token: Some(format!("COOKIE-{username}")),
            username: Some(username.to_owned()),
            last_use: "2001-09-09T01:46:40.512".to_owned(),
            alias: format!("{username} (alias)"),
            description: "ghi chu".to_owned(),
            password: String::new(),
            group: group.map(str::to_owned),
            user_id,
            fields: BTreeMap::from([("Window_Width".to_owned(), "880".to_owned())]),
            last_attempted_refresh: String::new(),
            browser_tracker_id: None,
            extra: BTreeMap::new(),
        }
    }

    fn state_with(accounts: Vec<Account>) -> Arc<ApiState> {
        Arc::new(ApiState {
            store: Arc::new(Mutex::new(AccountStore::from_accounts(accounts))),
            ..ApiState::open()
        })
    }

    async fn call(state: Arc<ApiState>, uri: &str) -> (StatusCode, String, Option<String>) {
        call_with(state, "GET", uri, "").await
    }

    async fn call_with(
        state: Arc<ApiState>,
        method: &str,
        uri: &str,
        body: &str,
    ) -> (StatusCode, String, Option<String>) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::from(body.to_owned()))
            .expect("request");

        let response = router_with_state(state)
            .oneshot(request)
            .await
            .expect("router khong duoc loi");

        let status = response.status();
        let ws_error = response
            .headers()
            .get("ws-error")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();

        (
            status,
            String::from_utf8_lossy(&bytes).into_owned(),
            ws_error,
        )
    }

    #[test]
    fn the_route_table_lists_the_22_methods_of_ram() {
        assert_eq!(METHODS.len(), 22);
        for name in [
            "SetDescription",
            "AppendDescription",
            "SetRecommendedServer",
        ] {
            assert!(METHODS.contains(&name), "thieu {name}");
        }
    }

    #[test]
    fn the_default_port_matches_ram() {
        assert_eq!(DEFAULT_PORT, 7963);
    }

    #[test]
    fn the_default_state_is_locked_down_like_rams_settings() {
        let state = ApiState::new();
        assert!(!state.allow_get_accounts);
        assert!(!state.allow_get_cookie);
        assert!(!state.allow_launch_account);
        assert!(!state.allow_account_editing);
        assert!(!state.allow_external_connections);
        assert!(state.password.is_none());
    }

    #[tokio::test]
    async fn running_returns_plain_true_not_json() {
        let (status, body, _) = call(state_with(vec![]), "/Running").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "true");
    }

    #[tokio::test]
    async fn running_v2_uses_the_pascal_case_envelope() {
        let (status, body, _) = call(state_with(vec![]), "/v2/Running").await;
        assert_eq!(status, StatusCode::OK);

        let value: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(value["Success"], true);
        assert_eq!(value["Message"], RUNNING_MESSAGE);
    }

    #[tokio::test]
    async fn favicon_is_empty() {
        let (status, body, _) = call(state_with(vec![]), "/favicon.ico").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn an_unknown_method_is_a_404_with_the_ram_message() {
        let state = state_with(vec![account("alice", 1, None)]);
        let (status, body, ws_error) = call(state, "/Nope?Account=alice").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, NOT_FOUND);
        assert_eq!(ws_error.as_deref(), Some(NOT_FOUND));
    }

    #[tokio::test]
    async fn the_account_check_happens_before_the_method_lookup_like_ram() {
        let (status, body, _) = call(state_with(vec![]), "/Nope").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, EMPTY_ACCOUNT);
    }

    #[tokio::test]
    async fn a_method_that_is_switched_off_is_refused_with_401() {
        let state = Arc::new(ApiState::new());
        let (status, body, ws_error) = call(state, "/GetAccounts").await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, METHOD_NOT_ALLOWED_RAW);
        assert_eq!(ws_error.as_deref(), Some(METHOD_NOT_ALLOWED_RAW));
    }

    #[tokio::test]
    async fn get_accounts_joins_usernames_with_commas() {
        let state = state_with(vec![account("alice", 1, None), account("bob", 2, None)]);
        let (status, body, _) = call(state, "/GetAccounts").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "alice,bob");
    }

    #[tokio::test]
    async fn get_accounts_with_no_accounts_is_an_empty_body_not_a_crash() {
        let (status, body, _) = call(state_with(vec![]), "/GetAccounts").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn get_accounts_can_be_filtered_by_group() {
        let state = state_with(vec![
            account("alice", 1, Some("1Main")),
            account("bob", 2, Some("007 Bank")),
        ]);
        let (_, body, _) = call(state, "/GetAccounts?Group=007%20Bank").await;
        assert_eq!(body, "bob");
    }

    #[tokio::test]
    async fn get_accounts_v2_wraps_the_joined_names_in_the_envelope() {
        let state = state_with(vec![account("alice", 1, None)]);
        let (_, body, _) = call(state, "/v2/GetAccounts").await;

        let value: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(value["Success"], true);
        assert_eq!(value["Message"], "alice");
    }

    #[tokio::test]
    async fn get_accounts_json_carries_the_ram_fields_and_hides_the_cookie() {
        let state = state_with(vec![account("alice", 100, Some("1Main"))]);
        let (status, body, _) = call(state, "/GetAccountsJson").await;

        assert_eq!(status, StatusCode::OK);
        let value: serde_json::Value = serde_json::from_str(&body).expect("json");
        let first = &value[0];

        assert_eq!(first["Username"], "alice");
        assert_eq!(first["UserID"], 100);
        assert_eq!(first["Alias"], "alice (alias)");
        assert_eq!(first["Group"], "1Main");
        assert!(first["CSRFToken"].is_null());
        assert!(first["Cookie"].is_null(), "cookie phai bi an");
        assert_eq!(first["Fields"]["Window_Width"], "880");

        let last_used = first["LastUsed"].as_f64().expect("so");
        assert!(
            (last_used - 1_000_000_000.512).abs() < 1e-6,
            "gap: {last_used}"
        );
    }

    #[tokio::test]
    async fn get_accounts_json_shows_the_cookie_only_when_both_switches_are_on() {
        let state = state_with(vec![account("alice", 1, None)]);
        let (_, body, _) = call(state, "/GetAccountsJson?IncludeCookies=true").await;

        let value: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(value[0]["Cookie"], "COOKIE-alice");
    }

    #[tokio::test]
    async fn a_password_longer_than_five_characters_is_accepted_on_a_sensitive_method() {
        let state = Arc::new(ApiState {
            password: Some("s3cret".to_owned()),
            ..ApiState::open()
        });

        let (status, body, _) = call(Arc::clone(&state), "/GetCookie?Account=alice").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, INVALID_PASSWORD_RAW, "thieu mat khau phai bi tu choi");
    }

    #[tokio::test]
    async fn the_right_password_lets_a_sensitive_method_through() {
        let mut state = ApiState {
            password: Some("s3cret".to_owned()),
            ..ApiState::open()
        };
        state.store = Arc::new(Mutex::new(AccountStore::from_accounts(vec![account(
            "alice", 1, None,
        )])));
        let state = Arc::new(state);

        let (status, body, _) = call(state, "/GetCookie?Account=alice&Password=s3cret").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "COOKIE-alice");
    }

    #[tokio::test]
    async fn a_password_shorter_than_six_characters_is_refused_even_when_it_matches() {
        let mut state = ApiState {
            password: Some("abc".to_owned()),
            ..ApiState::open()
        };
        state.store = Arc::new(Mutex::new(AccountStore::from_accounts(vec![account(
            "alice", 1, None,
        )])));
        let state = Arc::new(state);

        let (status, body, _) = call(state, "/GetCookie?Account=alice&Password=abc").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, INVALID_PASSWORD_RAW);
    }

    #[tokio::test]
    async fn every_request_requires_password_covers_methods_that_are_not_sensitive() {
        let mut state = ApiState {
            password: Some("s3cret".to_owned()),
            every_request_requires_password: true,
            ..ApiState::open()
        };
        state.store = Arc::new(Mutex::new(AccountStore::from_accounts(vec![account(
            "alice", 1, None,
        )])));
        let state = Arc::new(state);

        let (status, _, _) = call(Arc::clone(&state), "/GetAlias?Account=alice").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, body, _) = call(state, "/GetAlias?Account=alice&Password=s3cret").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "alice (alias)");
    }

    #[tokio::test]
    async fn asking_without_an_account_is_refused() {
        let (status, body, _) = call(state_with(vec![]), "/GetAlias").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, EMPTY_ACCOUNT);
    }

    #[tokio::test]
    async fn an_unknown_account_is_reported_with_the_ram_wording() {
        let (status, body, ws_error) = call(state_with(vec![]), "/GetAlias?Account=ghost").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, INVALID_ACCOUNT_RAW);
        assert_eq!(ws_error.as_deref(), Some(INVALID_ACCOUNT_RAW));
    }

    #[tokio::test]
    async fn an_account_can_be_addressed_by_user_id() {
        let state = state_with(vec![account("alice", 100, None)]);
        let (status, body, _) = call(state, "/GetAlias?Account=100").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "alice (alias)");
    }

    #[tokio::test]
    async fn fields_round_trip_through_get_set_and_remove() {
        let state = state_with(vec![account("alice", 1, None)]);

        let (_, body, _) = call(
            Arc::clone(&state),
            "/GetField?Account=alice&Field=Window_Width",
        )
        .await;
        assert_eq!(body, "880");

        let (status, body, _) = call(
            Arc::clone(&state),
            "/SetField?Account=alice&Field=Window_Width&Value=1024",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("1024"), "gap: {body}");

        let (_, body, _) = call(
            Arc::clone(&state),
            "/GetField?Account=alice&Field=Window_Width",
        )
        .await;
        assert_eq!(body, "1024");

        let (status, _, _) = call(
            Arc::clone(&state),
            "/RemoveField?Account=alice&Field=Window_Width",
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (_, body, _) = call(state, "/GetField?Account=alice&Field=Window_Width").await;
        assert!(body.is_empty(), "field da xoa phai tra ve rong");
    }

    #[tokio::test]
    async fn editing_methods_are_refused_when_account_editing_is_off() {
        let mut state = ApiState {
            allow_account_editing: false,
            ..ApiState::open()
        };
        state.store = Arc::new(Mutex::new(AccountStore::from_accounts(vec![account(
            "alice", 1, None,
        )])));
        let state = Arc::new(state);

        let (status, body, _) = call(state, "/SetField?Account=alice&Field=X&Value=1").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, METHOD_NOT_ALLOWED_RAW);
    }

    #[tokio::test]
    async fn set_alias_reads_the_body_and_writes_it_through() {
        let state = state_with(vec![account("alice", 1, None)]);

        let (status, _, _) = call_with(
            Arc::clone(&state),
            "POST",
            "/SetAlias?Account=alice",
            "Main Farm",
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (_, body, _) = call(state, "/GetAlias?Account=alice").await;
        assert_eq!(body, "Main Farm");
    }

    #[tokio::test]
    async fn set_description_replaces_while_append_description_adds() {
        let state = state_with(vec![account("alice", 1, None)]);

        call_with(
            Arc::clone(&state),
            "POST",
            "/SetDescription?Account=alice",
            "A",
        )
        .await;
        call_with(
            Arc::clone(&state),
            "POST",
            "/AppendDescription?Account=alice",
            "B",
        )
        .await;

        let (_, body, _) = call(state, "/GetDescription?Account=alice").await;
        assert_eq!(body, "AB");
    }

    #[tokio::test]
    async fn launch_account_parses_the_place_and_passes_the_switches() {
        let backend = Recording::answering(BackendReply::ok(""));
        let mut state = ApiState::open();
        state.store = Arc::new(Mutex::new(AccountStore::from_accounts(vec![account(
            "alice", 1, None,
        )])));
        let state = Arc::new(state.with_backend(backend.clone()));

        let (status, body, _) = call(
            state,
            "/LaunchAccount?Account=alice&PlaceId=55&JobId=JOB&FollowUser=true&JoinVIP=true",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "Launched alice to 55");
        assert_eq!(backend.calls(), ["join:alice:55:JOB:true:true"]);
    }

    #[tokio::test]
    async fn launch_account_needs_a_numeric_place_id() {
        let state = state_with(vec![account("alice", 1, None)]);
        let (status, body, _) = call(state, "/LaunchAccount?Account=alice&PlaceId=abc").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, INVALID_PLACE_ID_RAW);
    }

    #[tokio::test]
    async fn follow_user_needs_a_username() {
        let state = state_with(vec![account("alice", 1, None)]);
        let (status, body, _) = call(state, "/FollowUser?Account=alice").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, INVALID_USERNAME);
    }

    #[tokio::test]
    async fn set_server_needs_both_place_and_job() {
        let backend = Recording::answering(BackendReply::ok("done"));
        let mut state = ApiState::open();
        state.store = Arc::new(Mutex::new(AccountStore::from_accounts(vec![account(
            "alice", 1, None,
        )])));
        let state = Arc::new(state.with_backend(backend.clone()));

        let (status, _, _) = call(Arc::clone(&state), "/SetServer?Account=alice&PlaceId=1").await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _, _) = call(state, "/SetServer?Account=alice&PlaceId=1&JobId=J").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(backend.calls(), ["server:alice:1:J"]);
    }

    #[tokio::test]
    async fn set_recommended_server_passes_an_optional_place() {
        let backend = Recording::answering(BackendReply::ok("ok"));
        let mut state = ApiState::open();
        state.store = Arc::new(Mutex::new(AccountStore::from_accounts(vec![account(
            "alice", 1, None,
        )])));
        let state = Arc::new(state.with_backend(backend.clone()));

        call(
            Arc::clone(&state),
            "/SetRecommendedServer?Account=alice&PlaceId=9",
        )
        .await;
        call(state, "/SetRecommendedServer?Account=alice").await;

        assert_eq!(
            backend.calls(),
            ["recommended:alice:Some(9)", "recommended:alice:None"]
        );
    }

    #[tokio::test]
    async fn block_and_unblock_need_a_user_id_and_reach_the_backend() {
        let backend = Recording::answering(BackendReply::ok("{}").with_code(200));
        let mut state = ApiState::open();
        state.store = Arc::new(Mutex::new(AccountStore::from_accounts(vec![account(
            "alice", 1, None,
        )])));
        let state = Arc::new(state.with_backend(backend.clone()));

        let (status, _, _) = call(Arc::clone(&state), "/BlockUser?Account=alice").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        call(Arc::clone(&state), "/BlockUser?Account=alice&UserId=7").await;
        call(Arc::clone(&state), "/UnblockUser?Account=alice&UserId=7").await;
        call(Arc::clone(&state), "/GetBlockedList?Account=alice").await;
        call(state, "/UnblockEveryone?Account=alice").await;

        assert_eq!(
            backend.calls(),
            [
                "block:alice:7",
                "unblock:alice:7",
                "blocked:alice",
                "unblock-all:alice"
            ]
        );
    }

    #[tokio::test]
    async fn set_avatar_needs_a_json_body() {
        let backend = Recording::answering(BackendReply::ok("ok"));
        let mut state = ApiState::open();
        state.store = Arc::new(Mutex::new(AccountStore::from_accounts(vec![account(
            "alice", 1, None,
        )])));
        let state = Arc::new(state.with_backend(backend.clone()));

        let (status, _, _) = call_with(
            Arc::clone(&state),
            "POST",
            "/SetAvatar?Account=alice",
            "khong-phai-json",
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _, _) = call_with(
            Arc::clone(&state),
            "POST",
            "/SetAvatar?Account=alice",
            "{\"assetId\":1}",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(backend.calls(), ["avatar:alice:{\"assetId\":1}"]);
    }

    #[tokio::test]
    async fn get_csrf_token_goes_through_the_backend() {
        let backend = Recording::answering(BackendReply::ok("TOKEN"));
        let mut state = ApiState::open();
        state.store = Arc::new(Mutex::new(AccountStore::from_accounts(vec![account(
            "alice", 1, None,
        )])));
        let state = Arc::new(state.with_backend(backend.clone()));

        let (status, body, _) = call(state, "/GetCSRFToken?Account=alice").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "TOKEN");
        assert_eq!(backend.calls(), ["csrf:alice"]);
    }

    #[tokio::test]
    async fn import_cookie_returns_the_ram_true_false_body() {
        let backend = Recording::answering(BackendReply::ok("Cookie successfully imported"));
        let state = Arc::new(ApiState::open().with_backend(backend.clone()));

        let (status, body, _) = call(state, "/ImportCookie?Cookie=ABC").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "true");
        assert_eq!(backend.calls(), ["import:ABC"]);
    }

    #[tokio::test]
    async fn a_backend_that_is_not_configured_says_so_instead_of_pretending() {
        let state = state_with(vec![account("alice", 1, None)]);
        let (status, body, _) = call(state, "/GetCSRFToken?Account=alice").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, NOT_CONFIGURED);
    }

    #[tokio::test]
    async fn query_parameters_are_matched_without_regard_to_case() {
        let state = state_with(vec![account("alice", 1, None)]);
        let (status, body, _) = call(state, "/GetAlias?account=alice").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "alice (alias)");
    }

    #[tokio::test]
    async fn the_content_type_stays_plain_text_even_for_json_replies() {
        let state = state_with(vec![account("alice", 1, None)]);
        let request = Request::builder()
            .uri("/v2/GetAccounts")
            .body(Body::empty())
            .expect("request");

        let response = router_with_state(state)
            .oneshot(request)
            .await
            .expect("router");

        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/plain; charset=utf-8")
        );
    }
}
