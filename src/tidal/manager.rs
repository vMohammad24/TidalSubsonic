use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::db::DbManager;
use crate::tidal::{
	error::TidalError,
	session::{Session, SessionOptions, TokenUpdate},
};
use crate::util::crypto::{decrypt_string, encrypt_string};
use chrono::{TimeZone, Utc};
use moka::future::Cache;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

pub struct TidalClientManager {
	user_clients: Arc<RwLock<HashMap<String, Arc<Session>>>>,
	global_client: Arc<Session>,
	default_country_code: String,
	pub db: Arc<DbManager>,
	token_update_tx: UnboundedSender<TokenUpdate>,
	subsonic_user_cache: Cache<String, Arc<Session>>,
}

impl TidalClientManager {
	pub fn new(default_country_code: &str, db: Arc<DbManager>) -> Self {
		let (tx, mut rx) = unbounded_channel::<TokenUpdate>();
		let db_clone = db.clone();

		tokio::spawn(async move {
			while let Some(update) = rx.recv().await {
				let user_id_str = update.user_id.to_string();
				let encrypted_access = encrypt_string(&update.access_token).ok();
				let encrypted_refresh = update
					.refresh_token
					.as_ref()
					.and_then(|t| encrypt_string(t).ok());

				let _ = db_clone
					.save_tokens(
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
					.await;
			}
		});

		let global_options = SessionOptions {
			country_code: Some(default_country_code.to_string()),
			..Default::default()
		};

		Self {
			user_clients: Arc::new(RwLock::new(HashMap::new())),
			global_client: Arc::new(Session::new(global_options, Some(tx.clone()))),
			default_country_code: default_country_code.to_string(),
			db,
			token_update_tx: tx,
			subsonic_user_cache: Cache::builder()
				.time_to_live(Duration::from_secs(300))
				.max_capacity(1000)
				.build(),
		}
	}

	pub fn get_global_client(&self) -> Arc<Session> {
		Arc::clone(&self.global_client)
	}

	pub async fn get_client_for_subsonic_user(
		&self,
		subsonic_username: &str,
	) -> Result<Arc<Session>, TidalError> {
		if let Some(client) = self.subsonic_user_cache.get(subsonic_username).await {
			return Ok(client);
		}

		let tidal_id_opt = self
			.db
			.get_tidal_user_for_subsonic(subsonic_username)
			.await
			.map_err(|e| TidalError::Unexpected(e.to_string()))?;

		if let Some(tidal_id) = tidal_id_opt {
			let client = self.get_client_for_tidal_user(&tidal_id).await?;
			self.subsonic_user_cache
				.insert(subsonic_username.to_string(), Arc::clone(&client))
				.await;
			Ok(client)
		} else {
			Err(TidalError::Authentication(
				"No Tidal account linked to this Subsonic user. Please link your account via the web UI.".to_string(),
			))
		}
	}

	pub async fn get_client_for_tidal_user(
		&self,
		tidal_user_id: &str,
	) -> Result<Arc<Session>, TidalError> {
		let clients = self.user_clients.read().await;
		if let Some(client) = clients.get(tidal_user_id) {
			return Ok(Arc::clone(client));
		}
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

		let mut session = Session::new(options, Some(self.token_update_tx.clone()));
		if let Ok(id) = tidal_user_id.parse::<i64>() {
			session.user_id = Some(id);
		}
		let client = Arc::new(session);

		let mut clients_write = self.user_clients.write().await;
		clients_write.insert(tidal_user_id.to_string(), Arc::clone(&client));

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
		self.db
			.delete_tokens(tidal_user_id)
			.await
			.map_err(|e| TidalError::Unexpected(e.to_string()))
	}
}
