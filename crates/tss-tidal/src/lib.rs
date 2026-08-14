pub mod api;
pub mod config;
pub mod error;
pub mod favorites;
pub mod http_client;
pub mod models;
pub mod observability;
pub mod session;

pub mod tidal {
	pub use crate::{api, config, error, favorites, models, observability, session};
}
mod util {
	pub use crate::http_client::http_client;
}
