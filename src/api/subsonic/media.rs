use actix_web::{HttpMessage, HttpResponse, Responder, web};
use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use base64::{Engine as _, engine::general_purpose};
use futures_util::AsyncWriteExt;
use serde::Deserialize;
use std::sync::Arc;
use tokio_util::io::ReaderStream;

use crate::api::subsonic::models::SubsonicResponseWrapper;
use crate::api::subsonic::response::SubsonicResponder;
use crate::tidal::manager::TidalClientManager;
use crate::tidal::models::PlaybackInfo;
use crate::util::http_client;

use futures_util::stream::{self, StreamExt, TryStreamExt};
use quick_xml::Reader;
use quick_xml::events::Event;

#[derive(Deserialize)]
pub struct GetCoverArtQuery {
	pub id: String,
}

pub async fn get_cover_art(
	query: web::Query<GetCoverArtQuery>,
	_manager: web::Data<Arc<TidalClientManager>>,
	req: actix_web::HttpRequest,
) -> impl Responder {
	let id = &query.id;

	if id.starts_with("http") {
		return HttpResponse::Found()
			.append_header(("Location", id.as_str()))
			.finish();
	}

	let mut image_url = String::new();
	let is_uuid = id.len() == 36 && id.chars().filter(|c| *c == '-').count() == 4;

	if is_uuid {
		image_url = format!(
			"https://resources.tidal.com/images/{}/750x750.jpg",
			id.replace("-", "/")
		);
	} else if let Ok(numeric_id) = id.parse::<i64>() {
		let subsonic_ctx = req
			.extensions()
			.get::<crate::api::subsonic::middleware::SubsonicContext>()
			.cloned();

		if let Some(ctx) = subsonic_ctx {
			let api = ctx.tidal_api.clone();

			let mut cover_uuid = None;

			if let Ok(track) = api.get_track(numeric_id).await {
				cover_uuid = track.album.cover.clone();
			}

			if cover_uuid.is_none()
				&& let Ok(album) = api.get_album(numeric_id).await
			{
				cover_uuid = album.cover.clone();
			}

			if cover_uuid.is_none()
				&& let Ok(artist) = api.get_artist(numeric_id).await
			{
				cover_uuid = artist
					.picture
					.clone()
					.or(artist.selected_album_cover_fallback.clone());
			}

			if let Some(uuid) = cover_uuid {
				image_url = format!(
					"https://resources.tidal.com/images/{}/750x750.jpg",
					uuid.replace("-", "/")
				);
			}
		}
	} else if let Some(stripped) = id.strip_prefix("COLLAGE:") {
		let parts: Vec<&str> = stripped.split(',').collect();
		if !parts.is_empty() {
			image_url = format!(
				"https://resources.tidal.com/images/{}/750x750.jpg",
				parts[0].replace("-", "/")
			);
		}
	}

	if !image_url.is_empty() {
		return HttpResponse::Found()
			.append_header(("Location", image_url))
			.finish();
	}

	HttpResponse::NotFound().finish()
}

#[derive(Deserialize)]
pub struct StreamQuery {
	pub id: String,
}

#[derive(Deserialize)]
pub struct DownloadQuery {
	pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
	urls: Vec<String>,
}

struct DashManifest {
	mime_type: String,
	urls: Vec<String>,
}

