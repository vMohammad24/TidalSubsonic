use crate::tidal::manager::TidalClientManager;
use crate::util::http_client;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use futures_util::future::join_all;
use rand::RngExt;
use rand::seq::SliceRandom;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, LazyLock, RwLock};

#[derive(Deserialize)]
struct LastFmUserTopTagsResponse {
	toptags: LastFmTopTags,
}

#[derive(Deserialize)]
struct LastFmTopTags {
	tag: Vec<LastFmTag>,
}

#[derive(Deserialize)]
struct LastFmTag {
	name: String,
}

#[derive(Deserialize)]
struct LastFmTagTopAlbumsResponse {
	albums: LastFmTagAlbums,
}

#[derive(Deserialize)]
struct LastFmTagAlbums {
	album: Vec<LastFmTagAlbum>,
}

#[derive(Deserialize)]
struct LastFmTagAlbum {
	name: String,
	artist: LastFmArtist,
}

#[derive(Debug, Clone)]
pub struct LastFmAlbum {
	pub artist: String,
	pub album: String,
}

#[derive(Deserialize)]
struct LastFmTopAlbumsResponse {
	topalbums: LastFmTopAlbums,
}

#[derive(Deserialize)]
struct LastFmTopAlbums {
	album: Vec<LastFmTopAlbum>,
}

#[derive(Deserialize)]
struct LastFmTopAlbum {
	name: String,
	artist: LastFmArtist,
}

#[derive(Deserialize)]
struct LastFmArtist {
	name: String,
}

#[derive(Deserialize)]
struct LastFmRecentTracksResponse {
	recenttracks: LastFmRecentTracks,
}

#[derive(Deserialize)]
struct LastFmRecentTracks {
	track: Vec<LastFmRecentTrack>,
}

#[derive(Deserialize)]
struct LastFmRecentTrack {
	album: LastFmText,
	artist: LastFmText,
}

#[derive(Deserialize)]
struct LastFmText {
	#[serde(rename = "#text")]
	text: String,
}

fn sign_and_format_params(params: &mut BTreeMap<String, String>, secret: &str) {
	let mut sig_str = String::with_capacity(256);
	for (k, v) in params.iter() {
		sig_str.push_str(k);
		sig_str.push_str(v);
	}
	sig_str.push_str(secret);

	let api_sig = format!("{:x}", md5::compute(sig_str.as_bytes()));
	params.insert("api_sig".to_string(), api_sig);
	params.insert("format".to_string(), "json".to_string());
}

pub async fn get_top_albums(
	session_key: &str,
	username: &str,
	limit: u32,
) -> Result<Vec<LastFmAlbum>, Box<dyn std::error::Error>> {
	let lastfm_api_key = std::env::var("LASTFM_API_KEY")?;
	let lastfm_api_secret = std::env::var("LASTFM_API_SECRET")?;

	let mut params = BTreeMap::new();
	params.insert("api_key".to_string(), lastfm_api_key);
	params.insert("method".to_string(), "user.getTopAlbums".to_string());
	params.insert("sk".to_string(), session_key.to_string());
	params.insert("user".to_string(), username.to_string());
	params.insert("limit".to_string(), limit.to_string());

	sign_and_format_params(&mut params, &lastfm_api_secret);

	let res = http_client()
		.post("https://ws.audioscrobbler.com/2.0/")
		.form(&params)
		.send()
		.await?;

	let data: LastFmTopAlbumsResponse = res.json().await?;
	Ok(data
		.topalbums
		.album
		.into_iter()
		.map(|a| LastFmAlbum {
			artist: a.artist.name,
			album: a.name,
		})
		.collect())
}

