use crate::api::subsonic::models::{Album, Artist, Playlist, Song};
use crate::tidal::favorites::get_favorite_date;
use crate::tidal::models::entities::{
	Album as TidalAlbum, Artist as TidalArtist, Playlist as TidalPlaylist, Track as TidalTrack,
};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use std::collections::HashMap;

fn format_date(date_str: Option<&str>) -> String {
	tracing::debug!("Formatting date: {:?}", date_str);
	let date_str = match date_str {
		Some(d) if !d.trim().is_empty() => d.trim(),
		_ => return String::new(),
	};

	if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
		return dt.with_timezone(&Utc).to_rfc3339();
	}

	if let Ok(dt) = DateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S%.3f%z") {
		return dt.with_timezone(&Utc).to_rfc3339();
	}

	if let Ok(nd) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
		return nd.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
	}

	if let Ok(nd) = NaiveDate::parse_from_str(&format!("{}-01", date_str), "%Y-%m-%d") {
		return nd.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
	}

	if let Ok(nd) = NaiveDate::parse_from_str(&format!("{}-01-01", date_str), "%Y-%m-%d") {
		return nd.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
	}

	String::new()
}

fn extract_year(date_str: Option<&str>) -> Option<i32> {
	let date_str = date_str?.trim();

	if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
		return Some(dt.year());
	}
	if let Ok(nd) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
		return Some(nd.year());
	}
	if date_str.len() >= 4 {
		return date_str[0..4].parse::<i32>().ok();
	}

	None
}

pub fn map_tidal_artist_to_subsonic(artist: &TidalArtist, user_id: Option<i64>) -> Artist {
	artist.cache();
	let cover_art = artist
		.picture
		.clone()
		.or_else(|| artist.selected_album_cover_fallback.clone())
		.unwrap_or_default();

	let artist_image_url = if !cover_art.is_empty() {
		Some(format!(
			"https://resources.tidal.com/images/{}/750x750.jpg",
			cover_art.replace("-", "/")
		))
	} else {
		None
	};

	Artist {
		id: artist.id.to_string(),
		name: artist.name.clone(),
		cover_art: cover_art.clone(),
		album_count: 1, // TODO: this isnt provided anywhere, we can cache it for the artists we know tho?
		album: None,
		starred: user_id
			.and_then(|uid| get_favorite_date(uid, artist.id))
			.map(|d| format_date(Some(&d))),
		artist_image_url,
		user_rating: Some(0),
	}
}

pub fn map_tidal_album_to_subsonic(
	album: &TidalAlbum,
	user_id: Option<i64>,
	artists: Option<&[TidalArtist]>,
) -> Album {
	album.cache();
	let primary_artist_name = artists
		.and_then(|a| a.first())
		.map(|a| a.name.clone())
		.or_else(|| album.artist.as_ref().map(|a| a.name.clone()))
		.or_else(|| {
			album
				.artists
				.as_ref()
				.and_then(|a| a.first())
				.map(|a| a.name.clone())
		})
		.unwrap_or_else(|| "Unknown Artist".to_string());

	let primary_artist_id = artists
		.and_then(|a| a.first())
		.map(|a| a.id.to_string())
		.or_else(|| album.artist.as_ref().map(|a| a.id.to_string()))
		.or_else(|| {
			album
				.artists
				.as_ref()
				.and_then(|a| a.first())
				.map(|a| a.id.to_string())
		});

	let cover_art_id = album.cover.clone().unwrap_or_else(|| album.id.to_string());

	Album {
		id: album.id.to_string(),
		is_dir: true,
		name: album.title.clone(),
		title: Some(album.title.clone()),
		artist: primary_artist_name,
		artist_id: primary_artist_id.unwrap_or_default(),
		cover_art: cover_art_id,
		song_count: album.number_of_tracks.unwrap_or(0),
		duration: album.duration.unwrap_or(0),
		created: format_date(album.release_date.as_deref()),
		year: extract_year(album.release_date.as_deref()),
		starred: user_id
			.and_then(|uid| get_favorite_date(uid, album.id))
			.map(|d| format_date(Some(&d))),
		explicit_status: Some(if album.explicit.unwrap_or(false) {
			"explicit".to_string()
		} else {
			"clean".to_string()
		}),
		song: None,
	}
}

