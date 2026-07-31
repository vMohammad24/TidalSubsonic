use crate::tidal::{
	error::TidalError,
	favorites::{add_favorite, get_favorites_count, remove_favorite, set_favorites_map},
	models::{
		Album, Artist, JsonApiDocument, MixEndpointResponse, MixIncluded, MixPlaylistResource,
		Playlist, PlaylistDetailIncluded, SearchResult, Track,
	},
	session::{ApiVersion, Session},
};
use chrono::Utc;
use moka::future::Cache;
use moka::sync::Cache as SyncCache;
use reqwest::Method;
use std::sync::LazyLock;
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};

pub static ALBUM_CACHE: LazyLock<Cache<i64, Album>> = LazyLock::new(|| {
	Cache::builder()
		.time_to_live(Duration::from_secs(3600))
		.max_capacity(100000)
		.build()
});
pub static TRACK_CACHE: LazyLock<Cache<i64, Track>> = LazyLock::new(|| {
	Cache::builder()
		.time_to_live(Duration::from_secs(3600))
		.max_capacity(100000)
		.build()
});
pub static ARTIST_CACHE: LazyLock<Cache<i64, Artist>> = LazyLock::new(|| {
	Cache::builder()
		.time_to_live(Duration::from_secs(3600))
		.max_capacity(100000)
		.build()
});
pub static SEARCH_CACHE: LazyLock<Cache<String, SearchResult>> = LazyLock::new(|| {
	Cache::builder()
		.time_to_live(Duration::from_secs(3600))
		.max_capacity(1000)
		.build()
});

pub static MISC_CACHE: LazyLock<Cache<String, serde_json::Value>> = LazyLock::new(|| {
	Cache::builder()
		.time_to_live(Duration::from_secs(3600))
		.max_capacity(1000)
		.build()
});

pub static ARTIST_ALBUM_COUNT_CACHE: LazyLock<SyncCache<i64, i32>> = LazyLock::new(|| {
	SyncCache::builder()
		.time_to_live(Duration::from_secs(7200))
		.max_capacity(100000)
		.build()
});

// id : etag
// pub static PLAYLIST_CACHE: LazyLock<Cache<String, String>> = LazyLock::new(|| {
// 	Cache::builder()
// 		.time_to_live(Duration::from_secs(3600))
// 		.max_capacity(1000)
// 		.build()
// });

use crate::tidal::models::entities::ArtworkAttrs;

fn best_artwork_url(attrs: &ArtworkAttrs) -> Option<String> {
	let files = attrs.files.as_ref()?;
	files
		.iter()
		.find(|f| f.meta.width == 320 && f.meta.height == 320)
		.or_else(|| files.first())
		.map(|f| f.href.clone())
}

#[derive(Clone)]
pub struct TidalApi {
	session: Arc<Session>,
}

impl TidalApi {
	pub fn new(session: Arc<Session>) -> Self {
		let api = Self { session };

		if let Some(user_id) = api.user_id()
			&& get_favorites_count(user_id) == 0
		{
			api.preload();
		}
		api
	}

	pub fn user_id(&self) -> Option<i64> {
		self.session.user_id
	}

