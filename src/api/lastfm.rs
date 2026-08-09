use crate::tidal::manager::TidalClientManager;
use crate::util::http_client;
use crate::util::session::{extract_user_id, set_flash};
use actix_session::Session;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use futures_util::future::join_all;
use rand::RngExt;
use rand::seq::SliceRandom;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, LazyLock, RwLock};
use thiserror::Error;

const ENDPOINT: &str = "https://ws.audioscrobbler.com/2.0/";
const AUTH_URL: &str = "https://www.last.fm/api/auth/";
const MAX_SCROBBLES_PER_REQUEST: usize = 50;

#[derive(Debug, Error)]
pub enum LastFmError {
	#[error("Last.fm integration is not configured on the server")]
	NotConfigured,

	#[error("Network or HTTP request error: {0}")]
	Http(#[from] reqwest::Error),

	#[error("Failed to parse Last.fm response: {0}")]
	Parse(#[from] serde_json::Error),

	#[error("Last.fm API error ({0}): {1}")]
	Api(i16, String),

	#[error("{0}")]
	Message(String),
}

#[derive(Debug, Clone)]
pub struct LastFmAlbum {
	pub artist: String,
	pub album: String,
}

pub struct ScrobbleTrack {
	pub track: String,
	pub artist: String,
	pub album: Option<String>,
	pub duration: Option<i64>,
	pub timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct LastFmSession {
	pub name: String,
	pub key: String,
}

pub struct LastFmClient {
	api_key: String,
	api_secret: String,
}

pub static LASTFM_CLIENT: LazyLock<LastFmClient> = LazyLock::new(LastFmClient::from_env);

impl LastFmClient {
	fn from_env() -> Self {
		Self {
			api_key: std::env::var("LASTFM_API_KEY").unwrap_or_default(),
			api_secret: std::env::var("LASTFM_API_SECRET").unwrap_or_default(),
		}
	}

	pub fn is_configured(&self) -> bool {
		!self.api_key.is_empty() && !self.api_secret.is_empty()
	}

	pub fn auth_url(&self, callback_url: &str) -> String {
		format!(
			"{AUTH_URL}?api_key={}&cb={}",
			self.api_key,
			urlencoding::encode(callback_url)
		)
	}

	fn sign(&self, params: &BTreeMap<String, String>) -> String {
		let mut sig = String::with_capacity(256);
		for (key, value) in params.iter() {
			sig.push_str(key);
			sig.push_str(value);
		}
		sig.push_str(&self.api_secret);
		format!("{:x}", md5::compute(sig.as_bytes()))
	}

	async fn post(
		&self,
		method: &str,
		session_key: Option<&str>,
		params: &mut BTreeMap<String, String>,
	) -> Result<serde_json::Value, LastFmError> {
		if !self.is_configured() {
			return Err(LastFmError::NotConfigured);
		}

		params.insert("method".to_string(), method.to_string());
		params.insert("api_key".to_string(), self.api_key.clone());
		if let Some(session_key) = session_key {
			params.insert("sk".to_string(), session_key.to_string());
		}

		let api_sig = self.sign(params);
		params.insert("api_sig".to_string(), api_sig);
		params.insert("format".to_string(), "json".to_string());

		let res = http_client().post(ENDPOINT).form(&params).send().await?;
		let status = res.status();
		let body: serde_json::Value = res.json().await?;

		if !status.is_success() {
			let code = body
				.get("error")
				.and_then(|e| e.as_i64())
				.unwrap_or(status.as_u16() as i64);
			let message = body
				.get("message")
				.and_then(|m| m.as_str())
				.unwrap_or("HTTP error");
			return Err(LastFmError::Api(code as i16, message.to_string()));
		}

		if let Some(code) = body.get("error").and_then(|e| e.as_i64())
			&& code != 0
		{
			let message = body
				.get("message")
				.and_then(|m| m.as_str())
				.unwrap_or("Unknown error");
			return Err(LastFmError::Api(code as i16, message.to_string()));
		}

		Ok(body)
	}

	pub async fn top_albums(
		&self,
		username: &str,
		session_key: &str,
		limit: u32,
		page: Option<u32>,
	) -> Result<Vec<LastFmAlbum>, LastFmError> {
		let mut params = BTreeMap::new();
		params.insert("user".to_string(), username.to_string());
		params.insert("limit".to_string(), limit.to_string());
		if let Some(page) = page {
			params.insert("page".to_string(), page.to_string());
		}

		let body = self
			.post("user.getTopAlbums", Some(session_key), &mut params)
			.await?;
		let data: LastFmTopAlbumsResponse = serde_json::from_value(body)?;
		Ok(data.topalbums.album.into_iter().map(Into::into).collect())
	}

	pub async fn random_albums(
		&self,
		username: &str,
		session_key: &str,
		limit: u32,
	) -> Result<Vec<LastFmAlbum>, LastFmError> {
		let page = rand::rng().random_range(1..=10);
		let mut albums = self
			.top_albums(username, session_key, limit, Some(page))
			.await?;
		albums.shuffle(&mut rand::rng());
		Ok(albums)
	}

	pub async fn recent_tracks(
		&self,
		username: &str,
		session_key: &str,
		limit: u32,
	) -> Result<Vec<LastFmAlbum>, LastFmError> {
		let mut params = BTreeMap::new();
		params.insert("user".to_string(), username.to_string());
		params.insert("limit".to_string(), limit.to_string());

		let body = self
			.post("user.getrecenttracks", Some(session_key), &mut params)
			.await?;
		let data: LastFmRecentTracksResponse = serde_json::from_value(body)?;

		let mut seen = HashSet::with_capacity(limit as usize);
		let mut albums = Vec::with_capacity(limit as usize);

		for track in data.recenttracks.track {
			if track.album.text.is_empty() || track.artist.text.is_empty() {
				continue;
			}
			if seen.insert(format!("{}|{}", track.artist.text, track.album.text)) {
				albums.push(LastFmAlbum {
					artist: track.artist.text,
					album: track.album.text,
				});
			}
		}

		Ok(albums)
	}

	pub async fn top_albums_by_tags(
		&self,
		username: &str,
		limit: u32,
	) -> Result<Vec<LastFmAlbum>, LastFmError> {
		let mut tags_params = BTreeMap::new();
		tags_params.insert("user".to_string(), username.to_string());
		tags_params.insert("limit".to_string(), "5".to_string());

		let body = self.post("user.getTopTags", None, &mut tags_params).await?;
		let tags_data: LastFmUserTopTagsResponse = serde_json::from_value(body)?;

		let fetch_futures = tags_data.toptags.tag.into_iter().map(|tag| {
			let tag_name = tag.name.clone();
			let tag = tag.name;
			async move {
				let mut album_params = BTreeMap::new();
				album_params.insert("tag".to_string(), tag);
				album_params.insert("limit".to_string(), limit.to_string());

				match self.post("tag.getTopAlbums", None, &mut album_params).await {
					Ok(body) => match serde_json::from_value::<LastFmTagTopAlbumsResponse>(body) {
						Ok(data) => Some(data.albums.album),
						Err(error) => {
							tracing::warn!(
								tag = %tag_name,
								error = %error,
								"Failed to deserialize tag.getTopAlbums response"
							);
							None
						}
					},
					Err(error) => {
						tracing::warn!(
							tag = %tag_name,
							error = %error,
							"Request failed for tag.getTopAlbums"
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
			for entry in tag_albums {
				if albums.len() >= limit as usize {
					break;
				}
				if entry.name.is_empty() || entry.artist.name.is_empty() {
					continue;
				}
				let album: LastFmAlbum = entry.into();
				if seen.insert(format!("{}|{}", album.artist, album.album)) {
					albums.push(album);
				}
			}
			if albums.len() >= limit as usize {
				break;
			}
		}

		Ok(albums)
	}

	pub async fn now_playing(
		&self,
		session_key: &str,
		track: &str,
		artist: &str,
		album: Option<&str>,
		duration: Option<i64>,
	) -> Result<(), LastFmError> {
		let mut params = BTreeMap::new();
		params.insert("track".to_string(), track.to_string());
		params.insert("artist".to_string(), artist.to_string());
		if let Some(album) = album {
			params.insert("album".to_string(), album.to_string());
		}
		if let Some(duration) = duration {
			params.insert("duration".to_string(), duration.to_string());
		}

		self.post("track.updateNowPlaying", Some(session_key), &mut params)
			.await?;
		Ok(())
	}

	pub async fn scrobble(
		&self,
		session_key: &str,
		tracks: &[ScrobbleTrack],
	) -> Result<(), LastFmError> {
		for chunk in tracks.chunks(MAX_SCROBBLES_PER_REQUEST) {
			let mut params = BTreeMap::new();
			for (i, track) in chunk.iter().enumerate() {
				params.insert(format!("track[{i}]"), track.track.clone());
				params.insert(format!("artist[{i}]"), track.artist.clone());
				params.insert(format!("timestamp[{i}]"), track.timestamp.to_string());
				if let Some(album) = &track.album {
					params.insert(format!("album[{i}]"), album.clone());
				}
				if let Some(duration) = track.duration {
					params.insert(format!("duration[{i}]"), duration.to_string());
				}
			}
			self.post("track.scrobble", Some(session_key), &mut params)
				.await?;
		}
		Ok(())
	}

	pub async fn get_session(&self, token: &str) -> Result<LastFmSession, LastFmError> {
		let mut params = BTreeMap::new();
		params.insert("token".to_string(), token.to_string());

		let body = self.post("auth.getSession", None, &mut params).await?;
		let data: LastFmSessionResponse = serde_json::from_value(body)?;
		Ok(data.session)
	}
}

#[derive(Debug, Deserialize)]
struct LastFmTopAlbumsResponse {
	topalbums: LastFmAlbumList,
}

#[derive(Debug, Deserialize)]
struct LastFmTagTopAlbumsResponse {
	albums: LastFmAlbumList,
}

#[derive(Debug, Deserialize)]
struct LastFmAlbumList {
	album: Vec<LastFmAlbumEntry>,
}

#[derive(Debug, Deserialize)]
struct LastFmAlbumEntry {
	name: String,
	artist: LastFmArtist,
}

#[derive(Debug, Deserialize)]
struct LastFmArtist {
	name: String,
}

impl From<LastFmAlbumEntry> for LastFmAlbum {
	fn from(entry: LastFmAlbumEntry) -> Self {
		Self {
			artist: entry.artist.name,
			album: entry.name,
		}
	}
}

#[derive(Debug, Deserialize)]
struct LastFmUserTopTagsResponse {
	toptags: LastFmTopTags,
}

#[derive(Debug, Deserialize)]
struct LastFmTopTags {
	tag: Vec<LastFmTag>,
}

#[derive(Debug, Deserialize)]
struct LastFmTag {
	name: String,
}

#[derive(Debug, Deserialize)]
struct LastFmRecentTracksResponse {
	recenttracks: LastFmRecentTracks,
}

#[derive(Debug, Deserialize)]
struct LastFmRecentTracks {
	track: Vec<LastFmRecentTrack>,
}

#[derive(Debug, Deserialize)]
struct LastFmRecentTrack {
	album: LastFmText,
	artist: LastFmText,
}

#[derive(Debug, Deserialize)]
struct LastFmText {
	#[serde(rename = "#text")]
	text: String,
}

#[derive(Debug, Deserialize)]
struct LastFmSessionResponse {
	session: LastFmSession,
}

#[derive(Clone)]
struct TokenState {
	username: String,
	expiration_time: u64,
}

type TokenStore = Arc<RwLock<HashMap<String, TokenState>>>;

static TOKEN_STORE: LazyLock<TokenStore> = LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

#[derive(Deserialize)]
struct LinkForm {
	username: String,
}

#[derive(Deserialize)]
struct LinkQuery {
	username: Option<String>,
}

#[derive(Deserialize)]
struct CallbackQuery {
	token: Option<String>,
	state: Option<String>,
}

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.app_data(web::Data::new(TOKEN_STORE.clone()))
		.route("/api/lastfm/link", web::get().to(link))
		.route("/lastfm/link", web::post().to(link_form))
		.route("/lastfm/callback", web::get().to(callback));
}

async fn initiate_lastfm_auth(
	username: &str,
	req: &HttpRequest,
	store: &TokenStore,
) -> Result<String, LastFmError> {
	if !LASTFM_CLIENT.is_configured() {
		return Err(LastFmError::NotConfigured);
	}

	let now_ms = chrono::Utc::now().timestamp_millis() as u64;
	let state = now_ms.to_string();
	let origin = format!(
		"{}://{}",
		req.connection_info().scheme(),
		req.connection_info().host()
	);

	let callback_url = format!(
		"{}/lastfm/callback?state={}",
		origin,
		urlencoding::encode(&state)
	);
	let auth_url = LASTFM_CLIENT.auth_url(&callback_url);

	let expiration_time = now_ms + 10 * 60 * 1000;

	if let Ok(mut store_write) = store.write() {
		store_write.retain(|_, v| v.expiration_time >= now_ms);
		store_write.insert(
			state,
			TokenState {
				username: username.to_string(),
				expiration_time,
			},
		);
		Ok(auth_url)
	} else {
		Err(LastFmError::Message(
			"Failed to acquire write lock for TokenStore".to_string(),
		))
	}
}

async fn link(
	query: web::Query<LinkQuery>,
	req: HttpRequest,
	store: web::Data<TokenStore>,
	manager: web::Data<TidalClientManager>,
) -> impl Responder {
	let Some(tidal_user_id) = extract_user_id(&req, &manager).await else {
		return HttpResponse::Unauthorized().json(serde_json::json!({
			"error": "Unauthorized"
		}));
	};

	let q = query.into_inner();
	let Some(username) = q.username else {
		return HttpResponse::BadRequest().json(serde_json::json!({
			"error": "Missing username parameter"
		}));
	};

	if !manager
		.db
		.verify_user_ownership(&username, &tidal_user_id)
		.await
		.unwrap_or(false)
	{
		return HttpResponse::Forbidden().json(serde_json::json!({
			"error": "User not found or does not belong to this Tidal account"
		}));
	}

	match initiate_lastfm_auth(&username, &req, &store).await {
		Ok(auth_url) => HttpResponse::Ok().json(serde_json::json!({
			"link": auth_url,
			"instructions": "Visit the link to authenticate with Last.fm and link your account."
		})),
		Err(error) => HttpResponse::Forbidden().json(serde_json::json!({
			"error": error.to_string()
		})),
	}
}

async fn callback(
	query: web::Query<CallbackQuery>,
	manager: web::Data<TidalClientManager>,
	store: web::Data<TokenStore>,
) -> impl Responder {
	let q = query.into_inner();
	let (Some(token), Some(state)) = (q.token, q.state) else {
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

		if let Some(s) = store_write.get(&state) {
			let now = chrono::Utc::now().timestamp_millis() as u64;

			if s.expiration_time < now {
				store_write.remove(&state);
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

	match LASTFM_CLIENT.get_session(&token).await {
		Ok(session) => {
			if manager
				.db
				.link_lastfm_account(&state_data.username, &session.key, &session.name)
				.await
				.is_ok()
			{
				tracing::debug!(
					lastfm_username = %session.name,
					subsonic_username = %state_data.username,
					"Successfully linked Last.fm account"
				);

				let html = format!(
					"<html><body><h2>Successfully linked Last.fm account {} for user {}!</h2>\
					<p>You can close this window.</p></body></html>",
					session.name, state_data.username
				);
				HttpResponse::Ok().content_type("text/html").body(html)
			} else {
				HttpResponse::InternalServerError().json(serde_json::json!({
					"error": "Failed to save the Last.fm link"
				}))
			}
		}
		Err(error) => {
			tracing::error!(error = %error, "Failed to exchange Last.fm token for session");
			HttpResponse::InternalServerError().json(serde_json::json!({
				"error": "Failed to verify token with Last.fm"
			}))
		}
	}
}

async fn link_form(
	form: web::Form<LinkForm>,
	req: HttpRequest,
	session: Session,
	store: web::Data<TokenStore>,
	manager: web::Data<TidalClientManager>,
) -> impl Responder {
	let form = form.into_inner();
	let Some(tidal_user_id) = extract_user_id(&req, &manager).await else {
		set_flash(&session, "error", "Not authenticated");
		return HttpResponse::SeeOther()
			.append_header(("Location", "/"))
			.finish();
	};

	if !manager
		.db
		.verify_user_ownership(&form.username, &tidal_user_id)
		.await
		.unwrap_or(false)
	{
		set_flash(
			&session,
			"error",
			"User not found or does not belong to this Tidal account",
		);
		return HttpResponse::SeeOther()
			.append_header(("Location", "/"))
			.finish();
	}

	let auth_url = match initiate_lastfm_auth(&form.username, &req, &store).await {
		Ok(url) => url,
		Err(error) => {
			set_flash(&session, "error", &error.to_string());
			return HttpResponse::SeeOther()
				.append_header(("Location", "/"))
				.finish();
		}
	};

	if req.headers().contains_key("HX-Request") {
		return HttpResponse::Ok()
			.append_header((
				"HX-Trigger",
				serde_json::json!({ "copyToClipboard": auth_url }).to_string(),
			))
			.content_type("text/html")
			.body(r#"<div id="notification" hx-swap-oob="true" class="notification success">Last.fm link generated and copied to clipboard!</div>"#.to_string());
	}

	HttpResponse::Found()
		.append_header(("Location", auth_url))
		.finish()
}
