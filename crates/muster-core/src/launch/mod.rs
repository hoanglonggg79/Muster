#[cfg(windows)]
pub mod windows;

use std::path::PathBuf;

pub const SINGLETON_MUTEX_NAME: &str = "ROBLOX_singletonMutex";

const PLAYER_URL_PREFIX: &str = "roblox-player:1+launchmode:play";

const PLACE_LAUNCHER: &str = "https://assetgame.roblox.com/game/PlaceLauncher.ashx";

const PLAYER_URL_SUFFIX: &str = "+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp";

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("không lấy được số ngẫu nhiên từ hệ điều hành để sinh BrowserTrackerID")]
    Random,

    #[cfg(windows)]
    #[error("không tạo được named mutex {name} (mã lỗi Win32 {code})")]
    Mutex { name: &'static str, code: u32 },

    #[error("khởi chạy URL thất bại (mã lỗi ShellExecute {code})")]
    ShellExecute { code: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchTarget {
    Game {
        place_id: i64,
        job_id: Option<String>,
    },

    PrivateServer {
        place_id: i64,
        access_code: String,
        link_code: String,
    },

    FollowUser {
        user_id: i64,
    },
}

impl LaunchTarget {
    #[must_use]
    pub fn place_launcher_url(&self, browser_tracker_id: &str, is_teleport: bool) -> String {
        match self {
            Self::Game { place_id, job_id } => {
                let job = job_id.as_deref().filter(|value| !value.is_empty());
                let request = if job.is_some() {
                    "RequestGameJob"
                } else {
                    "RequestGame"
                };

                let mut url = format!(
                    "{PLACE_LAUNCHER}?request={request}&browserTrackerId={browser_tracker_id}\
                     &placeId={place_id}"
                );
                if let Some(job) = job {
                    url.push_str("&gameId=");
                    url.push_str(job);
                }
                url.push_str("&isPlayTogetherGame=false");
                if is_teleport {
                    url.push_str("&isTeleport=true");
                }
                url
            }

            Self::PrivateServer {
                place_id,
                access_code,
                link_code,
            } => format!(
                "{PLACE_LAUNCHER}?request=RequestPrivateGame&placeId={place_id}\
                 &accessCode={access_code}&linkCode={link_code}"
            ),

            Self::FollowUser { user_id } => {
                format!("{PLACE_LAUNCHER}?request=RequestFollowUser&userId={user_id}")
            }
        }
    }
}

#[must_use]
pub fn build_launch_url(
    auth_ticket: &str,
    target: &LaunchTarget,
    browser_tracker_id: &str,
    launch_time_ms: u64,
    is_teleport: bool,
) -> String {
    let encoded = url_encode(&target.place_launcher_url(browser_tracker_id, is_teleport));
    format!(
        "{PLAYER_URL_PREFIX}+gameinfo:{auth_ticket}+launchtime:{launch_time_ms}\
         +placelauncherurl:{encoded}+browsertrackerid:{browser_tracker_id}{PLAYER_URL_SUFFIX}"
    )
}

#[must_use]
pub fn build_legacy_args(auth_ticket: &str, target: &LaunchTarget) -> Vec<String> {
    let url = match target {
        LaunchTarget::Game { place_id, job_id } => {
            let job = job_id.as_deref().unwrap_or("");
            let request = if job.is_empty() {
                "RequestGame"
            } else {
                "RequestGameJob"
            };
            format!(
                "{PLACE_LAUNCHER}?request={request}&placeId={place_id}&gameId={job}\
                 &isPlayTogetherGame=false"
            )
        }

        LaunchTarget::PrivateServer {
            place_id,
            access_code,
            link_code,
        } => format!(
            "{PLACE_LAUNCHER}?request=RequestPrivateGame&placeId={place_id}\
             &accessCode={access_code}&linkCode={link_code}"
        ),

        LaunchTarget::FollowUser { user_id } => {
            format!("{PLACE_LAUNCHER}?request=RequestFollowUser&userId={user_id}")
        }
    };

    vec![
        "--app".to_owned(),
        "-t".to_owned(),
        auth_ticket.to_owned(),
        "-j".to_owned(),
        url,
    ]
}

#[must_use]
pub fn legacy_executable_candidates(
    current_version: &str,
    local_app_data: Option<&str>,
) -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from(format!(
        r"C:\Program Files (x86)\Roblox\Versions\{current_version}\RobloxPlayerBeta.exe"
    ))];

    if let Some(local) = local_app_data {
        candidates.push(PathBuf::from(format!(
            r"{local}\Roblox\Versions\{current_version}\RobloxPlayerBeta.exe"
        )));
    }

    candidates
}

