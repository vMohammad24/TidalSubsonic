use crate::api::lastfm::LASTFM_CLIENT;
use crate::api::subsonic::models::{
	AlbumList, Artist, Child, Directory, Index, Indexes, InternetRadioStations, MusicFolder,
	MusicFolders, SimilarSongs, SubsonicResponseWrapper, TopSongs,
};
use crate::api::subsonic::response::SubsonicResponder;
use crate::api::subsonic::{mapping, middleware::SubsonicContext};
use crate::tidal::api::{ALBUM_CACHE, ARTIST_CACHE};
use crate::tidal::manager::TidalClientManager;
use actix_web::{Responder, web};
use futures_util::{StreamExt, stream};
use rand::seq::IteratorRandom;
use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct IdQuery {
	pub id: String,
	pub count: Option<i32>,
}

pub async fn get_music_folders() -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();
	resp.response.music_folders = Some(MusicFolders {
		music_folder: vec![MusicFolder {
			id: 1,
			name: "Tidal".to_string(),
		}],
	});
	SubsonicResponder(resp)
}

pub async fn get_indexes(subsonic_ctx: actix_web::web::ReqData<SubsonicContext>) -> impl Responder {
	get_favorite_artists(subsonic_ctx, false).await
}

pub async fn get_artists(subsonic_ctx: actix_web::web::ReqData<SubsonicContext>) -> impl Responder {
	get_favorite_artists(subsonic_ctx, true).await
}

async fn get_favorite_artists(
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
	as_artists_node: bool,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();
	let mut index_map: std::collections::BTreeMap<String, Vec<Artist>> =
		std::collections::BTreeMap::new();

	for artist_entry in ARTIST_CACHE.iter() {
		let artist = crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
			&artist_entry.1,
			Some(&subsonic_ctx),
		);
		let first_char = artist.name.chars().next().unwrap_or('#');
		let initial = if first_char.is_ascii_alphabetic() {
			first_char.to_ascii_uppercase().to_string()
		} else {
			"#".to_string()
		};
		index_map.entry(initial).or_default().push(artist);
	}

	let mut indexes = Vec::new();
	for (name, mut artists) in index_map {
		artists.sort_unstable_by(|a, b| a.name.cmp(&b.name));
		indexes.push(Index {
			name,
			artist: artists,
		});
	}

	let payload = Indexes {
		last_modified: chrono::Utc::now().timestamp_millis() as u64,
		ignored_articles: Some("".to_string()),
		index: indexes,
	};

	if as_artists_node {
		resp.response.artists = Some(payload);
	} else {
		resp.response.indexes = Some(payload);
	}

	SubsonicResponder(resp)
}

#[derive(Deserialize)]
pub struct TopSongsQuery {
	#[serde(rename = "artistId")]
	pub artist_id: Option<String>,
	pub artist: Option<String>,
	pub count: Option<i32>,
}

pub async fn get_top_songs(
	query: web::Query<TopSongsQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let api = &subsonic_ctx.tidal_api;
	let mut resp = SubsonicResponseWrapper::ok();

	let count = query.count.unwrap_or(50).clamp(1, 500) as u32;
	let mut target_artist_id = None;

	if let Some(id_str) = &query.artist_id {
		if let Ok(id) = id_str.parse::<i64>() {
			target_artist_id = Some(id);
		}
	} else if let Some(artist_name) = &query.artist
		&& let Ok(search_res) = api.search(artist_name, 1, 0).await
		&& let Some(artists) = search_res.artists
		&& let Some(first_artist) = artists.items.first()
	{
		target_artist_id = Some(first_artist.id);
	}

	if let Some(artist_id) = target_artist_id
		&& let Ok(top_tracks) = api.get_artist_top_tracks(artist_id, count, 0).await
	{
		let songs: Vec<_> = top_tracks
			.items
			.into_iter()
			.take(count as usize)
			.map(|track| {
				crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
					&track,
					Some(&subsonic_ctx),
					None,
					None,
				)
			})
			.collect();
		resp.response.top_songs = Some(TopSongs { song: songs });
	}

	SubsonicResponder(resp)
}