	pub fn preload(&self) {
		if let Some(user_id) = self.user_id() {
			let cache_key = format!("preload_{}", user_id);
			if MISC_CACHE.contains_key(&cache_key) {
				return;
			}
			tracing::info!(user = %user_id, "Preloading user");
			let api = self.clone();

			tokio::spawn(async move {
				let mut favorites: HashMap<i64, String> = HashMap::new();
				MISC_CACHE
					.insert(cache_key, serde_json::Value::Bool(true))
					.await;

				let mut favorite_track_ids: Vec<i64> = Vec::new();
				let mut favorite_artist_ids: Vec<i64> = Vec::new();

				if let Ok(albums) = api.get_favorite_albums(user_id, 5000, 0).await {
					for album in albums.items {
						album.item.cache();
						favorites.insert(album.item.id, album.created);
					}
				}

				if let Ok(tracks) = api.get_favorite_tracks(user_id, 5000, 0).await {
					for track in tracks.items {
						track.item.cache();
						favorites.insert(track.item.id, track.created);
						favorite_track_ids.push(track.item.id);
					}
				}
				if let Ok(artists) = api.get_favorite_artists(user_id, 5000, 0).await {
					for artist in artists.items {
						artist.item.cache();
						favorites.insert(artist.item.id, artist.created);
						favorite_artist_ids.push(artist.item.id);
					}
				}

				set_favorites_map(user_id, favorites);

				match api.get_user_playlists(user_id).await {
					Ok(list) => {
						for pl in list.items {
							let playlist_id = pl.uuid;
							let mut offset: u32 = 0;
							let limit: u32 = 10000;

							loop {
								match api.get_playlist_tracks(&playlist_id, limit, offset).await {
									Ok(page) => {
										for track in &page.items {
											track.cache();
										}

										if page.items.len() < limit as usize {
											break;
										}
										offset += limit;
									}
									Err(e) => {
										tracing::error!(
											error = %e,
											playlist_id = %playlist_id,
											"Failed to fetch playlist tracks for preload"
										);
										break;
									}
								}
							}
						}
					}
					Err(e) => tracing::debug!(error = %e, "Failed to preload playlists"),
				}

				tokio::join!(
					async {
						match api.get_user_mixes().await {
							Ok(mixes) => {
								tracing::debug!(count = %mixes.len(), "Preloaded user mixes");
							}
							Err(e) => tracing::debug!(error = %e, "Failed to preload user mixes"),
						}
					},
					async {
						for fav_track_id in favorite_track_ids.iter().take(5) {
							if let Ok(recs) =
								api.get_track_recommendations(*fav_track_id, 20, 0).await
							{
								for rec in &recs.items {
									rec.track.cache();
								}
							}
						}
					},
					async {
						for fav_artist_id in favorite_artist_ids.iter().take(20) {
							if let Ok(tracks) =
								api.get_artist_top_tracks(*fav_artist_id, 20, 0).await
							{
								for track in &tracks.items {
									track.cache();
								}
							}
						}
					},
				);
			});
		} else {
			tracing::debug!("No user id available for preload");
		}
	}

