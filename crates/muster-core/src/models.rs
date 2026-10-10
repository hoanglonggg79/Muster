//! # `models` — hình dạng dữ liệu dùng chung
//!
//! [`Account`] là bản sao trung thực của `Classes/Account.cs` trong `RAM_Source/` ở **góc nhìn
//! serialization**: đúng những field mà Json.NET ghi ra `AccountData.json`, đúng tên, đúng kiểu.
//!
//! Đây là **hợp đồng giữa ba phía**: `core::vault` (đọc/ghi file), `muster-cli` (báo cáo nhập),
//! và nhánh UI (qua `tauri-specta`). Vì vậy nó nằm ở module riêng chứ không nằm trong `vault`.
//!
//! ## Vì sao không dùng kiểu ngày tháng có cấu trúc
//!
//! `LastUse` và `LastAttemptedRefresh` là `DateTime` trong C#, nhưng Json.NET ghi ra với
//! **độ chính xác biến đổi** (`yyyy-MM-ddTHH:mm:ss.FFFFFFF`, cắt số 0 thừa ở cuối). Mô hình hoá
//! lại bằng `chrono` sẽ phải tái lập đúng quy tắc cắt đó, và chỉ cần lệch một chữ số là file ghi
//! ra không còn giống định dạng RAM nữa. Nguyên tắc bất biến #2 nói tương thích ngược quan trọng
//! hơn vẻ đẹp của code, nên ở M0 hai field này giữ nguyên dạng chuỗi — round-trip là lossless
//! tuyệt đối. Việc chuyển sang kiểu có cấu trúc để lại cho M1, khi có nhu cầu sắp xếp theo thời gian.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Một tài khoản, khớp `RBX_Alt_Manager.Account`.
///
/// Mọi field đều có `#[serde(default)]` để một file bị sửa tay hoặc thiếu field vẫn đọc được
/// thay vì làm hỏng cả lần nhập.
///
/// Các field `[JsonIgnore]` của bản C# (`PinUnlocked`, `TokenSet`, `LastAppLaunch`, `CSRFToken`,
/// `Presence`) **không** có mặt ở đây — chúng không bao giờ được ghi xuống đĩa.
///
/// `Debug` được viết tay chứ không derive: `security_token` và `password` là secret, in ra log
/// là rò rỉ (nguyên tắc bất biến #4).
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Account {
    /// Cờ "tài khoản còn dùng được". `Account.Valid`.
    #[serde(rename = "Valid", default)]
    pub valid: bool,

    /// Cookie `.ROBLOSECURITY`. **Là secret** — không bao giờ được ghi vào log.
    #[serde(rename = "SecurityToken", default)]
    pub security_token: Option<String>,

    /// Tên đăng nhập Roblox. `Account.Username`.
    #[serde(rename = "Username", default)]
    pub username: Option<String>,

    /// Thời điểm dùng lần cuối. Chuỗi ISO 8601 do Json.NET sinh ra — xem ghi chú đầu module.
    #[serde(rename = "LastUse", default)]
    pub last_use: String,

    /// Tên hiển thị do người dùng đặt. Tối đa 50 ký tự theo `Account.cs:56`.
    #[serde(rename = "Alias", default)]
    pub alias: String,

    /// Ghi chú tự do. Tối đa 5000 ký tự theo `Account.cs:69`.
    #[serde(rename = "Description", default)]
    pub description: String,

    /// Mật khẩu đăng nhập, nếu người dùng bật `SavePasswords`. **Là secret**.
    #[serde(rename = "Password", default)]
    pub password: String,

    /// Nhóm để lọc và sắp xếp.
    ///
    /// `Account.cs:29` khai báo `[JsonProperty(NullValueHandling = NullValueHandling.Ignore)]`
    /// với giá trị mặc định `"Default"` — nghĩa là khi `null` thì **field bị bỏ hẳn khỏi JSON**.
    /// `Option` + `skip_serializing_if` tái lập đúng hành vi đó.
    #[serde(rename = "Group", default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,

    /// UserID Roblox. `long` trong C#.
    #[serde(rename = "UserID", default)]
    pub user_id: i64,

    /// Túi field tuỳ ý. RAM lưu vị trí cửa sổ ở đây
    /// (`Window_Position_X/Y`, `Window_Width/Height`) cùng mọi field người dùng tự thêm.
    ///
    /// **Phải giữ nguyên vẹn** — mất túi này là mất dữ liệu người dùng (nguyên tắc bất biến #2).
    #[serde(rename = "Fields", default)]
    pub fields: BTreeMap<String, String>,

    /// Lần cuối thử refresh cookie. Chuỗi ISO 8601, xem ghi chú đầu module.
    #[serde(rename = "LastAttemptedRefresh", default)]
    pub last_attempted_refresh: String,

    /// ID dùng cho cờ `-b` khi mở nhiều instance. `Account.cs:50`.
    #[serde(rename = "BrowserTrackerID", default)]
    pub browser_tracker_id: Option<String>,

    /// Mọi khoá JSON không nằm trong danh sách trên.
    ///
    /// RAM bản hiện tại không ghi field lạ, nhưng một bản RAM khác hoặc một công cụ bên thứ ba có
    /// thể ghi thêm. Giữ lại thay vì nuốt mất — cùng lý do với [`Account::fields`].
    ///
    /// Không xuất sang TypeScript: đây là túi chứa dữ liệu chưa biết hình dạng, UI không thể dùng
    /// và cũng không nên dùng. Vẫn được giữ nguyên khi ghi lại xuống đĩa.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl std::fmt::Debug for Account {
    /// Bản `Debug` đã khử secret.
    ///
    /// Chỉ hiện sự **tồn tại** của cookie/mật khẩu, không hiện giá trị. Nhờ vậy
    /// `tracing::debug!(?account)` an toàn, và test redaction ở bước 0.8 có cái để bám vào.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account")
            .field("valid", &self.valid)
            .field("username", &self.username)
            .field("user_id", &self.user_id)
            .field("alias", &self.alias)
            .field("group", &self.group)
            .field(
                "security_token",
                &self.security_token.as_ref().map(|_| "<đã ẩn>"),
            )
            .field(
                "password",
                &if self.password.is_empty() {
                    "<rỗng>"
                } else {
                    "<đã ẩn>"
                },
            )
            .field("fields", &self.fields.len())
            .finish_non_exhaustive()
    }
}

