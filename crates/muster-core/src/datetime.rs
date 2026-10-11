//! # `datetime` — định dạng thời gian khớp Json.NET
//!
//! `models::Account` giữ `LastUse` và `LastAttemptedRefresh` dạng **chuỗi** để round-trip không
//! mất dữ liệu (xem ghi chú đầu `models.rs`). Nhưng từ M1 trở đi ta phải **sinh** chuỗi đó —
//! khi thêm tài khoản mới hoặc làm mới cookie — nên cần biết Json.NET ghi ra chính xác thế nào.
//!
//! ## Định dạng
//!
//! `ROADMAP.md` mục 4.1 chốt: `yyyy-MM-ddTHH:mm:ss.FFFFFFF`, **cắt số 0 thừa ở cuối**. Nghĩa là:
//!
//! | Giá trị | Chuỗi ghi ra |
//! | :--- | :--- |
//! | không có phần lẻ | `2026-09-28T21:14:03` |
//! | 0,512 giây | `2026-09-28T21:14:03.512` |
//! | 0,5 giây | `2026-09-28T21:14:03.5` |
//! | 0,0000001 giây | `2026-09-28T21:14:03.0000001` |
//!
//! ⚠️ **Chưa xác minh:** Json.NET còn nối thêm hậu tố `K` (múi giờ) cho `DateTimeKind.Local`, tức
//! `...+07:00`. Fixture trong repo **không** có hậu tố đó. Ta bám theo fixture và `ROADMAP.md`,
//! nhưng đây là điểm cần đối chiếu lại với một `AccountData.json` do RAM thật ghi ra trước khi
//! coi là chốt — ghi ở mục rủi ro của `ROADMAP.md`.
//!
//! ## Vì sao không dùng crate lịch
//!
//! Chỉ cần đúng một phép đổi: giây Unix → (năm, tháng, ngày, giờ, phút, giây). Đó là thuật toán
//! `civil_from_days` của Howard Hinnant, ~15 dòng, kiểm thử được bằng vài mốc đã biết. Kéo thêm
//! `chrono`/`time` chỉ để làm một việc này là không tương xứng, và `muster-core` giữ được cây phụ
//! thuộc gọn — cùng lý do đã chọn ở `vault::dpapi`.

use std::time::{SystemTime, UNIX_EPOCH};

/// Định dạng một mốc thời gian Unix thành chuỗi kiểu Json.NET.
///
/// `subsec_nanos` là phần lẻ giây, 0..1_000_000_000. Giá trị ≥ 1 tỉ bị bỏ qua phần vượt (không
/// xảy ra với dữ liệu lấy từ `SystemTime`, nhưng hàm này là công khai nên không được panic).
#[must_use]
pub fn format_json_net(unix_seconds: i64, subsec_nanos: u32) -> String {
    // div_euclid/rem_euclid để mốc trước 1970 vẫn ra giờ dương trong ngày.
    let days = unix_seconds.div_euclid(86_400);
    let seconds_of_day = unix_seconds.rem_euclid(86_400);

    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    let mut out = format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}");

    // Json.NET dùng 7 chữ số (độ phân giải "tick" = 100 ns) rồi cắt số 0 thừa.
    let ticks = (subsec_nanos.min(999_999_999)) / 100;
    if ticks != 0 {
        let fraction = format!("{ticks:07}");
        out.push('.');
        out.push_str(fraction.trim_end_matches('0'));
    }

    out
}

/// Mốc thời gian hiện tại theo giờ hệ thống, ở định dạng Json.NET.
///
/// Trả mốc 0 (`1970-01-01T00:00:00`) nếu đồng hồ hệ thống đứng trước Unix epoch — không panic.
#[must_use]
pub fn now_json_net() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(delta) => format_json_net(delta.as_secs() as i64, delta.subsec_nanos()),
        Err(_) => format_json_net(0, 0),
    }
}

