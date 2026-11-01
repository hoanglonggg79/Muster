use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Account {
    #[serde(rename = "Valid", default)]
    pub valid: bool,

    #[serde(rename = "SecurityToken", default)]
    pub security_token: Option<String>,

    #[serde(rename = "Username", default)]
    pub username: Option<String>,

    #[serde(rename = "LastUse", default)]
    pub last_use: String,

    #[serde(rename = "Alias", default)]
    pub alias: String,

    #[serde(rename = "Description", default)]
    pub description: String,

    #[serde(rename = "Password", default)]
    pub password: String,

    #[serde(rename = "Group", default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,

    #[serde(rename = "UserID", default)]
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub user_id: i64,

    #[serde(rename = "Fields", default)]
    pub fields: BTreeMap<String, String>,

    #[serde(rename = "LastAttemptedRefresh", default)]
    pub last_attempted_refresh: String,

    #[serde(rename = "BrowserTrackerID", default)]
    pub browser_tracker_id: Option<String>,

    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl std::fmt::Debug for Account {
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
    #[must_use]
    pub fn display_name(&self) -> &str {
        if self.alias.is_empty() {
            self.username.as_deref().unwrap_or_default()
        } else {
            &self.alias
        }
    }

    #[must_use]
    pub fn group_or_default(&self) -> &str {
        self.group.as_deref().unwrap_or("Default")
    }

    #[must_use]
    pub fn cookie(&self) -> Option<&str> {
        self.security_token
            .as_deref()
            .filter(|token| !token.is_empty())
    }

    #[must_use]
    pub fn get_field(&self, name: &str) -> &str {
        self.fields.get(name).map_or("", String::as_str)
    }

    pub fn set_field(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.fields.insert(name.into(), value.into());
    }

    pub fn remove_field(&mut self, name: &str) -> Option<String> {
        self.fields.remove(name)
    }

    pub fn apply_profile(
        &mut self,
        username: String,
        display_name: String,
        robux: Option<u64>,
        avatar_url: Option<String>,
    ) {
        self.username = Some(username);
        self.set_field("DisplayName", display_name);
        if let Some(robux) = robux {
            self.set_field("Robux", robux.to_string());
        }
        if let Some(url) = avatar_url {
            self.set_field("AvatarUrl", url);
        }
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
    fn field_helpers_mirror_ram_behaviour() {
        let mut account = sample();

        assert_eq!(
            account.get_field("Window_Width"),
            "",
            "field vang phai tra ve chuoi rong, khong phai panic"
        );

        account.set_field("Window_Width", "880");
        assert_eq!(account.get_field("Window_Width"), "880");

        account.set_field("Window_Width", "1024");
        assert_eq!(
            account.get_field("Window_Width"),
            "1024",
            "ghi de phai thay gia tri cu"
        );

        assert_eq!(
            account.remove_field("Window_Width"),
            Some("1024".to_owned())
        );
        assert_eq!(account.get_field("Window_Width"), "");
        assert_eq!(account.remove_field("Window_Width"), None);
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

    #[test]
    fn apply_profile_keeps_previous_robux_and_avatar_when_fetch_failed() {
        let mut account = sample();
        account.set_field("Robux", "1500");
        account.set_field("AvatarUrl", "https://old.example/avatar.png");

        account.apply_profile("alice2".to_owned(), "Alice Two".to_owned(), None, None);

        assert_eq!(account.username.as_deref(), Some("alice2"));
        assert_eq!(account.get_field("DisplayName"), "Alice Two");
        assert_eq!(
            account.get_field("Robux"),
            "1500",
            "loi mang khong duoc ghi de so du bang 0"
        );
        assert_eq!(
            account.get_field("AvatarUrl"),
            "https://old.example/avatar.png"
        );
    }

    #[test]
    fn apply_profile_writes_a_real_zero_balance() {
        let mut account = sample();
        account.set_field("Robux", "1500");

        account.apply_profile("alice".to_owned(), "Alice".to_owned(), Some(0), None);

        assert_eq!(account.get_field("Robux"), "0");
    }

    #[test]
    fn apply_profile_writes_robux_and_avatar_when_present() {
        let mut account = sample();

        account.apply_profile(
            "alice".to_owned(),
            "Alice".to_owned(),
            Some(42),
            Some("https://new.example/avatar.png".to_owned()),
        );

        assert_eq!(account.get_field("Robux"), "42");
        assert_eq!(
            account.get_field("AvatarUrl"),
            "https://new.example/avatar.png"
        );
    }
}
