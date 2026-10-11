//! # `launch` — mở client Roblox
//!
//! Tái lập đường mở game của RAM (`Classes/Account.cs:505-678`). Toàn bộ phần **dựng chuỗi** nằm
//! ở đây và thuần tuý — không chạm đĩa, không spawn — nên kiểm thử được bằng `cargo test`. Việc
//! chạy thật chỉ là bước cuối, mỏng.
//!
//! ## Hai chế độ
//!
//! | Chế độ | RAM gọi | Cách mở |
//! | :--- | :--- | :--- |
//! | Hiện đại | `UseOldJoin = false` | URL scheme `roblox-player:1+...` qua trình xử lý giao thức |
//! | Legacy | `UseOldJoin = true` | chạy thẳng `RobloxPlayerBeta.exe` với `-t <ticket> -j <url>` |
//!
//! Chế độ hiện đại là mặc định và là đường chính. Legacy giữ lại làm dự phòng — RAM vẫn còn cờ
//! `UseOldJoin` cho trường hợp trình xử lý giao thức bị hỏng.
//!
//! ## Multi-instance
//!
//! Roblox chống mở nhiều client bằng hai cơ chế, và phải phá **cả hai**:
//!
//! 1. **Named mutex `ROBLOX_singletonMutex`.** Giữ một handle tới mutex này trong suốt thời gian
//!    app chạy là đủ để Roblox thấy nó "đã tồn tại" và bỏ qua kiểm tra single-instance —
//!    `AccountManager.cs:1258-1270`. Xem [`windows::SingletonMutex`].
//! 2. **Cờ `-b <BrowserTrackerID>` riêng cho từng client.** Mỗi client phải mang một tracker id
//!    khác nhau, nếu không client sau sẽ bị client trước coi là "cùng tài khoản" và bị đá ra
//!    (lỗi *Same account launched*). Xem [`generate_browser_tracker_id`].
//!
//! ## Vì sao tách `windows.rs`
//!
//! Cùng lý do đã ghi ở `vault/dpapi.rs`: sản phẩm chỉ nhắm Windows, nên phần chỉ-Windows nằm gọn
//! trong một file nhỏ đọc hết được trong một lần, thay vì rải `#[cfg]` khắp module.

#[cfg(windows)]
pub mod windows;

use std::path::PathBuf;

/// Tên named mutex mà Roblox dùng để chống mở nhiều client.
///
/// `AccountManager.cs:1261`. Giữ nguyên từng ký tự.
pub const SINGLETON_MUTEX_NAME: &str = "ROBLOX_singletonMutex";

/// Tiền tố URL scheme của chế độ mở hiện đại.
const PLAYER_URL_PREFIX: &str = "roblox-player:1+launchmode:play";

/// Gốc của `PlaceLauncher.ashx` — dùng cho cả chế độ hiện đại lẫn legacy.
const PLACE_LAUNCHER: &str = "https://assetgame.roblox.com/game/PlaceLauncher.ashx";

/// Hậu tố cố định của URL `roblox-player:`. `Account.cs:651-656`.
const PLAYER_URL_SUFFIX: &str = "+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp";

/// Lỗi của lớp `launch`.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    /// Không lấy được số ngẫu nhiên từ hệ điều hành khi sinh `BrowserTrackerID`.
    #[error("không lấy được số ngẫu nhiên từ hệ điều hành để sinh BrowserTrackerID")]
    Random,

    /// Không tạo được named mutex.
    #[cfg(windows)]
    #[error("không tạo được named mutex {name} (mã lỗi Win32 {code})")]
    Mutex {
        /// Tên mutex đã yêu cầu tạo.
        name: &'static str,
        /// Mã lỗi từ `GetLastError()`.
        code: u32,
    },
}

/// Đích cần mở.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchTarget {
    /// Vào một place, tuỳ chọn ghim vào một server cụ thể.
    Game {
        /// `PlaceId` của game.
        place_id: i64,
        /// `JobId` của server. `None` hoặc chuỗi rỗng nghĩa là để Roblox tự chọn server.
        job_id: Option<String>,
    },

    /// Vào server riêng (VIP server) bằng link.
    PrivateServer {
        /// `PlaceId` của game.
        place_id: i64,
        /// Mã truy cập server riêng.
        access_code: String,
        /// Mã link của server riêng (`privateServerLinkCode=...`).
        link_code: String,
    },

    /// Đi theo một người dùng vào server họ đang chơi.
    FollowUser {
        /// `UserId` của người cần đi theo (RAM truyền giá trị này vào tham số `userId`).
        user_id: i64,
    },
}

