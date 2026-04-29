use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Starred {
	#[serde(rename = "artist", skip_serializing_if = "Vec::is_empty", default)]
	pub artist: Vec<Artist>,
	#[serde(rename = "album", skip_serializing_if = "Vec::is_empty", default)]
	pub album: Vec<Album>,
	#[serde(rename = "song", skip_serializing_if = "Vec::is_empty", default)]
	pub song: Vec<Song>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicLyrics {
	#[serde(rename = "@artist", skip_serializing_if = "Option::is_none")]
	pub artist: Option<String>,
	#[serde(rename = "@title", skip_serializing_if = "Option::is_none")]
	pub title: Option<String>,
	#[serde(rename = "@value")]
	pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Child {
	#[serde(rename = "@id")]
	pub id: String,
	#[serde(rename = "@parent", skip_serializing_if = "Option::is_none")]
	pub parent: Option<String>,
	#[serde(rename = "@title")]
	pub title: String,
	#[serde(rename = "@album", skip_serializing_if = "Option::is_none")]
	pub album: Option<String>,
	#[serde(rename = "@artist", skip_serializing_if = "Option::is_none")]
	pub artist: Option<String>,
	#[serde(rename = "@isDir")]
	pub is_dir: bool,
	#[serde(rename = "@isVideo", skip_serializing_if = "Option::is_none")]
	pub is_video: Option<bool>,
	#[serde(rename = "@type", skip_serializing_if = "Option::is_none")]
	pub type_: Option<String>,
	#[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
	pub cover_art: Option<String>,
	#[serde(rename = "@duration", skip_serializing_if = "Option::is_none")]
	pub duration: Option<i64>,
	#[serde(rename = "@bitRate", skip_serializing_if = "Option::is_none")]
	pub bit_rate: Option<i32>,
	#[serde(rename = "@track", skip_serializing_if = "Option::is_none")]
	pub track: Option<i32>,
	#[serde(rename = "@albumId", skip_serializing_if = "Option::is_none")]
	pub album_id: Option<String>,
	#[serde(rename = "@artistId", skip_serializing_if = "Option::is_none")]
	pub artist_id: Option<String>,
	#[serde(rename = "@size", skip_serializing_if = "Option::is_none")]
	pub size: Option<i64>,
	#[serde(rename = "@suffix", skip_serializing_if = "Option::is_none")]
	pub suffix: Option<String>,
	#[serde(rename = "@contentType", skip_serializing_if = "Option::is_none")]
	pub content_type: Option<String>,
	#[serde(rename = "@year", skip_serializing_if = "Option::is_none")]
	pub year: Option<i32>,
	#[serde(rename = "@genre", skip_serializing_if = "Option::is_none")]
	pub genre: Option<String>,
	#[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
	pub starred: Option<String>,
	#[serde(rename = "@path", skip_serializing_if = "Option::is_none")]
	pub path: Option<String>,
	#[serde(rename = "@playCount", skip_serializing_if = "Option::is_none")]
	pub play_count: Option<i32>,
	#[serde(rename = "@discNumber", skip_serializing_if = "Option::is_none")]
	pub disc_number: Option<i32>,
	#[serde(rename = "@created", skip_serializing_if = "Option::is_none")]
	pub created: Option<String>,
	#[serde(rename = "@explicitStatus", skip_serializing_if = "Option::is_none")]
	pub explicit_status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Directory {
	#[serde(rename = "@id")]
	pub id: String,
	#[serde(rename = "@parent", skip_serializing_if = "Option::is_none")]
	pub parent: Option<String>,
	#[serde(rename = "@name")]
	pub name: String,
	#[serde(rename = "child", skip_serializing_if = "Option::is_none")]
	pub child: Option<Vec<Child>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(rename = "subsonic-response")]
pub struct SubsonicResponse {
	#[serde(rename = "@serverVersion")]
	pub server_version: String,
	#[serde(rename = "@status")]
	pub status: String,
	#[serde(rename = "@type")]
	pub type_: String,
	#[serde(rename = "@version")]
	pub version: String,
	#[serde(rename = "@openSubsonic", skip_serializing_if = "Option::is_none")]
	pub open_subsonic: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<SubsonicError>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub internet_radio_stations: Option<InternetRadioStations>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub album_list: Option<AlbumList>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub album_list2: Option<AlbumList>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub album: Option<Album>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub song: Option<Song>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub artist: Option<Artist>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub search_result2: Option<SearchResult2>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub search_result3: Option<SearchResult3>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub top_songs: Option<TopSongs>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub playlists: Option<Playlists>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub playlist: Option<Playlist>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub similar_songs: Option<SimilarSongs>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub music_folders: Option<MusicFolders>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub indexes: Option<Indexes>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub random_songs: Option<RandomSongs>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub songs_by_genre: Option<SongsByGenre>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub starred: Option<Starred>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub starred2: Option<Starred>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub user: Option<User>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub genres: Option<Genres>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub play_queue: Option<PlayQueue>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub artists: Option<Indexes>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub artist_info: Option<ArtistInfoBase>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub artist_info2: Option<ArtistInfoBase>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub album_info: Option<AlbumInfo>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub album_info2: Option<AlbumInfo>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub lyrics: Option<SubsonicLyrics>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub directory: Option<Directory>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInfo {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub notes: Option<String>,
	#[serde(rename = "musicBrainzId", skip_serializing_if = "Option::is_none")]
	pub music_brainz_id: Option<String>,
	#[serde(rename = "smallImageUrl", skip_serializing_if = "Option::is_none")]
	pub small_image_url: Option<String>,
	#[serde(rename = "mediumImageUrl", skip_serializing_if = "Option::is_none")]
	pub medium_image_url: Option<String>,
	#[serde(rename = "largeImageUrl", skip_serializing_if = "Option::is_none")]
	pub large_image_url: Option<String>,
	#[serde(rename = "lastFmUrl", skip_serializing_if = "Option::is_none")]
	pub last_fm_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInfoBase {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub biography: Option<String>,
	#[serde(rename = "similarArtist", skip_serializing_if = "Option::is_none")]
	pub similar_artist: Option<Vec<Artist>>,
	#[serde(rename = "smallImageUrl", skip_serializing_if = "Option::is_none")]
	pub small_image_url: Option<String>,
	#[serde(rename = "mediumImageUrl", skip_serializing_if = "Option::is_none")]
	pub medium_image_url: Option<String>,
	#[serde(rename = "largeImageUrl", skip_serializing_if = "Option::is_none")]
	pub large_image_url: Option<String>,
	#[serde(rename = "lastFmUrl", skip_serializing_if = "Option::is_none")]
	pub last_fm_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayQueue {
	#[serde(rename = "@current", skip_serializing_if = "Option::is_none")]
	pub current: Option<String>,
	#[serde(rename = "@position", skip_serializing_if = "Option::is_none")]
	pub position: Option<i64>,
	#[serde(rename = "@username")]
	pub username: String,
	#[serde(rename = "@changed")]
	pub changed: String,
	#[serde(rename = "@changedSchema")]
	pub changed_schema: i64,
	#[serde(rename = "entry", skip_serializing_if = "Option::is_none")]
	pub entry: Option<Vec<Song>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Genres {
	#[serde(rename = "genre", skip_serializing_if = "Option::is_none")]
	pub genre: Option<Vec<Genre>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
	#[serde(rename = "@value")]
	pub value: String,
	#[serde(rename = "@songCount")]
	pub song_count: i32,
	#[serde(rename = "@albumCount")]
	pub album_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
	#[serde(rename = "@username")]
	pub username: String,
	#[serde(rename = "@adminRole")]
	pub admin_role: bool,
	#[serde(rename = "@settingsRole")]
	pub settings_role: bool,
	#[serde(rename = "@downloadRole")]
	pub download_role: bool,
	#[serde(rename = "@uploadRole")]
	pub upload_role: bool,
	#[serde(rename = "@playlistRole")]
	pub playlist_role: bool,
	#[serde(rename = "@coverArtRole")]
	pub cover_art_role: bool,
	#[serde(rename = "@commentRole")]
	pub comment_role: bool,
	#[serde(rename = "@podcastRole")]
	pub podcast_role: bool,
	#[serde(rename = "@streamRole")]
	pub stream_role: bool,
	#[serde(rename = "@jukeboxRole")]
	pub jukebox_role: bool,
	#[serde(rename = "@shareRole")]
	pub share_role: bool,
	#[serde(rename = "@scrobblingEnabled")]
	pub scrobbling_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicError {
	#[serde(rename = "@code")]
	pub code: i32,
	#[serde(rename = "@message")]
	pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumList {
	pub album: Vec<Album>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Album {
	#[serde(rename = "@id")]
	pub id: String,
	#[serde(rename = "@isDir")]
	pub is_dir: bool,
	#[serde(rename = "@name")]
	pub name: String,
	#[serde(rename = "@title", skip_serializing_if = "Option::is_none")]
	pub title: Option<String>,
	#[serde(rename = "@artist")]
	pub artist: String,
	#[serde(rename = "@artistId")]
	pub artist_id: String,
	#[serde(rename = "@coverArt")]
	pub cover_art: String,
	#[serde(rename = "@songCount")]
	pub song_count: i32,
	#[serde(rename = "@duration")]
	pub duration: i64,
	#[serde(rename = "@created")]
	pub created: String,
	#[serde(rename = "@year", skip_serializing_if = "Option::is_none")]
	pub year: Option<i32>,
	#[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
	pub starred: Option<String>,
	#[serde(rename = "@explicitStatus", skip_serializing_if = "Option::is_none")]
	pub explicit_status: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub song: Option<Vec<Song>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Song {
	#[serde(rename = "@id")]
	pub id: String,
	#[serde(rename = "@parent")]
	pub parent: String,
	#[serde(rename = "@title")]
	pub title: String,
	#[serde(rename = "@album")]
	pub album: String,
	#[serde(rename = "@artist")]
	pub artist: String,
	#[serde(rename = "@isDir")]
	pub is_dir: bool,
	#[serde(rename = "@isVideo")]
	pub is_video: bool,
	#[serde(rename = "@type")]
	pub type_: String,
	#[serde(rename = "@coverArt")]
	pub cover_art: String,
	#[serde(rename = "@duration")]
	pub duration: i64,
	#[serde(rename = "@bitRate")]
	pub bit_rate: i32,
	#[serde(rename = "@track")]
	pub track: i32,
	#[serde(rename = "@albumId")]
	pub album_id: String,
	#[serde(rename = "@artistId")]
	pub artist_id: String,
	#[serde(rename = "@size")]
	pub size: i64,
	#[serde(rename = "@suffix")]
	pub suffix: String,
	#[serde(rename = "@contentType")]
	pub content_type: String,
	#[serde(rename = "@year", skip_serializing_if = "Option::is_none")]
	pub year: Option<i32>,
	#[serde(rename = "@genre", skip_serializing_if = "Option::is_none")]
	pub genre: Option<String>,
	#[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
	pub starred: Option<String>,
	#[serde(rename = "@path", skip_serializing_if = "Option::is_none")]
	pub path: Option<String>,
	#[serde(rename = "@playCount", skip_serializing_if = "Option::is_none")]
	pub play_count: Option<i32>,
	#[serde(rename = "@discNumber", skip_serializing_if = "Option::is_none")]
	pub disc_number: Option<i32>,
	#[serde(rename = "@created", skip_serializing_if = "Option::is_none")]
	pub created: Option<String>,
	#[serde(rename = "@explicitStatus", skip_serializing_if = "Option::is_none")]
	pub explicit_status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
	#[serde(rename = "@id")]
	pub id: String,
	#[serde(rename = "@name")]
	pub name: String,
	#[serde(rename = "@coverArt")]
	pub cover_art: String,
	#[serde(rename = "@albumCount")]
	pub album_count: i32,
	#[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
	pub starred: Option<String>,
	#[serde(rename = "@artistImageUrl", skip_serializing_if = "Option::is_none")]
	pub artist_image_url: Option<String>,
	#[serde(rename = "@userRating", skip_serializing_if = "Option::is_none")]
	pub user_rating: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub album: Option<Vec<Album>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicResponseWrapper {
	#[serde(rename = "subsonic-response")]
	pub response: SubsonicResponse,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult2 {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub artist: Option<Vec<Artist>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub album: Option<Vec<Album>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub song: Option<Vec<Song>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult3 {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub artist: Option<Vec<Artist>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub album: Option<Vec<Album>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub song: Option<Vec<Song>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopSongs {
	pub song: Vec<Song>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlists {
	pub playlist: Vec<Playlist>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
	#[serde(rename = "@id")]
	pub id: String,
	#[serde(rename = "@name")]
	pub name: String,
	#[serde(rename = "@owner", skip_serializing_if = "Option::is_none")]
	pub owner: Option<String>,
	#[serde(rename = "@public", skip_serializing_if = "Option::is_none")]
	pub public: Option<bool>,
	#[serde(rename = "@songCount")]
	pub song_count: i32,
	#[serde(rename = "@duration")]
	pub duration: i64,
	#[serde(rename = "@created")]
	pub created: String,
	#[serde(rename = "@changed", skip_serializing_if = "Option::is_none")]
	pub changed: Option<String>,
	#[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
	pub cover_art: Option<String>,
	#[serde(rename = "@comment", skip_serializing_if = "Option::is_none")]
	pub comment: Option<String>,
	#[serde(rename = "entry", skip_serializing_if = "Option::is_none")]
	pub entry: Option<Vec<Song>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimilarSongs {
	pub song: Vec<Song>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFolders {
	#[serde(rename = "musicFolder")]
	pub music_folder: Vec<MusicFolder>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFolder {
	#[serde(rename = "@id")]
	pub id: i32,
	#[serde(rename = "@name")]
	pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Indexes {
	#[serde(rename = "@lastModified")]
	pub last_modified: u64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub ignored_articles: Option<String>,
	pub index: Vec<Index>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
	#[serde(rename = "@name")]
	pub name: String,
	pub artist: Vec<Artist>,
}

impl SubsonicResponseWrapper {
	pub fn ok() -> Self {
		Self {
			response: SubsonicResponse {
				server_version: "0.1.0 (tidal)".to_string(),
				status: "ok".to_string(),
				type_: "tidal-subsonic".to_string(),
				version: "1.16.1".to_string(),
				open_subsonic: Some(true),
				error: None,
				internet_radio_stations: None,
				album_list: None,
				album_list2: None,
				album: None,
				song: None,
				artist: None,
				search_result2: None,
				search_result3: None,
				top_songs: None,
				playlists: None,
				playlist: None,
				similar_songs: None,
				music_folders: None,
				indexes: None,
				random_songs: None,
				songs_by_genre: None,
				starred: None,
				starred2: None,
				user: None,
				genres: None,
				play_queue: None,
				artists: None,
				artist_info: None,
				artist_info2: None,
				album_info: None,
				album_info2: None,
				lyrics: None,
				directory: None,
			},
		}
	}

	pub fn error(code: i32, message: &str) -> Self {
		Self {
			response: SubsonicResponse {
				server_version: "0.1.0 (tidal)".to_string(),
				status: "failed".to_string(),
				type_: "tidal-subsonic".to_string(),
				version: "1.16.1".to_string(),
				open_subsonic: None,
				error: Some(SubsonicError {
					code,
					message: message.to_string(),
				}),
				internet_radio_stations: None,
				album_list: None,
				album_list2: None,
				album: None,
				song: None,
				artist: None,
				search_result2: None,
				search_result3: None,
				top_songs: None,
				playlists: None,
				playlist: None,
				similar_songs: None,
				music_folders: None,
				indexes: None,
				random_songs: None,
				songs_by_genre: None,
				starred: None,
				starred2: None,
				user: None,
				genres: None,
				play_queue: None,
				artists: None,
				artist_info: None,
				artist_info2: None,
				album_info: None,
				album_info2: None,
				lyrics: None,
				directory: None,
			},
		}
	}

	pub fn with_lyrics(mut self, lyrics: SubsonicLyrics) -> Self {
		self.response.lyrics = Some(lyrics);
		self
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RandomSongs {
	pub song: Vec<Song>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SongsByGenre {
	pub song: Vec<Song>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternetRadioStations {
	#[serde(
		rename = "internetRadioStation",
		skip_serializing_if = "Option::is_none"
	)]
	pub internet_radio_station: Option<Vec<InternetRadioStation>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternetRadioStation {
	#[serde(rename = "@id")]
	pub id: String,
	#[serde(rename = "@name")]
	pub name: String,
	#[serde(rename = "@streamUrl")]
	pub stream_url: String,
	#[serde(rename = "@homePageUrl", skip_serializing_if = "Option::is_none")]
	pub home_page_url: Option<String>,
	#[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
	pub cover_art: Option<String>,
}