pub async fn get_random_albums(
	session_key: &str,
	username: &str,
	limit: u32,
) -> Result<Vec<LastFmAlbum>, Box<dyn std::error::Error>> {
	let lastfm_api_key = std::env::var("LASTFM_API_KEY")?;
	let lastfm_api_secret = std::env::var("LASTFM_API_SECRET")?;

	let random_page = rand::rng().random_range(1..=10);

	let mut params = BTreeMap::new();
	params.insert("api_key".to_string(), lastfm_api_key);
	params.insert("method".to_string(), "user.getTopAlbums".to_string());
	params.insert("sk".to_string(), session_key.to_string());
	params.insert("user".to_string(), username.to_string());
	params.insert("limit".to_string(), limit.to_string());
	params.insert("page".to_string(), random_page.to_string());

	sign_and_format_params(&mut params, &lastfm_api_secret);

	let res = http_client()
		.post("https://ws.audioscrobbler.com/2.0/")
		.form(&params)
		.send()
		.await?;

	let data: LastFmTopAlbumsResponse = res.json().await?;
	let mut albums: Vec<LastFmAlbum> = data
		.topalbums
		.album
		.into_iter()
		.map(|a| LastFmAlbum {
			artist: a.artist.name,
			album: a.name,
		})
		.collect();

	albums.shuffle(&mut rand::rng());
	Ok(albums)
}

pub async fn get_recent_tracks(
	session_key: &str,
	username: &str,
	limit: u32,
) -> Result<Vec<LastFmAlbum>, Box<dyn std::error::Error>> {
	let lastfm_api_key = std::env::var("LASTFM_API_KEY")?;
	let lastfm_api_secret = std::env::var("LASTFM_API_SECRET")?;

	let mut params = BTreeMap::new();
	params.insert("api_key".to_string(), lastfm_api_key);
	params.insert("method".to_string(), "user.getrecenttracks".to_string());
	params.insert("sk".to_string(), session_key.to_string());
	params.insert("user".to_string(), username.to_string());
	params.insert("limit".to_string(), limit.to_string());

	sign_and_format_params(&mut params, &lastfm_api_secret);

	let res = http_client()
		.post("https://ws.audioscrobbler.com/2.0/")
		.form(&params)
		.send()
		.await?;

	let data: LastFmRecentTracksResponse = res.json().await?;
	let mut seen = HashSet::with_capacity(limit as usize);
	let mut albums = Vec::with_capacity(limit as usize);

	for t in data.recenttracks.track {
		if t.album.text.is_empty() || t.artist.text.is_empty() {
			continue;
		}
		let key = format!("{}|{}", t.artist.text, t.album.text);
		if seen.insert(key) {
			albums.push(LastFmAlbum {
				artist: t.artist.text,
				album: t.album.text,
			});
		}
	}

	Ok(albums)
}

#[derive(Clone)]
struct TokenState {
	username: String,
	expiration_time: u64,
}

type TokenStore = Arc<RwLock<HashMap<String, TokenState>>>;

static TOKEN_STORE: LazyLock<TokenStore> = LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.app_data(web::Data::new(TOKEN_STORE.clone()))
		.route("/api/lastfm/link", web::get().to(link))
		.route("/lastfm/callback", web::get().to(callback));
}

#[derive(Deserialize)]
struct LinkQuery {
	username: Option<String>,
}

