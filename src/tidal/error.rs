use thiserror::Error;

#[derive(Error, Debug)]
pub enum TidalError {
	#[error("Authentication failed: {0}")]
	Authentication(String),

	#[error("Resource not found: {0} with ID {1}")]
	ResourceNotFound(String, String),

	#[error("API rate limit exceeded")]
	RateLimit,
	#[error("Payment required to access this content")]
	PaymentRequired,

	#[error("Network or HTTP request error: {0}")]
	Http(#[from] reqwest::Error),

	#[error("Failed to parse or serialize data: {0}")]
	Parse(#[from] serde_json::Error),

	#[error("API request failed with status {0}: {1}")]
	ApiError(u16, String),

	#[error("An unexpected error occurred: {0}")]
	Unexpected(String),
}