fn parse_dash_manifest(xml: &str) -> Result<DashManifest, String> {
	let mut reader = Reader::from_str(xml);
	reader.config_mut().trim_text(true);

	let mut urls = Vec::new();
	let mut mime_type = String::new();
	let mut init_url = None;
	let mut media_url = None;
	let mut start_number = 1;
	let mut total_segments = 0;

	let mut buf = Vec::new();
	loop {
		match reader.read_event_into(&mut buf) {
			Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => match e.name().as_ref() {
				b"AdaptationSet" => {
					for attr in e.attributes().flatten() {
						if attr.key.as_ref() == b"mimeType" {
							mime_type = attr.unescape_value().unwrap_or_default().to_string();
						}
					}
				}
				b"SegmentTemplate" => {
					for attr in e.attributes().flatten() {
						match attr.key.as_ref() {
							b"initialization" => {
								init_url =
									Some(attr.unescape_value().unwrap_or_default().to_string());
							}
							b"media" => {
								media_url =
									Some(attr.unescape_value().unwrap_or_default().to_string());
							}
							b"startNumber" => {
								start_number = attr
									.unescape_value()
									.unwrap_or_default()
									.parse()
									.unwrap_or(1);
							}
							_ => {}
						}
					}
				}
				b"S" => {
					let mut r = 0;
					for attr in e.attributes().flatten() {
						if attr.key.as_ref() == b"r" {
							r = attr
								.unescape_value()
								.unwrap_or_default()
								.parse()
								.unwrap_or(0);
						}
					}
					total_segments += 1 + r;
				}
				b"BaseURL" => {
					if let Ok(Event::Text(e)) = reader.read_event_into(&mut buf) {
						let url = String::from_utf8_lossy(e.as_ref()).to_string();
						if !url.is_empty() {
							urls.push(url);
						}
					}
				}
				_ => {}
			},
			Ok(Event::Eof) => break,
			Err(e) => return Err(e.to_string()),
			_ => {}
		}
		buf.clear();
	}

	if let Some(ref init) = init_url {
		urls.push(init.clone());
	}
	if let Some(ref media) = media_url {
		for i in 0..total_segments {
			let num = start_number + i;
			let url = media.replace("$Number$", &num.to_string());
			urls.push(url);
		}
	}

	tracing::debug!(
		"Parsed DASH manifest: {} segments, init URL: {:?}, media URL template: {:?}",
		total_segments,
		init_url,
		media_url
	);

	if urls.is_empty() {
		return Err("No URLs found in DASH manifest".to_string());
	}

	Ok(DashManifest { mime_type, urls })
}