async fn link(
	query: web::Query<LinkQuery>,
	req: HttpRequest,
	store: web::Data<TokenStore>,
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let mut tidal_user_id_opt = None;

	if let Some(cookie) = req.cookie("tidal_subsonic_wsid") {
		let session_id = cookie.value();
		if let Ok(Some((tidal_user_id, _username))) = manager.db.get_web_session(session_id).await {
			tidal_user_id_opt = Some(tidal_user_id);
		}
	}

	let Some(tidal_user_id) = tidal_user_id_opt else {
		return HttpResponse::Unauthorized().json(serde_json::json!({
			"error": "Unauthorized"
		}));
	};

	let Some(username) = &query.username else {
		return HttpResponse::BadRequest().json(serde_json::json!({
			"error": "Missing username parameter"
		}));
	};

	let user_details = manager.db.get_user_details(username).await;
	let (user_tidal_id, _, _, _) = match user_details {
		Ok(Some(d)) => d,
		_ => {
			return HttpResponse::Forbidden().json(serde_json::json!({
				"error": "User not found or does not belong to this Tidal account"
			}));
		}
	};

	if user_tidal_id != tidal_user_id {
		return HttpResponse::Forbidden().json(serde_json::json!({
			"error": "User not found or does not belong to this Tidal account"
		}));
	}

	let Ok(lastfm_api_key) = std::env::var("LASTFM_API_KEY") else {
		tracing::error!("LASTFM_API_KEY environment variable is not set");
		return HttpResponse::InternalServerError().json(serde_json::json!({
			"error": "Last.fm integration is not configured on the server."
		}));
	};

	let now_ms = chrono::Utc::now().timestamp_millis() as u64;

	let state = format!("{}", now_ms);
	let scheme = req.connection_info().scheme().to_string();
	let host = req.connection_info().host().to_string();
	let origin = format!("{}://{}", scheme, host);

	let callback_url = format!(
		"{}/lastfm/callback?state={}",
		origin,
		urlencoding::encode(&state)
	);
	let auth_url = format!(
		"https://www.last.fm/api/auth/?api_key={}&cb={}",
		lastfm_api_key,
		urlencoding::encode(&callback_url)
	);

	let expiration_time = now_ms + 10 * 60 * 1000;

	if let Ok(mut store_write) = store.write() {
		store_write.retain(|_, v| v.expiration_time >= now_ms);
		store_write.insert(
			state.clone(),
			TokenState {
				username: username.clone(),
				expiration_time,
			},
		);
	} else {
		tracing::error!("Failed to acquire write lock for TokenStore");
		return HttpResponse::InternalServerError().json(serde_json::json!({
			"error": "Internal server error"
		}));
	}

	HttpResponse::Ok().json(serde_json::json!({
		"link": auth_url,
		"instructions": "Visit the link to authenticate with Last.fm and link your account."
	}))
}

#[derive(Deserialize)]
struct CallbackQuery {
	token: Option<String>,
	state: Option<String>,
}

async fn callback(
	query: web::Query<CallbackQuery>,
	manager: web::Data<Arc<TidalClientManager>>,
	store: web::Data<TokenStore>,
) -> impl Responder {
	let (Some(token), Some(state)) = (&query.token, &query.state) else {
		return HttpResponse::BadRequest().json(serde_json::json!({
			"error": "Missing required parameters: token or state"
		}));
	};

	let state_data = {
		let Ok(mut store_write) = store.write() else {
			tracing::error!("Failed to acquire write lock for TokenStore");
			return HttpResponse::InternalServerError().json(serde_json::json!({
				"error": "Internal server error"
			}));
		};

		if let Some(s) = store_write.get(state) {
			let now = chrono::Utc::now().timestamp_millis() as u64;

			if s.expiration_time < now {
				store_write.remove(state);
				None
			} else {
				Some(s.clone())
			}
		} else {
			None
		}
	};

	let Some(state_data) = state_data else {
		return HttpResponse::BadRequest().json(serde_json::json!({
			"error": "Invalid or expired state"
		}));
	};

	let lastfm_api_key = std::env::var("LASTFM_API_KEY").unwrap_or_default();
	let lastfm_api_secret = std::env::var("LASTFM_API_SECRET").unwrap_or_default();

	if lastfm_api_key.is_empty() || lastfm_api_secret.is_empty() {
		return HttpResponse::InternalServerError().json(serde_json::json!({
			"error": "Last.fm API credentials not configured"
		}));
	}

	let mut params = BTreeMap::new();
	params.insert("api_key".to_string(), lastfm_api_key);
	params.insert("method".to_string(), "auth.getSession".to_string());
	params.insert("token".to_string(), token.clone());

	sign_and_format_params(&mut params, &lastfm_api_secret);

	let res = http_client()
		.post("https://ws.audioscrobbler.com/2.0/")
		.form(&params)
		.send()
		.await;

	match res {
		Ok(r) if r.status().is_success() => {
			if let Ok(data) = r.json::<serde_json::Value>().await
				&& let Some(session) = data.get("session")
			{
				let session_key = session.get("key").and_then(|v| v.as_str()).unwrap_or("");
				let lastfm_username = session.get("name").and_then(|v| v.as_str()).unwrap_or("");

				if session_key.is_empty() || lastfm_username.is_empty() {
					return HttpResponse::InternalServerError().json(serde_json::json!({
						"error": "Invalid session data received from Last.fm"
					}));
				}

				if manager
					.db
					.link_lastfm_account(&state_data.username, session_key, lastfm_username)
					.await
					.is_ok()
				{
					tracing::debug!(
						lastfm_username = %lastfm_username,
						subsonic_username = %state_data.username,
						"Successfully linked Last.fm account"
					);

					let html = format!(
						"<html><body><h2>Successfully linked Last.fm account {} for user {}!</h2>\
						<p>You can close this window.</p></body></html>",
						lastfm_username, state_data.username
					);
					return HttpResponse::Ok().content_type("text/html").body(html);
				}
			}
		}
		Ok(_) => {
			return HttpResponse::InternalServerError().json(serde_json::json!({
				"error": "Failed to get session from Last.fm API"
			}));
		}
		Err(e) => {
			tracing::error!(error = %e, "Error fetching session key");
		}
	}

	HttpResponse::InternalServerError().json(serde_json::json!({
		"error": "Failed to verify token with Last.fm"
	}))
}

