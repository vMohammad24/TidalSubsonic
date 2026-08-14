use super::api::TidalApi;
use super::models::Track;
use super::session::{Session, SessionOptions};
use crate::api::subsonic::mapping::{
	map_tidal_album_to_subsonic, map_tidal_artist_to_subsonic, map_tidal_track_to_subsonic,
};
use std::sync::Arc;

fn setup_test_api() -> Option<TidalApi> {
	dotenvy::dotenv().ok();
	let token = std::env::var("TIDAL_API_TOKEN").ok()?;
	let session = Arc::new(Session::new(
		SessionOptions {
			country_code: Some("US".to_string()),
			access_token: None,
			session_id: Some(token),
			..Default::default()
		},
		None,
	));
	Some(TidalApi::new(session))
}

#[tokio::test]
async fn test_live_tidal_fetch_and_subsonic_mapping() {
	let Some(api) = setup_test_api() else {
		tracing::warn!("Skipping live Tidal test: TIDAL_API_TOKEN not set");
		return;
	};

	let search_res = api.search("Get Lucky", 5, 0).await;
	let search = match search_res {
		Ok(res) => res,
		Err(e) => {
			tracing::warn!(
				error = ?e,
				"Skipping live Tidal fetch test: Upstream API returned error"
			);
			return;
		}
	};

	let tracks = search.tracks.expect("Tracks in search result");
	assert!(!tracks.items.is_empty(), "Should find at least one track");

	let live_track = &tracks.items[0];
	assert!(live_track.id > 0);
	assert!(!live_track.title.is_empty());

	let subsonic_song = map_tidal_track_to_subsonic(live_track, None, None, None);
	assert_eq!(subsonic_song.id, live_track.id.to_string());
	assert_eq!(subsonic_song.title, live_track.title);
	assert!(!subsonic_song.artist.is_empty());
	assert!(!subsonic_song.album.is_empty());
	assert!(subsonic_song.bit_rate > 0);
	assert!(subsonic_song.path.is_some());

	if let Some(artists) = search.artists
		&& let Some(live_artist) = artists.items.first()
	{
		let subsonic_artist = map_tidal_artist_to_subsonic(live_artist, None);
		assert_eq!(subsonic_artist.id, live_artist.id.to_string());
		assert_eq!(subsonic_artist.name, live_artist.name);
	}

	if let Some(albums) = search.albums
		&& let Some(live_album) = albums.items.first()
	{
		let subsonic_album = map_tidal_album_to_subsonic(live_album, None, None);
		assert_eq!(subsonic_album.id, live_album.id.to_string());
		assert_eq!(subsonic_album.name, live_album.title);
	}
}

#[tokio::test]
async fn test_tidal_payload_deserialization_and_mapping() {
	let raw_tidal_track_json = r#"{
		"id": 140516546,
		"title": "Blinding Lights",
		"duration": 200,
		"trackNumber": 9,
		"volumeNumber": 1,
		"explicit": false,
		"audioQuality": "HI_RES_LOSSLESS",
		"audioModes": ["STEREO", "DOLBY_ATMOS"],
		"releaseDate": "2020-03-20",
		"artist": {
			"id": 5092770,
			"name": "The Weeknd",
			"picture": "4f9d0c6c-9c09-4171-8931-155519db8e4d",
			"popularity": 98
		},
		"artists": [{
			"id": 5092770,
			"name": "The Weeknd",
			"picture": "4f9d0c6c-9c09-4171-8931-155519db8e4d"
		}],
		"album": {
			"id": 140516537,
			"title": "After Hours",
			"cover": "8d3e2329-873b-4c07-b353-0b0c5cbbf3b8",
			"releaseDate": "2020-03-20",
			"explicit": false
		},
		"mediaMetadata": {
			"tags": ["DOLBY_ATMOS"]
		},
		"url": "https://tidal.com/track/140516546"
	}"#;

	let track: Track =
		serde_json::from_str(raw_tidal_track_json).expect("Deserialization of Tidal track");

	assert_eq!(track.id, 140516546);
	assert_eq!(track.title, "Blinding Lights");
	assert_eq!(track.artist.name, "The Weeknd");
	assert_eq!(track.album.title, "After Hours");

	let song = map_tidal_track_to_subsonic(&track, None, None, None);
	assert_eq!(song.id, "140516546");
	assert_eq!(song.title, "Blinding Lights");
	assert_eq!(song.artist, "The Weeknd");
	assert_eq!(song.album, "After Hours");
	assert_eq!(song.bit_rate, 9216);
	assert_eq!(song.content_type, "audio/mp4");
	assert_eq!(song.suffix, "m4a");
}
