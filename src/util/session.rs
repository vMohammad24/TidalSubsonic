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
	let mut users = Vec::new();
	if let Ok(usernames) = db.list_users_for_tidal_account(tidal_user_id).await {
		for u in usernames {
			if let Ok(Some((_, _, use_playlists, use_favorites, use_event_batch))) =
				db.get_user_details(&u).await
			{
				let lastfm_username = db
					.get_lastfm_details(&u)
					.await
					.ok()
					.flatten()
					.map(|(_, name)| name);
				let scrobbles = db.get_scrobble_count(&u).await.ok().flatten().unwrap_or(0);
				users.push(UserInfo {
					username: u,
					lastfm_username,
					use_playlists,
					use_favorites,
					use_event_batch,
					scrobbles,
				});
			}
		}
	}
	users
}
