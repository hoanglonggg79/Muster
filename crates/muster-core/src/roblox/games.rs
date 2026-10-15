use std::path::Path;

use serde::{Deserialize, Serialize};

use super::http::{HttpMethod, HttpRequest, Transport, TransportError};

pub const GAMES_HOST: &str = "https://games.roblox.com";
pub const FAVORITES_FILE: &str = "FavoriteGames.json";
pub const RECENT_GAMES_FILE: &str = "RecentGames.json";
pub const DEFAULT_MAX_RECENT_GAMES: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum GamesError {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error("gọi API game thất bại: HTTP {status}")]
    Failed { status: u16 },

    #[error("phản hồi game không đúng cấu trúc: {0}")]
    Json(#[from] serde_json::Error),

    #[error("lỗi vào/ra: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct GameDetails {
    #[serde(rename = "placeId")]
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub place_id: i64,

    #[serde(rename = "universeId")]
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub universe_id: i64,

    pub name: String,

    #[serde(rename = "sourceName")]
    pub source_name: String,

    #[serde(rename = "sourceDescription")]
    pub source_description: String,

    pub description: String,

    pub builder: String,

    #[serde(rename = "builderId")]
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub builder_id: i64,

    #[serde(rename = "hasVerifiedBadge")]
    pub has_verified_badge: bool,

    #[serde(rename = "isPlayable")]
    pub is_playable: bool,

    #[serde(rename = "reasonProhibited")]
    pub reason_prohibited: Option<String>,

    #[serde(rename = "universeRootPlaceId")]
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub universe_root_place_id: i64,

    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub price: i64,

    pub url: String,

    #[serde(rename = "imageToken")]
    pub image_token: String,
}