pub async fn download(
	query: web::Query<DownloadQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> HttpResponse {
	let api = subsonic_ctx.tidal_api.clone();
	let id = match query.id.parse::<i64>() {
		Ok(i) => i,
		Err(e) => {
			tracing::error!("Failed to parse id: {}", e);
			return HttpResponse::BadRequest().finish();
		}
	};

	let playback_infos: Vec<(PlaybackInfo, String)> = match api.get_stream_url(id).await {
		Ok(info) => vec![(info, format!("track_{}", id))],
		Err(e) => {
			let album = match api.get_album_tracks(id, 500, 0).await {
				Ok(a) => a,
				Err(_) => {
					tracing::error!("Failed to fetch stream_url/album for id {}: {}", id, e);
					return HttpResponse::NotFound().finish();
				}
			};

			stream::iter(album.items)
				.map(|track| {
					let api_clone = api.clone();
					async move {
						match api_clone.get_stream_url(track.id).await {
							Ok(info) => Some((info, track.title)),
							Err(_) => None,
						}
					}
				})
				.buffered(15)
				.filter_map(|x| async { x })
				.collect()
				.await
		}
	};

	if playback_infos.is_empty() {
		return HttpResponse::NotFound().finish();
	}

	let client = http_client();

	if playback_infos.len() == 1 {
		let (info, title) = &playback_infos[0];
		if let Some(manifest_b64) = &info.manifest
			&& let Ok(decoded) = general_purpose::STANDARD.decode(manifest_b64)
		{
			let decoded_str = String::from_utf8_lossy(&decoded);
			let mime_type = info.manifest_mime_type.as_deref().unwrap_or("");

			if mime_type == "application/vnd.tidal.bts" {
				if let Ok(manifest) = serde_json::from_str::<Manifest>(&decoded_str)
					&& let Some(u) = manifest.urls.first()
				{
					return HttpResponse::Found()
						.append_header(("Location", u.clone()))
						.finish();
				}
			} else if mime_type == "application/dash+xml"
				&& let Ok(manifest) = parse_dash_manifest(&decoded_str)
			{
				let content_type = if manifest.mime_type.is_empty() {
					"audio/flac".to_string()
				} else {
					manifest.mime_type
				};

				let stream = stream::iter(manifest.urls)
					.map(move |url| {
						let client = client.clone();
						async move {
							match client.get(&url).send().await {
								Ok(res) if res.status().is_success() => Ok(res
									.bytes_stream()
									.map_err(actix_web::error::ErrorInternalServerError)),
								e => {
									tracing::error!(error = ?e, "Failed to fetch DASH segment: {}", url);
									Err(actix_web::error::ErrorInternalServerError(
										"Failed to fetch DASH segment",
									))
								}
							}
						}
					})
					.buffered(15)
					.map(|res| match res {
						Ok(st) => st.left_stream(),
						Err(e) => stream::once(async { Err(e) }).right_stream(),
					})
					.flatten();

				return HttpResponse::Ok()
					.content_type(content_type.clone())
					.append_header((
						"Content-Disposition",
						format!(
							"attachment; filename=\"{}.{}\"",
							title,
							if content_type.contains("mp4") || content_type.contains("m4a") {
								"m4a"
							} else {
								"flac"
							}
						),
					))
					.streaming(stream);
			}
		}
	}

	let (zip_writer, zip_reader) = tokio::io::duplex(52_428_800);
	let http_stream = ReaderStream::new(zip_reader);

	tokio::spawn(async move {
		let mut zip = ZipFileWriter::with_tokio(zip_writer);

		for (index, (info, title)) in playback_infos.into_iter().enumerate() {
			let manifest_b64 = match info.manifest {
				Some(m) => m,
				None => continue,
			};

			let decoded = match general_purpose::STANDARD.decode(&manifest_b64) {
				Ok(d) => d,
				Err(_) => continue,
			};

			let decoded_str = String::from_utf8_lossy(&decoded);
			let mime_type = info.manifest_mime_type.as_deref().unwrap_or("");

			let ext = if decoded_str.contains("mp4") || decoded_str.contains("m4a") {
				"m4a"
			} else {
				"flac"
			};
			let safe_title =
				title.replace(&['/', '\\', ':', '*', '?', '"', '<', '>', '|'][..], "_");
			let filename = format!("{:02} - {}.{}", index + 1, safe_title, ext);

			let mut entry = ZipEntryBuilder::new(filename.into(), Compression::Stored);

			if mime_type == "application/vnd.tidal.bts" {
				if let Ok(manifest) = serde_json::from_str::<Manifest>(&decoded_str)
					&& let Some(u) = manifest.urls.first()
					&& let Ok(mut res) = client.get(u).send().await
					&& let Ok(mut writer) = zip.write_entry_stream(entry).await
				{
					while let Ok(Some(chunk)) = res.chunk().await {
						let _ = writer.write_all(&chunk).await;
					}
					let _ = writer.close().await;
				}
			} else if mime_type == "application/dash+xml"
				&& let Ok(manifest) = parse_dash_manifest(&decoded_str)
			{
				let actual_mime = if manifest.mime_type.is_empty() {
					"audio/flac"
				} else {
					&manifest.mime_type
				};
				let new_ext = if actual_mime.contains("mp4") || actual_mime.contains("m4a") {
					"m4a"
				} else {
					"flac"
				};

				let new_filename = format!("{:02} - {}.{}", index + 1, safe_title, new_ext);
				entry = entry.filename(new_filename.into());

				if let Ok(mut writer) = zip.write_entry_stream(entry).await {
					let mut dash_stream = stream::iter(manifest.urls)
						.map(|url| {
							let client = client.clone();
							async move { client.get(&url).send().await.ok()?.bytes().await.ok() }
						})
						.buffered(5);

					while let Some(Some(bytes)) = dash_stream.next().await {
						let _ = writer.write_all(&bytes).await;
					}
					let _ = writer.close().await;
				}
			}
		}
		let _ = zip.close().await;
	});

	HttpResponse::Ok()
		.content_type("application/zip")
		.append_header((
			"Content-Disposition",
			format!("attachment; filename=\"album_{}.zip\"", id),
		))
		.streaming(http_stream)
}

pub async fn stream(
	query: web::Query<StreamQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> HttpResponse {
	let api = subsonic_ctx.tidal_api.clone();
	let id = match query.id.parse::<i64>() {
		Ok(i) => i,
		Err(e) => {
			tracing::error!("Failed to parse id: {}", e);
			return HttpResponse::NotFound().finish();
		}
	};

	let playback_info = match api.get_stream_url(id).await {
		Ok(info) => info,
		Err(e) => {
			tracing::error!("Failed to fetch stream_url: {}", e);
			return HttpResponse::NotFound().finish();
		}
	};

	let manifest_b64 = match playback_info.manifest {
		Some(m) => m,
		None => {
			tracing::error!("No manifest found in playback info");
			return HttpResponse::NotFound().finish();
		}
	};

	let decoded = match general_purpose::STANDARD.decode(&manifest_b64) {
		Ok(d) => d,
		Err(e) => {
			tracing::error!("Failed to decode base64 manifest: {}", e);
			return HttpResponse::NotFound().finish();
		}
	};

	let decoded_str = String::from_utf8_lossy(&decoded);
	let mime_type = playback_info.manifest_mime_type.as_deref().unwrap_or("");

	if mime_type == "application/vnd.tidal.bts" {
		let manifest: Manifest = match serde_json::from_str(&decoded_str) {
			Ok(m) => m,
			Err(e) => {
				tracing::error!("Failed to parse manifest JSON: {}", e);
				return HttpResponse::NotFound().finish();
			}
		};
		if let Some(u) = manifest.urls.first().cloned() {
			return HttpResponse::Found()
				.append_header(("Location", u))
				.finish();
		}
	} else if mime_type == "application/dash+xml" {
		match parse_dash_manifest(&decoded_str) {
			Ok(manifest) => {
				let client = http_client();
				let urls = manifest.urls;
				let content_type = if manifest.mime_type.is_empty() {
					"audio/flac".to_string()
				} else {
					manifest.mime_type
				};

				let stream = stream::iter(urls)
					.map(move |url| {
						let client = client.clone();
						async move {
							match client.get(&url).send().await {
								Ok(res) if res.status().is_success() => Ok(res
									.bytes_stream()
									.map_err(actix_web::error::ErrorInternalServerError)),
								e => {
									tracing::error!(error = ?e, "Failed to fetch DASH segment: {}", url);
									Err(actix_web::error::ErrorInternalServerError(
										"Failed to fetch DASH segment",
									))
								}
							}
						}
					})
					.buffered(3)
					.map(|res| match res {
						Ok(st) => st.left_stream(),
						Err(e) => stream::once(async { Err(e) }).right_stream(),
					})
					.flatten();

				return HttpResponse::Ok()
					.content_type(content_type)
					.streaming(stream);
			}
			Err(e) => {
				tracing::error!("Failed to parse DASH manifest: {}", e);
			}
		}
	} else {
		tracing::error!("Unknown manifest mime type: {}", mime_type);
	}

	tracing::error!("Could not extract URL from manifest");
	HttpResponse::NotFound().finish()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLyricsQuery {
	pub id: Option<String>,
	pub artist: Option<String>,
	pub title: Option<String>,
}

pub async fn get_lyrics(
	query: web::Query<GetLyricsQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> impl Responder {
	let api = subsonic_ctx.tidal_api.clone();
	let mut track_id = None;

	if let Some(id_str) = &query.id
		&& let Ok(id) = id_str.parse::<i64>()
	{
		track_id = Some(id);
	}

	if track_id.is_none()
		&& let (Some(artist), Some(title)) = (&query.artist, &query.title)
	{
		let search_query = format!("{} {}", artist, title);
		if let Ok(res) = api.search(&search_query, 10, 0).await
			&& let Some(tracks) = res.tracks
			&& let Some(first) = tracks.items.first()
		{
			track_id = Some(first.id);
		}
	}

	let mut subsonic_lyrics = crate::api::subsonic::models::SubsonicLyrics {
		artist: query.artist.clone(),
		title: query.title.clone(),
		value: "".to_string(),
	};

	if let Some(id) = track_id
		&& let Ok(lyrics) = api.get_lyrics(id).await
	{
		subsonic_lyrics.value = if !lyrics.subtitles.is_empty() {
			lyrics.subtitles
		} else {
			lyrics.lyrics
		};
	}

	SubsonicResponder(SubsonicResponseWrapper::ok().with_lyrics(subsonic_lyrics))
}
