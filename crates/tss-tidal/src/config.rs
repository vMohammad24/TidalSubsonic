use std::env;
use std::sync::LazyLock;

pub const API_URL: &str = "https://api.tidal.com/v1";
pub const API_V2_URL: &str = "https://api.tidal.com/v2";
pub const OPENAPI_V2_URL: &str = "https://openapi.tidal.com/v2";
pub const DESKTOP_V1_URL: &str = "https://desktop.tidal.com/v1/";
pub const DESKTOP_V2_URL: &str = "https://desktop.tidal.com/v2/";
pub const AUTH_URL: &str = "https://auth.tidal.com/v1/oauth2";
pub const TIDAL_VERSION: &str = "2025.5.6";
pub const CLIENT_USER_AGENT: &str = concat!(
	"Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 ",
	"(KHTML, like Gecko) TIDAL/9999.9999.9999 Chrome/126.0.6478.127 ",
	"Electron/31.2.1 Safari/537.36"
);

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

#[derive(Debug)]
pub struct TidalCredentials {
	pub api_token: String,
	pub client_id: String,
	pub client_secret: String,
}

impl TidalCredentials {
	pub fn from_env() -> Result<Self, String> {
		let mut missing_iter = ["TIDAL_API_TOKEN", "TIDAL_CLIENT_ID", "TIDAL_CLIENT_SECRET"]
			.into_iter()
			.filter(|&key| env::var(key).is_err());

		if let Some(first) = missing_iter.next() {
			let missing: Vec<_> = std::iter::once(first).chain(missing_iter).collect();
			return Err(format!("Missing env vars: {:?}", missing));
		}

		Ok(Self {
			api_token: env::var("TIDAL_API_TOKEN").unwrap(),
			client_id: env::var("TIDAL_CLIENT_ID").unwrap(),
			client_secret: env::var("TIDAL_CLIENT_SECRET").unwrap(),
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_tidal_credentials_from_env() {
		dotenvy::dotenv().ok();
		let result = TidalCredentials::from_env();
		let has_all_vars = env::var("TIDAL_API_TOKEN").is_ok()
			&& env::var("TIDAL_CLIENT_ID").is_ok()
			&& env::var("TIDAL_CLIENT_SECRET").is_ok();

		if has_all_vars {
			assert!(result.is_ok(), "Expected valid credentials from env");
			let creds = result.unwrap();
			assert!(!creds.api_token.is_empty());
			assert!(!creds.client_id.is_empty());
			assert!(!creds.client_secret.is_empty());
		} else {
			assert!(result.is_err(), "Expected error when env vars are missing");
			let err_msg = result.unwrap_err();
			assert!(err_msg.contains("Missing env vars"));
		}
	}
}