pub fn generate_browser_tracker_id() -> Result<String, LaunchError> {
    let first = random_in_range(100_000, 175_000)?;
    let second = random_in_range(100_000, 900_000)?;
    Ok(format!("{first}{second}"))
}

fn random_in_range(min: u32, max_exclusive: u32) -> Result<u32, LaunchError> {
    debug_assert!(max_exclusive > min);
    let span = max_exclusive - min;

    let mut buffer = [0u8; 4];
    getrandom::getrandom(&mut buffer).map_err(|_| LaunchError::Random)?;

    Ok(min + (u32::from_le_bytes(buffer) % span))
}

#[must_use]
pub fn parse_browser_tracker_id(command_line: &str) -> Option<&str> {
    let mut rest = command_line;

    loop {
        let position = rest.find("-b ")?;
        let after = &rest[position + 3..];

        let digits = after
            .split(|character: char| !character.is_ascii_digit())
            .next()
            .unwrap_or("");

        if !digits.is_empty() {
            return Some(digits);
        }

        rest = after;
        if rest.is_empty() {
            return None;
        }
    }
}

#[must_use]
pub fn url_encode(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'*'
            | b'('
            | b')' => out.push(byte as char),
            b' ' => out.push('+'),
            other => {
                out.push('%');
                out.push(HEX[(other >> 4) as usize] as char);
                out.push(HEX[(other & 0x0F) as usize] as char);
            }
        }
    }
    out
}

pub const CLOSE_WINDOW_DELAY_MS: u64 = 250;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningClient {
    pub pid: u32,
    pub command_line: String,
}

impl RunningClient {
    #[must_use]
    pub fn browser_tracker_id(&self) -> Option<&str> {
        parse_browser_tracker_id(&self.command_line)
    }
}

#[must_use]
pub fn clients_for_tracker<'a>(
    clients: &'a [RunningClient],
    tracker: &str,
) -> Vec<&'a RunningClient> {
    clients
        .iter()
        .filter(|client| client.browser_tracker_id() == Some(tracker))
        .collect()
}

pub trait ClientControl {
    fn close_main_window(&mut self, pid: u32) -> bool;

    fn kill(&mut self, pid: u32) -> bool;

    fn wait(&mut self, millis: u64);
}

