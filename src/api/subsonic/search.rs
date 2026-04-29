use crate::api::subsonic::models::{SearchResult2, SearchResult3, SubsonicResponseWrapper};
use crate::api::subsonic::response::SubsonicResponder;
use crate::tidal::api::{ALBUM_CACHE, ARTIST_CACHE, TRACK_CACHE};
use actix_web::{Responder, web};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
	pub query: Option<String>,
	pub artist_count: Option<i32>,
	pub artist_offset: Option<i32>,
	pub album_count: Option<i32>,
	pub album_offset: Option<i32>,
	pub song_count: Option<i32>,
	pub song_offset: Option<i32>,
}

pub async fn search3(
	query: web::Query<SearchQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	handle_search(query, subsonic_ctx, 3).await
}

pub async fn search2(
	query: web::Query<SearchQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	handle_search(query, subsonic_ctx, 2).await
}

async fn handle_search(
	query: web::Query<SearchQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	version: u8,
) -> impl Responder {
	let api = subsonic_ctx.tidal_api.clone();
	let mut resp = SubsonicResponseWrapper::ok();
	let song_limit = query.song_count.unwrap_or(20).clamp(0, 500) as usize;
	let album_limit = query.album_count.unwrap_or(20).clamp(0, 500) as usize;
	let artist_limit = query.artist_count.unwrap_or(20).clamp(0, 500) as usize;

	let song_offset = query.song_offset.unwrap_or(0) as usize;
	let album_offset = query.album_offset.unwrap_or(0) as usize;
	let artist_offset = query.artist_offset.unwrap_or(0) as usize;

	let (songs, albums, artists): (Vec<_>, Vec<_>, Vec<_>) = if let Some(q) = query
		.query
		.as_deref()
		.filter(|q| !q.trim().is_empty() && q.trim() != "\"\"")
	{
		let mut min_offset = usize::MAX;
		let mut max_end = 0;

		if song_limit > 0 {
			min_offset = min_offset.min(song_offset);
			max_end = max_end.max(song_offset + song_limit);
		}
		if album_limit > 0 {
			min_offset = min_offset.min(album_offset);
			max_end = max_end.max(album_offset + album_limit);
		}
		if artist_limit > 0 {
			min_offset = min_offset.min(artist_offset);
			max_end = max_end.max(artist_offset + artist_limit);
		}

		if min_offset == usize::MAX {
			min_offset = 0;
		}

		let fetch_limit = max_end.saturating_sub(min_offset) as u32;

		match api.search(q, fetch_limit, min_offset as u32).await {
			Ok(results) => {
				let s = if song_limit > 0 {
					results
						.tracks
						.map(|a| a.items)
						.unwrap_or_default()
						.into_iter()
						.skip(song_offset.saturating_sub(min_offset))
						.take(song_limit)
						.map(|t| {
							crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
								&t,
								api.user_id(),
								None,
								None,
							)
						})
						.collect()
				} else {
					Vec::new()
				};

				let b = if album_limit > 0 {
					results
						.albums
						.map(|a| a.items)
						.unwrap_or_default()
						.into_iter()
						.skip(album_offset.saturating_sub(min_offset))
						.take(album_limit)
						.map(|a| {
							crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
								&a,
								api.user_id(),
								None,
							)
						})
						.collect()
				} else {
					Vec::new()
				};

				let r = if artist_limit > 0 {
					results
						.artists
						.map(|a| a.items)
						.unwrap_or_default()
						.into_iter()
						.skip(artist_offset.saturating_sub(min_offset))
						.take(artist_limit)
						.map(|art| {
							crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
								&art,
								api.user_id(),
							)
						})
						.collect()
				} else {
					Vec::new()
				};

				(s, b, r)
			}
			Err(e) => {
				tracing::error!("Tidal API Error: {:?}", e);
				let msg = if cfg!(debug_assertions) {
					format!("Tidal API Error: {:?}", e)
				} else {
					"Upstream dependency failed".to_string()
				};
				return SubsonicResponder(SubsonicResponseWrapper::error(0, &msg)).customize();
			}
		}
	} else {
		let s = if song_limit > 0 {
			let mut cached_tracks: Vec<_> = TRACK_CACHE.iter().collect();
			cached_tracks.sort_by_key(|(k, _)| k.clone());

			cached_tracks
				.into_iter()
				.skip(song_offset)
				.take(song_limit)
				.map(|(_, track)| {
					crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
						&track,
						api.user_id(),
						None,
						None,
					)
				})
				.collect()
		} else {
			Vec::new()
		};

		let b = if album_limit > 0 {
			let mut cached_albums: Vec<_> = ALBUM_CACHE.iter().collect();
			cached_albums.sort_by_key(|(k, _)| k.clone());

			cached_albums
				.into_iter()
				.skip(album_offset)
				.take(album_limit)
				.map(|(_, album)| {
					crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
						&album,
						api.user_id(),
						None,
					)
				})
				.collect()
		} else {
			Vec::new()
		};

		let r = if artist_limit > 0 {
			let mut cached_artists: Vec<_> = ARTIST_CACHE.iter().collect();
			cached_artists.sort_by_key(|(k, _)| k.clone());

			cached_artists
				.into_iter()
				.skip(artist_offset)
				.take(artist_limit)
				.map(|(_, artist)| {
					crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
						&artist,
						api.user_id(),
					)
				})
				.collect()
		} else {
			Vec::new()
		};

		(s, b, r)
	};

	let total_count = songs.len() + albums.len() + artists.len();

	let artist_opt = if artists.is_empty() {
		None
	} else {
		Some(artists)
	};
	let album_opt = if albums.is_empty() {
		None
	} else {
		Some(albums)
	};
	let song_opt = if songs.is_empty() { None } else { Some(songs) };

	if version == 3 {
		resp.response.search_result3 = Some(SearchResult3 {
			artist: artist_opt,
			album: album_opt,
			song: song_opt,
		});
	} else {
		resp.response.search_result2 = Some(SearchResult2 {
			artist: artist_opt,
			album: album_opt,
			song: song_opt,
		});
	}

	SubsonicResponder(resp)
		.customize()
		.insert_header(("X-Total-Count", total_count.to_string()))
}
