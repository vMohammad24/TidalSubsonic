use crate::tidal::{
	config,
	error::TidalError,
	models::{AudioMode, Quality, VideoQuality},
};
use crate::util::http_client;
use chrono::Utc;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ApiVersion {
	#[default]
	V1,
	V2,
	OpenApi,
	Desktop,
	DesktopV2,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionOptions {
	pub quality: Option<Quality>,
	pub video_quality: Option<VideoQuality>,
	pub audio_mode: Option<AudioMode>,
	pub session_id: Option<String>,
	pub country_code: Option<String>,
	pub access_token: Option<String>,
	pub refresh_token: Option<String>,
	pub token_expiry: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TokenResponse {
	pub access_token: String,
	pub refresh_token: Option<String>,
	pub token_type: String,
	pub expires_in: u64,
	pub user: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DeviceAuthorizationResponse {
	#[serde(rename = "deviceCode")]
	pub device_code: String,
	#[serde(rename = "userCode")]
	pub user_code: String,
	#[serde(rename = "verificationUri")]
	pub verification_uri: String,
	#[serde(rename = "verificationUriComplete")]
	pub verification_uri_complete: String,
	#[serde(rename = "expiresIn")]
	pub expires_in: u64,
	pub interval: u64,
}

#[derive(Debug, Clone)]
pub struct TokenUpdate {
	pub user_id: i64,
	pub access_token: String,
	pub refresh_token: Option<String>,
	pub token_expiry: Option<u64>,
}

#[derive(Clone)]
pub struct Session {
	client: reqwest::Client,
	pub options: Arc<RwLock<SessionOptions>>,
	pub user_id: Option<i64>,
	pub token_update_tx: Option<UnboundedSender<TokenUpdate>>,
}

impl Session {
	pub fn new(
		options: SessionOptions,
		token_update_tx: Option<UnboundedSender<TokenUpdate>>,
	) -> Self {
		let client = http_client();

		Self {
			client,
			options: Arc::new(RwLock::new(SessionOptions {
				country_code: options.country_code.or_else(|| Some("US".to_string())),
				quality: options.quality.or(Some(Quality::HiRes)),
				video_quality: options.video_quality.or(Some(VideoQuality::High)),
				audio_mode: options.audio_mode.or(Some(AudioMode::DolbyAtmos)),
				..options
			})),
			user_id: None,
			token_update_tx,
		}
	}

	pub async fn device_authorization(&self) -> Result<DeviceAuthorizationResponse, TidalError> {
		let response = self
			.client
			.post(format!("{}/device_authorization", config::AUTH_URL))
			.form(&[
				("client_id", *config::CLIENT_ID),
				("scope", "r_usr w_usr r_sub"),
			])
			.send()
			.await?;

		if !response.status().is_success() {
			let error_text = response.text().await.unwrap_or_default();
			return Err(TidalError::Authentication(error_text));
		}

		let data = response.json::<serde_json::Value>().await?;

		Ok(DeviceAuthorizationResponse {
			device_code: data["deviceCode"].as_str().unwrap_or_default().to_string(),
			user_code: data["userCode"].as_str().unwrap_or_default().to_string(),
			verification_uri: data["verificationUri"]
				.as_str()
				.unwrap_or_default()
				.to_string(),
			verification_uri_complete: data["verificationUriComplete"]
				.as_str()
				.unwrap_or_default()
				.to_string(),
			expires_in: data["expiresIn"].as_u64().unwrap_or(300),
			interval: data["interval"].as_u64().unwrap_or(5),
		})
	}

	async fn send_request_inner(
		&self,
		method: Method,
		url: &str,
		api_version: ApiVersion,
		query: Option<&[(&str, &str)]>,
		form: Option<&[(&str, String)]>,
		json: Option<serde_json::Value>,
	) -> Result<reqwest::Response, TidalError> {
		let mut req = self
			.client
			.request(method.clone(), url)
			.header("X-Tidal-Token", *config::API_TOKEN)
			.header("user-agent", config::CLIENT_USER_AGENT)
			.header("x-tidal-client-version", config::TIDAL_VERSION);

		let (use_oauth, use_session_id, token, session_id, country_code) = {
			let opts = self.options.read().unwrap_or_else(|e| e.into_inner());
			let u_oauth = opts.access_token.is_some();
			let u_session = opts.session_id.is_some() && !u_oauth;
			(
				u_oauth,
				u_session,
				opts.access_token.clone(),
				opts.session_id.clone(),
				opts.country_code.clone(),
			)
		};

		if use_oauth {
			if let Some(tok) = token {
				req = req.header("Authorization", format!("Bearer {}", tok));

				if matches!(
					api_version,
					ApiVersion::V2 | ApiVersion::OpenApi | ApiVersion::Desktop
				) {
					let accept = match api_version {
						ApiVersion::V2 => "application/vnd.tidal.v1+json",
						ApiVersion::OpenApi => "application/vnd.api+json",
						_ => "application/json",
					};
					req = req.header("Accept", accept);
				}
			} else {
				return Err(TidalError::Authentication(
					"OAuth token required but not available.".to_string(),
				));
			}
		} else if use_session_id {
			if let Some(sid) = session_id {
				req = req.header("X-Tidal-SessionId", sid);
			} else {
				return Err(TidalError::Authentication(
					"Session ID required but not available.".to_string(),
				));
			}
		}

		let default_country = "US".to_string();
		let mut final_query = vec![
			(
				"countryCode".to_string(),
				country_code.unwrap_or(default_country),
			),
			("deviceType".to_string(), "DESKTOP".to_string()),
			("locale".to_string(), "en_US".to_string()),
			("platform".to_string(), "DESKTOP".to_string()),
		];

		if let Some(q) = query {
			for (k, v) in q {
				final_query.push((k.to_string(), v.to_string()));
			}
		}

		req = req.query(&final_query);

		if let Some(f) = form {
			req = req.form(f);
		}

		if let Some(j) = json {
			req = req.json(&j);
		}

		let res = req.send().await?;
		Ok(res)
	}

	pub async fn request<T: for<'de> Deserialize<'de>>(
		&self,
		method: Method,
		path: &str,
		query: Option<&[(&str, &str)]>,
		form: Option<&[(&str, String)]>,
		api_version: ApiVersion,
	) -> Result<T, TidalError> {
		self.request_full(method, path, query, form, None, api_version)
			.await
	}

	pub async fn request_full<T: for<'de> Deserialize<'de>>(
		&self,
		method: Method,
		path: &str,
		query: Option<&[(&str, &str)]>,
		form: Option<&[(&str, String)]>,
		json: Option<serde_json::Value>,
		api_version: ApiVersion,
	) -> Result<T, TidalError> {
		let (use_oauth, use_session_id) = {
			let opts = self.options.read().unwrap_or_else(|e| e.into_inner());
			let u_oauth = opts.access_token.is_some();
			let u_session = opts.session_id.is_some() && !u_oauth;
			(u_oauth, u_session)
		};

		if !use_oauth && !use_session_id {
			return Err(TidalError::Authentication(
				"Session is not valid. Please login first.".to_string(),
			));
		}

		let base_url = match api_version {
			ApiVersion::V2 => config::API_V2_URL,
			ApiVersion::OpenApi => config::OPENAPI_V2_URL,
			ApiVersion::Desktop => config::DESKTOP_V1_URL,
			ApiVersion::DesktopV2 => config::DESKTOP_V2_URL,
			ApiVersion::V1 => config::API_URL,
		};

		let url = format!("{}{}", base_url, path);

		let res = self
			.send_request_inner(method.clone(), &url, api_version, query, form, json.clone())
			.await?;
		let status = res.status();

		if status == reqwest::StatusCode::UNAUTHORIZED && use_oauth {
			let _ = self.refresh_access_token().await?;
			let res2 = self
				.send_request_inner(method, &url, api_version, query, form, json)
				.await?;
			let status2 = res2.status();
			if !status2.is_success() {
				let text = res2.text().await.unwrap_or_default();
				return Err(TidalError::ApiError(status2.as_u16(), text));
			}
			let text = res2.text().await.unwrap_or_default();
			if text.is_empty() && std::any::type_name::<T>() == "()" {
				return serde_json::from_str("null").map_err(Into::into);
			}
			return serde_json::from_str(&text).map_err(Into::into);
		}

		if !status.is_success() {
			if status == reqwest::StatusCode::UNAUTHORIZED {
				return Err(TidalError::Authentication(
					"Invalid or expired token".to_string(),
				));
			}
			if status == reqwest::StatusCode::NOT_FOUND {
				return Err(TidalError::ResourceNotFound(
					path.to_string(),
					"unknown".to_string(),
				));
			}
			if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
				return Err(TidalError::RateLimit);
			}
			if status == reqwest::StatusCode::PAYMENT_REQUIRED {
				return Err(TidalError::PaymentRequired);
			}

			let text = res.text().await.unwrap_or_default();
			return Err(TidalError::ApiError(status.as_u16(), text));
		}

		let text = res.text().await.unwrap_or_default();
		if text.is_empty() && std::any::type_name::<T>() == "()" {
			return serde_json::from_str("null").map_err(Into::into);
		}

		serde_json::from_str(&text).map_err(Into::into)
	}

	pub async fn refresh_access_token(&self) -> Result<TokenResponse, TidalError> {
		let (refresh_token, client_id, client_secret) = {
			let opts = self.options.read().unwrap_or_else(|e| e.into_inner());
			let refresh_token = match &opts.refresh_token {
				Some(t) => t.clone(),
				None => {
					return Err(TidalError::Authentication(
						"No refresh token available".to_string(),
					));
				}
			};
			let client_id = *config::CLIENT_ID;
			let client_secret = *config::CLIENT_SECRET;
			(refresh_token, client_id, client_secret)
		};

		let form = vec![
			("client_id", client_id),
			("client_secret", client_secret),
			("refresh_token", refresh_token.as_str()),
			("grant_type", "refresh_token"),
		];

		let response = self
			.client
			.post(format!("{}/token", config::AUTH_URL))
			.form(&form)
			.send()
			.await?;

		let status = response.status();
		let text = response.text().await.unwrap_or_default();

		if !status.is_success() {
			let mut opts = self.options.write().unwrap_or_else(|e| e.into_inner());
			opts.access_token = None;
			opts.refresh_token = None;
			opts.token_expiry = None;
			opts.session_id = None;
			return Err(TidalError::Authentication(format!(
				"Failed to refresh token: {}",
				text
			)));
		}

		let token_resp: TokenResponse = serde_json::from_str(&text)?;
		let now = Utc::now().timestamp() as u64;

		{
			let mut opts = self.options.write().unwrap_or_else(|e| e.into_inner());
			opts.access_token = Some(token_resp.access_token.clone());

			if let Some(ref new_refresh) = token_resp.refresh_token {
				opts.refresh_token = Some(new_refresh.clone());
			}

			opts.token_expiry = Some(now + token_resp.expires_in);
		}

		if let Some(user_id) = self.user_id
			&& let Some(tx) = &self.token_update_tx
		{
			let opts = self.options.read().unwrap_or_else(|e| e.into_inner());
			let _ = tx.send(TokenUpdate {
				user_id,
				access_token: opts.access_token.clone().unwrap_or_default(),
				refresh_token: opts.refresh_token.clone(),
				token_expiry: opts.token_expiry,
			});
		}

		Ok(token_resp)
	}

	pub async fn poll_device_authorization(
		&mut self,
		device_code: &str,
	) -> Result<TokenResponse, TidalError> {
		let form = vec![
			("client_id", *config::CLIENT_ID),
			("device_code", device_code),
			("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
			("scope", "r_usr w_usr"),
		];

		let response = self
			.client
			.post(format!("{}/token", config::AUTH_URL))
			.form(&form)
			.send()
			.await?;

		let status = response.status();
		let text = response.text().await.unwrap_or_default();

		if !status.is_success() {
			if text.contains("authorization_pending") {
				return Err(TidalError::Authentication("authorization_pending".into()));
			} else if text.contains("expired_token") {
				return Err(TidalError::Authentication("expired_token".into()));
			} else if text.contains("access_denied") {
				return Err(TidalError::Authentication("access_denied".into()));
			}
			return Err(TidalError::Authentication(text));
		}

		let token_resp: TokenResponse = serde_json::from_str(&text)?;
		let now = Utc::now().timestamp() as u64;

		{
			let mut opts = self.options.write().unwrap_or_else(|e| e.into_inner());
			opts.access_token = Some(token_resp.access_token.clone());
			opts.refresh_token = token_resp.refresh_token.clone();
			opts.token_expiry = Some(now + token_resp.expires_in);
		}

		if let Some(user_val) = &token_resp.user
			&& let Some(id) = user_val.get("userId").and_then(|v| v.as_i64())
		{
			self.user_id = Some(id);
		}

		Ok(token_resp)
	}
}