pub fn close_previous_instances<C: ClientControl>(
    control: &mut C,
    clients: &[RunningClient],
    tracker: &str,
) -> usize {
    let targets = clients_for_tracker(clients, tracker);

    for client in &targets {
        control.close_main_window(client.pid);
        control.wait(CLOSE_WINDOW_DELAY_MS);
        control.close_main_window(client.pid);
        control.wait(CLOSE_WINDOW_DELAY_MS);
        control.kill(client.pid);
    }

    targets.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICKET: &str = "AUTH-TICKET-123";
    const TRACKER: &str = "100000000000001";

    #[test]
    fn url_encode_turns_a_space_into_a_plus() {
        assert_eq!(url_encode("a b"), "a+b");
    }

    #[test]
    fn url_encode_uses_lowercase_hex_like_dotnet_framework() {
        assert_eq!(url_encode(":"), "%3a");
        assert_eq!(url_encode("/"), "%2f");
        assert_eq!(url_encode("?"), "%3f");
        assert_eq!(url_encode("="), "%3d");
        assert_eq!(url_encode("&"), "%26");
        assert_eq!(url_encode("~"), "%7e");
        assert_eq!(url_encode("+"), "%2b");
    }

    #[test]
    fn url_encode_leaves_the_unreserved_group_alone() {
        let untouched = "abcXYZ0189-_.!*()";
        assert_eq!(url_encode(untouched), untouched);
    }

    #[test]
    fn url_encode_handles_a_real_place_launcher_url() {
        let raw = "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame\
                   &browserTrackerId=100000000000001&placeId=4483381587&isPlayTogetherGame=false";
        let expected = "https%3a%2f%2fassetgame.roblox.com%2fgame%2fPlaceLauncher.ashx\
                        %3frequest%3dRequestGame%26browserTrackerId%3d100000000000001\
                        %26placeId%3d4483381587%26isPlayTogetherGame%3dfalse";
        assert_eq!(url_encode(raw), expected);
    }

    #[test]
    fn url_encode_does_not_lose_non_ascii_bytes() {
        assert_eq!(url_encode("é"), "%c3%a9");
    }

    #[test]
    fn a_plain_game_has_no_job_id_in_the_url() {
        let target = LaunchTarget::Game {
            place_id: 4483381587,
            job_id: None,
        };
        assert_eq!(
            target.place_launcher_url(TRACKER, false),
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame\
             &browserTrackerId=100000000000001&placeId=4483381587&isPlayTogetherGame=false"
        );
    }

    #[test]
    fn an_empty_job_id_is_treated_the_same_as_none() {
        let with_none = LaunchTarget::Game {
            place_id: 1,
            job_id: None,
        };
        let with_empty = LaunchTarget::Game {
            place_id: 1,
            job_id: Some(String::new()),
        };
        assert_eq!(
            with_none.place_launcher_url(TRACKER, false),
            with_empty.place_launcher_url(TRACKER, false)
        );
    }

    #[test]
    fn a_pinned_server_switches_the_request_and_adds_the_game_id() {
        let target = LaunchTarget::Game {
            place_id: 4483381587,
            job_id: Some("abc-def".to_owned()),
        };
        let url = target.place_launcher_url(TRACKER, false);

        assert!(url.contains("request=RequestGameJob"), "url: {url}");
        assert!(url.contains("&gameId=abc-def"), "url: {url}");
        assert!(
            url.ends_with("&isPlayTogetherGame=false"),
            "isTeleport phai dung sau cung khi tat: {url}"
        );
    }

    #[test]
    fn teleport_appends_its_flag_last() {
        let target = LaunchTarget::Game {
            place_id: 1,
            job_id: None,
        };
        let url = target.place_launcher_url(TRACKER, true);
        assert!(
            url.ends_with("&isPlayTogetherGame=false&isTeleport=true"),
            "url: {url}"
        );
    }

    #[test]
    fn a_private_server_url_carries_the_access_and_link_codes() {
        let target = LaunchTarget::PrivateServer {
            place_id: 4483381587,
            access_code: "ACCESS".to_owned(),
            link_code: "LINK".to_owned(),
        };
        assert_eq!(
            target.place_launcher_url(TRACKER, false),
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestPrivateGame\
             &placeId=4483381587&accessCode=ACCESS&linkCode=LINK"
        );
    }

    #[test]
    fn a_follow_url_carries_the_user_id() {
        let target = LaunchTarget::FollowUser { user_id: 42 };
        assert_eq!(
            target.place_launcher_url(TRACKER, false),
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestFollowUser&userId=42"
        );
    }

    #[test]
    fn private_server_and_follow_urls_do_not_carry_a_tracker_id() {
        let private = LaunchTarget::PrivateServer {
            place_id: 1,
            access_code: "A".to_owned(),
            link_code: "L".to_owned(),
        };
        let follow = LaunchTarget::FollowUser { user_id: 1 };
        assert!(
            !private
                .place_launcher_url(TRACKER, false)
                .contains("rowserTracker")
        );
        assert!(
            !follow
                .place_launcher_url(TRACKER, false)
                .contains("rowserTracker")
        );
    }

    #[test]
    fn the_modern_url_matches_the_shape_ram_sends() {
        let target = LaunchTarget::Game {
            place_id: 4483381587,
            job_id: None,
        };
        let url = build_launch_url(TICKET, &target, TRACKER, 1_760_000_000_000, false);

        assert!(
            url.starts_with("roblox-player:1+launchmode:play+gameinfo:AUTH-TICKET-123+launchtime:1760000000000+placelauncherurl:"),
            "url: {url}"
        );
        assert!(
            url.ends_with("+browsertrackerid:100000000000001+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp"),
            "url: {url}"
        );
        assert!(
            url.contains("%3a%2f%2fassetgame.roblox.com"),
            "placelauncherurl phai duoc ma hoa: {url}"
        );
        assert!(!url.contains(" "), "URL khong duoc chua dau cach: {url}");
    }

    #[test]
    fn the_modern_url_encodes_the_inner_url_but_not_the_ticket() {
        let target = LaunchTarget::FollowUser { user_id: 7 };
        let url = build_launch_url("ticket+with/slash", &target, TRACKER, 1, false);

        assert!(
            url.contains("+gameinfo:ticket+with/slash+"),
            "ticket phai duoc nhet nguyen ven: {url}"
        );
        assert!(url.contains("request%3dRequestFollowUser"), "url: {url}");
    }

    #[test]
    fn legacy_args_start_with_the_expected_flags() {
        let target = LaunchTarget::Game {
            place_id: 4483381587,
            job_id: None,
        };
        let args = build_legacy_args(TICKET, &target);
        assert_eq!(args[0], "--app");
        assert_eq!(args[1], "-t");
        assert_eq!(args[2], TICKET);
        assert_eq!(args[3], "-j");
    }

    #[test]
    fn legacy_args_keep_rams_empty_game_id_quirk() {
        let target = LaunchTarget::Game {
            place_id: 99,
            job_id: None,
        };
        let args = build_legacy_args(TICKET, &target);
        assert_eq!(
            args[4],
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame\
             &placeId=99&gameId=&isPlayTogetherGame=false"
        );
    }

    #[test]
    fn legacy_args_switch_the_request_when_a_job_id_is_present() {
        let target = LaunchTarget::Game {
            place_id: 99,
            job_id: Some("job-1".to_owned()),
        };
        let args = build_legacy_args(TICKET, &target);
        assert!(args[4].contains("request=RequestGameJob"));
        assert!(args[4].contains("&gameId=job-1"));
    }

    #[test]
    fn legacy_args_are_not_url_encoded() {
        let target = LaunchTarget::FollowUser { user_id: 1 };
        let args = build_legacy_args(TICKET, &target);
        assert!(
            args[4].starts_with("https://"),
            "khong duoc ma hoa: {}",
            args[4]
        );
    }

    #[test]
    fn legacy_exe_candidates_try_program_files_before_local_app_data() {
        let candidates =
            legacy_executable_candidates("version-abc123", Some(r"C:\Users\x\AppData\Local"));
        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0],
            PathBuf::from(
                r"C:\Program Files (x86)\Roblox\Versions\version-abc123\RobloxPlayerBeta.exe"
            )
        );
        assert_eq!(
            candidates[1],
            PathBuf::from(
                r"C:\Users\x\AppData\Local\Roblox\Versions\version-abc123\RobloxPlayerBeta.exe"
            )
        );
    }

    #[test]
    fn legacy_exe_candidates_skip_local_app_data_when_it_is_missing() {
        assert_eq!(legacy_executable_candidates("version-abc", None).len(), 1);
    }

    #[test]
    fn a_generated_tracker_id_is_twelve_digits_in_the_ram_ranges() {
        for _ in 0..200 {
            let id = generate_browser_tracker_id().expect("sinh tracker id");
            assert_eq!(id.len(), 12, "phai la 12 chu so: {id}");
            assert!(id.bytes().all(|byte| byte.is_ascii_digit()), "id: {id}");

            let first: u32 = id[..6].parse().expect("6 so dau");
            let second: u32 = id[6..].parse().expect("6 so sau");
            assert!(
                (100_000..175_000).contains(&first),
                "so dau ngoai mien: {first}"
            );
            assert!(
                (100_000..900_000).contains(&second),
                "so sau ngoai mien: {second}"
            );
        }
    }

    #[test]
    fn generated_tracker_ids_are_not_all_the_same() {
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..50 {
            seen.insert(generate_browser_tracker_id().expect("sinh tracker id"));
        }
        assert!(seen.len() > 1, "tracker id phai khac nhau giua cac lan");
    }

    #[test]
    fn a_tracker_id_is_read_from_a_real_looking_command_line() {
        let line = r#""C:\Users\x\AppData\Local\Roblox\Versions\version-abc\RobloxPlayerBeta.exe" -b 123456789012 --app"#;
        assert_eq!(parse_browser_tracker_id(line), Some("123456789012"));
    }

    #[test]
    fn parsing_returns_none_for_an_empty_or_unrelated_command_line() {
        assert_eq!(parse_browser_tracker_id(""), None);
        assert_eq!(
            parse_browser_tracker_id(r#"\??\C:\Windows\System32\conhost.exe"#),
            None
        );
        assert_eq!(parse_browser_tracker_id("RobloxPlayerBeta.exe --app"), None);
    }

    #[test]
    fn a_trailing_dash_b_without_digits_does_not_stop_the_search() {
        let line = "-b --app -b 555000111222";
        assert_eq!(parse_browser_tracker_id(line), Some("555000111222"));
    }

    #[test]
    fn the_first_valid_tracker_id_wins() {
        let line = "-b 111111111111 -b 222222222222";
        assert_eq!(parse_browser_tracker_id(line), Some("111111111111"));
    }

    #[derive(Default)]
    struct RecordingControl {
        actions: Vec<String>,
    }

    impl ClientControl for RecordingControl {
        fn close_main_window(&mut self, pid: u32) -> bool {
            self.actions.push(format!("close:{pid}"));
            true
        }

        fn kill(&mut self, pid: u32) -> bool {
            self.actions.push(format!("kill:{pid}"));
            true
        }

        fn wait(&mut self, millis: u64) {
            self.actions.push(format!("wait:{millis}"));
        }
    }

    fn client(pid: u32, tracker: &str) -> RunningClient {
        RunningClient {
            pid,
            command_line: format!(
                r#""C:\Roblox\Versions\version-x\RobloxPlayerBeta.exe" -b {tracker} --app"#
            ),
        }
    }

    #[test]
    fn only_clients_with_the_matching_tracker_are_selected() {
        let clients = vec![client(1, "111111111111"), client(2, "222222222222")];
        let selected = clients_for_tracker(&clients, "222222222222");

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].pid, 2);
    }

    #[test]
    fn a_client_without_a_tracker_id_is_never_selected() {
        let clients = vec![
            RunningClient {
                pid: 7,
                command_line: String::new(),
            },
            RunningClient {
                pid: 8,
                command_line: r#"\??\C:\Windows\System32\conhost.exe"#.to_owned(),
            },
        ];
        assert!(clients_for_tracker(&clients, "111111111111").is_empty());
    }

    #[test]
    fn closing_follows_the_exact_two_close_one_kill_sequence() {
        let mut control = RecordingControl::default();
        let clients = vec![client(42, "123456789012")];

        let closed = close_previous_instances(&mut control, &clients, "123456789012");

        assert_eq!(closed, 1);
        assert_eq!(
            control.actions,
            ["close:42", "wait:250", "close:42", "wait:250", "kill:42"],
            "trinh tu phai khop Account.cs:537-540"
        );
    }

    #[test]
    fn closing_touches_nothing_when_no_client_matches() {
        let mut control = RecordingControl::default();
        let clients = vec![client(1, "111111111111")];

        let closed = close_previous_instances(&mut control, &clients, "999999999999");

        assert_eq!(closed, 0);
        assert!(
            control.actions.is_empty(),
            "khong duoc cham gi: {:?}",
            control.actions
        );
    }

    #[test]
    fn closing_handles_several_instances_of_the_same_tracker() {
        let mut control = RecordingControl::default();
        let clients = vec![
            client(10, "555555555555"),
            client(11, "555555555555"),
            client(12, "666666666666"),
        ];

        let closed = close_previous_instances(&mut control, &clients, "555555555555");

        assert_eq!(closed, 2);
        assert!(!control.actions.iter().any(|action| action.ends_with(":12")));
    }
}