pub async fn get_similar_songs(
	query: web::Query<IdQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let api = &subsonic_ctx.tidal_api;
	let mut resp = SubsonicResponseWrapper::ok();
	let limit = query.count.unwrap_or(50).clamp(1, 500) as u32;

	if let Ok(track_id) = query.id.parse::<i64>() {
		if let Ok(recommendations) = api.get_track_recommendations(track_id, limit, 0).await {
			let songs: Vec<_> = recommendations
				.items
				.into_iter()
				.map(|rec_item| {
					crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
						&rec_item.track,
						Some(&subsonic_ctx),
						None,
						None,
					)
				})
				.collect();
			resp.response.similar_songs = Some(SimilarSongs { song: songs });
		} else {
			return SubsonicResponder(SubsonicResponseWrapper::error(
				70,
				"Song not found or recommendations not available",
			));
		}
	} else {
		return SubsonicResponder(SubsonicResponseWrapper::error(70, "Invalid ID format"));
	}

	SubsonicResponder(resp)
}

pub async fn get_album_info(
	req: actix_web::HttpRequest,
	query: web::Query<IdQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	let is_v2 = req.path().contains("getAlbumInfo2");
	let mut resp = SubsonicResponseWrapper::ok();
	let api = &subsonic_ctx.tidal_api;

	if let Ok(id) = query.id.parse::<i64>() {
		let album = if let Ok(album) = api.get_album(id).await {
			Some(album)
		} else if let Ok(track) = api.get_track(id).await {
			api.get_album(track.album.id).await.ok()
		} else {
			None
		};

		if let Some(album) = album {
			let primary_artist = album
				.artists
				.as_ref()
				.and_then(|a| a.first())
				.or(album.artist.as_ref());
			let info = crate::api::subsonic::models::AlbumInfo {
				notes: None,
				music_brainz_id: None,
				small_image_url: album.cover.as_ref().map(|c| api.get_image_url(c, 750, 750)),
				medium_image_url: album.cover.as_ref().map(|c| api.get_image_url(c, 750, 750)),
				large_image_url: album
					.cover
					.as_ref()
					.map(|c| api.get_image_url(c, 1280, 1280)),
				last_fm_url: Some(format!(
					"https://www.last.fm/music/{}/{}",
					urlencoding::encode(primary_artist.map(|a| a.name.as_str()).unwrap_or("")),
					urlencoding::encode(&album.title)
				)),
			};

			if is_v2 {
				resp.response.album_info2 = Some(info);
			} else {
				resp.response.album_info = Some(info);
			}
			return SubsonicResponder(resp);
		}
	}

	SubsonicResponder(SubsonicResponseWrapper::error(70, "Album not found"))
}

#[derive(Deserialize)]
pub struct GetArtistInfoQuery {
	pub id: String,
}

pub async fn get_artist_info(
	req: actix_web::HttpRequest,
	query: web::Query<GetArtistInfoQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	let is_v2 = req.path().contains("getArtistInfo2");
	let mut resp = SubsonicResponseWrapper::ok();
	let api = &subsonic_ctx.tidal_api;

	if let Ok(artist_id) = query.id.parse::<i64>()
		&& let Ok(artist) = api.get_artist(artist_id).await
	{
		let mut bio_str = None;
		if let Ok(bio) = api.get_artist_bio(artist_id).await {
			static RE_TAGS: std::sync::LazyLock<Regex> =
				std::sync::LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
			static RE_WIMP: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
				Regex::new(r"\[wimpLink.*?\](.*?)\[/wimpLink\]").unwrap()
			});
			static RE_FOOTER: std::sync::LazyLock<Regex> =
				std::sync::LazyLock::new(|| Regex::new(r"~.*$").unwrap());

			let cleaned = RE_TAGS.replace_all(&bio.text, "");
			let cleaned = RE_WIMP.replace_all(&cleaned, "$1");
			let cleaned = RE_FOOTER.replace_all(&cleaned, "");

			bio_str = Some(cleaned.trim().to_string());
		}

		let mut similar_artists = Vec::new();
		if let Ok(similars) = api.get_similar_artists(artist_id, 20, 0).await {
			for a in similars.items {
				similar_artists.push(crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
					&a,
					Some(&subsonic_ctx),
				));
			}
		}

		let info = crate::api::subsonic::models::ArtistInfoBase {
			biography: bio_str,
			similar_artist: if similar_artists.is_empty() {
				None
			} else {
				Some(similar_artists)
			},
			small_image_url: artist
				.picture
				.as_ref()
				.map(|p| api.get_image_url(p, 750, 750)),
			medium_image_url: artist
				.picture
				.as_ref()
				.map(|p| api.get_image_url(p, 1000, 1000)),
			large_image_url: artist
				.picture
				.as_ref()
				.map(|p| api.get_image_url(p, 1500, 1500)),
			last_fm_url: Some(format!(
				"https://www.last.fm/music/{}",
				urlencoding::encode(&artist.name)
			)),
		};

		if is_v2 {
			resp.response.artist_info2 = Some(info);
		} else {
			resp.response.artist_info = Some(info);
		}
		return SubsonicResponder(resp);
	}

	SubsonicResponder(SubsonicResponseWrapper::error(70, "Artist not found"))
}

