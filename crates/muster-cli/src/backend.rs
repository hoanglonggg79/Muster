use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use muster_api::{ApiBackend, BackendReply};
use muster_core::datetime::{now_local_json_net, now_unix_millis};
use muster_core::launch::windows::{SystemClientControl, launch_roblox_url, list_running_clients};
use muster_core::launch::{
    LaunchTarget, build_launch_url, close_previous_instances, generate_browser_tracker_id,
};
use muster_core::models::Account;
use muster_core::roblox::account::AccountStore;
use muster_core::roblox::account_ext;
use muster_core::roblox::api::{self, fetch_user_avatar_headshot, fetch_user_robux};
use muster_core::roblox::auth::{self, AuthClient};
use muster_core::roblox::limiter::{RateLimiter, RetryTransport};
use muster_core::roblox::reqwest_transport::ReqwestTransport;
use muster_core::roblox::servers::{self, ServerPick};
use muster_core::roblox::vip::{extract_link_code, fetch_access_code, strip_vip_prefix};

pub const HTTP_MIN_INTERVAL_MS: u64 = 200;
pub const SAVED_PLACE_ID_FIELD: &str = "SavedPlaceId";
pub const SAVED_JOB_ID_FIELD: &str = "SavedJobId";
pub const SHUFFLE_PAGES: u32 = 5;

pub struct LiveBackend {
    store: Arc<Mutex<AccountStore>>,
    limiter: Arc<RateLimiter>,
}

impl LiveBackend {
    #[must_use]
    pub fn new(store: Arc<Mutex<AccountStore>>) -> Self {
        Self {
            store,
            limiter: Arc::new(RateLimiter::new(Duration::from_millis(
                HTTP_MIN_INTERVAL_MS,
            ))),
        }
    }

    fn transport(&self) -> Result<RetryTransport<ReqwestTransport>, String> {
        ReqwestTransport::new()
            .map(|transport| RetryTransport::new(transport).with_limiter(Arc::clone(&self.limiter)))
            .map_err(|error| error.to_string())
    }

    fn index_of(store: &AccountStore, account: &str) -> Option<usize> {
        if let Ok(user_id) = account.parse::<i64>() {
            if let Some(index) = store.find_by_user_id(user_id) {
                return Some(index);
            }
        }
        store.find_by_username(account)
    }

    fn cookie_of(&self, account: &str) -> Result<String, String> {
        let store = self.store.lock().map_err(|error| error.to_string())?;
        let index = Self::index_of(&store, account)
            .ok_or_else(|| format!("không tìm thấy tài khoản {account}"))?;

        store
            .get(index)
            .map_err(|error| error.to_string())?
            .cookie()
            .map(str::to_owned)
            .ok_or_else(|| "tài khoản không có cookie".to_owned())
    }

    fn tracker_of(&self, account: &str) -> Result<String, String> {
        let mut store = self.store.lock().map_err(|error| error.to_string())?;
        let index = Self::index_of(&store, account)
            .ok_or_else(|| format!("không tìm thấy tài khoản {account}"))?;
        let entry = store.get_mut(index).map_err(|error| error.to_string())?;

        if let Some(tracker) = entry
            .browser_tracker_id
            .clone()
            .filter(|value| !value.is_empty())
        {
            return Ok(tracker);
        }

        let tracker = generate_browser_tracker_id().map_err(|error| error.to_string())?;
        entry.browser_tracker_id = Some(tracker.clone());
        Ok(tracker)
    }

    fn session(
        &self,
        account: &str,
    ) -> Result<(String, String, RetryTransport<ReqwestTransport>), String> {
        let cookie = self.cookie_of(account)?;
        let transport = self.transport()?;
        let csrf = AuthClient::new(&transport)
            .csrf_token(&cookie)
            .map_err(|error| error.to_string())?;
        Ok((cookie, csrf, transport))
    }

    fn set_saved_server(&self, account: &str, place_id: Option<i64>, job_id: Option<&str>) -> bool {
        let Ok(mut store) = self.store.lock() else {
            return false;
        };
        let Some(index) = Self::index_of(&store, account) else {
            return false;
        };
        let Ok(entry) = store.get_mut(index) else {
            return false;
        };

        match place_id {
            Some(place_id) => entry.set_field(SAVED_PLACE_ID_FIELD, place_id.to_string()),
            None => {
                entry.remove_field(SAVED_PLACE_ID_FIELD);
            }
        }
        match job_id.filter(|value| !value.is_empty()) {
            Some(job_id) => entry.set_field(SAVED_JOB_ID_FIELD, job_id.to_owned()),
            None => {
                entry.remove_field(SAVED_JOB_ID_FIELD);
            }
        }

        true
    }