pub async fn get_top_albums_by_tags(
	username: &str,
	limit: u32,
) -> Result<Vec<LastFmAlbum>, Box<dyn std::error::Error>> {
	let lastfm_api_key = std::env::var("LASTFM_API_KEY")?;
	let lastfm_api_secret = std::env::var("LASTFM_API_SECRET")?;

	let mut tags_params = BTreeMap::new();
	tags_params.insert("api_key".to_string(), lastfm_api_key.clone());
	tags_params.insert("method".to_string(), "user.getTopTags".to_string());
	tags_params.insert("user".to_string(), username.to_string());
	tags_params.insert("limit".to_string(), "5".to_string());

	sign_and_format_params(&mut tags_params, &lastfm_api_secret);

	let tags_res = http_client()
		.post("https://ws.audioscrobbler.com/2.0/")
		.form(&tags_params)
		.send()
		.await?;

	let tags_data: LastFmUserTopTagsResponse = tags_res.json().await?;

	let fetch_futures = tags_data.toptags.tag.into_iter().map(|tag| {
		let api_key = lastfm_api_key.clone();
		let api_secret = lastfm_api_secret.clone();
		let limit_str = limit.to_string();

		async move {
			let mut album_params = BTreeMap::new();
			album_params.insert("api_key".to_string(), api_key);
			album_params.insert("method".to_string(), "tag.getTopAlbums".to_string());
			album_params.insert("tag".to_string(), tag.name.clone());
			album_params.insert("limit".to_string(), limit_str);

			sign_and_format_params(&mut album_params, &api_secret);

			match http_client()
				.post("https://ws.audioscrobbler.com/2.0/")
				.form(&album_params)
				.send()
				.await
			{
				Ok(res) => {
					if let Ok(data) = res.json::<LastFmTagTopAlbumsResponse>().await {
						Some(data.albums.album)
					} else {
						tracing::warn!(
							tag = %tag.name,
							"Failed to deserialize tag.getTopAlbums JSON response"
						);
						None
					}
				}
				Err(e) => {
					tracing::warn!(
						tag = %tag.name,
						error = %e,
						"Network request failed for tag.getTopAlbums"
					);
					None
				}
			}
		}
	});

	let results = join_all(fetch_futures).await;

	let mut albums = Vec::with_capacity(limit as usize);
	let mut seen = HashSet::with_capacity(limit as usize);

	for tag_albums in results.into_iter().flatten() {
		for a in tag_albums {
			if albums.len() >= limit as usize {
				break;
			}
			if a.name.is_empty() || a.artist.name.is_empty() {
				continue;
			}
			let key = format!("{}|{}", a.artist.name, a.name);
			if seen.insert(key) {
				albums.push(LastFmAlbum {
					artist: a.artist.name,
					album: a.name,
				});
			}
		}

		if albums.len() >= limit as usize {
			break;
		}
	}

	Ok(albums)
}
