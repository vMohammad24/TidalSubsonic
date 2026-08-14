use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::db::DbManager;
use crate::metrics::Metrics;
use crate::tidal::{
	error::TidalError,
	session::{Session, SessionOptions, TokenUpdate},
};
use crate::util::crypto::{decrypt_string, encrypt_string};
use chrono::{TimeZone, Utc};
use moka::future::Cache;
use moka::notification::RemovalCause;
use tokio::sync::mpsc::{Sender, channel};

#[derive(Clone)]
pub struct TidalClientManager {
	user_clients: Arc<RwLock<HashMap<String, Arc<Session>>>>,
	global_client: Arc<Session>,
	default_country_code: String,
	pub db: Arc<DbManager>,
	token_update_tx: Sender<TokenUpdate>,
	subsonic_user_cache: Cache<String, Arc<Session>>,
	metrics: Metrics,
}

use tokio_util::sync::CancellationToken;

impl TidalClientManager {
	pub fn new(
		default_country_code: &str,
		db: Arc<DbManager>,
		cancel_token: CancellationToken,
		metrics: Metrics,
	) -> (Self, tokio::task::JoinHandle<()>) {
		let (tx, mut rx) = channel::<TokenUpdate>(100);
		let db_clone = db.clone();

		let handle = tokio::spawn(async move {
			loop {
				tokio::select! {
					_ = cancel_token.cancelled() => {
						tracing::info!("Shutdown signal received: draining remaining token updates...");
						rx.close();
						while let Some(update) = rx.recv().await {
							let _ = Self::persist_token_update(&db_clone, update).await;
						}
						break;
					}
					maybe_update = rx.recv() => {
						match maybe_update {
							Some(update) => {
								let _ = Self::persist_token_update(&db_clone, update).await;
							}
							None => break,
						}
					}
				}
			}
		});

		let global_options = SessionOptions {
			country_code: Some(default_country_code.to_string()),
			..Default::default()
		};
		let global_client =
			metrics.observe_tidal_session(Session::new(global_options, Some(tx.clone())));
		let eviction_metrics = metrics.clone();
		let subsonic_user_cache = Cache::builder()
			.time_to_live(Duration::from_secs(300))
			.max_capacity(1000)
			.eviction_listener(move |_, _, cause| {
				let cause = match cause {
					RemovalCause::Expired => "expired",
					RemovalCause::Explicit => "explicit",
					RemovalCause::Replaced => "replaced",
					RemovalCause::Size => "size",
				};
				eviction_metrics.record_cache_eviction("subsonic_user", cause);
			})
			.build();

		(
			Self {
				user_clients: Arc::new(RwLock::new(HashMap::new())),
				global_client: Arc::new(global_client),
				default_country_code: default_country_code.to_string(),
				db,
				token_update_tx: tx,
				subsonic_user_cache,
				metrics,
			},
			handle,
		)
	}

	async fn persist_token_update(db: &DbManager, update: TokenUpdate) -> Result<(), sqlx::Error> {
		let user_id_str = update.user_id.to_string();
		let encrypted_access = encrypt_string(&update.access_token).ok();
		let encrypted_refresh = update
			.refresh_token
			.as_ref()
			.and_then(|t| encrypt_string(t).ok());

		db.save_tokens(
			&user_id_str,
			crate::db::StoredTokens {
				access_token: encrypted_access,
				refresh_token: encrypted_refresh,
				token_expiry: update
					.token_expiry
					.and_then(|ts| Utc.timestamp_opt(ts as i64, 0).single()),
				last_data_request: None,
			},
		)
		.await
	}

	pub fn get_global_client(&self) -> Arc<Session> {
		Arc::clone(&self.global_client)
	}

	pub async fn get_client_for_subsonic_user(
		&self,
		subsonic_username: &str,
	) -> Result<Arc<Session>, TidalError> {
		if let Some(client) = self.subsonic_user_cache.get(subsonic_username).await {
			self.record_subsonic_cache_access("hit");
			return Ok(client);
		}
		self.record_subsonic_cache_access("miss");

		let Some(tidal_id) = self
			.db
			.get_tidal_user_for_subsonic(subsonic_username)
			.await
			.map_err(|e| TidalError::Unexpected(e.to_string()))?
		else {
			return Err(TidalError::Authentication(
				"No Tidal account linked to this Subsonic user. Please link your account via the web UI.".to_string(),
			));
		};

		let client = self.get_client_for_tidal_user(&tidal_id).await?;
		self.subsonic_user_cache
			.insert(subsonic_username.to_string(), Arc::clone(&client))
			.await;
		self.update_cache_entries();
		Ok(client)
	}