    fn block(&self, account: &str, target: i64, unblock: bool) -> BackendReply {
        let (cookie, csrf, transport) = match self.session(account) {
            Ok(session) => session,
            Err(error) => return BackendReply::failed(error),
        };

        match account_ext::block_user(&transport, &cookie, &csrf, target, unblock) {
            Ok(status) => BackendReply {
                body: status.to_string(),
                success: (200..300).contains(&status),
                code: Some(status),
            },
            Err(error) => BackendReply::failed(error.to_string()),
        }
    }

    fn launch(&self, account: &str, target: LaunchTarget, tracker: &str) -> Result<(), String> {
        let (cookie, csrf, transport) = self.session(account)?;
        let ticket = AuthClient::new(&transport)
            .auth_ticket_with_csrf(&cookie, &csrf)
            .map_err(|error| error.to_string())?;

        let clients = list_running_clients();
        let mut control = SystemClientControl;
        close_previous_instances(&mut control, &clients, tracker);

        let url = build_launch_url(ticket.as_str(), &target, tracker, now_unix_millis(), false);
        launch_roblox_url(&url).map_err(|error| error.to_string())
    }
}

impl ApiBackend for LiveBackend {
    fn import_cookie(&self, cookie: &str) -> BackendReply {
        let cookie = auth::sanitize_cookie(cookie);
        if cookie.is_empty() {
            return BackendReply::failed("Empty cookie");
        }

        let transport = match self.transport() {
            Ok(transport) => transport,
            Err(error) => return BackendReply::failed(error),
        };

        let user = match api::fetch_authenticated_user(&transport, &cookie) {
            Ok(user) => user,
            Err(error) => return BackendReply::failed(error.to_string()),
        };

        let robux = fetch_user_robux(&transport, user.id, &cookie).unwrap_or(0);
        let avatar = fetch_user_avatar_headshot(&transport, user.id).unwrap_or(None);
        let tracker = match generate_browser_tracker_id() {
            Ok(tracker) => tracker,
            Err(error) => return BackendReply::failed(error.to_string()),
        };

        let mut fields = BTreeMap::new();
        fields.insert("DisplayName".to_owned(), user.display_name.clone());
        fields.insert("Robux".to_owned(), robux.to_string());
        if let Some(url) = avatar {
            fields.insert("AvatarUrl".to_owned(), url);
        }

        let account = Account {
            valid: true,
            security_token: Some(cookie),
            username: Some(user.name),
            last_use: now_local_json_net(),
            alias: user.display_name,
            description: String::new(),
            password: String::new(),
            group: None,
            user_id: user.id,
            fields,
            last_attempted_refresh: now_local_json_net(),
            browser_tracker_id: Some(tracker),
            extra: BTreeMap::new(),
        };

        let Ok(mut store) = self.store.lock() else {
            return BackendReply::failed("store bị khoá");
        };

        match store.add(account) {
            Ok(_) => BackendReply::ok("Cookie successfully imported"),
            Err(error) => BackendReply::failed(error.to_string()),
        }
    }

    fn csrf_token(&self, account: &str) -> BackendReply {
        let cookie = match self.cookie_of(account) {
            Ok(cookie) => cookie,
            Err(error) => return BackendReply::failed(error),
        };
        let transport = match self.transport() {
            Ok(transport) => transport,
            Err(error) => return BackendReply::failed(error),
        };

        match AuthClient::new(&transport).csrf_token(&cookie) {
            Ok(token) => BackendReply::ok(token),
            Err(error) => BackendReply::failed(error.to_string()),
        }
    }

    fn join_server(
        &self,
        account: &str,
        place_id: i64,
        job_id: &str,
        follow_user: bool,
        join_vip: bool,
    ) -> BackendReply {
        let tracker = match self.tracker_of(account) {
            Ok(tracker) => tracker,
            Err(error) => return BackendReply::failed(error),
        };

        let job = job_id.trim();
        let target = if follow_user {
            LaunchTarget::FollowUser { user_id: place_id }
        } else if join_vip && !job.is_empty() {
            let (cookie, csrf, transport) = match self.session(account) {
                Ok(session) => session,
                Err(error) => return BackendReply::failed(error),
            };
            match extract_link_code(job) {
                Some(link_code) => {
                    match fetch_access_code(&transport, place_id, link_code, &csrf, &cookie) {
                        Ok(access_code) => LaunchTarget::PrivateServer {
                            place_id,
                            access_code,
                            link_code: link_code.to_owned(),
                        },
                        Err(error) => return BackendReply::failed(error.to_string()),
                    }
                }
                None => LaunchTarget::Game {
                    place_id,
                    job_id: Some(job.to_owned()),
                },
            }
        } else {
            LaunchTarget::Game {
                place_id,
                job_id: (!job.is_empty()).then(|| strip_vip_prefix(job).to_owned()),
            }
        };

        match self.launch(account, target, &tracker) {
            Ok(()) => BackendReply::ok(format!("Launched {account} to {place_id}")),
            Err(error) => BackendReply::failed(error),
        }
    }

