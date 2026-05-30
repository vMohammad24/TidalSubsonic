use actix_web::{Responder, web};
use serde::Deserialize;

use crate::api::subsonic::models::SubsonicResponseWrapper;
use crate::api::subsonic::response::SubsonicResponder;
use crate::util::http_client::QsQuery;

#[derive(Deserialize)]
pub struct CreatePlaylistQuery {
	pub name: String,
	pub description: Option<String>,
	pub comment: Option<String>,
	#[serde(
		rename = "songId",
		default,
		deserialize_with = "crate::util::http_client::deserialize_list"
	)]
	pub song_ids: Option<Vec<String>>,
}

pub async fn create_playlist(
	query: QsQuery<CreatePlaylistQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<crate::db::DbManager>>,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();

	let api = subsonic_ctx.tidal_api.clone();
	let desc = query.description.as_ref().or(query.comment.as_ref());

	if subsonic_ctx.use_playlists {
		match db
			.create_local_playlist(&subsonic_ctx.user, &query.name, desc.map(|s| s.as_str()))
			.await
		{
			Ok(playlist) => {
				if let Some(ids) = &query.song_ids {
					let _ = db.add_tracks_to_local_playlist(playlist.id, ids).await;
				}
				if let Ok(Some(full_playlist)) = db.get_local_playlist(playlist.id).await {
					resp.response.playlist = Some(map_local_playlist_to_subsonic(&full_playlist));
				}
			}
			Err(e) => {
				tracing::error!("Failed to create local playlist: {:?}", e);
				resp = SubsonicResponseWrapper::error(0, "Failed to create local playlist");
			}
		}
		return SubsonicResponder(resp);
	}

	match api
		.create_playlist(&query.name, desc.map(|s| s.as_str()))
		.await
	{
		Ok(playlist) => {
			if let Some(ids) = &query.song_ids {
				let ids_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
				let _ = api.add_tracks_to_playlist(&playlist.uuid, &ids_refs).await;
			}
			resp.response.playlist = Some(map_tidal_playlist_to_subsonic(
				&playlist,
				Some(&subsonic_ctx.user),
			));
		}
		Err(e) => {
			tracing::error!("Failed to create playlist: {:?}", e);
			let msg = if cfg!(debug_assertions) {
				format!("Failed to create playlist: {:?}", e)
			} else {
				"Upstream dependency failed".to_string()
			};
			resp = SubsonicResponseWrapper::error(0, &msg);
		}
	}

	SubsonicResponder(resp)
}

#[derive(Deserialize)]
pub struct DeletePlaylistQuery {
	pub id: String,
}

pub async fn delete_playlist(
	query: QsQuery<DeletePlaylistQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<crate::db::DbManager>>,
) -> impl Responder {
	if subsonic_ctx.use_playlists {
		let id = match Uuid::parse_str(&query.id) {
			Ok(uuid) => uuid,
			Err(_) => {
				return SubsonicResponder(SubsonicResponseWrapper::error(
					10,
					"Invalid playlist ID format",
				));
			}
		};
		if let Err(e) = db.delete_local_playlist(id, &subsonic_ctx.user).await {
			tracing::error!("Failed to delete local playlist: {:?}", e);
			return SubsonicResponder(SubsonicResponseWrapper::error(
				70,
				"Failed to delete local playlist",
			));
		}
	} else {
		let api = subsonic_ctx.tidal_api.clone();
		let _ = api.delete_playlist(&query.id).await;
	}

	SubsonicResponder(SubsonicResponseWrapper::ok())
}

#[derive(Deserialize)]
pub struct UpdatePlaylistQuery {
	#[serde(rename = "playlistId")]
	pub playlist_id: String,
	pub name: Option<String>,
	pub description: Option<String>,
	pub comment: Option<String>,
	#[serde(
		rename = "songIdToAdd",
		default,
		deserialize_with = "crate::util::http_client::deserialize_list"
	)]
	pub song_id_to_add: Option<Vec<String>>,
	#[serde(rename = "songIndexToRemove")]
	pub song_index_to_remove: Option<u32>,
}

