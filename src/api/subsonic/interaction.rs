use crate::api::subsonic::models::{PlayQueue, SubsonicResponseWrapper};
use crate::api::subsonic::response::SubsonicResponder;
use crate::db::{DbManager, PlayQueue as DbPlayQueue};
use crate::util::http_client;
use actix_web::{Responder, web};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct SetRatingQuery {
	pub id: String,
	pub rating: i32,
}

pub async fn set_rating(query: web::Query<SetRatingQuery>) -> impl Responder {
	tracing::warn!(
		"Ignoring setRating (rating: {}) for id: {}. Tidal does not support ratings.",
		query.rating,
		query.id
	);
	SubsonicResponder(SubsonicResponseWrapper::ok())
}

pub async fn scrobble(
	req: actix_web::HttpRequest,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<DbManager>>,
) -> impl Responder {
	let mut ids = Vec::new();
	let mut times = Vec::new();
	let mut submission = true;

	let q = req.query_string();
	let parsed: HashMap<String, String> = serde_qs::from_str(q).unwrap_or_default();
	for (k, v) in parsed {
		if k == "id" {
			ids.push(v);
		} else if k == "time" {
			if let Ok(t) = v.parse::<i64>() {
				times.push(t);
			}
		} else if k == "submission" && v == "false" {
			submission = false;
		}
	}

	if ids.is_empty() {
		return SubsonicResponder(SubsonicResponseWrapper::ok());
	}

	let lastfm_details = match db.get_lastfm_details(&subsonic_ctx.user).await {
		Ok(Some(details)) => details,
		_ => return SubsonicResponder(SubsonicResponseWrapper::ok()),
	};
	let session_key = lastfm_details.0;

	let api_key = match std::env::var("LASTFM_API_KEY") {
		Ok(key) => key,
		Err(_) => return SubsonicResponder(SubsonicResponseWrapper::ok()),
	};
	let api_secret = match std::env::var("LASTFM_API_SECRET") {
		Ok(secret) => secret,
		Err(_) => return SubsonicResponder(SubsonicResponseWrapper::ok()),
	};

	let api = &subsonic_ctx.tidal_api;

	let reqwest_client = http_client();
	let url = "http://ws.audioscrobbler.com/2.0/";

	if !submission {
		if let Ok(tid) = ids[0].parse::<i64>()
			&& let Ok(track) = api.get_track(tid).await
		{
			let played_at = times
				.first()
				.copied()
				.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
				.unwrap_or(chrono::Utc::now());
			let _ = db
				.record_scrobble(&subsonic_ctx.user, &ids[0], played_at, false)
				.await;

			let mut params = vec![
				("method", "track.updateNowPlaying"),
				("track", &track.title),
				("artist", &track.artist.name),
				("api_key", &api_key),
				("sk", &session_key),
			];
			params.push(("album", &track.album.title));
			let duration_str = track.duration.to_string();
			params.push(("duration", &duration_str));

			params.sort_by_key(|k| k.0);
			let mut sig_str = String::new();
			for (k, v) in &params {
				sig_str.push_str(k);
				sig_str.push_str(v);
			}
			sig_str.push_str(&api_secret);
			let api_sig = format!("{:x}", md5::compute(sig_str));

			params.push(("api_sig", &api_sig));
			let _ = reqwest_client.post(url).form(&params).send().await;
		}
	} else {
		let mut params = vec![
			("method".to_string(), "track.scrobble".to_string()),
			("api_key".to_string(), api_key),
			("sk".to_string(), session_key),
		];

		let mut count = 0;
		for (i, id_str) in ids.iter().enumerate() {
			if count >= 50 {
				break;
			}
			if let Ok(tid) = id_str.parse::<i64>()
				&& let Ok(track) = api.get_track(tid).await
			{
				let timestamp = times.get(i).copied().unwrap_or_else(|| {
					let now = chrono::Utc::now().timestamp();
					now - (ids.len() - i) as i64 * 180
				});

				let played_at =
					chrono::DateTime::from_timestamp(timestamp, 0).unwrap_or(chrono::Utc::now());
				let _ = db
					.record_scrobble(&subsonic_ctx.user, id_str, played_at, true)
					.await;

				params.push((format!("track[{}]", count), track.title));
				params.push((format!("artist[{}]", count), track.artist.name));
				params.push((format!("timestamp[{}]", count), timestamp.to_string()));
				params.push((format!("album[{}]", count), track.album.title));
				params.push((format!("duration[{}]", count), track.duration.to_string()));

				count += 1;
			}
		}

		if count > 0 {
			params.sort_by(|a, b| a.0.cmp(&b.0));
			let mut sig_str = String::new();
			for (k, v) in &params {
				sig_str.push_str(k);
				sig_str.push_str(v);
			}
			sig_str.push_str(&api_secret);
			let api_sig = format!("{:x}", md5::compute(sig_str));

			params.push(("api_sig".to_string(), api_sig));
			let _ = reqwest_client.post(url).form(&params).send().await;
		}
	}

	SubsonicResponder(SubsonicResponseWrapper::ok())
}