impl Account {
    /// Tên để hiển thị: `Alias` nếu có, ngược lại là `Username`.
    #[must_use]
    pub fn display_name(&self) -> &str {
        if self.alias.is_empty() {
            self.username.as_deref().unwrap_or_default()
        } else {
            &self.alias
        }
    }

    /// Nhóm của tài khoản, đã áp dụng giá trị mặc định `"Default"` của RAM.
    #[must_use]
    pub fn group_or_default(&self) -> &str {
        self.group.as_deref().unwrap_or("Default")
    }

    /// Cookie `.ROBLOSECURITY`, nếu có. Trả `None` khi field vắng hoặc rỗng.
    #[must_use]
    pub fn cookie(&self) -> Option<&str> {
        self.security_token
            .as_deref()
            .filter(|token| !token.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Account {
        Account {
            valid: true,
            security_token: Some("COOKIE".to_owned()),
            username: Some("alice".to_owned()),
            last_use: "2026-09-28T21:14:03.512".to_owned(),
            alias: String::new(),
            description: String::new(),
            password: String::new(),
            group: None,
            user_id: 100_000_001,
            fields: BTreeMap::new(),
            last_attempted_refresh: String::new(),
            browser_tracker_id: None,
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn field_names_match_the_csharp_model_exactly() {
        let json = serde_json::to_string(&sample()).expect("serialize");
        for name in [
            "Valid",
            "SecurityToken",
            "Username",
            "LastUse",
            "Alias",
            "Description",
            "Password",
            "UserID",
            "Fields",
            "LastAttemptedRefresh",
            "BrowserTrackerID",
        ] {
            assert!(
                json.contains(&format!("\"{name}\"")),
                "thieu field {name}: {json}"
            );
        }
        assert!(
            !json.contains("\"PinUnlocked\""),
            "field [JsonIgnore] khong duoc ghi ra"
        );
        assert!(
            !json.contains("\"CSRFToken\""),
            "field [JsonIgnore] khong duoc ghi ra"
        );
    }

    #[test]
    fn a_null_group_is_omitted_like_ram_does() {
        let json = serde_json::to_string(&sample()).expect("serialize");
        assert!(
            !json.contains("\"Group\""),
            "Group null phai bi bo di: {json}"
        );

        let mut with_group = sample();
        with_group.group = Some("007 Bank".to_owned());
        let json = serde_json::to_string(&with_group).expect("serialize");
        assert!(json.contains("\"Group\":\"007 Bank\""));
    }

    #[test]
    fn unknown_fields_survive_a_round_trip() {
        let raw = r#"{"Username":"alice","SomeFutureField":[1,2,3]}"#;
        let account: Account = serde_json::from_str(raw).expect("deserialize");
        assert_eq!(account.username.as_deref(), Some("alice"));
        assert!(account.extra.contains_key("SomeFutureField"));

        let json = serde_json::to_string(&account).expect("serialize");
        assert!(
            json.contains("SomeFutureField"),
            "field la phai duoc giu lai: {json}"
        );
    }

    #[test]
    fn a_missing_field_does_not_break_deserialization() {
        let account: Account = serde_json::from_str(r#"{"Username":"bob"}"#).expect("deserialize");
        assert_eq!(account.username.as_deref(), Some("bob"));
        assert!(!account.valid);
        assert_eq!(account.user_id, 0);
    }

    #[test]
    fn display_name_prefers_alias_and_group_falls_back_to_default() {
        let mut account = sample();
        assert_eq!(account.display_name(), "alice");
        assert_eq!(account.group_or_default(), "Default");

        account.alias = "Alice (main)".to_owned();
        account.group = Some("1Main".to_owned());
        assert_eq!(account.display_name(), "Alice (main)");
        assert_eq!(account.group_or_default(), "1Main");
    }

    #[test]
    fn cookie_is_none_when_absent_or_empty() {
        let mut account = sample();
        assert_eq!(account.cookie(), Some("COOKIE"));

        account.security_token = Some(String::new());
        assert_eq!(account.cookie(), None);

        account.security_token = None;
        assert_eq!(account.cookie(), None);
    }

    #[test]
    fn debug_redacts_the_cookie_and_the_password() {
        let mut account = sample();
        account.password = "mat-khau-that".to_owned();

        let text = format!("{account:?}");
        assert!(!text.contains("COOKIE"), "lo cookie trong Debug: {text}");
        assert!(
            !text.contains("mat-khau-that"),
            "lo mat khau trong Debug: {text}"
        );
        assert!(text.contains("alice"), "van phai hien phan khong bi mat");
    }
}
