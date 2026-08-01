use serde::Deserialize;

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
		}
	}
}