impl LaunchTarget {
    /// URL `PlaceLauncher.ashx` **chưa** mã hoá cho đích này.
    ///
    /// `browser_tracker_id` chỉ xuất hiện ở nhánh [`LaunchTarget::Game`] — đúng như RAM
    /// (`Account.cs:656`): hai nhánh còn lại không gửi tracker trong URL.
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

/// Dựng URL `roblox-player:` cho chế độ mở hiện đại.
///
/// `launch_time_ms` là mili giây kể từ Unix epoch — lấy bằng [`crate::datetime::now_unix_millis`].
/// Tham số này không ảnh hưởng tới việc vào game nhưng RAM gửi nó, và Roblox dùng nó để sắp thứ
/// tự các lần mở; giữ lại cho khớp.
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

/// Dựng tham số dòng lệnh cho chế độ legacy (`RobloxPlayerBeta.exe`).
///
/// ⚠️ **Giữ nguyên một lỗi của RAM.** Ở nhánh [`LaunchTarget::Game`], RAM luôn nối `&gameId=` kể cả
/// khi `JobID` rỗng (`Account.cs:636`: `"&gameId=" + JobID`), cho ra `...&gameId=&isPlayTogetherGame=false`.
/// Trông vô nghĩa nhưng đó là chuỗi RAM gửi đi, và tương thích ngược là ưu tiên cao hơn vẻ đẹp
/// của code (nguyên tắc bất biến #2).
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

/// Các đường dẫn có thể chứa `RobloxPlayerBeta.exe` của một version, theo thứ tự thử.
///
/// `Account.cs:615-623`: RAM thử `Program Files (x86)` trước, rồi `%LOCALAPPDATA%`. Trả về danh
/// sách chứ không tự kiểm tra tồn tại, để hàm này thuần tuý và kiểm thử được.
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

/// Sinh một `BrowserTrackerID` mới.
///
/// Tái lập `Account.cs:509-511`: hai số ngẫu nhiên nối chuỗi — số đầu trong `[100000, 175000)`,
/// số sau trong `[100000, 900000)` — cho ra một chuỗi 12 chữ số.
///
/// Không phải giá trị mật mã: nó chỉ cần **khác nhau giữa các client** để Roblox không coi hai
/// client là cùng một phiên. Vì vậy lấy số ngẫu nhiên của hệ điều hành rồi lấy dư là đủ, và ta
/// không kéo thêm crate `rand`.
///
/// # Errors
///
/// Trả [`LaunchError::Random`] nếu hệ điều hành không cấp được số ngẫu nhiên.
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

    // Lấy dư có sai lệch rất nhỏ về phân bố; chấp nhận được vì giá trị này không dùng cho bảo mật.
    Ok(min + (u32::from_le_bytes(buffer) % span))
}

/// Rút `BrowserTrackerID` ra khỏi dòng lệnh của một tiến trình Roblox.
///
/// RAM khớp bằng regex `-b (\d+)` (`Account.cs:529`). Ở đây làm tương đương không cần crate regex:
/// tìm lần xuất hiện **đầu tiên** của `-b ` rồi lấy dãy chữ số theo sau.
///
/// Trả `None` khi không có, hoặc khi dòng lệnh rỗng — trường hợp thật đã gặp với tiến trình thứ
/// hai của Roblox (xem ghi chú ở `ROADMAP.md` mục 7): khớp nhầm nó sẽ làm client bị đá (lỗi 268).
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

        // "-b " xuất hiện nhưng không theo sau bởi chữ số: tìm tiếp chỗ khác.
        rest = after;
        if rest.is_empty() {
            return None;
        }
    }
}

/// Mã hoá một chuỗi theo đúng ngữ nghĩa `HttpUtility.UrlEncode` của .NET Framework.
///
/// Ba điểm phải khớp mới ra đúng chuỗi RAM gửi:
///
/// 1. **Dấu cách thành `+`**, không phải `%20`.
/// 2. **Hex viết thường** (`%3a` chứ không `%3A`) — đặc trưng của .NET Framework. .NET Core đã đổi
///    sang chữ hoa, nên đây là chi tiết dễ sai nếu tra tài liệu nhầm phiên bản. Khác biệt này
///    không ảnh hưởng tới việc vào game (trình mở giải mã cả hai), nhưng ta bám RAM.
/// 3. **Nhóm ký tự không mã hoá**: chữ, số, và `- _ . ! * ( )`. Mọi byte khác đi thành `%XX`.
///
/// Chuỗi vào được coi là UTF-8; byte ngoài ASCII bị mã hoá theo từng byte, đúng như .NET làm.
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

#[cfg(test)]
mod tests {
    use super::*;

    const TICKET: &str = "AUTH-TICKET-123";
    const TRACKER: &str = "100000000000001";

    // ---------------------------------------------------------------- url_encode

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
        // "é" là 0xC3 0xA9 trong UTF-8 → hai byte bị mã hoá.
        assert_eq!(url_encode("é"), "%c3%a9");
    }

    // ---------------------------------------------------------------- place launcher url

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
        // Dung nhu RAM: chi nhanh Game moi gui browserTrackerId trong URL (Account.cs:656).
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

    // ---------------------------------------------------------------- modern url

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

    // ---------------------------------------------------------------- legacy args

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
        // Account.cs:636 luon noi "&gameId=" ke ca khi JobID rong. Giu nguyen.
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
        // Chế độ legacy dat URL trong dau ngoac kep, RAM khong ma hoa.
        let target = LaunchTarget::FollowUser { user_id: 1 };
        let args = build_legacy_args(TICKET, &target);
        assert!(
            args[4].starts_with("https://"),
            "khong duoc ma hoa: {}",
            args[4]
        );
    }

    // ---------------------------------------------------------------- legacy exe

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

    // ---------------------------------------------------------------- tracker id

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

    // ---------------------------------------------------------------- parse tracker

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
}
