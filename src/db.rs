use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredTokens {
	pub access_token: Option<String>,
	pub refresh_token: Option<String>,
	pub token_expiry: Option<DateTime<Utc>>,
	pub last_data_request: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PlayQueue {
	pub username: String,
	pub current_track_id: Option<String>,
	pub position_ms: Option<i64>,
	pub track_ids: Vec<String>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct DbManager {
	pub pool: Pool<Postgres>,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct UserExportData {
	pub username: String,
	pub tidal_user_id: String,
	pub use_playlists: bool,
	pub use_favorites: bool,
	pub lastfm_username: Option<String>,
}

impl DbManager {
	pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
		let pool = PgPoolOptions::new()
			.max_connections(25)
			.connect(database_url)
			.await?;
		Ok(Self { pool })
	}

	pub async fn get_tokens_by_tidal_id(
		&self,
		tidal_id: &str,
	) -> Result<Option<StoredTokens>, sqlx::Error> {
		sqlx::query_as::<_, StoredTokens>(
			"SELECT access_token, refresh_token, token_expiry, last_data_request FROM tidal_tokens WHERE tidal_user_id = $1",
		)
		.bind(tidal_id)
		.fetch_optional(&self.pool)
		.await
	}

	pub async fn save_tokens(
		&self,
		tidal_id: &str,
		tokens: StoredTokens,
	) -> Result<(), sqlx::Error> {
		sqlx::query(
			"INSERT INTO tidal_tokens (tidal_user_id, access_token, refresh_token, token_expiry, last_data_request)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (tidal_user_id) DO UPDATE SET
                 access_token = EXCLUDED.access_token,
                 refresh_token = EXCLUDED.refresh_token,
                 token_expiry = EXCLUDED.token_expiry,
                 last_data_request = COALESCE(EXCLUDED.last_data_request, tidal_tokens.last_data_request)",
		)
		.bind(tidal_id)
		.bind(tokens.access_token)
		.bind(tokens.refresh_token)
		.bind(tokens.token_expiry)
		.bind(tokens.last_data_request)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	pub async fn delete_tokens(&self, tidal_id: &str) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM tidal_tokens WHERE tidal_user_id = $1")
			.bind(tidal_id)
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	pub async fn get_tidal_user_for_subsonic(
		&self,
		subsonic_username: &str,
	) -> Result<Option<String>, sqlx::Error> {
		let row: Option<(String,)> =
			sqlx::query_as("SELECT tidal_user_id FROM subsonic_users WHERE username = $1")
				.bind(subsonic_username)
				.fetch_optional(&self.pool)
				.await?;

		Ok(row.map(|r| r.0))
	}

	pub async fn create_user(
		&self,
		username: &str,
		tidal_user_id: &str,
		encrypted_password: Option<&str>,
		use_playlists: bool,
		use_favorites: bool,
	) -> Result<(), sqlx::Error> {
		sqlx::query("INSERT INTO subsonic_users (username, tidal_user_id, password, use_playlists, use_favorites) VALUES ($1, $2, $3, $4, $5)")
            .bind(username)
            .bind(tidal_user_id)
            .bind(encrypted_password)
            .bind(use_playlists)
            .bind(use_favorites)
            .execute(&self.pool)
            .await?;
		Ok(())
	}

	pub async fn delete_user(&self, username: &str) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM subsonic_users WHERE username = $1")
			.bind(username)
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	pub async fn list_users_for_tidal_account(
		&self,
		tidal_user_id: &str,
	) -> Result<Vec<String>, sqlx::Error> {
		let rows: Vec<(String,)> =
			sqlx::query_as("SELECT username FROM subsonic_users WHERE tidal_user_id = $1")
				.bind(tidal_user_id)
				.fetch_all(&self.pool)
				.await?;

		Ok(rows.into_iter().map(|r| r.0).collect())
	}

	pub async fn update_user_feature_flags(
		&self,
		username: &str,
		use_playlists: bool,
		use_favorites: bool,
	) -> Result<bool, sqlx::Error> {
		let result = sqlx::query(
			"UPDATE subsonic_users SET use_playlists = $1, use_favorites = $2 WHERE username = $3",
		)
		.bind(use_playlists)
		.bind(use_favorites)
		.bind(username)
		.execute(&self.pool)
		.await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn get_user_details(
		&self,
		username: &str,
	) -> Result<Option<(String, String, bool, bool)>, sqlx::Error> {
		sqlx::query_as(
            "SELECT tidal_user_id, COALESCE(password, ''), use_playlists, use_favorites FROM subsonic_users WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
	}

	pub async fn link_lastfm_account(
		&self,
		subsonic_username: &str,
		session_key: &str,
		lastfm_username: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query("INSERT INTO user_lastfm_links (subsonic_username, lastfm_session_key, lastfm_username) VALUES ($1, $2, $3) ON CONFLICT (subsonic_username) DO UPDATE SET lastfm_session_key = EXCLUDED.lastfm_session_key, lastfm_username = EXCLUDED.lastfm_username")
            .bind(subsonic_username)
            .bind(session_key)
            .bind(lastfm_username)
            .execute(&self.pool)
            .await?;
		Ok(())
	}

	pub async fn get_lastfm_details(
		&self,
		subsonic_username: &str,
	) -> Result<Option<(String, String)>, sqlx::Error> {
		sqlx::query_as(
			"SELECT lastfm_session_key, lastfm_username FROM user_lastfm_links WHERE subsonic_username = $1",
		)
		.bind(subsonic_username)
		.fetch_optional(&self.pool)
		.await
	}

	#[allow(dead_code)]
	pub async fn unlink_lastfm_account(&self, subsonic_username: &str) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM user_lastfm_links WHERE subsonic_username = $1")
			.bind(subsonic_username)
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	pub async fn save_web_session(
		&self,
		session_id: &str,
		tidal_user_id: &str,
		username: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query(
			"INSERT INTO web_sessions (session_id, tidal_user_id, username)
             VALUES ($1, $2, $3)
             ON CONFLICT (session_id) DO UPDATE SET
                 tidal_user_id = EXCLUDED.tidal_user_id,
                 username = EXCLUDED.username",
		)
		.bind(session_id)
		.bind(tidal_user_id)
		.bind(username)
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	pub async fn get_web_session(
		&self,
		session_id: &str,
	) -> Result<Option<(String, String)>, sqlx::Error> {
		let row: Option<(String, String)> = sqlx::query_as(
			"SELECT tidal_user_id, username FROM web_sessions WHERE session_id = $1",
		)
		.bind(session_id)
		.fetch_optional(&self.pool)
		.await?;

		Ok(row)
	}

	pub async fn delete_web_session(&self, session_id: &str) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM web_sessions WHERE session_id = $1")
			.bind(session_id)
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	pub async fn save_play_queue(&self, queue: &PlayQueue) -> Result<(), sqlx::Error> {
		sqlx::query(
            "INSERT INTO play_queues (username, current_track_id, position_ms, track_ids, updated_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (username) DO UPDATE SET
                 current_track_id = EXCLUDED.current_track_id,
                 position_ms = EXCLUDED.position_ms,
                 track_ids = EXCLUDED.track_ids,
                 updated_at = EXCLUDED.updated_at",
        )
        .bind(&queue.username)
        .bind(&queue.current_track_id)
        .bind(queue.position_ms)
        .bind(&queue.track_ids)
        .bind(queue.updated_at)
        .execute(&self.pool)
        .await?;
		Ok(())
	}

	pub async fn get_all_users_export_data(
		&self,
		tidal_user_id: &str,
	) -> Result<Vec<UserExportData>, sqlx::Error> {
		sqlx::query_as::<_, UserExportData>(
			"SELECT u.username, u.tidal_user_id, u.use_playlists, u.use_favorites, l.lastfm_username
             FROM subsonic_users u
             LEFT JOIN user_lastfm_links l ON u.username = l.subsonic_username
             WHERE u.tidal_user_id = $1",
		)
		.bind(tidal_user_id)
		.fetch_all(&self.pool)
		.await
	}

	pub async fn update_last_data_request(&self, tidal_user_id: &str) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE tidal_tokens SET last_data_request = $1 WHERE tidal_user_id = $2")
			.bind(Utc::now())
			.bind(tidal_user_id)
			.execute(&self.pool)
			.await?;
		Ok(())
	}
	pub async fn get_play_queue(&self, username: &str) -> Result<Option<PlayQueue>, sqlx::Error> {
		sqlx::query_as::<_, PlayQueue>(
            "SELECT username, current_track_id, position_ms, track_ids, updated_at FROM play_queues WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
	}

	pub async fn get_play_queues_for_tidal_user(
		&self,
		tidal_user_id: &str,
	) -> Result<Vec<PlayQueue>, sqlx::Error> {
		sqlx::query_as::<_, PlayQueue>(
			"SELECT q.username, q.current_track_id, q.position_ms, q.track_ids, q.updated_at
             FROM play_queues q
             JOIN subsonic_users u ON q.username = u.username
             WHERE u.tidal_user_id = $1",
		)
		.bind(tidal_user_id)
		.fetch_all(&self.pool)
		.await
	}
}
