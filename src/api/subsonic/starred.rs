use actix_web::{Responder, web};
use serde::Deserialize;

use crate::api::subsonic::models::SubsonicResponseWrapper;
use crate::api::subsonic::response::SubsonicResponder;
use crate::db::DbManager;

pub async fn get_starred(
	req: actix_web::HttpRequest,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<DbManager>,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();

	let api = &subsonic_ctx.tidal_api;

	if subsonic_ctx.use_favorites {
		let api_clone = api;
		let ctx_clone = &*subsonic_ctx;

		let songs_fut = async {
			let track_ids = db
				.get_local_favorite_tracks(&subsonic_ctx.user)
				.await
				.unwrap_or_default();
			crate::util::fetch_concurrently(track_ids, 50, |id_str| async move {
				let id = id_str
					.parse::<i64>()
					.map_err(
						|e| tracing::error!(track_id = %id_str, error = ?e, "Failed to parse favorited track ID"),
					)
					.ok()?;
				let track = api_clone
					.get_track(id)
					.await
					.map_err(
						|e| tracing::warn!(track_id = %id, error = ?e, "Failed to fetch favorited track"),
					)
					.ok()?;
				Some(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
					&track,
					Some(ctx_clone),
					None,
					None,
				))
			})
			.await
		};

		let albums_fut = async {
			let album_ids = db
				.get_local_favorite_albums(&subsonic_ctx.user)
				.await
				.unwrap_or_default();
			crate::util::fetch_concurrently(album_ids, 50, |id_str| async move {
				let id = id_str
					.parse::<i64>()
					.map_err(
						|e| tracing::error!(album_id = %id_str, error = ?e, "Failed to parse favorited album ID"),
					)
					.ok()?;
				let album = api_clone
					.get_album(id)
					.await
					.map_err(
						|e| tracing::warn!(album_id = %id, error = ?e, "Failed to fetch favorited album"),
					)
					.ok()?;
				Some(crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
					&album,
					Some(ctx_clone),
					None,
				))
			})
			.await
		};

		let artists_fut = async {
			let artist_ids = db
				.get_local_favorite_artists(&subsonic_ctx.user)
				.await
				.unwrap_or_default();
			crate::util::fetch_concurrently(artist_ids, 50, |id_str| async move {
				let id = id_str
					.parse::<i64>()
					.map_err(
						|e| tracing::error!(artist_id = %id_str, error = ?e, "Failed to parse favorited artist ID"),
					)
					.ok()?;
				let artist = api_clone
					.get_artist(id)
					.await
					.map_err(
						|e| tracing::warn!(artist_id = %id, error = ?e, "Failed to fetch favorited artist"),
					)
					.ok()?;
				Some(crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
					&artist,
					Some(ctx_clone),
				))
			})
			.await
		};

		let (songs, albums, artists) = futures_util::join!(songs_fut, albums_fut, artists_fut);

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
		return SubsonicResponder(resp);
	}

	if let Some(user_id) = api.user_id() {
		let mut songs = Vec::new();
		let mut albums = Vec::new();
		let mut artists = Vec::new();

		if let Ok(mut fav_tracks) = api.get_favorite_tracks(user_id, 5000, 0).await {
			fav_tracks
				.items
				.sort_unstable_by(|a, b| b.created.cmp(&a.created));
			for track in fav_tracks.items {
				songs.push(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
					&track.item,
					Some(&subsonic_ctx),
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
					Some(&subsonic_ctx),
					None,
				));
			}
		}

		if let Ok(fav_artists) = api.get_favorite_artists(user_id, 500, 0).await {
			for tidal_artist in fav_artists.items {
				let tidal_artist = tidal_artist.item;
				artists.push(crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
					&tidal_artist,
					Some(&subsonic_ctx),
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
	db: web::Data<DbManager>,
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

	if subsonic_ctx.use_favorites {
		if let Some(id_str) = &query.id
			&& let Ok(id) = id_str.parse::<i64>()
			&& db
				.star_track_local(&subsonic_ctx.user, id_str)
				.await
				.is_ok()
		{
			crate::tidal::favorites::add_local_favorite(
				&subsonic_ctx.user,
				id,
				chrono::Utc::now().to_rfc3339(),
			);
		}
		if let Some(id_str) = &query.album_id
			&& let Ok(id) = id_str.parse::<i64>()
			&& db
				.star_album_local(&subsonic_ctx.user, id_str)
				.await
				.is_ok()
		{
			crate::tidal::favorites::add_local_favorite(
				&subsonic_ctx.user,
				id,
				chrono::Utc::now().to_rfc3339(),
			);
		}
		if let Some(id_str) = &query.artist_id
			&& let Ok(id) = id_str.parse::<i64>()
			&& db
				.star_artist_local(&subsonic_ctx.user, id_str)
				.await
				.is_ok()
		{
			crate::tidal::favorites::add_local_favorite(
				&subsonic_ctx.user,
				id,
				chrono::Utc::now().to_rfc3339(),
			);
		}
	} else {
		let api = &subsonic_ctx.tidal_api;

		if let Some(id) = id_parsed {
			let _ = api.add_favorite_track(id).await;
		}

		if let Some(album_id) = album_id_parsed {
			let _ = api.add_favorite_album(album_id).await;
		}

		if let Some(artist_id) = artist_id_parsed {
			let _ = api.add_favorite_artist(artist_id).await;
		}
	}

	SubsonicResponder(SubsonicResponseWrapper::ok())
}

pub async fn unstar(
	query: web::Query<StarQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<DbManager>,
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

	if subsonic_ctx.use_favorites {
		if let Some(id_str) = &query.id
			&& db
				.unstar_track_local(&subsonic_ctx.user, id_str)
				.await
				.is_ok() && let Ok(id) = id_str.parse::<i64>()
		{
			crate::tidal::favorites::remove_local_favorite(&subsonic_ctx.user, id);
		}
		if let Some(id_str) = &query.album_id
			&& db
				.unstar_album_local(&subsonic_ctx.user, id_str)
				.await
				.is_ok() && let Ok(id) = id_str.parse::<i64>()
		{
			crate::tidal::favorites::remove_local_favorite(&subsonic_ctx.user, id);
		}
		if let Some(id_str) = &query.artist_id
			&& db
				.unstar_artist_local(&subsonic_ctx.user, id_str)
				.await
				.is_ok() && let Ok(id) = id_str.parse::<i64>()
		{
			crate::tidal::favorites::remove_local_favorite(&subsonic_ctx.user, id);
		}
	} else {
		let api = &subsonic_ctx.tidal_api;

		if let Some(id) = id_parsed {
			let _ = api.remove_favorite_track(id).await;
		}

		if let Some(album_id) = album_id_parsed {
			let _ = api.remove_favorite_album(album_id).await;
		}

		if let Some(artist_id) = artist_id_parsed {
			let _ = api.remove_favorite_artist(artist_id).await;
		}
	}

	SubsonicResponder(SubsonicResponseWrapper::ok())
}
