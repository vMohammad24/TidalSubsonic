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
use std::sync::Arc;

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
	let api = subsonic_ctx.tidal_api.clone();
	let mut index_map: std::collections::BTreeMap<String, Vec<Artist>> =
		std::collections::BTreeMap::new();

	for artist_entry in ARTIST_CACHE.iter() {
		let artist = crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
			&artist_entry.1,
			api.user_id(),
		);
		let name_upper = artist.name.to_uppercase();
		let mut initial = name_upper.chars().next().unwrap_or('#').to_string();
		if !initial.chars().next().unwrap().is_ascii_alphabetic() {
			initial = "#".to_string();
		}
		index_map.entry(initial).or_default().push(artist);
	}

	let mut indexes = Vec::new();
	for (name, mut artists) in index_map {
		artists.sort_by(|a, b| a.name.cmp(&b.name));
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
	let api = subsonic_ctx.tidal_api.clone();
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
		let mut songs = Vec::new();
		for track in top_tracks.items.into_iter().take(count as usize) {
			songs.push(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
				&track,
				api.user_id(),
				None,
				None,
			));
		}
		resp.response.top_songs = Some(TopSongs { song: songs });
	}

	SubsonicResponder(resp)
}

pub async fn get_similar_songs(
	query: web::Query<IdQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let api = subsonic_ctx.tidal_api.clone();
	let mut resp = SubsonicResponseWrapper::ok();
	let limit = query.count.unwrap_or(50).clamp(1, 500) as u32;

	if let Ok(track_id) = query.id.parse::<i64>() {
		if let Ok(recommendations) = api.get_track_recommendations(track_id, limit, 0).await {
			let mut songs = Vec::new();
			for rec_item in recommendations.items {
				songs.push(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
					&rec_item.track,
					api.user_id(),
					None,
					None,
				));
			}
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
	query: web::Query<IdQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	get_album_info_impl(query, subsonic_ctx, false).await
}

pub async fn get_album_info2(
	query: web::Query<IdQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	get_album_info_impl(query, subsonic_ctx, true).await
}

async fn get_album_info_impl(
	query: web::Query<IdQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
	is_v2: bool,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();
	let api = subsonic_ctx.tidal_api.clone();

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

pub async fn get_artist_info(
	query: web::Query<GetArtistInfoQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	get_artist_info_impl(query, subsonic_ctx, false).await
}

pub async fn get_artist_info2(
	query: web::Query<GetArtistInfoQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	get_artist_info_impl(query, subsonic_ctx, true).await
}

#[derive(Deserialize)]
pub struct GetArtistInfoQuery {
	pub id: String,
}

async fn get_artist_info_impl(
	query: web::Query<GetArtistInfoQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
	is_v2: bool,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();
	let api = subsonic_ctx.tidal_api.clone();

	if let Ok(artist_id) = query.id.parse::<i64>()
		&& let Ok(artist) = api.get_artist(artist_id).await
	{
		let mut bio_str = None;
		if let Ok(bio) = api.get_artist_bio(artist_id).await {
			let re_tags = Regex::new(r"<[^>]+>").unwrap();
			let re_wimp = Regex::new(r"\[wimpLink.*?\](.*?)\[/wimpLink\]").unwrap();
			let re_footer = Regex::new(r"~.*$").unwrap();

			let cleaned = re_tags.replace_all(&bio.text, "");
			let cleaned = re_wimp.replace_all(&cleaned, "$1");
			let cleaned = re_footer.replace_all(&cleaned, "");

			bio_str = Some(cleaned.trim().to_string());
		}

		let mut similar_artists = Vec::new();
		if let Ok(similars) = api.get_similar_artists(artist_id, 20, 0).await {
			for a in similars.items {
				similar_artists.push(crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
					&a,
					api.user_id(),
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
	let size = query.size.unwrap_or(10).clamp(1, 500);
	let genre = query.genre.clone().unwrap_or_else(|| "16".to_string());

	let mut response = SubsonicResponseWrapper::ok();

	let api = subsonic_ctx.tidal_api.clone();
	let mut random_songs = Vec::new();

	if let Ok(tracks_result) = api
		.get_genre_tracks(&genre, std::cmp::max(size * 2, 50), 0)
		.await
	{
		let mut tracks = tracks_result.items;
		tracks.sort_by_key(|t| t.id.wrapping_mul(123456789) % 100);

		for track in tracks.into_iter().take(size as usize) {
			random_songs.push(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
				&track,
				api.user_id(),
				None,
				None,
			));
		}
	}

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
	let genre = match &query.genre {
		Some(g) => g,
		None => {
			return SubsonicResponder(SubsonicResponseWrapper::error(
				10,
				"Missing parameter 'genre'",
			));
		}
	};
	let count = query.count.unwrap_or(20).clamp(1, 500);
	let offset = query.offset.unwrap_or(0);

	let mut response = SubsonicResponseWrapper::ok();

	let api = subsonic_ctx.tidal_api.clone();
	let mut genre_songs = Vec::new();

	if let Ok(tracks_result) = api.get_genre_tracks(genre, count, offset).await {
		for track in tracks_result.items {
			genre_songs.push(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
				&track,
				api.user_id(),
				None,
				None,
			));
		}
	}

	response.response.songs_by_genre =
		Some(crate::api::subsonic::models::SongsByGenre { song: genre_songs });
	SubsonicResponder(response)
}

pub async fn get_genres(subsonic_ctx: actix_web::web::ReqData<SubsonicContext>) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();

	let api = subsonic_ctx.tidal_api.clone();

	if let Ok(categories) = api.get_categories().await {
		let mut genres = Vec::new();
		for cat in categories {
			genres.push(crate::api::subsonic::models::Genre {
				value: cat.name,
				song_count: 1,
				album_count: 1,
			});
		}

		genres.sort_by(|a, b| a.value.cmp(&b.value));
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
	manager: web::Data<Arc<TidalClientManager>>,
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
				crate::api::lastfm::get_random_albums(&session_key, &username, lfm_limit).await
			}
			"recent" => {
				crate::api::lastfm::get_recent_tracks(&session_key, &username, lfm_limit).await
			}
			"frequent" => {
				crate::api::lastfm::get_top_albums(&session_key, &username, lfm_limit).await
			}
			_ => crate::api::lastfm::get_top_albums_by_tags(&username, lfm_limit).await,
		};

		if let Ok(lfm_albums) = lfm_res {
			let mut search_stream = stream::iter(lfm_albums)
				.map(|lfm| {
					let api = api.clone();
					async move {
						let query = format!("{} {}", lfm.album, lfm.artist);
						api.search(&query, 1, 0).await.ok()
					}
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
							api.user_id(),
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
						api.user_id(),
						None,
					)
				})
				.collect();
		} else {
			let mut albums: Vec<_> = ALBUM_CACHE.iter().collect();
			albums.sort_by_key(|a| *a.0);

			subsonic_albums = albums
				.into_iter()
				.skip(offset as usize)
				.take(size)
				.map(|album| {
					crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
						&album.1,
						api.user_id(),
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

pub async fn get_album(
	query: web::Query<IdQuery>,
	_manager: web::Data<Arc<TidalClientManager>>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	let api = subsonic_ctx.tidal_api.clone();
	let mut resp = SubsonicResponseWrapper::ok();

	if let Ok(album_id) = query.id.parse::<i64>() {
		match api.get_album(album_id).await {
			Ok(tidal_album) => {
				let mut songs = Vec::new();
				if let Ok(tracks) = api.get_album_tracks(album_id, 50, 0).await {
					for track in tracks.items {
						songs.push(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
							&track,
							api.user_id(),
							Some(&tidal_album),
							None,
						));
					}
				}

				let mut subsonic_album = crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
					&tidal_album,
					api.user_id(),
					None,
				);
				subsonic_album.song = Some(songs);
				resp.response.album = Some(subsonic_album);
			}
			Err(e) => {
				tracing::error!("Tidal API Error: {:?}", e);
				let msg = if cfg!(debug_assertions) {
					format!("Tidal API Error: {:?}", e)
				} else {
					"Upstream dependency failed".to_string()
				};
				return SubsonicResponder(SubsonicResponseWrapper::error(0, &msg));
			}
		}
	}

	SubsonicResponder(resp)
}

pub async fn get_artist(
	query: web::Query<IdQuery>,
	_manager: web::Data<Arc<TidalClientManager>>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	let api = subsonic_ctx.tidal_api.clone();
	let mut resp = SubsonicResponseWrapper::ok();

	if let Ok(artist_id) = query.id.parse::<i64>() {
		match api.get_artist(artist_id).await {
			Ok(tidal_artist) => {
				let mut albums = Vec::new();
				if let Ok(artist_albums) = api.get_artist_albums(artist_id, 50, 0).await
					&& let Ok(artist_singles) = api.get_artist_singles(artist_id, 50, 0).await
				{
					for tidal_album in
						mapping::dedupe_albums([artist_albums.items, artist_singles.items].concat())
					{
						albums.push(mapping::map_tidal_album_to_subsonic(
							&tidal_album,
							api.user_id(),
							Some(std::slice::from_ref(&tidal_artist)),
						));
					}
				}

				let mut subsonic_artist =
					crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
						&tidal_artist,
						api.user_id(),
					);
				subsonic_artist.album_count = albums.len() as i32;
				subsonic_artist.album = Some(albums);
				resp.response.artist = Some(subsonic_artist);
			}
			Err(e) => {
				tracing::error!("Tidal API Error: {:?}", e);
				let msg = if cfg!(debug_assertions) {
					format!("Tidal API Error: {:?}", e)
				} else {
					"Upstream dependency failed".to_string()
				};
				return SubsonicResponder(SubsonicResponseWrapper::error(0, &msg));
			}
		}
	}

	SubsonicResponder(resp)
}

pub async fn get_song(
	query: web::Query<IdQuery>,
	_manager: web::Data<Arc<TidalClientManager>>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	let api = subsonic_ctx.tidal_api.clone();
	let mut resp = SubsonicResponseWrapper::ok();

	if let Ok(track_id) = query.id.parse::<i64>() {
		match api.get_track(track_id).await {
			Ok(track) => {
				resp.response.song =
					Some(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
						&track,
						api.user_id(),
						None,
						None,
					));
			}
			Err(e) => {
				tracing::error!("Tidal API Error: {:?}", e);
				let msg = if cfg!(debug_assertions) {
					format!("Tidal API Error: {:?}", e)
				} else {
					"Upstream dependency failed".to_string()
				};
				return SubsonicResponder(SubsonicResponseWrapper::error(0, &msg));
			}
		}
	}

	SubsonicResponder(resp)
}

pub async fn get_music_directory(
	query: web::Query<IdQuery>,
	subsonic_ctx: actix_web::web::ReqData<SubsonicContext>,
) -> impl Responder {
	let api = subsonic_ctx.tidal_api.clone();
	let mut resp = SubsonicResponseWrapper::ok();

	if query.id == "1" {
		let mut children = Vec::new();

		let mut artists: Vec<_> = ARTIST_CACHE.iter().collect();
		artists.sort_by(|a, b| a.1.name.cmp(&b.1.name));

		for artist_entry in artists {
			let artist = crate::api::subsonic::mapping::map_tidal_artist_to_subsonic(
				&artist_entry.1,
				api.user_id(),
			);
			children.push(Child {
				id: artist.id.clone(),
				parent: Some("1".to_string()),
				title: artist.name.clone(),
				album: None,
				artist: Some(artist.name.clone()),
				is_dir: true,
				is_video: None,
				type_: None,
				cover_art: Some(artist.cover_art),
				duration: None,
				bit_rate: None,
				track: None,
				album_id: None,
				artist_id: Some(artist.id),
				size: None,
				suffix: None,
				content_type: None,
				year: None,
				genre: None,
				starred: artist.starred,
				path: None,
				play_count: None,
				disc_number: None,
				created: None,
				explicit_status: None,
			});
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
		if let Ok(tidal_album) = api.get_album(num_id).await {
			let mut children = Vec::new();
			if let Ok(tracks) = api.get_album_tracks(num_id, 500, 0).await {
				for track in tracks.items {
					let song = crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
						&track,
						api.user_id(),
						Some(&tidal_album),
						None,
					);
					children.push(Child {
						id: song.id.clone(),
						parent: Some(query.id.clone()),
						title: song.title.clone(),
						album: Some(song.album.clone()),
						artist: Some(song.artist.clone()),
						is_dir: false,
						is_video: Some(song.is_video),
						type_: Some(song.type_.clone()),
						cover_art: Some(song.cover_art.clone()),
						duration: Some(song.duration),
						bit_rate: Some(song.bit_rate),
						track: Some(song.track),
						album_id: Some(song.album_id.clone()),
						artist_id: Some(song.artist_id.clone()),
						size: Some(song.size),
						suffix: Some(song.suffix.clone()),
						content_type: Some(song.content_type.clone()),
						year: song.year,
						genre: song.genre.clone(),
						starred: song.starred,
						path: song.path.clone(),
						play_count: song.play_count,
						disc_number: song.disc_number,
						created: song.created.clone(),
						explicit_status: song.explicit_status.clone(),
					});
				}
			}
			resp.response.directory = Some(Directory {
				id: query.id.clone(),
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
				for tidal_album in
					mapping::dedupe_albums([artist_albums.items, artist_singles.items].concat())
				{
					let album = crate::api::subsonic::mapping::map_tidal_album_to_subsonic(
						&tidal_album,
						api.user_id(),
						Some(std::slice::from_ref(&tidal_artist)),
					);
					children.push(Child {
						id: album.id.clone(),
						parent: Some(query.id.clone()),
						title: album.name.clone(),
						album: None,
						artist: Some(album.artist.clone()),
						is_dir: true,
						is_video: None,
						type_: None,
						cover_art: Some(album.cover_art.clone()),
						duration: Some(album.duration),
						bit_rate: None,
						track: None,
						album_id: None,
						artist_id: Some(album.artist_id.clone()),
						size: None,
						suffix: None,
						content_type: None,
						year: album.year,
						genre: None,
						starred: album.starred,
						path: None,
						play_count: None,
						disc_number: None,
						created: Some(album.created.clone()),
						explicit_status: album.explicit_status.clone(),
					});
				}
			}
			resp.response.directory = Some(Directory {
				id: query.id.clone(),
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