#[derive(Deserialize)]
pub struct RandomSongsQuery {
	pub size: Option<u32>,
	pub genre: Option<String>,
}

pub async fn get_random_songs(
	query: web::Query<RandomSongsQuery>,
	subsonic_ctx: web::ReqData<SubsonicContext>,
) -> impl Responder {
	let q = query.into_inner();
	let size = q.size.unwrap_or(10).clamp(1, 500);
	let genre = q.genre.as_deref().unwrap_or("16");

	let mut response = SubsonicResponseWrapper::ok();

	let api = &subsonic_ctx.tidal_api;
	let random_songs = if let Ok(tracks_result) = api
		.get_genre_tracks(genre, std::cmp::max(size * 2, 50), 0)
		.await
	{
		let mut tracks = tracks_result.items;
		tracks.sort_by_key(|t| t.id.wrapping_mul(123456789) % 100);

		tracks
			.into_iter()
			.take(size as usize)
			.map(|track| {
				crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
					&track,
					Some(&subsonic_ctx),
					None,
					None,
				)
			})
			.collect()
	} else {
		Vec::new()
	};

	response.response.random_songs =
		Some(crate::api::subsonic::models::RandomSongs { song: random_songs });
	SubsonicResponder(response)
}

#[derive(Deserialize)]
pub struct SongsByGenreQuery {
	pub genre: Option<String>,
	pub count: Option<u32>,
	pub offset: Option<u32>,
}

pub async fn get_songs_by_genre(
	query: web::Query<SongsByGenreQuery>,
	subsonic_ctx: web::ReqData<SubsonicContext>,
) -> impl Responder {
	let Some(genre) = &query.genre else {
		return SubsonicResponder(SubsonicResponseWrapper::error(
			10,
			"Missing parameter 'genre'",
		));
	};
	let count = query.count.unwrap_or(20).clamp(1, 500);
	let offset = query.offset.unwrap_or(0);

	let mut response = SubsonicResponseWrapper::ok();

	let api = &subsonic_ctx.tidal_api;
	let genre_songs = if let Ok(tracks_result) = api.get_genre_tracks(genre, count, offset).await {
		tracks_result
			.items
			.into_iter()
			.map(|track| {
				crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
					&track,
					Some(&subsonic_ctx),
					None,
					None,
				)
			})
			.collect()
	} else {
		Vec::new()
	};

	response.response.songs_by_genre =
		Some(crate::api::subsonic::models::SongsByGenre { song: genre_songs });
	SubsonicResponder(response)
}

pub async fn get_genres(subsonic_ctx: actix_web::web::ReqData<SubsonicContext>) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();

	let api = &subsonic_ctx.tidal_api;

	if let Ok(categories) = api.get_categories().await {
		let mut genres: Vec<_> = categories
			.into_iter()
			.map(|cat| crate::api::subsonic::models::Genre {
				value: cat.name,
				song_count: 1,
				album_count: 1,
			})
			.collect();

		genres.sort_unstable_by(|a, b| a.value.cmp(&b.value));
		resp.response.genres = Some(crate::api::subsonic::models::Genres {
			genre: Some(genres),
		});
	}

	SubsonicResponder(resp)
}

#[derive(Deserialize)]
pub struct AlbumListQuery {
	pub r#type: String,
	pub size: Option<i32>,
	pub offset: Option<i32>,
}

