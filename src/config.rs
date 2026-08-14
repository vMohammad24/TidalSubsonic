use serde::Deserialize;

fn env_flag_enabled(value: Option<&str>) -> bool {
	value.is_some_and(|value| {
		matches!(
			value.trim().to_ascii_lowercase().as_str(),
			"1" | "true" | "yes"
		)
	})
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
	pub database_url: String,
	pub host: String,
	pub port: u16,
	pub default_country_code: String,
	pub request_timeout_secs: u64,
	pub max_http_retries: u32,
	pub dash_stream_concurrency: usize,
	pub cache_ttl_secs: u64,
	pub prometheus_enabled: bool,
}

impl AppConfig {
	pub fn from_env() -> Self {
		Self {
			database_url: std::env::var("DATABASE_URL")
				.unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/tss".into()),
			host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
			port: std::env::var("PORT")
				.unwrap_or_else(|_| "8080".into())
				.parse()
				.unwrap_or(8080),
			default_country_code: std::env::var("DEFAULT_COUNTRY_CODE")
				.unwrap_or_else(|_| "US".into()),
			request_timeout_secs: std::env::var("REQUEST_TIMEOUT_SECS")
				.unwrap_or_else(|_| "15".into())
				.parse()
				.unwrap_or(15),
			max_http_retries: std::env::var("MAX_HTTP_RETRIES")
				.unwrap_or_else(|_| "3".into())
				.parse()
				.unwrap_or(3),
			dash_stream_concurrency: std::env::var("DASH_STREAM_CONCURRENCY")
				.unwrap_or_else(|_| "15".into())
				.parse()
				.unwrap_or(15),
			cache_ttl_secs: std::env::var("CACHE_TTL_SECS")
				.unwrap_or_else(|_| "300".into())
				.parse()
				.unwrap_or(300),
			prometheus_enabled: env_flag_enabled(
				std::env::var("PROMETHEUS_ENABLED").ok().as_deref(),
			),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_app_config_from_env_defaults() {
		let config = AppConfig::from_env();
		assert!(!config.host.is_empty());
		assert!(config.port > 0);
		assert!(!config.default_country_code.is_empty());
		assert!(config.request_timeout_secs > 0);
	}

	#[test]
	fn test_env_flag_accepts_enabled_values() {
		for value in ["1", "true", "TRUE", "yes", " Yes "] {
			assert!(env_flag_enabled(Some(value)));
		}
	}

	#[test]
	fn test_env_flag_rejects_disabled_or_invalid_values() {
		for value in [None, Some("0"), Some("false"), Some("no"), Some("")] {
			assert!(!env_flag_enabled(value));
		}
	}
}
