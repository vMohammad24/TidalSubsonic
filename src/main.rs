use actix_session::SessionMiddleware;
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::Key;
use actix_web::middleware::Condition;
use actix_web::{App, HttpServer, web};
use actix_web_prom::PrometheusMetricsBuilder;
use dotenvy::dotenv;
use sha2::{Digest, Sha512};
use std::env;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{Level, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

mod api;
mod config;
mod db;
mod metrics;
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

	let app_config = config::AppConfig::from_env();

	match tidal::config::TidalCredentials::from_env() {
		Ok(creds) => {
			info!(
				"Tidal credentials validated (API Token len: {}, Client ID: {}, Client Secret len: {})",
				creds.api_token.len(),
				creds.client_id,
				creds.client_secret.len()
			);
		}
		Err(e) => {
			tracing::warn!("Tidal credentials warning: {}", e);
		}
	}

	info!("Connecting to database...");
	let db_manager = match db::DbManager::new(&app_config.database_url).await {
		Ok(db) => db,
		Err(e) => {
			tracing::error!("Failed to connect to database: {}", e);
			std::process::exit(1);
		}
	};

	sqlx::migrate!("./migrations")
		.run(&db_manager.pool)
		.await
		.unwrap_or_else(|e| {
			tracing::error!("Failed to run database migrations: {}", e);
			std::process::exit(1);
		});

	let secret_key = match env::var("SESSION_SECRET") {
		Ok(s) => {
			if s.len() < 64 {
				panic!(
					"SESSION_SECRET is too short ({} chars), it should be at least 64 characters for security.",
					s.len()
				);
			}
			let mut hasher = Sha512::new();
			hasher.update(s.as_bytes());
			Key::from(&hasher.finalize())
		}
		Err(_) => {
			if cfg!(debug_assertions) {
				info!("No SESSION_SECRET found, generating temporary key for development");
				Key::generate()
			} else {
				panic!(
					"SESSION_SECRET environment variable is required in production (min 64 chars recommended)"
				);
			}
		}
	};

	let cancel_token = CancellationToken::new();
	let metrics = metrics::Metrics::new(app_config.prometheus_enabled)
		.map_err(|error| std::io::Error::other(format!("failed to initialize metrics: {error}")))?;
	let (tidal_manager, token_drain_handle) = tidal::manager::TidalClientManager::new(
		&app_config.default_country_code,
		Arc::new(db_manager.clone()),
		cancel_token.clone(),
		metrics.clone(),
	);

	let bind_addr = format!("{}:{}", app_config.host, app_config.port);
	info!("Starting server on {}", bind_addr);
	let prometheus_enabled = app_config.prometheus_enabled;
	let prometheus = PrometheusMetricsBuilder::new("tss")
		.exclude("/metrics")
		.mask_unmatched_patterns("UNKNOWN")
		.registry(metrics.registry.clone())
		.build()
		.map_err(|error| std::io::Error::other(format!("failed to initialize metrics: {error}")))?;
	if prometheus_enabled {
		info!("Prometheus metrics enabled at /metrics");
	}

	let cancel_token_clone = cancel_token.clone();
	tokio::spawn(async move {
		if let Ok(()) = tokio::signal::ctrl_c().await {
			info!("Ctrl+C signal received, triggering shutdown token...");
			cancel_token_clone.cancel();
		}
	});

	let app_config_data = web::Data::new(app_config);
	let tidal_manager_data = web::Data::new(tidal_manager);
	let db_manager_data = web::Data::new(db_manager);
	let metrics_data = web::Data::new(metrics);

	let server_res = HttpServer::new(move || {
		App::new()
			.wrap(Condition::new(prometheus_enabled, prometheus.clone()))
			.wrap(SessionMiddleware::new(
				CookieSessionStore::default(),
				secret_key.clone(),
			))
			.app_data(app_config_data.clone())
			.app_data(tidal_manager_data.clone())
			.app_data(db_manager_data.clone())
			.app_data(metrics_data.clone())
			.route("/metrics", web::get().to(metrics::endpoint))
			.route("/health", web::get().to(api::health::health_check))
			.configure(api::auth::config)
			.configure(api::lastfm::config)
			.configure(api::subsonic::config)
			.configure(api::ui::config)
			.configure(api::users::config)
	})
	.shutdown_timeout(30)
	.bind(bind_addr)?
	.run()
	.await;

	cancel_token.cancel();
	let _ = token_drain_handle.await;

	server_res
}