pub async fn get_album_list(
	list_query: web::Query<AlbumListQuery>,
	req: actix_web::HttpRequest,
	manager: web::Data<TidalClientManager>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	let api = &subsonic_ctx.tidal_api;
	let size = list_query.size.unwrap_or(10).clamp(1, 500) as usize;
	let offset = list_query.offset.unwrap_or(0).max(0) as u32;

	let mut subsonic_albums = Vec::with_capacity(size);
	let mut fetched_via_external = false;

	if matches!(list_query.r#type.as_str(), "random" | "recent" | "frequent")
		&& let Ok(Some((session_key, username))) =
			manager.db.get_lastfm_details(&subsonic_ctx.user).await
	{
		let lfm_limit = (size + offset as usize).max(20) as u32;
		let lfm_res = match list_query.r#type.as_str() {
			"random" => {
				LASTFM_CLIENT
					.random_albums(&username, &session_key, lfm_limit)
					.await
			}
			"recent" => {
				LASTFM_CLIENT
					.recent_tracks(&username, &session_key, lfm_limit)
					.await
			}
			"frequent" => {
				LASTFM_CLIENT
					.top_albums(&username, &session_key, lfm_limit, None)
					.await
			}
			_ => LASTFM_CLIENT.top_albums_by_tags(&username, lfm_limit).await,
		};

		if let Ok(lfm_albums) = lfm_res {
			let mut search_stream = stream::iter(lfm_albums)
				.map(|lfm| {
					let query = format!("{} {}", lfm.album, lfm.artist);
					async move { api.search(&query, 1, 0).await.ok() }
				})
				.buffered(50);

			let mut skipped = 0;
			while let Some(Some(search_res)) = search_stream.next().await {
				if let Some(item) = search_res.albums.and_then(|a| a.items.into_iter().next()) {
					if skipped < offset {
						skipped += 1;
						continue;
					}

					subsonic_albums.push(
						crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
							&item,
							Some(&subsonic_ctx),
							None,
						),
					);
				}
				if subsonic_albums.len() >= size {
					break;
				}
			}
			fetched_via_external = true;
		}
	}

	if !fetched_via_external {
		if list_query.r#type == "random" {
			let mut rng = rand::rng();
			subsonic_albums = ALBUM_CACHE
				.iter()
				.sample(&mut rng, size)
				.into_iter()
				.map(|album| {
					crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
						&album.1,
						Some(&subsonic_ctx),
						None,
					)
				})
				.collect();
		} else {
			let mut albums: Vec<_> = ALBUM_CACHE.iter().collect();
			albums.sort_unstable_by(|a, b| a.0.cmp(&b.0));

			subsonic_albums = albums
				.into_iter()
				.skip(offset as usize)
				.take(size)
				.map(|album| {
					crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
						&album.1,
						Some(&subsonic_ctx),
						None,
					)
				})
				.collect();
		}
	}

	let mut resp = SubsonicResponseWrapper::ok();
	let album_list = Some(AlbumList {
		album: subsonic_albums,
	});

	if req.path().contains("getAlbumList2") {
		resp.response.album_list2 = album_list;
	} else {
		resp.response.album_list = album_list;
	}

	SubsonicResponder(resp)
}

use crate::api::error::AppError;
use crate::api::subsonic::response::ApiResult;

pub async fn get_album(
	query: web::Query<IdQuery>,
	_manager: web::Data<TidalClientManager>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> ApiResult {
	ApiResult::from_result(
		async move {
			let api = &subsonic_ctx.tidal_api;
			let mut resp = SubsonicResponseWrapper::ok();

			let album_id = query
				.id
				.parse::<i64>()
				.map_err(|_| AppError::BadRequest("Invalid album ID".into()))?;

			let tidal_album = api.get_album(album_id).await?;
			let mut songs = Vec::new();
			if let Ok(tracks) = api.get_album_tracks(album_id, 50, 0).await {
				for track in tracks.items {
					songs.push(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
						&track,
						Some(&subsonic_ctx),
						Some(&tidal_album),
						None,
					));
				}
			}

			let mut subsonic_album = crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
				&tidal_album,
				Some(&subsonic_ctx),
				None,
			);
			subsonic_album.song = Some(songs);
			resp.response.album = Some(subsonic_album);

			Ok::<_, AppError>(resp)
		}
		.await,
	)
}

pub async fn get_artist(
	query: web::Query<IdQuery>,
	_manager: web::Data<TidalClientManager>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> ApiResult {
	ApiResult::from_result(
		async move {
			let api = &subsonic_ctx.tidal_api;
			let mut resp = SubsonicResponseWrapper::ok();

			let artist_id = query
				.id
				.parse::<i64>()
				.map_err(|_| AppError::BadRequest("Invalid artist ID".into()))?;

			let tidal_artist = api.get_artist(artist_id).await?;
			let mut albums = Vec::new();
			if let Ok(artist_albums) = api.get_artist_albums(artist_id, 50, 0).await
				&& let Ok(artist_singles) = api.get_artist_singles(artist_id, 50, 0).await
			{
				for tidal_album in mapping::dedupe_albums(
					artist_albums.items.into_iter().chain(artist_singles.items),
				) {
					albums.push(mapping::map_tidal_album_to_subsonic(
						&tidal_album,
						Some(&subsonic_ctx),
						Some(std::slice::from_ref(&tidal_artist)),
					));
				}
			}

			let mut subsonic_artist = crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
				&tidal_artist,
				Some(&subsonic_ctx),
			);
			subsonic_artist.album_count = albums.len() as i32;
			subsonic_artist.album = Some(albums);
			resp.response.artist = Some(subsonic_artist);

			Ok::<_, AppError>(resp)
		}
		.await,
	)
}

