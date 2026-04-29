use actix_web::{Responder, web};
use serde::Deserialize;

use crate::api::subsonic::models::SubsonicResponseWrapper;
use crate::api::subsonic::response::SubsonicResponder;

pub async fn get_starred(
	req: actix_web::HttpRequest,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();

	let api = subsonic_ctx.tidal_api.clone();

	if let Some(user_id) = api.user_id() {
		let mut songs = Vec::new();
		let mut albums = Vec::new();
		let mut artists = Vec::new();

		if let Ok(mut fav_tracks) = api.get_favorite_tracks(user_id, 5000, 0).await {
			fav_tracks.items.sort_by(|a, b| b.created.cmp(&a.created));
			for track in fav_tracks.items {
				songs.push(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
					&track.item,
					api.user_id(),
					None,
					None,
				));
			}
		}

		if let Ok(fav_albums) = api.get_favorite_albums(user_id, 500, 0).await {
			for tidal_album in fav_albums.items {
				let tidal_album = tidal_album.item;
				albums.push(crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
					&tidal_album,
					api.user_id(),
					None,
				));
			}
		}

		if let Ok(fav_artists) = api.get_favorite_artists(user_id, 500, 0).await {
			for tidal_artist in fav_artists.items {
				let tidal_artist = tidal_artist.item;
				artists.push(crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
					&tidal_artist,
					api.user_id(),
				));
			}
		}

		let starred_node = crate::api::subsonic::models::Starred {
			song: songs,
			album: albums,
			artist: artists,
		};

		if req.path().contains("getStarred2") {
			resp.response.starred2 = Some(starred_node);
		} else {
			resp.response.starred = Some(starred_node);
		}
	} else {
		tracing::error!("user_id is None in client session");
		return SubsonicResponder(SubsonicResponseWrapper::error(
			0,
			"Tidal user_id is missing from session",
		));
	}

	SubsonicResponder(resp)
}

pub async fn get_starred2(
	req: actix_web::HttpRequest,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	get_starred(req, subsonic_ctx).await
}

#[derive(Deserialize)]
pub struct StarQuery {
	pub id: Option<String>,
	#[serde(rename = "albumId")]
	pub album_id: Option<String>,
	#[serde(rename = "artistId")]
	pub artist_id: Option<String>,
}

pub async fn star(
	query: web::Query<StarQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let id_parsed = query.id.as_ref().and_then(|id| id.parse::<i64>().ok());
	let album_id_parsed = query
		.album_id
		.as_ref()
		.and_then(|id| id.parse::<i64>().ok());
	let artist_id_parsed = query
		.artist_id
		.as_ref()
		.and_then(|id| id.parse::<i64>().ok());

	if id_parsed.is_none() && album_id_parsed.is_none() && artist_id_parsed.is_none() {
		return SubsonicResponder(SubsonicResponseWrapper::error(
			10,
			"Missing or invalid required parameter 'id', 'albumId', or 'artistId'",
		));
	}

	let api = subsonic_ctx.tidal_api.clone();

	if let Some(id) = id_parsed {
		let _ = api.add_favorite_track(id).await;
	}

	if let Some(album_id) = album_id_parsed {
		let _ = api.add_favorite_album(album_id).await;
	}

	if let Some(artist_id) = artist_id_parsed {
		let _ = api.add_favorite_artist(artist_id).await;
	}

	SubsonicResponder(SubsonicResponseWrapper::ok())
}

pub async fn unstar(
	query: web::Query<StarQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let id_parsed = query.id.as_ref().and_then(|id| id.parse::<i64>().ok());
	let album_id_parsed = query
		.album_id
		.as_ref()
		.and_then(|id| id.parse::<i64>().ok());
	let artist_id_parsed = query
		.artist_id
		.as_ref()
		.and_then(|id| id.parse::<i64>().ok());

	if id_parsed.is_none() && album_id_parsed.is_none() && artist_id_parsed.is_none() {
		return SubsonicResponder(SubsonicResponseWrapper::error(
			10,
			"Missing or invalid required parameter 'id', 'albumId', or 'artistId'",
		));
	}

	let api = subsonic_ctx.tidal_api.clone();

	if let Some(id) = id_parsed {
		let _ = api.remove_favorite_track(id).await;
	}

	if let Some(album_id) = album_id_parsed {
		let _ = api.remove_favorite_album(album_id).await;
	}

	if let Some(artist_id) = artist_id_parsed {
		let _ = api.remove_favorite_artist(artist_id).await;
	}

	SubsonicResponder(SubsonicResponseWrapper::ok())
}