impl GameDetails {
    #[must_use]
    pub fn unknown(place_id: i64) -> Self {
        Self {
            place_id,
            name: "Unknown".to_owned(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn filtered_name(&self) -> String {
        let stripped = strip_brackets(&self.name);
        stripped
            .chars()
            .filter(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '%' | ' ' | '.' | '_')
            })
            .collect::<String>()
            .trim()
            .to_owned()
    }
}

fn strip_brackets(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0usize;

    for character in name.chars() {
        match character {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(character),
            _ => {}
        }
    }

    out.trim().to_owned()
}

#[must_use]
pub fn multiget_place_details_url(place_ids: &[i64]) -> String {
    let query = place_ids
        .iter()
        .map(|place_id| format!("placeIds={place_id}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{GAMES_HOST}/v1/games/multiget-place-details?{query}")
}

pub fn fetch_place_details<T: Transport>(
    transport: &T,
    cookie: &str,
    place_ids: &[i64],
) -> Result<Vec<GameDetails>, GamesError> {
    let request = HttpRequest::new(HttpMethod::Get, multiget_place_details_url(place_ids))
        .with_header("Cookie", format!(".ROBLOSECURITY={cookie}"));

    let response = transport.execute(&request)?;
    if !(200..300).contains(&response.status) {
        return Err(GamesError::Failed {
            status: response.status,
        });
    }

    Ok(serde_json::from_slice(&response.body)?)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct FavoriteGame {
    pub name: String,

    #[serde(rename = "placeId", alias = "PlaceID")]
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub place_id: i64,

    #[serde(rename = "privateServer", alias = "PrivateServer", default)]
    pub private_server: String,
}

impl FavoriteGame {
    #[must_use]
    pub fn new(name: impl Into<String>, place_id: i64) -> Self {
        Self {
            name: name.into(),
            place_id,
            private_server: String::new(),
        }
    }

    #[must_use]
    pub fn with_private_server(mut self, private_server: impl Into<String>) -> Self {
        self.private_server = private_server.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RecentGame {
    pub name: String,

    #[serde(rename = "placeId", alias = "PlaceID")]
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub place_id: i64,

    #[serde(rename = "imageUrl", alias = "ImageUrl", default)]
    pub image_url: Option<String>,

    #[serde(rename = "lastUsed", alias = "LastUsed", default)]
    pub last_used: Option<String>,
}

impl RecentGame {
    #[must_use]
    pub fn new(name: impl Into<String>, place_id: i64) -> Self {
        Self {
            name: name.into(),
            place_id,
            image_url: None,
            last_used: None,
        }
    }
}

#[must_use]
pub fn load_favorites(path: &Path) -> Vec<FavoriteGame> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_favorites(path: &Path, favorites: &[FavoriteGame]) -> Result<(), GamesError> {
    std::fs::write(path, serde_json::to_string_pretty(favorites)?)?;
    Ok(())
}

#[must_use]
pub fn add_favorite(favorites: &mut Vec<FavoriteGame>, game: FavoriteGame) -> bool {
    let exists = favorites.iter().any(|existing| {
        existing.place_id == game.place_id && existing.private_server == game.private_server
    });

    if exists {
        return false;
    }

    favorites.push(game);
    true
}

#[must_use]
pub fn remove_favorite(
    favorites: &mut Vec<FavoriteGame>,
    place_id: i64,
    private_server: &str,
) -> bool {
    let before = favorites.len();
    favorites.retain(|existing| {
        !(existing.place_id == place_id && existing.private_server == private_server)
    });
    favorites.len() != before
}

#[must_use]
pub fn load_recent(path: &Path) -> Vec<RecentGame> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_recent(path: &Path, recent: &[RecentGame]) -> Result<(), GamesError> {
    std::fs::write(path, serde_json::to_string_pretty(recent)?)?;
    Ok(())
}

pub fn push_recent(recent: &mut Vec<RecentGame>, game: RecentGame, max: usize) {
    recent.retain(|existing| existing.place_id != game.place_id);
    recent.push(game);

    let keep = max.max(1);
    while recent.len() > keep {
        recent.remove(0);
    }
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

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("muster-games-tests");
        std::fs::create_dir_all(&dir).expect("tao thu muc tam");
        dir.join(name)
    }

    #[test]
    fn the_place_details_url_repeats_the_place_ids_key() {
        assert_eq!(
            multiget_place_details_url(&[11, 22]),
            "https://games.roblox.com/v1/games/multiget-place-details?placeIds=11&placeIds=22"
        );
    }

    #[test]
    fn place_details_are_read_with_the_cookie_attached() {
        let body = r#"[{"placeId":1,"universeId":7,"name":"Hi","isPlayable":true}]"#;
        let mock = Mock::new(HttpResponse::new(200).with_body(body.as_bytes().to_vec()));

        let games = fetch_place_details(&mock, "COOKIE", &[1]).expect("tai");

        assert_eq!(games.len(), 1);
        assert_eq!(games[0].universe_id, 7);
        assert!(games[0].is_playable);
        assert_eq!(
            mock.seen.borrow()[0].header("cookie"),
            Some(".ROBLOSECURITY=COOKIE")
        );
    }

    #[test]
    fn a_401_from_place_details_is_reported_as_a_status() {
        let mock = Mock::new(HttpResponse::new(401));
        let error = fetch_place_details(&mock, "COOKIE", &[1]).expect_err("phai loi");
        assert!(matches!(error, GamesError::Failed { status: 401 }));
    }

    #[test]
    fn the_filtered_name_matches_the_ram_sanitizer() {
        let mut details = GameDetails::unknown(1);
        details.name = "[UPD] Blox Fruits!".to_owned();
        assert_eq!(details.filtered_name(), "Blox Fruits");

        details.name = "Grow a Garden 🌱".to_owned();
        assert_eq!(details.filtered_name(), "Grow a Garden");
    }

    #[test]
    fn a_favorite_is_only_added_once_per_place_and_private_server() {
        let mut favorites = Vec::new();

        assert!(add_favorite(&mut favorites, FavoriteGame::new("A", 1)));
        assert!(
            !add_favorite(&mut favorites, FavoriteGame::new("A again", 1)),
            "cung place + khong private server thi khong them lai"
        );
        assert!(add_favorite(
            &mut favorites,
            FavoriteGame::new("A vip", 1).with_private_server("CODE")
        ));
        assert_eq!(favorites.len(), 2);
    }

    #[test]
    fn removing_a_favorite_reports_whether_anything_changed() {
        let mut favorites = vec![
            FavoriteGame::new("A", 1),
            FavoriteGame::new("A vip", 1).with_private_server("CODE"),
        ];

        assert!(remove_favorite(&mut favorites, 1, "CODE"));
        assert_eq!(favorites.len(), 1);
        assert!(!remove_favorite(&mut favorites, 1, "CODE"));
        assert!(!remove_favorite(&mut favorites, 99, ""));
    }

    #[test]
    fn recent_games_are_unique_and_trimmed_from_the_front() {
        let mut recent = Vec::new();

        for place_id in 1..=3 {
            push_recent(
                &mut recent,
                RecentGame::new(format!("G{place_id}"), place_id),
                3,
            );
        }
        assert_eq!(
            recent.iter().map(|game| game.place_id).collect::<Vec<_>>(),
            [1, 2, 3]
        );

        push_recent(&mut recent, RecentGame::new("G2 again", 2), 3);
        assert_eq!(
            recent.iter().map(|game| game.place_id).collect::<Vec<_>>(),
            [1, 3, 2],
            "place da co phai duoc doi len cuoi"
        );

        push_recent(&mut recent, RecentGame::new("G9", 9), 3);
        assert_eq!(
            recent.iter().map(|game| game.place_id).collect::<Vec<_>>(),
            [3, 2, 9],
            "vuot qua max thi cat tu dau"
        );
    }

    #[test]
    fn favorites_round_trip_through_the_ram_file_name() {
        let path = temp_path(FAVORITES_FILE);
        let favorites = vec![FavoriteGame::new("Blox Fruits", 4483381587)];

        save_favorites(&path, &favorites).expect("ghi");
        let back = load_favorites(&path);

        assert_eq!(back, favorites);
        let raw = std::fs::read_to_string(&path).expect("doc");
        assert!(
            raw.contains("\"placeId\""),
            "phai ghi dung ten truong cua RAM"
        );
        assert!(raw.contains("privateServer"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn recent_games_round_trip_and_a_broken_file_reads_as_empty() {
        let path = temp_path(RECENT_GAMES_FILE);
        let recent = vec![RecentGame::new("Arsenal", 286090429)];

        save_recent(&path, &recent).expect("ghi");
        assert_eq!(load_recent(&path), recent);

        std::fs::write(&path, "khong phai json").expect("ghi rac");
        assert!(load_recent(&path).is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_missing_file_reads_as_empty_instead_of_failing() {
        let path = temp_path("khong-ton-tai.json");
        assert!(load_favorites(&path).is_empty());
        assert!(load_recent(&path).is_empty());
    }
}
