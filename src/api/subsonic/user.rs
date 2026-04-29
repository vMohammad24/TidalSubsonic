use actix_web::{Responder, web};
use serde::Deserialize;

use crate::api::subsonic::models::{SubsonicResponseWrapper, User};
use crate::api::subsonic::response::SubsonicResponder;

pub async fn get_avatar() -> impl Responder {
	tracing::warn!("getAvatar called, but avatars are not supported.");
	SubsonicResponder(SubsonicResponseWrapper::error(70, "Avatars not supported"))
}

#[derive(Deserialize)]
pub struct GetUserQuery {
	pub username: String,
}

pub async fn get_user(query: web::Query<GetUserQuery>) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();
	resp.response.user = Some(User {
		username: query.username.clone(),
		admin_role: true,
		settings_role: true,
		download_role: true,
		upload_role: true,
		playlist_role: true,
		cover_art_role: true,
		comment_role: true,
		podcast_role: true,
		stream_role: true,
		jukebox_role: true,
		share_role: true,
		scrobbling_enabled: true,
	});
	SubsonicResponder(resp)
}
