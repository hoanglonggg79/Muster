use serde::{Deserialize, Serialize};

use super::http::{HttpMethod, HttpRequest, Transport, TransportError};

pub const GAMES_HOST: &str = "https://games.roblox.com";
pub const PUBLIC_SERVERS_LIMIT: u32 = 100;
pub const VIP_SERVERS_LIMIT: u32 = 25;
pub const DEFAULT_SHUFFLE_PAGES: u32 = 5;
pub const MAX_SHUFFLE_PAGES: u32 = 20;

#[derive(Debug, thiserror::Error)]
pub enum ServersError {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error("lấy danh sách server thất bại: HTTP {status}")]
    Failed { status: u16 },

    #[error("phản hồi server không đúng cấu trúc: {0}")]
    Json(#[from] serde_json::Error),
}

fn lenient_number<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.parse().ok(),
        _ => None,
    }))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct ServerInfo {
    pub id: String,

    #[serde(rename = "maxPlayers")]
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub max_players: i64,

    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub playing: i64,

    #[serde(default, deserialize_with = "lenient_number")]
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub fps: Option<f64>,

    #[serde(default, deserialize_with = "lenient_number")]
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub ping: Option<f64>,

    pub region: Option<String>,

    #[serde(rename = "playerTokens")]
    pub player_tokens: Vec<String>,

    #[serde(rename = "vipServerId")]
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub vip_server_id: Option<i64>,

    #[serde(rename = "accessCode")]
    pub access_code: Option<String>,
}

impl ServerInfo {
    #[must_use]
    pub fn free_slots(&self) -> i64 {
        (self.max_players - self.playing).max(0)
    }