pub async fn update_playlist(
	query: QsQuery<UpdatePlaylistQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<crate::db::DbManager>>,
) -> impl Responder {
	let api = subsonic_ctx.tidal_api.clone();
	let desc = query.description.as_ref().or(query.comment.as_ref());

	if subsonic_ctx.use_playlists {
		let id = match Uuid::parse_str(&query.playlist_id) {
			Ok(uuid) => uuid,
			Err(_) => {
				return SubsonicResponder(SubsonicResponseWrapper::error(
					10,
					"Invalid playlist ID format",
				));
			}
		};

		if let Err(e) = db
			.update_local_playlist(
				id,
				&subsonic_ctx.user,
				query.name.as_deref(),
				desc.map(|s| s.as_str()),
			)
			.await
		{
			tracing::error!("Failed to update local playlist: {:?}", e);
			return SubsonicResponder(SubsonicResponseWrapper::error(
				70,
				"Failed to update local playlist",
			));
		}

		if let Some(ids) = &query.song_id_to_add {
			let _ = db.add_tracks_to_local_playlist(id, ids).await;
		}

		if let Some(index) = query.song_index_to_remove {
			let _ = db.remove_tracks_from_local_playlist(id, index as i32).await;
		}
	} else {
		let _ = api
			.update_playlist(
				&query.playlist_id,
				query.name.as_deref(),
				desc.map(|s| s.as_str()),
			)
			.await;

		if let Some(ids) = &query.song_id_to_add {
			let ids_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
			let _ = api
				.add_tracks_to_playlist(&query.playlist_id, &ids_refs)
				.await;
		}

		if let Some(index) = query.song_index_to_remove {
			let _ = api
				.remove_tracks_from_playlist(&query.playlist_id, index)
				.await;
		}
	}

	SubsonicResponder(SubsonicResponseWrapper::ok())
}

use crate::api::subsonic::mapping::{
	map_local_playlist_to_subsonic, map_tidal_playlist_to_subsonic, map_tidal_track_to_subsonic,
};
use crate::api::subsonic::models::{Playlist as SubsonicPlaylist, Playlists};
use futures_util::StreamExt;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct GetPlaylistQuery {
	pub id: String,
}

pub async fn get_playlists(
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<crate::db::DbManager>>,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();

	if subsonic_ctx.use_playlists {
		if let Ok(playlists) = db.get_local_playlists(&subsonic_ctx.user).await {
			let mapped: Vec<SubsonicPlaylist> = playlists
				.into_iter()
				.map(|p| map_local_playlist_to_subsonic(&p))
				.collect();
			resp.response.playlists = Some(Playlists { playlist: mapped });
		}
		return SubsonicResponder(resp);
	}

	let api = subsonic_ctx.tidal_api.clone();

	if let Some(user_id) = api.user_id()
		&& let Ok(playlists_result) = api.get_user_playlists(user_id).await
	{
		let mapped: Vec<SubsonicPlaylist> = playlists_result
			.items
			.into_iter()
			.map(|p| map_tidal_playlist_to_subsonic(&p, Some(&subsonic_ctx.user)))
			.collect();

		resp.response.playlists = Some(Playlists { playlist: mapped });
	}

	SubsonicResponder(resp)
}

pub async fn get_playlist(
	query: QsQuery<GetPlaylistQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<crate::db::DbManager>>,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();
	let api = subsonic_ctx.tidal_api.clone();

	if subsonic_ctx.use_playlists {
		let id = match Uuid::parse_str(&query.id) {
			Ok(uuid) => uuid,
			Err(_) => {
				return SubsonicResponder(SubsonicResponseWrapper::error(
					10,
					"Invalid playlist ID format",
				));
			}
		};

		if let Ok(Some(playlist)) = db.get_local_playlist(id).await {
			let mut sub_playlist = map_local_playlist_to_subsonic(&playlist);

			if let Ok(track_ids) = db.get_local_playlist_tracks(id).await {
				let api_clone = api.clone();
				let subsonic_ctx_clone = subsonic_ctx.clone();

				let songs: Vec<_> = futures_util::stream::iter(track_ids)
					.map(|track_id| {
						let api = api_clone.clone();
						let ctx = subsonic_ctx_clone.clone();
						async move {
							match track_id.parse::<i64>() {
								Ok(track_id_i64) => match api.get_track(track_id_i64).await {
									Ok(track) => {
										Some(map_tidal_track_to_subsonic(&track, Some(&ctx), None, None))
									}
									Err(e) => {
										tracing::warn!(track_id = %track_id_i64, error = ?e, "Failed to fetch track for playlist");
										None
									}
								},
								Err(e) => {
									tracing::error!(track_id = %track_id, error = ?e, "Failed to parse track ID");
									None
								}
							}
						}
					})
					.buffered(50)
					.filter_map(|s| async { s })
					.collect()
					.await;

				sub_playlist.entry = Some(songs);
			}

			resp.response.playlist = Some(sub_playlist);
			return SubsonicResponder(resp);
		}
	} else if let Ok(playlist) = api.get_playlist(&query.id).await {
		let mut sub_playlist = map_tidal_playlist_to_subsonic(&playlist, Some(&subsonic_ctx.user));

		if let Ok(tracks_res) = api.get_playlist_tracks(&query.id, 1000, 0).await {
			let songs: Vec<_> = tracks_res
				.items
				.into_iter()
				.map(|item| map_tidal_track_to_subsonic(&item, Some(&subsonic_ctx), None, None))
				.collect();

			sub_playlist.entry = Some(songs);
		}

		resp.response.playlist = Some(sub_playlist);
		return SubsonicResponder(resp);
	}

	SubsonicResponder(SubsonicResponseWrapper::error(70, "Playlist not found"))
}
