use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredTokens {
	pub access_token: Option<String>,
	pub refresh_token: Option<String>,
	pub token_expiry: Option<DateTime<Utc>>,
	pub last_data_request: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LocalPlaylistWithCount {
	pub id: Uuid,
	pub username: String,
	pub name: String,
	pub comment: Option<String>,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub song_count: i64,
	pub duration: i64,
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
		sqlx::query_as!(
            StoredTokens,
            "SELECT access_token, refresh_token, token_expiry, last_data_request FROM tidal_tokens WHERE tidal_user_id = $1",
            tidal_id
        )
        .fetch_optional(&self.pool)
        .await
	}

	pub async fn save_tokens(
		&self,
		tidal_id: &str,
		tokens: StoredTokens,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
            "INSERT INTO tidal_tokens (tidal_user_id, access_token, refresh_token, token_expiry, last_data_request)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (tidal_user_id) DO UPDATE SET
                 access_token = EXCLUDED.access_token,
                 refresh_token = EXCLUDED.refresh_token,
                 token_expiry = EXCLUDED.token_expiry,
                 last_data_request = COALESCE(EXCLUDED.last_data_request, tidal_tokens.last_data_request)",
            tidal_id,
            tokens.access_token,
            tokens.refresh_token,
            tokens.token_expiry,
            tokens.last_data_request
        )
        .execute(&self.pool)
        .await?;
		Ok(())
	}

	pub async fn delete_tokens(&self, tidal_id: &str) -> Result<(), sqlx::Error> {
		sqlx::query!(
			"DELETE FROM tidal_tokens WHERE tidal_user_id = $1",
			tidal_id
		)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	pub async fn get_tidal_user_for_subsonic(
		&self,
		subsonic_username: &str,
	) -> Result<Option<String>, sqlx::Error> {
		let row = sqlx::query!(
			"SELECT tidal_user_id FROM subsonic_users WHERE username = $1",
			subsonic_username
		)
		.fetch_optional(&self.pool)
		.await?;

		Ok(row.map(|r| r.tidal_user_id))
	}

	pub async fn create_user(
		&self,
		username: &str,
		tidal_user_id: &str,
		encrypted_password: Option<&str>,
		use_playlists: bool,
		use_favorites: bool,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
            "INSERT INTO subsonic_users (username, tidal_user_id, password, use_playlists, use_favorites) VALUES ($1, $2, $3, $4, $5)",
            username,
            tidal_user_id,
            encrypted_password,
            use_playlists,
            use_favorites
        )
        .execute(&self.pool)
        .await?;
		Ok(())
	}

	pub async fn delete_user(&self, username: &str) -> Result<(), sqlx::Error> {
		sqlx::query!("DELETE FROM subsonic_users WHERE username = $1", username)
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	pub async fn list_users_for_tidal_account(
		&self,
		tidal_user_id: &str,
	) -> Result<Vec<String>, sqlx::Error> {
		let rows = sqlx::query!(
			"SELECT username FROM subsonic_users WHERE tidal_user_id = $1",
			tidal_user_id
		)
		.fetch_all(&self.pool)
		.await?;

		Ok(rows.into_iter().map(|r| r.username).collect())
	}

	pub async fn update_user_feature_flags(
		&self,
		username: &str,
		use_playlists: bool,
		use_favorites: bool,
	) -> Result<bool, sqlx::Error> {
		let result = sqlx::query!(
			"UPDATE subsonic_users SET use_playlists = $1, use_favorites = $2 WHERE username = $3",
			use_playlists,
			use_favorites,
			username
		)
		.execute(&self.pool)
		.await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn get_user_details(
		&self,
		username: &str,
	) -> Result<Option<(String, String, bool, bool)>, sqlx::Error> {
		let row = sqlx::query!(
			r#"SELECT
                tidal_user_id,
                COALESCE(password, '') as "password!",
                use_playlists as "use_playlists!",
                use_favorites as "use_favorites!"
            FROM subsonic_users WHERE username = $1"#,
			username
		)
		.fetch_optional(&self.pool)
		.await?;

		Ok(row.map(|r| {
			(
				r.tidal_user_id,
				r.password,
				r.use_playlists,
				r.use_favorites,
			)
		}))
	}

	pub async fn link_lastfm_account(
		&self,
		subsonic_username: &str,
		session_key: &str,
		lastfm_username: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
            "INSERT INTO user_lastfm_links (subsonic_username, lastfm_session_key, lastfm_username) VALUES ($1, $2, $3) ON CONFLICT (subsonic_username) DO UPDATE SET lastfm_session_key = EXCLUDED.lastfm_session_key, lastfm_username = EXCLUDED.lastfm_username",
            subsonic_username,
            session_key,
            lastfm_username
        )
        .execute(&self.pool)
        .await?;
		Ok(())
	}

	pub async fn get_lastfm_details(
		&self,
		subsonic_username: &str,
	) -> Result<Option<(String, String)>, sqlx::Error> {
		let row = sqlx::query!(
			"SELECT lastfm_session_key, lastfm_username FROM user_lastfm_links WHERE subsonic_username = $1",
			subsonic_username
		)
		.fetch_optional(&self.pool)
		.await?;

		Ok(row.map(|r| (r.lastfm_session_key, r.lastfm_username)))
	}

	#[allow(dead_code)]
	pub async fn unlink_lastfm_account(&self, subsonic_username: &str) -> Result<(), sqlx::Error> {
		sqlx::query!(
			"DELETE FROM user_lastfm_links WHERE subsonic_username = $1",
			subsonic_username
		)
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
		sqlx::query!(
			"INSERT INTO web_sessions (session_id, tidal_user_id, username)
             VALUES ($1, $2, $3)
             ON CONFLICT (session_id) DO UPDATE SET
                 tidal_user_id = EXCLUDED.tidal_user_id,
                 username = EXCLUDED.username",
			session_id,
			tidal_user_id,
			username
		)
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	pub async fn get_web_session(
		&self,
		session_id: &str,
	) -> Result<Option<(String, String)>, sqlx::Error> {
		let row = sqlx::query!(
			"SELECT tidal_user_id, username FROM web_sessions WHERE session_id = $1",
			session_id
		)
		.fetch_optional(&self.pool)
		.await?;

		Ok(row.map(|r| (r.tidal_user_id, r.username)))
	}

	pub async fn delete_web_session(&self, session_id: &str) -> Result<(), sqlx::Error> {
		sqlx::query!("DELETE FROM web_sessions WHERE session_id = $1", session_id)
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	pub async fn save_play_queue(&self, queue: &PlayQueue) -> Result<(), sqlx::Error> {
		sqlx::query!(
            "INSERT INTO play_queues (username, current_track_id, position_ms, track_ids, updated_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (username) DO UPDATE SET
                 current_track_id = EXCLUDED.current_track_id,
                 position_ms = EXCLUDED.position_ms,
                 track_ids = EXCLUDED.track_ids,
                 updated_at = EXCLUDED.updated_at",
            queue.username,
            queue.current_track_id,
            queue.position_ms,
            &queue.track_ids,
            queue.updated_at
        )
        .execute(&self.pool)
        .await?;
		Ok(())
	}

	pub async fn get_all_users_export_data(
		&self,
		tidal_user_id: &str,
	) -> Result<Vec<UserExportData>, sqlx::Error> {
		sqlx::query_as!(
			UserExportData,
			r#"SELECT
                u.username,
                u.tidal_user_id,
                u.use_playlists as "use_playlists!",
                u.use_favorites as "use_favorites!",
                l.lastfm_username
             FROM subsonic_users u
             LEFT JOIN user_lastfm_links l ON u.username = l.subsonic_username
             WHERE u.tidal_user_id = $1"#,
			tidal_user_id
		)
		.fetch_all(&self.pool)
		.await
	}

	pub async fn update_last_data_request(&self, tidal_user_id: &str) -> Result<(), sqlx::Error> {
		sqlx::query!(
			"UPDATE tidal_tokens SET last_data_request = $1 WHERE tidal_user_id = $2",
			Utc::now(),
			tidal_user_id
		)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	pub async fn get_play_queue(&self, username: &str) -> Result<Option<PlayQueue>, sqlx::Error> {
		sqlx::query_as!(
            PlayQueue,
            "SELECT username, current_track_id, position_ms, track_ids, updated_at FROM play_queues WHERE username = $1",
            username
        )
        .fetch_optional(&self.pool)
        .await
	}

	pub async fn get_play_queues_for_tidal_user(
		&self,
		tidal_user_id: &str,
	) -> Result<Vec<PlayQueue>, sqlx::Error> {
		sqlx::query_as!(
			PlayQueue,
			"SELECT q.username, q.current_track_id, q.position_ms, q.track_ids, q.updated_at
             FROM play_queues q
             JOIN subsonic_users u ON q.username = u.username
             WHERE u.tidal_user_id = $1",
			tidal_user_id
		)
		.fetch_all(&self.pool)
		.await
	}

	pub async fn create_local_playlist(
		&self,
		username: &str,
		name: &str,
		comment: Option<&str>,
	) -> Result<LocalPlaylistWithCount, sqlx::Error> {
		sqlx::query_as!(
            LocalPlaylistWithCount,
            r#"INSERT INTO local_playlists (username, name, comment)
             VALUES ($1, $2, $3)
             RETURNING id, username, name, comment, created_at, updated_at, 0::bigint as "song_count!", 0::bigint as "duration!""#,
            username,
            name,
            comment
        )
        .fetch_one(&self.pool)
        .await
	}

	pub async fn get_local_playlists(
		&self,
		username: &str,
	) -> Result<Vec<LocalPlaylistWithCount>, sqlx::Error> {
		sqlx::query_as!(
			LocalPlaylistWithCount,
			r#"SELECT
                p.id, p.username, p.name, p.comment, p.created_at, p.updated_at,
                COUNT(t.track_id) as "song_count!",
                0::bigint as "duration!"
            FROM local_playlists p
            LEFT JOIN local_playlist_tracks t ON p.id = t.playlist_id
            WHERE p.username = $1
            GROUP BY p.id"#,
			username
		)
		.fetch_all(&self.pool)
		.await
	}

	pub async fn get_local_playlist(
		&self,
		id: Uuid,
	) -> Result<Option<LocalPlaylistWithCount>, sqlx::Error> {
		sqlx::query_as!(
			LocalPlaylistWithCount,
			r#"SELECT
                p.id, p.username, p.name, p.comment, p.created_at, p.updated_at,
                COUNT(t.track_id) as "song_count!",
                0::bigint as "duration!"
            FROM local_playlists p
            LEFT JOIN local_playlist_tracks t ON p.id = t.playlist_id
            WHERE p.id = $1
            GROUP BY p.id"#,
			id
		)
		.fetch_optional(&self.pool)
		.await
	}

	pub async fn delete_local_playlist(&self, id: Uuid, username: &str) -> Result<(), sqlx::Error> {
		sqlx::query!(
			"DELETE FROM local_playlists WHERE id = $1 AND username = $2",
			id,
			username
		)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	pub async fn update_local_playlist(
		&self,
		id: Uuid,
		username: &str,
		name: Option<&str>,
		comment: Option<&str>,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
            "UPDATE local_playlists SET name = COALESCE($1, name), comment = COALESCE($2, comment), updated_at = NOW() WHERE id = $3 AND username = $4",
            name,
            comment,
            id,
            username
        )
        .execute(&self.pool)
        .await?;
		Ok(())
	}

	pub async fn add_tracks_to_local_playlist(
		&self,
		playlist_id: Uuid,
		track_ids: &[String],
	) -> Result<(), sqlx::Error> {
		let mut tx = self.pool.begin().await?;

		sqlx::query!(
			r#"SELECT 1 as "locked!" FROM local_playlists WHERE id = $1 FOR UPDATE"#,
			playlist_id
		)
		.fetch_one(&mut *tx)
		.await?;

		let max_pos: i32 = sqlx::query_scalar!(
			"SELECT MAX(position) FROM local_playlist_tracks WHERE playlist_id = $1",
			playlist_id
		)
		.fetch_one(&mut *tx)
		.await?
		.unwrap_or(-1);

		for (i, track_id) in track_ids.iter().enumerate() {
			sqlx::query!(
				"INSERT INTO local_playlist_tracks (playlist_id, track_id, position) VALUES ($1, $2, $3)",
				playlist_id,
				track_id,
				max_pos + 1 + i as i32
			)
			.execute(&mut *tx)
			.await?;
		}

		tx.commit().await?;
		Ok(())
	}

	pub async fn remove_tracks_from_local_playlist(
		&self,
		playlist_id: Uuid,
		position: i32,
	) -> Result<(), sqlx::Error> {
		let mut tx = self.pool.begin().await?;

		sqlx::query!(
			r#"SELECT 1 as "locked!" FROM local_playlists WHERE id = $1 FOR UPDATE"#,
			playlist_id
		)
		.fetch_one(&mut *tx)
		.await?;

		sqlx::query!(
			"DELETE FROM local_playlist_tracks WHERE playlist_id = $1 AND position = $2",
			playlist_id,
			position
		)
		.execute(&mut *tx)
		.await?;

		sqlx::query!(
            "UPDATE local_playlist_tracks SET position = position - 1 WHERE playlist_id = $1 AND position > $2",
            playlist_id,
            position
        )
        .execute(&mut *tx)
        .await?;

		tx.commit().await?;
		Ok(())
	}

	pub async fn get_local_playlist_tracks(
		&self,
		playlist_id: Uuid,
	) -> Result<Vec<String>, sqlx::Error> {
		let rows = sqlx::query!(
			"SELECT track_id FROM local_playlist_tracks WHERE playlist_id = $1 ORDER BY position",
			playlist_id
		)
		.fetch_all(&self.pool)
		.await?;

		Ok(rows.into_iter().map(|r| r.track_id).collect())
	}

	pub async fn star_track_local(
		&self,
		username: &str,
		track_id: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
			"INSERT INTO local_favorite_tracks (username, track_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
			username,
			track_id
		)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	pub async fn unstar_track_local(
		&self,
		username: &str,
		track_id: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
			"DELETE FROM local_favorite_tracks WHERE username = $1 AND track_id = $2",
			username,
			track_id
		)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	pub async fn star_album_local(
		&self,
		username: &str,
		album_id: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
			"INSERT INTO local_favorite_albums (username, album_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
			username,
			album_id
		)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	pub async fn unstar_album_local(
		&self,
		username: &str,
		album_id: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
			"DELETE FROM local_favorite_albums WHERE username = $1 AND album_id = $2",
			username,
			album_id
		)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	pub async fn star_artist_local(
		&self,
		username: &str,
		artist_id: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
			"INSERT INTO local_favorite_artists (username, artist_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
			username,
			artist_id
		)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	pub async fn unstar_artist_local(
		&self,
		username: &str,
		artist_id: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
			"DELETE FROM local_favorite_artists WHERE username = $1 AND artist_id = $2",
			username,
			artist_id
		)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	pub async fn get_local_favorite_tracks(
		&self,
		username: &str,
	) -> Result<Vec<String>, sqlx::Error> {
		let rows = sqlx::query!(
			"SELECT track_id FROM local_favorite_tracks WHERE username = $1 ORDER BY created_at DESC",
			username
		)
		.fetch_all(&self.pool)
		.await?;
		Ok(rows.into_iter().map(|r| r.track_id).collect())
	}

	pub async fn get_local_favorite_albums(
		&self,
		username: &str,
	) -> Result<Vec<String>, sqlx::Error> {
		let rows = sqlx::query!(
			"SELECT album_id FROM local_favorite_albums WHERE username = $1 ORDER BY created_at DESC",
			username
		)
		.fetch_all(&self.pool)
		.await?;
		Ok(rows.into_iter().map(|r| r.album_id).collect())
	}

	pub async fn get_local_favorite_artists(
		&self,
		username: &str,
	) -> Result<Vec<String>, sqlx::Error> {
		let rows = sqlx::query!(
			"SELECT artist_id FROM local_favorite_artists WHERE username = $1 ORDER BY created_at DESC",
			username
		)
		.fetch_all(&self.pool)
		.await?;
		Ok(rows.into_iter().map(|r| r.artist_id).collect())
	}

	pub async fn get_all_local_favorites_map(
		&self,
		username: &str,
	) -> Result<std::collections::HashMap<i64, String>, sqlx::Error> {
		let mut favorites = std::collections::HashMap::new();

		let tracks = sqlx::query!(
			"SELECT track_id, created_at FROM local_favorite_tracks WHERE username = $1",
			username
		)
		.fetch_all(&self.pool)
		.await?;
		for row in tracks {
			if let Ok(id) = row.track_id.parse::<i64>() {
				favorites.insert(id, row.created_at.to_rfc3339());
			}
		}

		let albums = sqlx::query!(
			"SELECT album_id, created_at FROM local_favorite_albums WHERE username = $1",
			username
		)
		.fetch_all(&self.pool)
		.await?;
		for row in albums {
			if let Ok(id) = row.album_id.parse::<i64>() {
				favorites.insert(id, row.created_at.to_rfc3339());
			}
		}

		let artists = sqlx::query!(
			"SELECT artist_id, created_at FROM local_favorite_artists WHERE username = $1",
			username
		)
		.fetch_all(&self.pool)
		.await?;
		for row in artists {
			if let Ok(id) = row.artist_id.parse::<i64>() {
				favorites.insert(id, row.created_at.to_rfc3339());
			}
		}

		Ok(favorites)
	}
}
