use crate::api::subsonic::middleware::SubsonicContext;
use crate::api::subsonic::models::{Album, Artist, Playlist, Song};
use crate::db::LocalPlaylistWithCount;
use crate::tidal::api::ARTIST_ALBUM_COUNT_CACHE;
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

	if date_str.contains('T') {
		if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
			return dt.with_timezone(&Utc).to_rfc3339();
		}
		if let Ok(dt) = DateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S%.3f%z") {
			return dt.with_timezone(&Utc).to_rfc3339();
		}
	} else {
		match date_str.len() {
			10 => {
				if let Ok(nd) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
					return nd.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
				}
			}
			7 => {
				if let (Ok(year), Ok(month)) =
					(date_str[0..4].parse::<i32>(), date_str[5..7].parse::<u32>())
					&& let Some(nd) = NaiveDate::from_ymd_opt(year, month, 1)
				{
					return nd.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
				}
			}
			4 => {
				if let Ok(year) = date_str[0..4].parse::<i32>()
					&& let Some(nd) = NaiveDate::from_ymd_opt(year, 1, 1)
				{
					return nd.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
				}
			}
			_ => {}
		}
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

pub fn map_tidal_artist_to_subsonic(
	artist: &TidalArtist,
	subsonic_ctx: Option<&SubsonicContext>,
) -> Artist {
	artist.cache();
	let cover_art = artist
		.picture
		.as_deref()
		.or(artist.selected_album_cover_fallback.as_deref())
		.unwrap_or("");

	let artist_image_url = (!cover_art.is_empty()).then(|| {
		format!(
			"https://resources.tidal.com/images/{}/750x750.jpg",
			cover_art.replace('-', "/")
		)
	});

	let starred = subsonic_ctx.and_then(|ctx| ctx.get_starred_date(artist.id));

	Artist {
		id: artist.id.to_string(),
		name: artist.name.clone(),
		cover_art: cover_art.to_string(),
		album_count: ARTIST_ALBUM_COUNT_CACHE.get(&artist.id).unwrap_or(1),
		album: None,
		starred: starred.map(|d| format_date(Some(&d))),
		artist_image_url,
		user_rating: Some(0),
	}
}

pub fn map_tidal_album_to_subsonic(
	album: &TidalAlbum,
	subsonic_ctx: Option<&SubsonicContext>,
	artists: Option<&[TidalArtist]>,
) -> Album {
	album.cache();
	let primary_artist_name = artists
		.and_then(|a| a.first())
		.map(|a| a.name.as_str())
		.or_else(|| album.artist.as_ref().map(|a| a.name.as_str()))
		.or_else(|| {
			album
				.artists
				.as_ref()
				.and_then(|a| a.first())
				.map(|a| a.name.as_str())
		})
		.unwrap_or("Unknown Artist")
		.to_string();

	let primary_artist_id = artists
		.and_then(|a| a.first())
		.map(|a| a.id)
		.or_else(|| album.artist.as_ref().map(|a| a.id))
		.or_else(|| album.artists.as_ref().and_then(|a| a.first()).map(|a| a.id))
		.map(|id| id.to_string());

	let cover_art_id = album.cover.clone().unwrap_or_else(|| album.id.to_string());

	let starred = subsonic_ctx.and_then(|ctx| ctx.get_starred_date(album.id));

	let album_title = album.title.clone();

	Album {
		id: album.id.to_string(),
		is_dir: true,
		name: album_title.clone(),
		title: Some(album_title),
		artist: primary_artist_name,
		artist_id: primary_artist_id.unwrap_or_default(),
		cover_art: cover_art_id,
		song_count: album.number_of_tracks.unwrap_or(0),
		duration: album.duration.unwrap_or(0),
		created: format_date(album.release_date.as_deref()),
		year: extract_year(album.release_date.as_deref()),
		starred: starred.map(|d| format_date(Some(&d))),
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
	subsonic_ctx: Option<&SubsonicContext>,
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
		.as_deref()
		.or(track.stream_start_date.as_deref());

	let year =
		extract_year(date_value).or_else(|| extract_year(resolved_album.release_date.as_deref()));

	let created_str = date_value.or(resolved_album.release_date.as_deref());
	let created = format_date(created_str);

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
					let tag_clean = t.replace('_', " ");
					let mut words = tag_clean.split_whitespace();
					match (words.next(), words.next()) {
						(Some(_), Some(_)) => tag_clean
							.split_whitespace()
							.filter_map(|w| w.chars().next())
							.collect(),
						_ => t.chars().take(2).collect(),
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

	let starred = subsonic_ctx.and_then(|ctx| ctx.get_starred_date(track.id));

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
		starred: starred.map(|d| format_date(Some(&d))),
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
			.and_then(|c| c.name.as_deref())
			.or(owner_name)
			.or(Some("Unknown"))
			.map(|s| s.to_string()),
		public: Some(false),
		song_count: playlist.number_of_tracks,
		duration: playlist.duration,
		created: format_date(playlist.created.as_deref()),
		changed: Some(format_date(playlist.last_updated.as_deref())),
		cover_art: Some(
			playlist
				.custom_image_url
				.as_deref()
				.or(playlist.square_image.as_deref())
				.or(playlist.image.as_deref())
				.unwrap_or_default()
				.to_string(),
		),
		comment: playlist.description.clone(),
		entry: None,
	}
}

pub fn map_local_playlist_to_subsonic(playlist: &LocalPlaylistWithCount) -> Playlist {
	Playlist {
		id: playlist.id.to_string(),
		name: playlist.name.clone(),
		owner: Some(playlist.username.clone()),
		public: Some(false),
		song_count: playlist.song_count as i32,
		duration: playlist.duration,
		created: playlist.created_at.to_rfc3339(),
		changed: Some(playlist.updated_at.to_rfc3339()),
		cover_art: None,
		comment: playlist.comment.clone(),
		entry: None,
	}
}

pub fn dedupe_albums(albums: impl IntoIterator<Item = TidalAlbum>) -> Vec<TidalAlbum> {
	let mut map: HashMap<(String, i64), TidalAlbum> = HashMap::new();

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

		let artist_id = album.artist.as_ref().map(|a| a.id).unwrap_or(0);
		let key = (album.title.trim().to_lowercase(), artist_id);

		let get_rank = |a: &TidalAlbum| match a.audio_quality.as_deref() {
			Some("HI_RES_LOSSLESS") => 4,
			Some("LOSSLESS") => 3,
			Some("HIGH") => 2,
			Some("LOW") => 1,
			_ => 0,
		};

		match map.entry(key) {
			std::collections::hash_map::Entry::Occupied(mut entry) => {
				let new_rank = get_rank(&album);
				let old_rank = get_rank(entry.get());

				let should_replace = if new_rank != old_rank {
					new_rank > old_rank
				} else {
					album.explicit == Some(true) && entry.get().explicit != Some(true)
				};

				if should_replace {
					entry.insert(album);
				}
			}
			std::collections::hash_map::Entry::Vacant(entry) => {
				entry.insert(album);
			}
		}
	}

	map.into_values().collect()
}