    fn follow_user(&self, account: &str, username: &str) -> BackendReply {
        let transport = match self.transport() {
            Ok(transport) => transport,
            Err(error) => return BackendReply::failed(error),
        };

        let user_id = match api::fetch_user_id_by_username(&transport, username) {
            Ok(Some(user_id)) => user_id,
            Ok(None) => {
                return BackendReply::failed(format!("Failed to obtain UserId of {username}"));
            }
            Err(error) => return BackendReply::failed(error.to_string()),
        };

        let tracker = match self.tracker_of(account) {
            Ok(tracker) => tracker,
            Err(error) => return BackendReply::failed(error),
        };

        match self.launch(account, LaunchTarget::FollowUser { user_id }, &tracker) {
            Ok(()) => BackendReply::ok(format!("Joining {username}'s game on {account}")),
            Err(error) => BackendReply::failed(error),
        }
    }

    fn set_server(&self, account: &str, place_id: i64, job_id: &str) -> BackendReply {
        if self.set_saved_server(account, Some(place_id), Some(job_id)) {
            BackendReply::ok("Success")
        } else {
            BackendReply::failed("Invalid Account")
        }
    }

    fn recommended_server(&self, account: &str, place_id: Option<i64>) -> BackendReply {
        let resolved_place = match place_id {
            Some(place_id) => Some(place_id),
            None => {
                let Ok(store) = self.store.lock() else {
                    return BackendReply::failed("store bị khoá");
                };
                Self::index_of(&store, account)
                    .and_then(|index| store.get(index).ok())
                    .and_then(|entry| entry.get_field(SAVED_PLACE_ID_FIELD).parse::<i64>().ok())
            }
        };

        let Some(place_id) = resolved_place else {
            return BackendReply::failed("Invalid PlaceId");
        };

        let transport = match self.transport() {
            Ok(transport) => transport,
            Err(error) => return BackendReply::failed(error),
        };

        let job = match servers::choose_job_id(
            &transport,
            place_id,
            SHUFFLE_PAGES,
            ServerPick::LowestPlayers,
            |count| servers::random_index_below(count).unwrap_or(0),
        ) {
            Ok(job) => job,
            Err(error) => return BackendReply::failed(error.to_string()),
        };

        match job {
            Some(job) => {
                self.set_saved_server(account, Some(place_id), Some(&job));
                BackendReply::ok("Success")
            }
            None => BackendReply::failed("No joinable server found"),
        }
    }

    fn block_user(&self, account: &str, user_id: &str) -> BackendReply {
        match user_id.parse::<i64>() {
            Ok(target) => self.block(account, target, false),
            Err(_) => BackendReply::failed("Empty UserId"),
        }
    }

    fn unblock_user(&self, account: &str, user_id: &str) -> BackendReply {
        match user_id.parse::<i64>() {
            Ok(target) => self.block(account, target, true),
            Err(_) => BackendReply::failed("Empty UserId"),
        }
    }

    fn blocked_list(&self, account: &str) -> BackendReply {
        let cookie = match self.cookie_of(account) {
            Ok(cookie) => cookie,
            Err(error) => return BackendReply::failed(error),
        };
        let transport = match self.transport() {
            Ok(transport) => transport,
            Err(error) => return BackendReply::failed(error),
        };

        match account_ext::blocked_list(&transport, &cookie) {
            Ok(body) => BackendReply::ok(body),
            Err(error) => BackendReply::failed(error.to_string()),
        }
    }

    fn unblock_everyone(&self, account: &str) -> BackendReply {
        let (cookie, csrf, transport) = match self.session(account) {
            Ok(session) => session,
            Err(error) => return BackendReply::failed(error),
        };

        match account_ext::unblock_everyone(&transport, &cookie, &csrf) {
            Ok(_) => BackendReply::ok("Unblocking Everyone"),
            Err(error) => BackendReply::failed(error.to_string()),
        }
    }

    fn set_avatar(&self, account: &str, body: &str) -> BackendReply {
        let (cookie, csrf, transport) = match self.session(account) {
            Ok(session) => session,
            Err(error) => return BackendReply::failed(error),
        };

        match account_ext::set_avatar(&transport, &cookie, &csrf, body) {
            Ok(outcome) => {
                let ids = outcome
                    .invalid_asset_ids
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                BackendReply::ok(format!("{{\"invalidAssetIds\":[{ids}]}}"))
            }
            Err(error) => BackendReply::failed(error.to_string()),
        }
    }
}
