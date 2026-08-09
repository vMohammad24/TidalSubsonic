use actix_web::{HttpMessage, HttpResponse, Responder, web};
use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use base64::{Engine as _, engine::general_purpose};
use futures_util::AsyncWriteExt;
use serde::Deserialize;
use tokio_util::io::ReaderStream;

use crate::api::subsonic::models::SubsonicResponseWrapper;
use crate::api::subsonic::response::SubsonicResponder;
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
		let api = req
			.extensions()
			.get::<crate::api::subsonic::middleware::SubsonicContext>()
			.map(|ctx| ctx.tidal_api.clone());

		if let Some(api) = api {
			let mut cover_uuid = None;

			if let Ok(track) = api.get_track(numeric_id).await {
				cover_uuid = track.album.cover;
			}

			if cover_uuid.is_none()
				&& let Ok(album) = api.get_album(numeric_id).await
			{
				cover_uuid = album.cover;
			}

			if cover_uuid.is_none()
				&& let Ok(artist) = api.get_artist(numeric_id).await
			{
				cover_uuid = artist.picture.or(artist.selected_album_cover_fallback);
			}

			if let Some(uuid) = cover_uuid {
				image_url = format!(
					"https://resources.tidal.com/images/{}/750x750.jpg",
					uuid.replace("-", "/")
				);
			}
		}
	} else if let Some(stripped) = id.strip_prefix("COLLAGE:")
		&& let Some(first) = stripped.split(',').next()
	{
		image_url = format!(
			"https://resources.tidal.com/images/{}/750x750.jpg",
			first.replace("-", "/")
		);
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

pub(crate) struct DashManifest {
	pub(crate) mime_type: String,
	pub(crate) urls: Vec<String>,
}

pub struct DashManifestParser;

impl DashManifestParser {
	pub fn parse(xml: &str) -> Result<DashManifest, String> {
		parse_dash_manifest(xml)
	}
}

pub fn build_dash_segment_stream(
	urls: Vec<String>,
	client: reqwest::Client,
	concurrency: usize,
) -> impl futures_util::Stream<Item = Result<actix_web::web::Bytes, actix_web::Error>> {
	stream::iter(urls)
		.map(move |url| {
			let client = client.clone();
			async move {
				let fetch_fut = client.get(&url).send();
				match tokio::time::timeout(std::time::Duration::from_secs(10), fetch_fut).await {
					Ok(Ok(res)) if res.status().is_success() => Ok(res
						.bytes_stream()
						.map_err(actix_web::error::ErrorInternalServerError)),
					Ok(Ok(res)) => {
						tracing::error!(
							"DASH segment returned non-success status: {}",
							res.status()
						);
						Err(actix_web::error::ErrorInternalServerError(
							"DASH segment fetch error",
						))
					}
					Ok(Err(e)) => {
						tracing::error!(error = %e, "Network error fetching DASH segment");
						Err(actix_web::error::ErrorInternalServerError(
							"DASH segment network error",
						))
					}
					Err(_) => {
						tracing::error!("Timeout fetching DASH segment URL: {}", url);
						Err(actix_web::error::ErrorGatewayTimeout(
							"DASH segment fetch timeout",
						))
					}
				}
			}
		})
		.buffered(concurrency)
		.map(|res| match res {
			Ok(st) => st.left_stream(),
			Err(e) => stream::once(async { Err(e) }).right_stream(),
		})
		.flatten()
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
							mime_type = attr
								.normalized_value(quick_xml::XmlVersion::Explicit1_1)
								.unwrap_or_default()
								.to_string();
						}
					}
				}
				b"SegmentTemplate" => {
					for attr in e.attributes().flatten() {
						match attr.key.as_ref() {
							b"initialization" => {
								init_url = Some(
									attr.normalized_value(quick_xml::XmlVersion::Explicit1_1)
										.unwrap_or_default()
										.to_string(),
								);
							}
							b"media" => {
								media_url = Some(
									attr.normalized_value(quick_xml::XmlVersion::Explicit1_1)
										.unwrap_or_default()
										.to_string(),
								);
							}
							b"startNumber" => {
								start_number = attr
									.normalized_value(quick_xml::XmlVersion::Explicit1_1)
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
								.normalized_value(quick_xml::XmlVersion::Explicit1_1)
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

	tracing::debug!(
		"Parsed DASH manifest: {} segments, init URL: {:?}, media URL template: {:?}",
		total_segments,
		init_url,
		media_url
	);

	if let Some(init) = init_url {
		urls.push(init);
	}
	if let Some(media) = media_url {
		for i in 0..total_segments {
			let num = start_number + i;
			let url = media.replace("$Number$", &num.to_string());
			urls.push(url);
		}
	}

	if urls.is_empty() {
		return Err("No URLs found in DASH manifest".to_string());
	}

	Ok(DashManifest { mime_type, urls })
}

pub async fn download(
	query: web::Query<DownloadQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
) -> HttpResponse {
	let api = &subsonic_ctx.tidal_api;
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
				.map(|track| async move {
					match api.get_stream_url(track.id).await {
						Ok(info) => Some((info, track.title)),
						Err(_) => None,
					}
				})
				.buffered(15)
				.filter_map(std::future::ready)
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
						.append_header(("Location", u.as_str()))
						.finish();
				}
			} else if mime_type == "application/dash+xml"
				&& let Ok(manifest) = DashManifestParser::parse(&decoded_str)
			{
				let content_type = if manifest.mime_type.is_empty() {
					"audio/flac".to_string()
				} else {
					manifest.mime_type
				};
				let ext = if content_type.contains("mp4") || content_type.contains("m4a") {
					"m4a"
				} else {
					"flac"
				};

				let stream = build_dash_segment_stream(manifest.urls, client, 15);

				return HttpResponse::Ok()
					.content_type(content_type)
					.append_header((
						"Content-Disposition",
						format!("attachment; filename=\"{}.{}\"", title, ext),
					))
					.streaming(stream);
			}
		}
	}

	let (zip_writer, zip_reader) = tokio::io::duplex(4 * 1024 * 1024);
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
				&& let Ok(manifest) = DashManifestParser::parse(&decoded_str)
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
	let api = &subsonic_ctx.tidal_api;
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
		if let Some(u) = manifest.urls.first() {
			return HttpResponse::Found()
				.append_header(("Location", u.as_str()))
				.finish();
		}
	} else if mime_type == "application/dash+xml" {
		match DashManifestParser::parse(&decoded_str) {
			Ok(manifest) => {
				let client = http_client();
				let urls = manifest.urls;
				let content_type = if manifest.mime_type.is_empty() {
					"audio/flac".to_string()
				} else {
					manifest.mime_type
				};

				let stream = build_dash_segment_stream(urls, client, 3);

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
	let api = &subsonic_ctx.tidal_api;
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

	let q = query.into_inner();
	let mut subsonic_lyrics = crate::api::subsonic::models::SubsonicLyrics {
		artist: q.artist,
		title: q.title,
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

#[cfg(test)]
mod tests {
	use super::*;
	use actix_web::test;

	#[actix_web::test]
	async fn test_parse_dash_manifest_template_urls() {
		let dash_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
		<MPD xmlns="urn:mpeg:dash:schema:mpd:2011">
			<Period>
				<AdaptationSet mimeType="audio/flac">
					<SegmentTemplate initialization="init.mp4" media="segment_$Number$.m4s" startNumber="1">
						<SegmentTimeline>
							<S r="2" />
						</SegmentTimeline>
					</SegmentTemplate>
				</AdaptationSet>
			</Period>
		</MPD>"#;

		let manifest = DashManifestParser::parse(dash_xml).expect("Failed to parse DASH manifest");
		assert_eq!(manifest.mime_type, "audio/flac");
		assert_eq!(manifest.urls.len(), 4); // 1 init URL + 3 segment URLs (r=2 means 1 + 2 = 3 segments)
		assert_eq!(manifest.urls[0], "init.mp4");
		assert_eq!(manifest.urls[1], "segment_1.m4s");
		assert_eq!(manifest.urls[2], "segment_2.m4s");
		assert_eq!(manifest.urls[3], "segment_3.m4s");
	}

	#[actix_web::test]
	async fn test_parse_dash_manifest_base_url() {
		let dash_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
		<MPD xmlns="urn:mpeg:dash:schema:mpd:2011">
			<Period>
				<AdaptationSet mimeType="audio/mp4">
					<BaseURL>https://stream.tidal.com/chunk1.mp4</BaseURL>
					<BaseURL>https://stream.tidal.com/chunk2.mp4</BaseURL>
				</AdaptationSet>
			</Period>
		</MPD>"#;

		let manifest =
			DashManifestParser::parse(dash_xml).expect("Failed to parse BaseURL manifest");
		assert_eq!(manifest.mime_type, "audio/mp4");
		assert_eq!(manifest.urls.len(), 2);
		assert_eq!(manifest.urls[0], "https://stream.tidal.com/chunk1.mp4");
		assert_eq!(manifest.urls[1], "https://stream.tidal.com/chunk2.mp4");
	}

	#[actix_web::test]
	async fn test_parse_dash_manifest_invalid_xml() {
		let result = DashManifestParser::parse("<invalid>xml");
		assert!(result.is_err() || result.unwrap().urls.is_empty());
	}

	#[actix_web::test]
	async fn test_get_cover_art_http_redirect() {
		let req = test::TestRequest::get().to_http_request();
		let query = web::Query(GetCoverArtQuery {
			id: "http://example.com/cover.png".to_string(),
		});

		let resp = get_cover_art(query, req).await.respond_to(
			&test::TestRequest::get()
				.uri("/rest/getCoverArt?id=http://example.com/cover.png")
				.to_http_request(),
		);

		assert_eq!(resp.status(), actix_web::http::StatusCode::FOUND);
		assert_eq!(
			resp.headers().get("Location").unwrap(),
			"http://example.com/cover.png"
		);
	}

	#[actix_web::test]
	async fn test_get_cover_art_uuid_transformation() {
		let req = test::TestRequest::get().to_http_request();
		let query = web::Query(GetCoverArtQuery {
			id: "12345678-1234-1234-1234-1234567890ab".to_string(),
		});

		let resp = get_cover_art(query, req).await.respond_to(
			&test::TestRequest::get()
				.uri("/rest/getCoverArt?id=12345678-1234-1234-1234-1234567890ab")
				.to_http_request(),
		);

		assert_eq!(resp.status(), actix_web::http::StatusCode::FOUND);
		assert_eq!(
			resp.headers().get("Location").unwrap(),
			"https://resources.tidal.com/images/12345678/1234/1234/1234/1234567890ab/750x750.jpg"
		);
	}

	#[actix_web::test]
	async fn test_get_cover_art_collage_transformation() {
		let req = test::TestRequest::get().to_http_request();
		let query = web::Query(GetCoverArtQuery {
			id: "COLLAGE:aaaa-bbbb-cccc,dddd-eeee-ffff".to_string(),
		});

		let resp = get_cover_art(query, req).await.respond_to(
			&test::TestRequest::get()
				.uri("/rest/getCoverArt?id=COLLAGE:aaaa-bbbb-cccc,dddd-eeee-ffff")
				.to_http_request(),
		);

		assert_eq!(resp.status(), actix_web::http::StatusCode::FOUND);
		assert_eq!(
			resp.headers().get("Location").unwrap(),
			"https://resources.tidal.com/images/aaaa/bbbb/cccc/750x750.jpg"
		);
	}
}
