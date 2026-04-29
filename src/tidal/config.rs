use std::env;
use std::sync::LazyLock;

pub const API_URL: &str = "https://api.tidal.com/v1";
pub const API_V2_URL: &str = "https://api.tidal.com/v2";
pub const OPENAPI_V2_URL: &str = "https://openapi.tidal.com/v2";
pub const DESKTOP_V1_URL: &str = "https://desktop.tidal.com/v1/";
pub const DESKTOP_V2_URL: &str = "https://desktop.tidal.com/v2/";
pub const AUTH_URL: &str = "https://auth.tidal.com/v1/oauth2";
pub const TIDAL_VERSION: &str = "2025.5.6";
pub const CLIENT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) TIDAL/9999.9999.9999 Chrome/126.0.6478.127 Electron/31.2.1 Safari/537.36";

pub static API_TOKEN: LazyLock<&'static str> = LazyLock::new(|| {
	env::var("TIDAL_API_TOKEN")
		.expect("FATAL: TIDAL_API_TOKEN environment variable is missing")
		.leak()
});

pub static CLIENT_ID: LazyLock<&'static str> = LazyLock::new(|| {
	env::var("TIDAL_CLIENT_ID")
		.expect("FATAL: TIDAL_CLIENT_ID environment variable is missing")
		.leak()
});

pub static CLIENT_SECRET: LazyLock<&'static str> = LazyLock::new(|| {
	env::var("TIDAL_CLIENT_SECRET")
		.expect("FATAL: TIDAL_CLIENT_SECRET environment variable is missing")
		.leak()
});