/// Số mili giây kể từ Unix epoch, theo giờ hệ thống.
///
/// Đây là giá trị RAM nhét vào `launchtime:` của URL `roblox-player:` —
/// `Account.cs:611`: `Math.Floor((DateTime.UtcNow - epoch).TotalSeconds * 1000)`.
#[must_use]
pub fn now_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |delta| delta.as_millis() as u64)
}

/// Đổi số ngày kể từ 1970-01-01 thành (năm, tháng, ngày).
///
/// Thuật toán `civil_from_days` của Howard Hinnant, đã kiểm chứng bằng các mốc ở phần test.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    // Dời mốc về 0000-03-01 để năm nhuận rơi ở cuối chu kỳ 400 năm.
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365; // [0, 399]
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // [0, 365]
    let month_prime = (5 * day_of_year + 2) / 153; // [0, 11]
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1; // [1, 31]
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };

    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_is_the_start_of_1970() {
        assert_eq!(format_json_net(0, 0), "1970-01-01T00:00:00");
    }

    #[test]
    fn a_well_known_timestamp_matches() {
        // 1_000_000_000 = 2001-09-09T01:46:40 UTC
        assert_eq!(format_json_net(1_000_000_000, 0), "2001-09-09T01:46:40");
    }

    #[test]
    fn a_leap_day_is_handled() {
        // 951_782_400 = 2000-02-29T00:00:00 UTC (2000 là năm nhuận)
        assert_eq!(format_json_net(951_782_400, 0), "2000-02-29T00:00:00");
    }

    #[test]
    fn a_non_leap_year_has_no_29th_of_february() {
        // 983_318_400 = 2001-02-28T00:00:00 UTC (2001 không nhuận), và ngày kế tiếp là 03-01.
        assert_eq!(format_json_net(983_318_400, 0), "2001-02-28T00:00:00");
        assert_eq!(format_json_net(983_404_800, 0), "2001-03-01T00:00:00");
    }

    #[test]
    fn a_moment_before_the_epoch_still_reads_as_a_positive_time_of_day() {
        assert_eq!(format_json_net(-1, 0), "1969-12-31T23:59:59");
        assert_eq!(format_json_net(-86_400, 0), "1969-12-31T00:00:00");
    }

    #[test]
    fn the_fraction_keeps_meaningful_digits_and_drops_trailing_zeros() {
        let base = 1_000_000_000;
        assert_eq!(
            format_json_net(base, 512_000_000),
            "2001-09-09T01:46:40.512"
        );
        assert_eq!(format_json_net(base, 500_000_000), "2001-09-09T01:46:40.5");
        assert_eq!(format_json_net(base, 900_000_000), "2001-09-09T01:46:40.9");
        assert_eq!(format_json_net(base, 100), "2001-09-09T01:46:40.0000001");
    }

    #[test]
    fn a_whole_second_has_no_decimal_point_at_all() {
        let text = format_json_net(1_000_000_000, 0);
        assert!(!text.contains('.'), "khong duoc co dau cham: {text}");
    }

    #[test]
    fn sub_tick_precision_is_truncated_not_rounded() {
        // 150 ns < 1 tick (100 ns) * 2 → giữ đúng 1 tick, không làm tròn lên 2.
        assert_eq!(format_json_net(0, 150), "1970-01-01T00:00:00.0000001");
    }

    #[test]
    fn an_absurd_nanosecond_value_does_not_panic_or_overflow() {
        // Hàm công khai nên phải chịu được giá trị vô lý.
        let text = format_json_net(0, u32::MAX);
        assert!(text.starts_with("1970-01-01T00:00:00."));
        assert_eq!(text.len(), "1970-01-01T00:00:00.".len() + 7);
    }

    #[test]
    fn now_is_a_parseable_looking_timestamp() {
        let now = now_json_net();
        assert!(now.len() >= 19, "qua ngan: {now}");
        assert_eq!(now.as_bytes()[4], b'-');
        assert_eq!(now.as_bytes()[10], b'T');
        assert_eq!(now.as_bytes()[13], b':');
        assert_eq!(now.as_bytes()[16], b':');
    }
}