pub async fn get_queue(
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<DbManager>>,
) -> impl Responder {
	get_play_queue_impl(subsonic_ctx, db).await
}

#[derive(Deserialize)]
pub struct SavePlayQueueQuery {
	pub id: Option<Vec<String>>,
	pub current: Option<String>,
	pub position: Option<i64>,
}

pub async fn save_queue(
	query: web::Query<SavePlayQueueQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<DbManager>>,
) -> impl Responder {
	save_play_queue_impl(query, subsonic_ctx, db).await
}

pub async fn get_play_queue(
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<DbManager>>,
) -> impl Responder {
	get_play_queue_impl(subsonic_ctx, db).await
}

pub async fn save_play_queue(
	query: web::Query<SavePlayQueueQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<DbManager>>,
) -> impl Responder {
	save_play_queue_impl(query, subsonic_ctx, db).await
}

async fn save_play_queue_impl(
	query: web::Query<SavePlayQueueQuery>,
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<DbManager>>,
) -> impl Responder {
	let q = query.into_inner();
	let track_ids = q.id.unwrap_or_default();
	let current_track_id = q.current;

	let position_ms = q.position;
	let updated_at = chrono::Utc::now();

	let queue = DbPlayQueue {
		username: subsonic_ctx.into_inner().user,
		current_track_id,
		position_ms,
		track_ids,
		updated_at,
	};

	if let Err(e) = db.save_play_queue(&queue).await {
		return SubsonicResponder(SubsonicResponseWrapper::error(
			0,
			&format!("Failed to save play queue: {}", e),
		));
	}

	SubsonicResponder(SubsonicResponseWrapper::ok())
}

async fn get_play_queue_impl(
	subsonic_ctx: actix_web::web::ReqData<crate::api::subsonic::middleware::SubsonicContext>,
	db: web::Data<Arc<DbManager>>,
) -> impl Responder {
	let mut resp = SubsonicResponseWrapper::ok();
	let username = subsonic_ctx.user.clone();

	match db.get_play_queue(&subsonic_ctx.user).await {
		Ok(Some(db_queue)) => {
			let changed = "2026-01-01T00:00:00.000Z".to_string();
			let mut play_queue = PlayQueue {
				current: db_queue.current_track_id,
				position: db_queue.position_ms,
				username,
				changed,
				changed_schema: db_queue.updated_at.timestamp_millis(),
				entry: None,
			};

			if !db_queue.track_ids.is_empty() {
				let api = &subsonic_ctx.tidal_api;
				let mut songs = Vec::new();

				for id_str in db_queue.track_ids.iter() {
					if let Ok(tid) = id_str.parse::<i64>()
						&& let Ok(track) = api.get_track(tid).await
					{
						songs.push(crate::api::subsonic::mapping::map_tidal_track_to_subsonic(
							&track,
							Some(&subsonic_ctx),
							None,
							None,
						));
					}
				}
				play_queue.entry = Some(songs);
			}

			resp.response.play_queue = Some(play_queue);
			SubsonicResponder(resp)
		}
		Ok(None) => {
			let changed = "2026-01-01T00:00:00.000Z".to_string();
			resp.response.play_queue = Some(PlayQueue {
				current: None,
				position: None,
				username,
				changed,
				changed_schema: 0,
				entry: Some(Vec::new()),
			});
			SubsonicResponder(resp)
		}
		Err(e) => SubsonicResponder(SubsonicResponseWrapper::error(
			0,
			&format!("Failed to retrieve play queue: {}", e),
		)),
	}
}
