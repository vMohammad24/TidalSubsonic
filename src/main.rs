use actix_web::{App, HttpServer, web};
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use tracing::{Level, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

mod api;
mod db;
mod tidal;
mod util;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	dotenv().ok();
	let subscriber = FmtSubscriber::builder()
		.with_env_filter(
			EnvFilter::builder()
				.with_default_directive(Level::INFO.into())
				.from_env_lossy(),
		)
		.finish();
	let _ = tracing::subscriber::set_global_default(subscriber);

	let database_url = env::var("DATABASE_URL")
		.unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/tss".to_string());

	info!("Connecting to database...");
	let db_manager = match db::DbManager::new(&database_url).await {
		Ok(db) => Arc::new(db),
		Err(e) => {
			tracing::error!("Failed to connect to database: {}", e);
			std::process::exit(1);
		}
	};

	let default_country_code =
		env::var("DEFAULT_COUNTRY_CODE").unwrap_or_else(|_| "US".to_string());
	let tidal_manager = Arc::new(tidal::manager::TidalClientManager::new(
		&default_country_code,
		db_manager.clone(),
	));

	let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
	let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
	let bind_addr = format!("{}:{}", host, port);

	info!("Starting server on {}", bind_addr);

	HttpServer::new(move || {
		App::new()
			.app_data(web::Data::new(tidal_manager.clone()))
			.app_data(web::Data::new(db_manager.clone()))
			.configure(api::auth::config)
			.configure(api::lastfm::config)
			.configure(api::subsonic::config)
			.configure(api::ui::config)
			.configure(api::users::config)
	})
	.bind(bind_addr)?
	.run()
	.await
}