	pub async fn get_client_for_tidal_user(
		&self,
		tidal_user_id: &str,
	) -> Result<Arc<Session>, TidalError> {
		let clients = self.user_clients.read().await;
		if let Some(client) = clients.get(tidal_user_id) {
			self.metrics
				.record_cache_access("tidal_user", "hit", clients.len() as u64);
			return Ok(Arc::clone(client));
		}
		self.metrics
			.record_cache_access("tidal_user", "miss", clients.len() as u64);
		drop(clients);

		let stored_tokens = self
			.db
			.get_tokens_by_tidal_id(tidal_user_id)
			.await
			.map_err(|e| TidalError::Unexpected(e.to_string()))?;

		let (access_token, refresh_token) = if let Some(tokens) = stored_tokens.as_ref() {
			let access = tokens
				.access_token
				.as_ref()
				.map(|t| decrypt_string(t).unwrap_or_else(|_| t.clone()));
			let refresh = tokens
				.refresh_token
				.as_ref()
				.map(|t| decrypt_string(t).unwrap_or_else(|_| t.clone()));
			(access, refresh)
		} else {
			(None, None)
		};

		let options = SessionOptions {
			country_code: Some(self.default_country_code.clone()),
			access_token,
			refresh_token,
			token_expiry: stored_tokens
				.as_ref()
				.and_then(|t| t.token_expiry.map(|dt| dt.timestamp() as u64)),
			..Default::default()
		};

		let mut session = self.metrics.observe_tidal_session(Session::new(
			options,
			Some(self.token_update_tx.clone()),
		));
		if let Ok(id) = tidal_user_id.parse::<i64>() {
			session.user_id = Some(id);
		}
		let client = Arc::new(session);

		let mut clients_write = self.user_clients.write().await;
		let client = clients_write
			.entry(tidal_user_id.to_string())
			.or_insert(client)
			.clone();
		self.metrics.set_tidal_clients_loaded(clients_write.len());
		Ok(client)
	}

	pub async fn save_tokens_for_tidal_user(
		&self,
		tidal_user_id: &str,
		access_token: String,
		refresh_token: Option<String>,
		token_expiry: Option<u64>,
	) -> Result<(), TidalError> {
		let encrypted_access = encrypt_string(&access_token).ok();
		let encrypted_refresh = refresh_token.as_ref().and_then(|t| encrypt_string(t).ok());

		self.db
			.save_tokens(
				tidal_user_id,
				crate::db::StoredTokens {
					access_token: encrypted_access,
					refresh_token: encrypted_refresh,
					token_expiry: token_expiry
						.and_then(|ts| Utc.timestamp_opt(ts as i64, 0).single()),
					last_data_request: None,
				},
			)
			.await
			.map_err(|e| TidalError::Unexpected(e.to_string()))
	}

	pub async fn clear_tokens_for_tidal_user(&self, tidal_user_id: &str) -> Result<(), TidalError> {
		let mut clients = self.user_clients.write().await;
		clients.remove(tidal_user_id);
		self.metrics.set_tidal_clients_loaded(clients.len());

		if let Ok(users) = self.db.list_users_for_tidal_account(tidal_user_id).await {
			for u in users {
				self.subsonic_user_cache.invalidate(&u).await;
			}
		}
		self.update_cache_entries();

		self.db
			.delete_tokens(tidal_user_id)
			.await
			.map_err(|e| TidalError::Unexpected(e.to_string()))
	}

	fn update_cache_entries(&self) {
		self.metrics
			.set_cache_entries("subsonic_user", self.subsonic_user_cache.entry_count());
	}

	fn record_subsonic_cache_access(&self, result: &'static str) {
		self.metrics.record_cache_access(
			"subsonic_user",
			result,
			self.subsonic_user_cache.entry_count(),
		);
	}
}
