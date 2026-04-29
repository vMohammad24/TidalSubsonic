use crate::tidal::api::{ALBUM_CACHE, ARTIST_CACHE, TRACK_CACHE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Album {
	pub id: i64,
	pub title: String,
	pub duration: Option<i64>,
	pub stream_ready: Option<bool>,
	pub stream_start_date: Option<String>,
	pub allow_streaming: Option<bool>,
	pub premium_streaming_only: Option<bool>,
	pub number_of_tracks: Option<i32>,
	pub number_of_videos: Option<i32>,
	pub number_of_volumes: Option<i32>,
	pub release_date: Option<String>,
	pub explicit: Option<bool>,
	pub cover: Option<String>,
	pub video_cover: Option<String>,
	pub artist: Option<Artist>,
	pub artists: Option<Vec<Artist>>,
	pub audio_quality: Option<String>,
	pub audio_modes: Option<Vec<String>>,
	#[serde(rename = "mediaMetadata")]
	pub media_metadata: Option<MediaMetadata>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Artist {
	pub id: i64,
	pub name: String,
	pub url: Option<String>,
	pub picture: Option<String>,
	pub popularity: Option<i32>,
	#[serde(rename = "artistTypes")]
	pub artist_types: Option<Vec<String>>,
	#[serde(rename = "selectedAlbumCoverFallback")]
	pub selected_album_cover_fallback: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistBio {
	pub source: Option<String>,
	#[serde(rename = "lastUpdated")]
	pub last_updated: Option<String>,
	pub text: String,
	pub summary: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Track {
	pub id: i64,
	pub title: String,
	pub duration: i64,
	#[serde(rename = "trackNumber")]
	pub track_number: i32,
	#[serde(rename = "volumeNumber")]
	pub volume_number: i32,
	pub explicit: bool,
	#[serde(rename = "audioQuality")]
	pub audio_quality: Option<String>,
	pub artist: Artist,
	pub artists: Vec<Artist>,
	pub album: Album,
	pub url: String,
	#[serde(rename = "audioModes")]
	pub audio_modes: Option<Vec<String>>,
	#[serde(rename = "releaseDate")]
	pub release_date: Option<String>,
	#[serde(rename = "streamStartDate")]
	pub stream_start_date: Option<String>,
	#[serde(rename = "discNumber")]
	pub disc_number: Option<i32>,
	#[serde(rename = "mediaMetadata")]
	pub media_metadata: Option<MediaMetadata>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct MediaMetadata {
	pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Playlist {
	pub uuid: String,
	pub title: String,
	#[serde(rename = "numberOfTracks")]
	pub number_of_tracks: i32,
	pub duration: i64,
	pub description: Option<String>,
	pub creator: Option<Creator>,
	#[serde(rename = "image")]
	pub image: Option<String>,
	pub url: Option<String>,
	pub created: Option<String>,
	#[serde(rename = "lastUpdated")]
	pub last_updated: Option<String>,
	#[serde(rename = "squareImage")]
	pub square_image: Option<String>,
	#[serde(rename = "customImageUrl")]
	pub custom_image_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JsonApiResource<A> {
	pub id: String,
	pub r#type: String,
	pub attributes: A,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JsonApiResponse<A> {
	pub data: JsonApiResource<A>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistAttributesV2 {
	pub name: String,
	pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Creator {
	pub id: i64,
	pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Video {
	pub id: i64,
	pub title: String,
	pub duration: i64,
	pub explicit: bool,
	pub artist: Artist,
	pub artists: Vec<Artist>,
	pub album: Option<Album>,
	pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchResult {
	pub artists: Option<SearchResultItems<Artist>>,
	pub albums: Option<SearchResultItems<Album>>,
	pub tracks: Option<SearchResultItems<Track>>,
	pub playlists: Option<SearchResultItems<Playlist>>,
	pub videos: Option<SearchResultItems<Video>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchResultItems<T> {
	pub items: Vec<T>,
	pub limit: Option<i32>,
	pub offset: Option<i32>,
	#[serde(rename = "totalNumberOfItems")]
	pub total_number_of_items: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct PlaybackInfo {
	#[serde(rename = "trackId")]
	pub track_id: Option<i64>,
	#[serde(rename = "assetPresentation")]
	pub asset_presentation: Option<String>,
	#[serde(rename = "audioMode")]
	pub audio_mode: Option<String>,
	#[serde(rename = "audioQuality")]
	pub audio_quality: Option<String>,
	#[serde(rename = "manifestMimeType")]
	pub manifest_mime_type: Option<String>,
	pub manifest: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Lyrics {
	#[serde(rename = "trackId")]
	pub track_id: i64,
	#[serde(rename = "lyricsProvider")]
	pub lyrics_provider: String,
	#[serde(rename = "providerCommontrackId")]
	pub provider_commontrack_id: String,
	#[serde(rename = "providerLyricsId")]
	pub provider_lyrics_id: String,
	pub lyrics: String,
	pub subtitles: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct RecommendationItem {
	pub track: Track,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Category {
	pub id: String,
	pub name: String,
	pub path: String,
	pub image: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct UserProfile {
	#[serde(rename = "userId")]
	pub user_id: i64,
	pub username: Option<String>,
	#[serde(rename = "firstName")]
	pub first_name: Option<String>,
	#[serde(rename = "lastName")]
	pub last_name: Option<String>,
	pub email: Option<String>,
	#[serde(rename = "countryCode")]
	pub country_code: Option<String>,
	pub created: Option<String>,
	pub picture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct FavoriteItem<T> {
	pub created: String,
	pub item: T,
}

impl Album {
	pub fn cache(&self) {
		let album = self.clone();
		tokio::spawn(async move {
			ALBUM_CACHE.insert(album.id, album).await;
		});
	}
}
impl Track {
	pub fn cache(&self) {
		let track = self.clone();
		tokio::spawn(async move {
			TRACK_CACHE.insert(track.id, track).await;
		});
	}
}
impl Artist {
	pub fn cache(&self) {
		let artist = self.clone();
		tokio::spawn(async move {
			ARTIST_CACHE.insert(artist.id, artist).await;
		});
	}
}