    #[must_use]
    pub fn is_joinable(&self) -> bool {
        self.playing != self.max_players && self.playing > 0 && self.max_players > 1
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct ServersPage {
    #[serde(rename = "previousPageCursor")]
    pub previous_page_cursor: Option<String>,

    #[serde(rename = "nextPageCursor")]
    pub next_page_cursor: Option<String>,

    pub data: Vec<ServerInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerPick {
    Random,
    LowestPlayers,
}

#[must_use]
pub fn public_servers_url(place_id: i64, cursor: Option<&str>) -> String {
    let mut url = format!(
        "{GAMES_HOST}/v1/games/{place_id}/servers/public?sortOrder=Asc&limit={PUBLIC_SERVERS_LIMIT}"
    );
    if let Some(cursor) = cursor.filter(|cursor| !cursor.is_empty()) {
        url.push_str("&cursor=");
        url.push_str(cursor);
    }
    url
}

#[must_use]
pub fn vip_servers_url(place_id: i64, cursor: Option<&str>) -> String {
    let mut url = format!(
        "{GAMES_HOST}/v1/games/{place_id}/servers/VIP?sortOrder=Asc&limit={VIP_SERVERS_LIMIT}"
    );
    if let Some(cursor) = cursor.filter(|cursor| !cursor.is_empty()) {
        url.push_str("&cursor=");
        url.push_str(cursor);
    }
    url
}

fn read_page<T: Transport>(
    transport: &T,
    url: String,
    cookie: Option<&str>,
) -> Result<ServersPage, ServersError> {
    let mut request = HttpRequest::new(HttpMethod::Get, url);
    if let Some(cookie) = cookie.filter(|cookie| !cookie.is_empty()) {
        request = request
            .with_header("Cookie", format!(".ROBLOSECURITY={cookie}"))
            .with_header("Accept", "application/json");
    }

    let response = transport.execute(&request)?;
    if !(200..300).contains(&response.status) {
        return Err(ServersError::Failed {
            status: response.status,
        });
    }

    Ok(serde_json::from_slice(&response.body)?)
}

pub fn fetch_public_servers<T: Transport>(
    transport: &T,
    place_id: i64,
    cursor: Option<&str>,
) -> Result<ServersPage, ServersError> {
    read_page(transport, public_servers_url(place_id, cursor), None)
}

pub fn fetch_vip_servers<T: Transport>(
    transport: &T,
    place_id: i64,
    cursor: Option<&str>,
    cookie: &str,
) -> Result<ServersPage, ServersError> {
    read_page(transport, vip_servers_url(place_id, cursor), Some(cookie))
}

pub fn fetch_all_public_servers<T: Transport>(
    transport: &T,
    place_id: i64,
    max_pages: u32,
) -> Result<Vec<ServerInfo>, ServersError> {
    let stop_at = max_pages.clamp(1, MAX_SHUFFLE_PAGES);
    let mut servers = Vec::new();
    let mut cursor: Option<String> = None;

    for _ in 0..stop_at {
        let page = fetch_public_servers(transport, place_id, cursor.as_deref())?;
        servers.extend(page.data);

        match page.next_page_cursor.filter(|value| !value.is_empty()) {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }

    Ok(servers)
}

#[must_use]
pub fn random_index_below(max: usize) -> Option<usize> {
    if max == 0 {
        return None;
    }

    let mut bytes = [0u8; 8];
    getrandom::getrandom(&mut bytes).ok()?;

    Some((u64::from_le_bytes(bytes) % max as u64) as usize)
}

#[must_use]
pub fn pick_job_id(
    servers: &[ServerInfo],
    pick: ServerPick,
    random_index: impl FnOnce(usize) -> usize,
) -> Option<String> {
    let joinable: Vec<&ServerInfo> = servers
        .iter()
        .filter(|server| server.is_joinable())
        .collect();

    if joinable.is_empty() {
        return None;
    }

    let chosen = match pick {
        ServerPick::LowestPlayers => joinable
            .iter()
            .min_by_key(|server| (server.playing, server.free_slots()))
            .copied(),
        ServerPick::Random => {
            let index = random_index(joinable.len()) % joinable.len();
            joinable.get(index).copied()
        }
    }?;

    Some(chosen.id.clone())
}

pub fn choose_job_id<T: Transport>(
    transport: &T,
    place_id: i64,
    max_pages: u32,
    pick: ServerPick,
    random_index: impl FnOnce(usize) -> usize,
) -> Result<Option<String>, ServersError> {
    let servers = fetch_all_public_servers(transport, place_id, max_pages)?;
    Ok(pick_job_id(&servers, pick, random_index))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::super::http::HttpResponse;
    use super::*;

    #[derive(Default)]
    struct Mock {
        responses: RefCell<VecDeque<HttpResponse>>,
        seen: RefCell<Vec<HttpRequest>>,
    }

    impl Mock {
        fn new(responses: Vec<HttpResponse>) -> Self {
            Self {
                responses: RefCell::new(responses.into()),
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl Transport for Mock {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            self.seen.borrow_mut().push(request.clone());
            self.responses
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| TransportError::new("het kich ban"))
        }
    }

    fn json(body: &str) -> HttpResponse {
        HttpResponse::new(200).with_body(body.as_bytes().to_vec())
    }

    fn server(id: &str, playing: i64, max_players: i64) -> ServerInfo {
        ServerInfo {
            id: id.to_owned(),
            playing,
            max_players,
            ..ServerInfo::default()
        }
    }

    #[test]
    fn the_public_url_matches_ram_and_appends_the_cursor_only_when_present() {
        assert_eq!(
            public_servers_url(123, None),
            "https://games.roblox.com/v1/games/123/servers/public?sortOrder=Asc&limit=100"
        );
        assert_eq!(
            public_servers_url(123, Some("CUR")),
            "https://games.roblox.com/v1/games/123/servers/public?sortOrder=Asc&limit=100&cursor=CUR"
        );
        assert_eq!(
            public_servers_url(123, Some("")),
            "https://games.roblox.com/v1/games/123/servers/public?sortOrder=Asc&limit=100",
            "cursor rong phai bi bo qua"
        );
    }

    #[test]
    fn the_vip_url_uses_the_smaller_limit() {
        assert_eq!(
            vip_servers_url(9, None),
            "https://games.roblox.com/v1/games/9/servers/VIP?sortOrder=Asc&limit=25"
        );
    }

    #[test]
    fn a_server_is_joinable_only_when_it_has_room_and_is_not_empty() {
        assert!(server("a", 5, 10).is_joinable());
        assert!(!server("b", 10, 10).is_joinable(), "day");
        assert!(!server("c", 0, 10).is_joinable(), "rong");
        assert!(!server("d", 1, 1).is_joinable(), "maxPlayers phai > 1");
    }

    #[test]
    fn free_slots_never_goes_negative() {
        assert_eq!(server("a", 3, 10).free_slots(), 7);
        assert_eq!(server("b", 12, 10).free_slots(), 0);
    }

    #[test]
    fn a_page_reads_fps_and_ping_whether_they_arrive_as_numbers_or_strings() {
        let body = r#"{"data":[
            {"id":"a","playing":1,"maxPlayers":10,"fps":59.8,"ping":42,"region":"Singapore"},
            {"id":"b","playing":2,"maxPlayers":10,"fps":"60","ping":"30"}
        ]}"#;
        let mock = Mock::new(vec![json(body)]);

        let page = fetch_public_servers(&mock, 1, None).expect("tai");

        assert_eq!(page.data.len(), 2);
        assert_eq!(page.data[0].fps, Some(59.8));
        assert_eq!(page.data[0].ping, Some(42.0));
        assert_eq!(page.data[0].region.as_deref(), Some("Singapore"));
        assert_eq!(page.data[1].fps, Some(60.0));
        assert_eq!(page.data[1].ping, Some(30.0));
    }

    #[test]
    fn paging_follows_the_next_cursor_and_stops_when_it_disappears() {
        let mock = Mock::new(vec![
            json(r#"{"nextPageCursor":"P2","data":[{"id":"s1","playing":1,"maxPlayers":10}]}"#),
            json(r#"{"nextPageCursor":"P3","data":[{"id":"s2","playing":2,"maxPlayers":10}]}"#),
            json(r#"{"data":[{"id":"s3","playing":3,"maxPlayers":10}]}"#),
        ]);

        let servers = fetch_all_public_servers(&mock, 1, 5).expect("tai");

        assert_eq!(servers.len(), 3);
        assert_eq!(
            servers.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["s1", "s2", "s3"]
        );

        let seen = mock.seen.borrow();
        assert_eq!(seen.len(), 3, "dung 3 trang");
        assert!(seen[1].url.contains("cursor=P2"));
        assert!(seen[2].url.contains("cursor=P3"));
    }

    #[test]
    fn the_page_cap_is_respected() {
        let mock = Mock::new(vec![
            json(r#"{"nextPageCursor":"P2","data":[{"id":"s1","playing":1,"maxPlayers":10}]}"#),
            json(r#"{"nextPageCursor":"P3","data":[{"id":"s2","playing":2,"maxPlayers":10}]}"#),
        ]);

        let servers = fetch_all_public_servers(&mock, 1, 1).expect("tai");

        assert_eq!(servers.len(), 1);
        assert_eq!(mock.seen.borrow().len(), 1);
    }

    #[test]
    fn a_zero_page_cap_still_fetches_one_page() {
        let mock = Mock::new(vec![json(
            r#"{"data":[{"id":"s1","playing":1,"maxPlayers":10}]}"#,
        )]);
        let servers = fetch_all_public_servers(&mock, 1, 0).expect("tai");
        assert_eq!(servers.len(), 1);
    }

    #[test]
    fn the_lowest_players_strategy_picks_the_emptiest_server() {
        let servers = vec![
            server("full", 10, 10),
            server("busy", 9, 10),
            server("quiet", 2, 10),
            server("empty", 0, 10),
        ];

        let chosen = pick_job_id(&servers, ServerPick::LowestPlayers, |_| 0);
        assert_eq!(
            chosen.as_deref(),
            Some("quiet"),
            "bo qua server day va server rong"
        );
    }

    #[test]
    fn the_random_strategy_uses_the_index_it_is_given() {
        let servers = vec![server("a", 1, 10), server("b", 2, 10), server("c", 3, 10)];

        assert_eq!(
            pick_job_id(&servers, ServerPick::Random, |_| 0).as_deref(),
            Some("a")
        );
        assert_eq!(
            pick_job_id(&servers, ServerPick::Random, |_| 2).as_deref(),
            Some("c")
        );
        assert_eq!(
            pick_job_id(&servers, ServerPick::Random, |_| 99).as_deref(),
            Some("a"),
            "chi so ngoai pham vi phai duoc mod lai"
        );
    }

    #[test]
    fn no_joinable_server_yields_no_job_id() {
        let servers = vec![server("full", 10, 10), server("empty", 0, 10)];
        assert_eq!(pick_job_id(&servers, ServerPick::Random, |_| 0), None);
        assert_eq!(pick_job_id(&[], ServerPick::LowestPlayers, |_| 0), None);
    }

    #[test]
    fn choosing_a_job_id_fetches_then_picks() {
        let mock = Mock::new(vec![json(
            r#"{"data":[
                {"id":"full","playing":10,"maxPlayers":10},
                {"id":"good","playing":4,"maxPlayers":10}
            ]}"#,
        )]);

        let chosen = choose_job_id(&mock, 55, 1, ServerPick::LowestPlayers, |_| 0).expect("chon");

        assert_eq!(chosen.as_deref(), Some("good"));
        assert!(
            mock.seen.borrow()[0]
                .url
                .contains("/v1/games/55/servers/public")
        );
    }

    #[test]
    fn the_vip_request_carries_the_cookie() {
        let mock = Mock::new(vec![json(r#"{"data":[]}"#)]);
        fetch_vip_servers(&mock, 1, None, "COOKIE").expect("tai");

        let seen = mock.seen.borrow();
        assert_eq!(seen[0].header("cookie"), Some(".ROBLOSECURITY=COOKIE"));
        assert!(seen[0].url.contains("/servers/VIP"));
    }

    #[test]
    fn a_failed_page_is_reported_with_its_status() {
        let mock = Mock::new(vec![HttpResponse::new(403)]);
        let error = fetch_public_servers(&mock, 1, None).expect_err("phai loi");
        assert!(matches!(error, ServersError::Failed { status: 403 }));
    }

    #[test]
    fn a_random_index_always_stays_inside_the_range() {
        assert_eq!(random_index_below(0), None);

        for _ in 0..200 {
            let index = random_index_below(7).expect("co so ngau nhien");
            assert!(index < 7, "chi so {index} ngoai pham vi");
        }

        assert_eq!(random_index_below(1), Some(0));
    }
}