pub async fn get_song(
	query: web::Query<IdQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> ApiResult {
	ApiResult::from_result(
		async move {
			let api = &subsonic_ctx.tidal_api;
			let mut resp = SubsonicResponseWrapper::ok();

			let track_id = query
				.id
				.parse::<i64>()
				.map_err(|_| AppError::BadRequest("Invalid track ID".into()))?;

			let track = api.get_track(track_id).await?;
			resp.response.song = Some(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
				&track,
				Some(&subsonic_ctx),
				None,
				None,
			));

			Ok::<_, AppError>(resp)
		}
		.await,
	)
}

pub async fn get_music_directory(
	query: web::Query<IdQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	let api = &subsonic_ctx.tidal_api;
	let mut resp = SubsonicResponseWrapper::ok();

	if query.id == "1" {
		let mut children = Vec::new();

		let mut artists: Vec<_> = ARTIST_CACHE.iter().collect();
		artists.sort_unstable_by(|a, b| a.1.name.cmp(&b.1.name));

		for artist_entry in artists {
			let artist = crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
				&artist_entry.1,
				Some(&subsonic_ctx),
			);
			children.push(Child::from_artist(artist, "1"));
		}
		resp.response.directory = Some(Directory {
			id: "1".to_string(),
			parent: None,
			name: "Tidal".to_string(),
			child: Some(children),
		});
		return SubsonicResponder(resp);
	}

	if let Ok(num_id) = query.id.parse::<i64>() {
		let folder_id = query.into_inner().id;
		if let Ok(tidal_album) = api.get_album(num_id).await {
			let mut children = Vec::new();
			if let Ok(tracks) = api.get_album_tracks(num_id, 500, 0).await {
				for track in tracks.items {
					let song = crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
						&track,
						Some(&subsonic_ctx),
						Some(&tidal_album),
						None,
					);
					children.push(Child::from_song(song, &folder_id));
				}
			}
			resp.response.directory = Some(Directory {
				id: folder_id,
				parent: None,
				name: tidal_album.title,
				child: Some(children),
			});
			return SubsonicResponder(resp);
		}

		if let Ok(tidal_artist) = api.get_artist(num_id).await {
			let mut children = Vec::new();
			if let Ok(artist_albums) = api.get_artist_albums(num_id, 500, 0).await
				&& let Ok(artist_singles) = api.get_artist_singles(num_id, 500, 0).await
			{
				for tidal_album in mapping::dedupe_albums(
					artist_albums.items.into_iter().chain(artist_singles.items),
				) {
					let album = crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
						&tidal_album,
						Some(&subsonic_ctx),
						Some(std::slice::from_ref(&tidal_artist)),
					);
					children.push(Child::from_album(album, &folder_id));
				}
			}
			resp.response.directory = Some(Directory {
				id: folder_id,
				parent: Some("1".to_string()),
				name: tidal_artist.name,
				child: Some(children),
			});
			return SubsonicResponder(resp);
		}
	}

	SubsonicResponder(SubsonicResponseWrapper::error(70, "Folder not found"))
}

pub async fn get_internet_radio_stations() -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();
	resp.response.internet_radio_stations = Some(InternetRadioStations {
		internet_radio_station: Some(vec![]),
	});
	SubsonicResponder(resp)
}

#[cfg(test)]
mod tests {
	use super::*;
	use actix_web::body::to_bytes;
	use actix_web::test;

	#[actix_web::test]
	async fn test_get_music_folders_response() {
		let req = test::TestRequest::get()
			.uri("/rest/getMusicFolders")
			.to_http_request();
		let resp = get_music_folders().await.respond_to(&req);
		assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

		let body_bytes = to_bytes(resp.into_body()).await.unwrap_or_default();
		let body_str = String::from_utf8_lossy(&body_bytes);
		assert!(body_str.contains("<musicFolders>"));
		assert!(body_str.contains("name=\"Tidal\""));
	}

	#[actix_web::test]
	async fn test_get_internet_radio_stations_response() {
		let req = test::TestRequest::get()
			.uri("/rest/getInternetRadioStations")
			.to_http_request();
		let resp = get_internet_radio_stations().await.respond_to(&req);
		assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

		let body_bytes = to_bytes(resp.into_body()).await.unwrap_or_default();
		let body_str = String::from_utf8_lossy(&body_bytes);
		assert!(body_str.contains("<internetRadioStations"));
	}
}