pub fn map_tidal_track_to_subsonic(
	track: &TidalTrack,
	user_id: Option<i64>,
	album_attrs: Option<&TidalAlbum>,
	artists: Option<&[TidalArtist]>,
) -> Song {
	track.cache();
	let resolved_album = album_attrs.unwrap_or(&track.album);

	let primary_artist = artists
		.and_then(|a| a.first())
		.or_else(|| track.artists.first())
		.or(Some(&track.artist));

	let primary_artist_name = primary_artist
		.map(|a| a.name.clone())
		.unwrap_or_else(|| "Unknown Artist".to_string());
	let primary_artist_id = primary_artist.map(|a| a.id.to_string());

	let album_name = resolved_album.title.clone();
	let cover_art_id = resolved_album
		.cover
		.clone()
		.unwrap_or_else(|| resolved_album.id.to_string());

	let date_value = track
		.release_date
		.clone()
		.or_else(|| track.stream_start_date.clone());

	let year = extract_year(date_value.as_deref())
		.or_else(|| extract_year(resolved_album.release_date.as_deref()));

	let created_str = date_value.or_else(|| resolved_album.release_date.clone());
	let created = format_date(created_str.as_deref());

	let bit_rate = match track.audio_quality.as_deref() {
		Some("LOW") => 96,
		Some("HIGH") => 320,
		Some("LOSSLESS") => 1411,
		Some("HI_RES") | Some("HI_RES_LOSSLESS") => 9216,
		_ => 320,
	};

	let tag = track
		.media_metadata
		.as_ref()
		.and_then(|m| m.tags.as_ref())
		.map(|tags| {
			tags.iter()
				.map(|t| {
					let tag = t.replace('_', " ");
					let words = tag.split_whitespace().peekable();
					if words.clone().nth(1).is_some() {
						words.filter_map(|w| w.chars().next()).collect()
					} else {
						t.chars().take(2).collect()
					}
				})
				.collect::<Vec<String>>()
				.join("|")
		})
		.unwrap_or_default();
	let is_mpeg = track
		.media_metadata
		.as_ref()
		.and_then(|m| m.tags.as_ref())
		.map(|tags| tags.iter().any(|t| t == "DOLBY_ATMOS"))
		.unwrap_or(false);
	Song {
		id: track.id.to_string(),
		parent: resolved_album.id.to_string(),
		is_dir: false,
		title: track.title.clone(),
		album: album_name,
		artist: primary_artist_name,
		track: track.track_number,
		year,
		genre: Some(tag),
		cover_art: cover_art_id,
		size: 0,
		content_type: if is_mpeg { "audio/mp4" } else { "audio/flac" }.to_string(),
		suffix: if is_mpeg { "m4a" } else { "flac" }.to_string(),
		starred: user_id
			.and_then(|uid| get_favorite_date(uid, track.id))
			.map(|d| format_date(Some(&d))),
		duration: track.duration,
		bit_rate,
		path: Some(format!("tidal/track/{}", track.id)),
		play_count: Some(0),
		disc_number: Some(track.disc_number.unwrap_or(1)),
		created: Some(created),
		album_id: resolved_album.id.to_string(),
		artist_id: primary_artist_id.unwrap_or_else(|| "0".to_string()),
		type_: "music".to_string(),
		explicit_status: Some(if track.explicit {
			"explicit".to_string()
		} else {
			"clean".to_string()
		}),
		is_video: false,
	}
}

pub fn map_tidal_playlist_to_subsonic(
	playlist: &TidalPlaylist,
	owner_name: Option<&str>,
) -> Playlist {
	Playlist {
		id: playlist.uuid.clone(),
		name: playlist.title.clone(),
		owner: playlist
			.creator
			.as_ref()
			.and_then(|c| c.name.clone())
			.or_else(|| owner_name.map(|s| s.to_string()))
			.or_else(|| Some("Unknown".to_string())),
		public: Some(false),
		song_count: playlist.number_of_tracks,
		duration: playlist.duration,
		created: format_date(playlist.created.as_deref()),
		changed: Some(format_date(playlist.last_updated.as_deref())),
		cover_art: Some(
			playlist
				.custom_image_url
				.clone()
				.or_else(|| playlist.square_image.clone())
				.or_else(|| playlist.image.clone())
				.unwrap_or_default(),
		),
		comment: playlist.description.clone(),
		entry: None,
	}
}

pub fn dedupe_albums(albums: Vec<TidalAlbum>) -> Vec<TidalAlbum> {
	let mut map: HashMap<String, TidalAlbum> = HashMap::new();

	for mut album in albums {
		let is_atmos = album
			.media_metadata
			.as_ref()
			.and_then(|m| m.tags.as_ref())
			.map(|tags| tags.iter().any(|t| t == "DOLBY_ATMOS"))
			.unwrap_or(false);

		if is_atmos && !album.title.starts_with("[DA]") {
			album.title = format!("[DA] {}", album.title);
		}

		let artist_key = album
			.artist
			.as_ref()
			.map(|a| a.id.to_string())
			.unwrap_or_else(|| "unknown".to_string());
		let key = format!("{}-{}", album.title.to_lowercase().trim(), artist_key);

		let get_rank = |a: &TidalAlbum| match a.audio_quality.as_deref() {
			Some("HI_RES_LOSSLESS") => 4,
			Some("LOSSLESS") => 3,
			Some("HIGH") => 2,
			Some("LOW") => 1,
			_ => 0,
		};

		if let Some(existing) = map.get(&key) {
			let new_rank = get_rank(&album);
			let old_rank = get_rank(existing);

			let should_replace = if new_rank != old_rank {
				new_rank > old_rank
			} else {
				album.explicit == Some(true) && existing.explicit != Some(true)
			};

			if should_replace {
				map.insert(key, album);
			}
		} else {
			map.insert(key, album);
		}
	}

	map.into_values().collect()
}
