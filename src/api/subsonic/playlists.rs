use actix_web::{Responder, web};
use serde::Deserialize;

use crate::api::subsonic::models::SubsonicResponseWrapper;
use crate::api::subsonic::response::SubsonicResponder;

#[derive(Deserialize)]
pub struct CreatePlaylistQuery {
	pub name: String,
	pub description: Option<String>,
	pub comment: Option<String>,
	#[serde(rename = "songId")]
	pub song_ids: Option<Vec<String>>,
}

pub async fn create_playlist(
	_req: actix_web::HttpRequest,
	query: web::Query<CreatePlaylistQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();

	let api = subsonic_ctx.tidal_api.clone();
	let desc = query.description.as_ref().or(query.comment.as_ref());

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
	query: web::Query<DeletePlaylistQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let api = subsonic_ctx.tidal_api.clone();
	let _ = api.delete_playlist(&query.id).await;

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
	query: web::Query<UpdatePlaylistQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let api = subsonic_ctx.tidal_api.clone();
	let desc = query.description.as_ref().or(query.comment.as_ref());
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

	SubsonicResponder(SubsonicResponseWrapper::ok())
}

use crate::api::subsonic::mapping::{map_tidal_playlist_to_subsonic, map_tidal_track_to_subsonic};
use crate::api::subsonic::models::{Playlist as SubsonicPlaylist, Playlists};

#[derive(Deserialize)]
pub struct GetPlaylistQuery {
	pub id: String,
}

pub async fn get_playlists(
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();

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
	query: web::Query<GetPlaylistQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();

	let api = subsonic_ctx.tidal_api.clone();

	if let Ok(playlist) = api.get_playlist(&query.id).await {
		let mut sub_playlist = map_tidal_playlist_to_subsonic(&playlist, Some(&subsonic_ctx.user));

		if let Ok(tracks_res) = api.get_playlist_tracks(&query.id, 1000, 0).await {
			let songs: Vec<_> = tracks_res
				.items
				.into_iter()
				.map(|item| map_tidal_track_to_subsonic(&item, api.user_id(), None, None))
				.collect();

			sub_playlist.entry = Some(songs);
		}

		resp.response.playlist = Some(sub_playlist);
		return SubsonicResponder(resp);
	}

	SubsonicResponder(SubsonicResponseWrapper::error(70, "Playlist not found"))
}
