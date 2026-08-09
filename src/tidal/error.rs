use actix_web::{HttpResponse, ResponseError, http::StatusCode};
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

impl ResponseError for TidalError {
	fn status_code(&self) -> StatusCode {
		match self {
			TidalError::Authentication(_) => StatusCode::UNAUTHORIZED,
			TidalError::ResourceNotFound(_, _) => StatusCode::NOT_FOUND,
			TidalError::RateLimit => StatusCode::TOO_MANY_REQUESTS,
			TidalError::PaymentRequired => StatusCode::PAYMENT_REQUIRED,
			TidalError::ApiError(code, _) => {
				StatusCode::from_u16(*code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
			}
			_ => StatusCode::INTERNAL_SERVER_ERROR,
		}
	}

	fn error_response(&self) -> HttpResponse {
		HttpResponse::build(self.status_code()).json(serde_json::json!({
			"error": self.to_string()
		}))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_tidal_error_status_codes() {
		assert_eq!(
			TidalError::Authentication("token expired".into()).status_code(),
			StatusCode::UNAUTHORIZED
		);
		assert_eq!(
			TidalError::ResourceNotFound("Track".into(), "123".into()).status_code(),
			StatusCode::NOT_FOUND
		);
		assert_eq!(
			TidalError::RateLimit.status_code(),
			StatusCode::TOO_MANY_REQUESTS
		);
		assert_eq!(
			TidalError::PaymentRequired.status_code(),
			StatusCode::PAYMENT_REQUIRED
		);
		assert_eq!(
			TidalError::ApiError(503, "Service Unavailable".into()).status_code(),
			StatusCode::SERVICE_UNAVAILABLE
		);
	}
}
