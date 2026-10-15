pub mod cdp;
pub mod detect;
pub mod launch;
pub mod login;
pub mod ws;

#[cfg(feature = "http-reqwest")]
pub mod download;

pub const LOGIN_PROFILE_NAME: &str = "login";

#[must_use]
pub fn default_login_url() -> &'static str {
    launch::ROBLOX_LOGIN_URL
}
