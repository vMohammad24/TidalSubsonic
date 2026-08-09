use crate::db::DbManager;
use crate::tidal::manager::TidalClientManager;
use actix_session::Session;
use actix_web::HttpRequest;

#[derive(Clone)]
pub enum StatusMessage {
	Success(String),
	Error(String),
}

pub struct UserInfo {
	pub username: String,
	pub lastfm_username: Option<String>,
	pub use_playlists: bool,
	pub use_favorites: bool,
	pub use_event_batch: bool,
	pub scrobbles: i64,
}

pub fn set_flash(session: &Session, kind: &str, msg: &str) {
	let _ = session.insert("flash_kind", kind.to_string());
	let _ = session.insert("flash_msg", msg.to_string());
}

pub fn get_flash(session: &Session) -> Option<StatusMessage> {
	let kind: Option<String> = session.get("flash_kind").ok().flatten();
	let msg: Option<String> = session.get("flash_msg").ok().flatten();

	let (Some(k), Some(m)) = (kind, msg) else {
		return None;
	};

	let _ = session.remove("flash_kind");
	let _ = session.remove("flash_msg");
	if k == "success" {
		Some(StatusMessage::Success(m))
	} else {
		Some(StatusMessage::Error(m))
	}
}

pub async fn extract_user_id(req: &HttpRequest, manager: &TidalClientManager) -> Option<String> {
	if let Some(cookie) = req.cookie("tidal_subsonic_wsid") {
		let session_id = cookie.value();
		if let Ok(Some((tidal_user_id, _username))) = manager.db.get_web_session(session_id).await {
			return Some(tidal_user_id);
		}
	}
	None
}

pub async fn get_users_info(tidal_user_id: &str, db: &DbManager) -> Vec<UserInfo> {
	let rows = sqlx::query!(
		r#"
		SELECT
			u.username,
			u.use_playlists as "use_playlists!",
			u.use_favorites as "use_favorites!",
			u.use_event_batch as "use_event_batch!",
			l.lastfm_username,
			COALESCE(s.scrobble_count, 0) as "scrobbles!"
		FROM subsonic_users u
		LEFT JOIN user_lastfm_links l ON u.username = l.subsonic_username
		LEFT JOIN (
			SELECT username, COUNT(*) as scrobble_count
			FROM scrobbles
			WHERE submission = true
			GROUP BY username
		) s ON u.username = s.username
		WHERE u.tidal_user_id = $1
		"#,
		tidal_user_id
	)
	.fetch_all(&db.pool)
	.await
	.unwrap_or_default();

	rows.into_iter()
		.map(|r| UserInfo {
			username: r.username,
			lastfm_username: r.lastfm_username,
			use_playlists: r.use_playlists,
			use_favorites: r.use_favorites,
			use_event_batch: r.use_event_batch,
			scrobbles: r.scrobbles,
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use actix_session::SessionExt;
	use actix_web::test;

	#[actix_web::test]
	async fn test_flash_messages() {
		let req = test::TestRequest::default().to_http_request();
		let session = req.get_session();

		set_flash(&session, "success", "Operation completed!");
		let flash = get_flash(&session);
		assert!(flash.is_some());

		match flash.unwrap() {
			StatusMessage::Success(msg) => assert_eq!(msg, "Operation completed!"),
			_ => panic!("Expected Success status message"),
		}
		let flash_after = get_flash(&session);
		assert!(flash_after.is_none());

		set_flash(&session, "error", "Operation failed!");
		let flash_err = get_flash(&session);
		assert!(flash_err.is_some());
		match flash_err.unwrap() {
			StatusMessage::Error(msg) => assert_eq!(msg, "Operation failed!"),
			_ => panic!("Expected Error status message"),
		}
	}
}
