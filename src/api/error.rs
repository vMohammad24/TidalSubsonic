use crate::tidal::error::TidalError;
use thiserror::Error;

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum AppError {
	#[error("Tidal API error: {0}")]
	Tidal(#[from] TidalError),

	#[error("Database error: {0}")]
	Database(#[from] sqlx::Error),

	#[error("Not found: {0}")]
	NotFound(String),

	#[error("Bad request: {0}")]
	BadRequest(String),

	#[error("Authentication error: {0}")]
	Auth(String),

	#[error("Internal error: {0}")]
	Internal(String),
}

impl AppError {
	pub fn into_subsonic_code_and_msg(self) -> (i32, std::borrow::Cow<'static, str>) {
		match self {
			AppError::Tidal(e) => {
				tracing::error!("Upstream Tidal Error: {:?}", e);
				let msg = if cfg!(debug_assertions) {
					format!("Tidal API Error: {:?}", e).into()
				} else {
					"Upstream dependency failed".into()
				};
				(0, msg)
			}
			AppError::Database(e) => {
				tracing::error!("Database Error: {:?}", e);
				(0, "Internal server error".into())
			}
			AppError::NotFound(msg) => (70, msg.into()),
			AppError::BadRequest(msg) => (10, msg.into()),
			AppError::Auth(msg) => (40, msg.into()),
			AppError::Internal(msg) => (0, msg.into()),
		}
	}
}