	pub async fn get_album(&self, album_id: i64) -> Result<Album, TidalError> {
		if let Some(album) = ALBUM_CACHE.get(&album_id).await {
			return Ok(album);
		}
		let album = self
			.session
			.request::<Album>(
				Method::GET,
				&format!("/albums/{}", album_id),
				None,
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await?;
		ALBUM_CACHE.insert(album_id, album.clone()).await;
		Ok(album)
	}

	pub async fn get_track(&self, track_id: i64) -> Result<Track, TidalError> {
		if let Some(track) = TRACK_CACHE.get(&track_id).await {
			return Ok(track);
		}
		let track = self
			.session
			.request::<Track>(
				Method::GET,
				&format!("/tracks/{}", track_id),
				None,
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await?;
		TRACK_CACHE.insert(track_id, track.clone()).await;
		Ok(track)
	}

	pub async fn get_track_recommendations(
		&self,
		track_id: i64,
		limit: u32,
		offset: u32,
	) -> Result<
		crate::tidal::models::SearchResultItems<crate::tidal::models::entities::RecommendationItem>,
		TidalError,
	> {
		let limit_str = limit.to_string();
		let offset_str = offset.to_string();
		let query = [
			("limit", limit_str.as_str()),
			("offset", offset_str.as_str()),
		];
		self.session
			.request::<crate::tidal::models::SearchResultItems<
				crate::tidal::models::entities::RecommendationItem,
			>>(
				Method::GET,
				&format!("/tracks/{}/recommendations", track_id),
				Some(&query),
				None,
				ApiVersion::V1,
			)
			.await
	}

	pub async fn get_artist(&self, artist_id: i64) -> Result<Artist, TidalError> {
		if let Some(artist) = ARTIST_CACHE.get(&artist_id).await {
			return Ok(artist);
		}
		let artist = self
			.session
			.request::<Artist>(
				Method::GET,
				&format!("/artists/{}", artist_id),
				None,
				None,
				ApiVersion::V1,
			)
			.await?;
		ARTIST_CACHE.insert(artist_id, artist.clone()).await;
		Ok(artist)
	}

	pub async fn get_artist_bio(
		&self,
		artist_id: i64,
	) -> Result<crate::tidal::models::ArtistBio, TidalError> {
		let cache_key = format!("artist_bio_{}", artist_id);

		if let Some(bio) = MISC_CACHE.get(&cache_key).await {
			if let Ok(bio) = serde_json::from_value::<crate::tidal::models::ArtistBio>(bio) {
				return Ok(bio);
			}
			MISC_CACHE.invalidate(&cache_key).await;
			tracing::warn!(
				artist_id = %artist_id,
				"Corrupted cache entry for artist bio. Cache entry has been invalidated."
			);
		}

		let res = self
			.session
			.request::<crate::tidal::models::ArtistBio>(
				Method::GET,
				&format!("/artists/{}/bio", artist_id),
				None,
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await;

		if let Ok(ref bio_res) = res
			&& let Ok(bio_value) = serde_json::to_value(bio_res)
		{
			let key = cache_key;

			tokio::spawn(async move {
				MISC_CACHE.insert(key, bio_value).await;
			});
		}

		res
	}

	pub async fn get_lyrics(
		&self,
		track_id: i64,
	) -> Result<crate::tidal::models::entities::Lyrics, TidalError> {
		self.session
			.request::<crate::tidal::models::entities::Lyrics>(
				Method::GET,
				&format!("/tracks/{}/lyrics", track_id),
				None,
				None,
				ApiVersion::V1,
			)
			.await
	}

	pub async fn get_playlist(&self, playlist_id: &str) -> Result<Playlist, TidalError> {
		self.session
			.request::<Playlist>(
				Method::GET,
				&format!("/playlists/{}", playlist_id),
				None,
				None,
				ApiVersion::V1,
			)
			.await
	}

	pub async fn get_user_mixes(
		&self,
	) -> Result<Vec<crate::tidal::models::entities::MixPlaylistInfo>, TidalError> {
		let user_id = self.user_id().ok_or(TidalError::Authentication(
			"No authenticated user for mix fetch".to_string(),
		))?;
		let cache_key = format!("mixes_{}", user_id);

		if let Some(cached) = MISC_CACHE.get(&cache_key).await
			&& let Ok(mixes) = serde_json::from_value::<
				Vec<crate::tidal::models::entities::MixPlaylistInfo>,
			>(cached)
		{
			return Ok(mixes);
		}

		let mix_endpoints = [
			"/userDailyMixes/me",
			"/userDiscoveryMixes/me",
			"/userNewReleaseMixes/me",
			"/userOfflineMixes/me",
		];

		let mut all_mixes: Vec<crate::tidal::models::entities::MixPlaylistInfo> = Vec::new();

		for endpoint in &mix_endpoints {
			match self
				.session
				.request::<MixEndpointResponse>(
					Method::GET,
					endpoint,
					Some(&[("include", "items,items.coverArt")]),
					None,
					ApiVersion::OpenApi,
				)
				.await
			{
				Ok(resp) => {
					if let Some(included) = resp.included {
						let mut art_to_url: HashMap<String, String> = HashMap::new();
						let mut playlist_items: Vec<MixPlaylistResource> = Vec::new();

						for item in included {
							match item {
								MixIncluded::Playlist(pl) => playlist_items.push(pl),
								MixIncluded::Artwork { id, attributes } => {
									if let Some(url) = best_artwork_url(&attributes) {
										art_to_url.insert(id, url);
									}
								}
								MixIncluded::Unknown => {}
							}
						}

						for pl in playlist_items {
							let cover_art = pl
								.relationships
								.and_then(|r| r.cover_art)
								.and_then(|ca| ca.data)
								.and_then(|d| d.into_iter().next())
								.and_then(|d| art_to_url.get(&d.id).cloned());

							all_mixes.push(crate::tidal::models::entities::MixPlaylistInfo {
								id: pl.id,
								name: pl.attributes.name.unwrap_or_else(|| "Mix".to_string()),
								description: pl.attributes.description,
								cover_art,
							});
						}
					}
					tracing::debug!(
						endpoint = %endpoint,
						found = %all_mixes.len(),
						"Fetched mix playlists"
					);
				}
				Err(e) => {
					tracing::warn!(endpoint = %endpoint, error = %e, "Failed to fetch mix endpoint");
				}
			}
		}

		if let Ok(json) = serde_json::to_value(&all_mixes) {
			MISC_CACHE.insert(cache_key, json).await;
		}

		Ok(all_mixes)
	}

	fn parse_iso_duration(s: &str) -> (i64, i64) {
		let Some(s) = s.strip_prefix("PT") else {
			tracing::warn!(raw = %s, "ISO duration missing PT prefix, expected track duration");
			return (0, 0);
		};
		let mut minutes = 0i64;
		let mut seconds = 0i64;
		let remaining = if let Some(h_pos) = s.find('H') {
			minutes += s[..h_pos].parse::<i64>().unwrap_or(0) * 60;
			&s[h_pos + 1..]
		} else {
			s
		};
		if let Some(m_pos) = remaining.find('M') {
			minutes += remaining[..m_pos].parse().unwrap_or(0);
			let after = &remaining[m_pos + 1..];
			if let Some(s_pos) = after.find('S') {
				seconds = after[..s_pos].parse().unwrap_or(0);
			}
		} else if let Some(s_pos) = remaining.find('S') {
			seconds = remaining[..s_pos].parse().unwrap_or(0);
		}
		(minutes, seconds)
	}

	pub async fn get_openapi_playlist_detail(
		&self,
		playlist_id: &str,
	) -> Result<crate::tidal::models::entities::PlaylistDetail, TidalError> {
		let doc = self
			.session
			.request::<JsonApiDocument<crate::tidal::models::PlaylistAttributesV2, PlaylistDetailIncluded>>(
				Method::GET,
				&format!("/playlists/{}", playlist_id),
				Some(&[("include", "items,coverArt")]),
				None,
				ApiVersion::OpenApi,
			)
			.await?;

		let attrs = doc.data.attributes;
		let mut duration_secs: i64 = 0;
		let mut track_ids = Vec::new();
		let mut cover_art: Option<String> = None;

		if let Some(included) = doc.included {
			for item in included {
				match item {
					PlaylistDetailIncluded::Track { id, attributes } => {
						track_ids.push(id);
						if let Some(dur_str) = attributes.duration {
							let (m, s) = Self::parse_iso_duration(&dur_str);
							duration_secs += m * 60 + s;
						}
					}
					PlaylistDetailIncluded::Artwork { attributes } => {
						if cover_art.is_none() {
							cover_art = best_artwork_url(&attributes);
						}
					}
					PlaylistDetailIncluded::Unknown => {}
				}
			}
		}

		Ok(crate::tidal::models::entities::PlaylistDetail {
			id: doc.data.id,
			name: attrs.name,
			description: attrs.description,
			song_count: attrs.number_of_items.unwrap_or(track_ids.len() as i32),
			duration_secs,
			cover_art,
			track_ids,
		})
	}

	pub fn get_image_url(&self, id: &str, width: u32, height: u32) -> String {
		let path = id.replace('-', "/");
		format!(
			"https://resources.tidal.com/images/{}/{}x{}.jpg",
			path, width, height
		)
	}

	pub async fn get_user_playlists(
		&self,
		user_id: i64,
	) -> Result<crate::tidal::models::SearchResultItems<Playlist>, TidalError> {
		self.session
			.request::<crate::tidal::models::SearchResultItems<Playlist>>(
				Method::GET,
				&format!("/users/{}/playlists", user_id),
				None,
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}

	pub async fn search(
		&self,
		query: &str,
		limit: u32,
		offset: u32,
	) -> Result<crate::tidal::models::SearchResult, TidalError> {
		let result = self
			.session
			.request::<crate::tidal::models::SearchResult>(
				Method::GET,
				"/search",
				Some(&[
					("query", query),
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await;

		if let Ok(ref res) = result {
			let query_key = query.to_string();
			let res_for_cache = res.clone();

			tokio::spawn(async move {
				if let Some(ref tracks) = res_for_cache.tracks {
					for track in &tracks.items {
						TRACK_CACHE.insert(track.id, track.clone()).await;
					}
				}

				if let Some(ref albums) = res_for_cache.albums {
					for album in &albums.items {
						ALBUM_CACHE.insert(album.id, album.clone()).await;
					}
				}

				if let Some(ref artists) = res_for_cache.artists {
					for artist in &artists.items {
						ARTIST_CACHE.insert(artist.id, artist.clone()).await;
					}
				}

				SEARCH_CACHE.insert(query_key, res_for_cache).await;
			});
		}

		result
	}

	pub async fn get_album_tracks(
		&self,
		album_id: i64,
		limit: u32,
		offset: u32,
	) -> Result<crate::tidal::models::SearchResultItems<Track>, TidalError> {
		self.session
			.request::<crate::tidal::models::SearchResultItems<Track>>(
				Method::GET,
				&format!("/albums/{}/tracks", album_id),
				Some(&[
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}

	pub async fn get_artist_albums(
		&self,
		artist_id: i64,
		limit: u32,
		offset: u32,
	) -> Result<crate::tidal::models::SearchResultItems<Album>, TidalError> {
		let result = self
			.session
			.request::<crate::tidal::models::SearchResultItems<Album>>(
				Method::GET,
				&format!("/artists/{}/albums", artist_id),
				Some(&[
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await;

		if let Ok(ref albums) = result
			&& let Some(total) = albums.total_number_of_items
		{
			ARTIST_ALBUM_COUNT_CACHE.insert(artist_id, total);
		}

		result
	}

	pub async fn get_artist_top_tracks(
		&self,
		artist_id: i64,
		limit: u32,
		offset: u32,
	) -> Result<crate::tidal::models::SearchResultItems<Track>, TidalError> {
		self.session
			.request::<crate::tidal::models::SearchResultItems<Track>>(
				Method::GET,
				&format!("/artists/{}/toptracks", artist_id),
				Some(&[
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}

	pub async fn get_similar_artists(
		&self,
		artist_id: i64,
		limit: u32,
		offset: u32,
	) -> Result<crate::tidal::models::SearchResultItems<Artist>, TidalError> {
		self.session
			.request::<crate::tidal::models::SearchResultItems<Artist>>(
				Method::GET,
				&format!("/artists/{}/similar", artist_id),
				Some(&[
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}

	pub async fn get_playlist_tracks(
		&self,
		playlist_id: &str,
		limit: u32,
		offset: u32,
	) -> Result<crate::tidal::models::SearchResultItems<Track>, TidalError> {
		self.session
			.request::<crate::tidal::models::SearchResultItems<Track>>(
				Method::GET,
				&format!("/playlists/{}/tracks", playlist_id),
				Some(&[
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}

	pub async fn get_stream_url(
		&self,
		track_id: i64,
	) -> Result<crate::tidal::models::PlaybackInfo, TidalError> {
		self.session
			.request::<crate::tidal::models::PlaybackInfo>(
				reqwest::Method::GET,
				&format!("/tracks/{}/playbackinfopostpaywall", track_id),
				Some(&[
					("audioquality", "HI_RES_LOSSLESS"),
					("playbackmode", "STREAM"),
					("assetpresentation", "FULL"),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}
	pub async fn get_favorite_tracks(
		&self,
		user_id: i64,
		limit: u32,
		offset: u32,
	) -> Result<
		crate::tidal::models::SearchResultItems<crate::tidal::models::FavoriteItem<Track>>,
		TidalError,
	> {
		self.session
			.request::<crate::tidal::models::SearchResultItems<crate::tidal::models::FavoriteItem<Track>>>(
				reqwest::Method::GET,
				&format!("/users/{}/favorites/tracks", user_id),
				Some(&[
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}

	pub async fn get_favorite_albums(
		&self,
		user_id: i64,
		limit: u32,
		offset: u32,
	) -> Result<
		crate::tidal::models::SearchResultItems<crate::tidal::models::FavoriteItem<Album>>,
		TidalError,
	> {
		self.session
			.request::<crate::tidal::models::SearchResultItems<crate::tidal::models::FavoriteItem<Album>>>(
				reqwest::Method::GET,
				&format!("/users/{}/favorites/albums", user_id),
				Some(&[
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}

	pub async fn get_favorite_artists(
		&self,
		user_id: i64,
		limit: u32,
		offset: u32,
	) -> Result<
		crate::tidal::models::SearchResultItems<crate::tidal::models::FavoriteItem<Artist>>,
		TidalError,
	> {
		self.session
			.request::<crate::tidal::models::SearchResultItems<crate::tidal::models::FavoriteItem<Artist>>>(
				reqwest::Method::GET,
				&format!("/users/{}/favorites/artists", user_id),
				Some(&[
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}

	pub async fn get_artist_singles(
		&self,
		artist_id: i64,
		limit: u32,
		offset: u32,
	) -> Result<crate::tidal::models::SearchResultItems<Album>, TidalError> {
		self.session
			.request::<crate::tidal::models::SearchResultItems<Album>>(
				reqwest::Method::GET,
				&format!("/artists/{}/albums", artist_id),
				Some(&[
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
					("filter", "EPSANDSINGLES"),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}

	pub async fn add_favorite_track(&self, track_id: i64) -> Result<(), TidalError> {
		let body = [("trackIds", track_id.to_string())];
		if let Some(user_id) = self.user_id() {
			add_favorite(user_id, track_id, Utc::now().to_rfc3339());
		}
		self.session
			.request::<()>(
				reqwest::Method::POST,
				"/users/favorites/tracks",
				None,
				Some(&body),
				crate::tidal::session::ApiVersion::V1,
			)
			.await
			.map(|_| ())
	}

	pub async fn remove_favorite_track(&self, track_id: i64) -> Result<(), TidalError> {
		if let Some(user_id) = self.user_id() {
			remove_favorite(user_id, track_id);
		}
		self.session
			.request::<()>(
				reqwest::Method::DELETE,
				&format!("/users/favorites/tracks/{}", track_id),
				None,
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
			.map(|_| ())
	}

	pub async fn add_favorite_album(&self, album_id: i64) -> Result<(), TidalError> {
		let body = [("albumIds", album_id.to_string())];
		if let Some(user_id) = self.user_id() {
			add_favorite(user_id, album_id, Utc::now().to_rfc3339());
		}
		self.session
			.request::<()>(
				reqwest::Method::POST,
				&format!("/users/{}/favorites/albums", self.user_id().unwrap_or(0)),
				None,
				Some(&body),
				crate::tidal::session::ApiVersion::V1,
			)
			.await
			.map(|_| ())
	}

	pub async fn remove_favorite_album(&self, album_id: i64) -> Result<(), TidalError> {
		if let Some(user_id) = self.user_id() {
			remove_favorite(user_id, album_id);
		}
		self.session
			.request::<()>(
				reqwest::Method::DELETE,
				&format!(
					"/users/{}/favorites/albums/{}",
					self.user_id().unwrap_or(0),
					album_id
				),
				None,
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
			.map(|_| ())
	}

	pub async fn add_favorite_artist(&self, artist_id: i64) -> Result<(), TidalError> {
		let body = [("artistIds", artist_id.to_string())];

		if let Some(user_id) = self.user_id() {
			add_favorite(user_id, artist_id, Utc::now().to_rfc3339());
		}
		self.session
			.request::<()>(
				reqwest::Method::POST,
				&format!("/users/{}/favorites/artists", self.user_id().unwrap_or(0)),
				None,
				Some(&body),
				crate::tidal::session::ApiVersion::V1,
			)
			.await
			.map(|_| ())
	}

	pub async fn remove_favorite_artist(&self, artist_id: i64) -> Result<(), TidalError> {
		if let Some(user_id) = self.user_id() {
			remove_favorite(user_id, artist_id);
		}
		self.session
			.request::<()>(
				reqwest::Method::DELETE,
				&format!(
					"/users/{}/favorites/artists/{}",
					self.user_id().unwrap_or(0),
					artist_id
				),
				None,
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
			.map(|_| ())
	}

	pub async fn create_playlist(
		&self,
		title: &str,
		description: Option<&str>,
	) -> Result<Playlist, TidalError> {
		let body = serde_json::json!({
			"data": {
				"type": "playlists",
				"attributes": {
					"name": title,
					"description": description,
					"accessType": "UNLISTED"
				}
			}
		});

		let response = self
			.session
			.request_full::<crate::tidal::models::JsonApiResponse<
				crate::tidal::models::PlaylistAttributesV2,
			>>(
				reqwest::Method::POST,
				"/playlists",
				None,
				None,
				Some(body),
				crate::tidal::session::ApiVersion::OpenApi,
			)
			.await?;

		Ok(Playlist {
			uuid: response.data.id,
			title: response.data.attributes.name,
			description: response.data.attributes.description,
			number_of_tracks: 0,
			duration: 0,
			..Default::default()
		})
	}

	pub async fn delete_playlist(&self, playlist_id: &str) -> Result<(), TidalError> {
		self.session
			.request::<()>(
				reqwest::Method::PUT,
				"/my-collection/playlists/folders/remove",
				Some(&[("trns", format!("trn:playlist:{}", playlist_id).as_str())]),
				None,
				crate::tidal::session::ApiVersion::V2,
			)
			.await
			.map(|_| ())
	}

	pub async fn add_tracks_to_playlist(
		&self,
		playlist_id: &str,
		track_ids: &[&str],
	) -> Result<(), TidalError> {
		let joined = track_ids.join(",");
		let body = [
			("trackIds", joined),
			("onArtifactNotFound", "FAIL".to_string()),
		];
		self.session
			.request::<()>(
				reqwest::Method::POST,
				&format!("/playlists/{}/items", playlist_id),
				None,
				Some(&body),
				crate::tidal::session::ApiVersion::V1,
			)
			.await
			.map(|_| ())
	}

	pub async fn remove_tracks_from_playlist(
		&self,
		playlist_id: &str,
		index: u32,
	) -> Result<(), TidalError> {
		self.session
			.request::<()>(
				reqwest::Method::DELETE,
				&format!("/playlists/{}/items/{}", playlist_id, index),
				None,
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
			.map(|_| ())
	}

	pub async fn update_playlist(
		&self,
		playlist_id: &str,
		title: Option<&str>,
		description: Option<&str>,
	) -> Result<(), TidalError> {
		let mut body: Vec<(&str, String)> = Vec::new();
		if let Some(t) = title {
			body.push(("title", t.to_string()));
		}
		if let Some(desc) = description {
			body.push(("description", desc.to_string()));
		}

		self.session
			.request::<()>(
				reqwest::Method::POST,
				&format!("/playlists/{}", playlist_id),
				None,
				Some(&body),
				crate::tidal::session::ApiVersion::V1,
			)
			.await
			.map(|_| ())
	}

	pub async fn get_genre_tracks(
		&self,
		genre_id: &str,
		limit: u32,
		offset: u32,
	) -> Result<crate::tidal::models::SearchResultItems<Track>, TidalError> {
		self.session
			.request::<crate::tidal::models::SearchResultItems<Track>>(
				reqwest::Method::GET,
				&format!("/genres/{}/tracks", genre_id),
				Some(&[
					("limit", &limit.to_string()),
					("offset", &offset.to_string()),
				]),
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await
	}

	pub async fn get_categories(
		&self,
	) -> Result<Vec<crate::tidal::models::entities::Category>, TidalError> {
		#[derive(serde::Deserialize)]
		#[serde(untagged)]
		enum Response {
			Array(Vec<crate::tidal::models::entities::Category>),
			Object {
				items: Vec<crate::tidal::models::entities::Category>,
			},
		}

		let response = self
			.session
			.request::<Response>(
				reqwest::Method::GET,
				"/genres",
				None,
				None,
				crate::tidal::session::ApiVersion::V1,
			)
			.await?;

		Ok(match response {
			Response::Array(arr) => arr,
			Response::Object { items } => items,
		})
	}
}
