use crate::db::DbManager;
use crate::tidal::manager::TidalClientManager;
use actix_web::HttpRequest;
use actix_web::{HttpResponse, Responder, web};
use async_zip::base::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;

#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceAuthSession {
	#[serde(rename = "deviceCode")]
	pub device_code: String,
	#[serde(rename = "userCode")]
	pub user_code: String,
	#[serde(rename = "verificationUri")]
	pub verification_uri: String,
	#[serde(rename = "verificationUriComplete")]
	pub verification_uri_complete: String,
	#[serde(rename = "expiresAt")]
	pub expires_at: u64,
	pub interval: u64,
	pub status: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<String>,
}

pub type DeviceAuthStore = Arc<RwLock<HashMap<String, DeviceAuthSession>>>;

static SESSION_STORE: LazyLock<DeviceAuthStore> =
	LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.app_data(web::Data::new(SESSION_STORE.clone()));
	cfg.route("/login", web::get().to(initiate_login))
		.route("/auth/status", web::get().to(check_auth_status))
		.route("/logout", web::post().to(logout))
		.route("/logout", web::get().to(logout))
		.route("/delete_account", web::post().to(delete_account))
		.route("/request_data", web::post().to(request_data));
}

async fn initiate_login(
	manager: web::Data<Arc<TidalClientManager>>,
	store: web::Data<DeviceAuthStore>,
) -> impl Responder {
	let ui_client = manager.get_global_client();
	match ui_client.device_authorization().await {
		Ok(device_auth) => {
			let now = Utc::now().timestamp() as u64;
			let auth_attempt_id = format!("{}-{}", now, rand::random::<u32>());
			let expires_at = now + device_auth.expires_in;

			store.write().unwrap_or_else(|e| e.into_inner()).insert(
				auth_attempt_id.clone(),
				DeviceAuthSession {
					device_code: device_auth.device_code.clone(),
					user_code: device_auth.user_code.clone(),
					verification_uri: device_auth.verification_uri.clone(),
					verification_uri_complete: device_auth.verification_uri_complete.clone(),
					expires_at,
					interval: device_auth.interval,
					status: "pending".to_string(),
					error: None,
				},
			);

			let response = serde_json::json!({
				"sessionId": auth_attempt_id,
				"userCode": device_auth.user_code,
				"verificationUri": device_auth.verification_uri,
				"verificationUriComplete": device_auth.verification_uri_complete,
				"expiresIn": device_auth.expires_in,
				"message": format!("Please visit {} or go to {} and enter code {}", device_auth.verification_uri_complete, device_auth.verification_uri, device_auth.user_code)
			});
			HttpResponse::Ok().json(response)
		}
		Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
			"error": "Failed to start authorization",
			"details": e.to_string()
		})),
	}
}

#[derive(Deserialize)]
struct AuthStatusQuery {
	#[serde(rename = "sessionId")]
	session_id: Option<String>,
}

async fn check_auth_status(
	query: web::Query<AuthStatusQuery>,
	manager: web::Data<Arc<TidalClientManager>>,
	store: web::Data<DeviceAuthStore>,
) -> impl Responder {
	let session_id = match &query.session_id {
		Some(id) => id,
		None => {
			return HttpResponse::Ok().json(serde_json::json!({
				"status": "failed",
				"error": "Invalid or expired session"
			}));
		}
	};

	let mut auth_attempt = {
		let store_read = store.read().unwrap_or_else(|e| e.into_inner());
		match store_read.get(session_id) {
			Some(s) => s.clone(),
			None => {
				return HttpResponse::Ok().json(serde_json::json!({
					"status": "failed",
					"error": "Invalid or expired session"
				}));
			}
		}
	};

	let now = Utc::now().timestamp() as u64;
	if now > auth_attempt.expires_at {
		store
			.write()
			.unwrap_or_else(|e| e.into_inner())
			.remove(session_id);
		return HttpResponse::Ok().json(serde_json::json!({
			"status": "expired",
			"message": "Authorization session has expired. Please try again."
		}));
	}

	if auth_attempt.status != "pending" {
		return HttpResponse::Ok().json(serde_json::json!({
            "status": auth_attempt.status,
            "error": auth_attempt.error,
            "message": if auth_attempt.status == "complete" { "Authorization successful!" } else { "Authorization failed." }
        }));
	}

	let mut temp_session = crate::tidal::session::Session::new(Default::default(), None);
	match temp_session
		.poll_device_authorization(&auth_attempt.device_code)
		.await
	{
		Ok(token_resp) => {
			if let Some(user_id) = temp_session.user_id {
				let user_id_str = user_id.to_string();
				let token_expiry = temp_session
					.options
					.read()
					.unwrap_or_else(|e| e.into_inner())
					.token_expiry;
				if let Err(_e) = manager
					.save_tokens_for_tidal_user(
						&user_id_str,
						token_resp.access_token,
						token_resp.refresh_token,
						token_expiry,
					)
					.await
				{
					auth_attempt.status = "failed".to_string();
					auth_attempt.error =
						Some("Failed to get user info after authorization".to_string());
					store
						.write()
						.unwrap_or_else(|e| e.into_inner())
						.insert(session_id.clone(), auth_attempt.clone());
					return HttpResponse::Ok().json(serde_json::json!({
						"status": "failed",
						"error": "Failed to get user info after authorization"
					}));
				}

				let mut display_name = user_id_str.clone();

				let profile_res = temp_session
					.request::<crate::tidal::models::UserProfile>(
						reqwest::Method::GET,
						&format!("/users/{}", user_id),
						None,
						None,
						crate::tidal::session::ApiVersion::V1,
					)
					.await;

				if let Ok(profile) = profile_res {
					if let Some(uname) = profile.username {
						display_name = uname;
					} else if let Some(email) = profile.email {
						display_name = email;
					} else if let Some(fname) = profile.first_name {
						display_name = fname;
					}
				}

				let now_nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0);
				let session_uuid =
					hex::encode(md5::compute(format!("{}-{}", now_nanos, std::process::id())).0);

				if let Err(_e) = manager
					.db
					.save_web_session(&session_uuid, &user_id_str, &display_name)
					.await
				{
					auth_attempt.status = "failed".to_string();
					auth_attempt.error = Some("Failed to save web session".to_string());
					store
						.write()
						.unwrap_or_else(|e| e.into_inner())
						.insert(session_id.clone(), auth_attempt.clone());
					return HttpResponse::Ok().json(serde_json::json!({
						"status": "failed",
						"error": "Failed to save web session"
					}));
				}

				auth_attempt.status = "complete".to_string();
				store
					.write()
					.unwrap_or_else(|e| e.into_inner())
					.insert(session_id.clone(), auth_attempt.clone());

				HttpResponse::Ok()
					.cookie(
						actix_web::cookie::Cookie::build("tidal_subsonic_wsid", session_uuid)
							.path("/")
							.http_only(true)
							.secure(true)
							.same_site(actix_web::cookie::SameSite::Strict)
							.max_age(actix_web::cookie::time::Duration::days(365 * 2))
							.finish(),
					)
					.json(serde_json::json!({
						"status": "complete",
						"userId": user_id_str,
						"message": "Authorization successful!"
					}))
			} else {
				auth_attempt.status = "failed".to_string();
				auth_attempt.error =
					Some("Failed to get user info after authorization".to_string());
				store
					.write()
					.unwrap_or_else(|e| e.into_inner())
					.insert(session_id.clone(), auth_attempt.clone());
				HttpResponse::Ok().json(serde_json::json!({
					"status": "failed",
					"error": "Failed to get user info after authorization"
				}))
			}
		}
		Err(crate::tidal::error::TidalError::Authentication(msg))
			if msg == "authorization_pending" =>
		{
			HttpResponse::Ok().json(serde_json::json!({
				"status": "pending",
				"message": "Waiting for user to complete authorization"
			}))
		}
		Err(crate::tidal::error::TidalError::Authentication(msg))
			if msg == "expired_token" || msg == "access_denied" =>
		{
			auth_attempt.status = "failed".to_string();
			auth_attempt.error = Some(msg.clone());
			store
				.write()
				.unwrap_or_else(|e| e.into_inner())
				.insert(session_id.clone(), auth_attempt.clone());
			HttpResponse::Ok().json(serde_json::json!({
				"status": "failed",
				"error": msg
			}))
		}
		Err(e) => HttpResponse::Ok().json(serde_json::json!({
			"status": "error",
			"message": "Server error checking authorization status",
			"details": e.to_string()
		})),
	}
}

async fn logout(req: HttpRequest, db: web::Data<Arc<DbManager>>) -> impl Responder {
	let cookie_value = req
		.cookie("tidal_subsonic_wsid")
		.map(|c| c.value().to_string());
	if let Some(session_id) = cookie_value
		&& let Err(e) = db.delete_web_session(&session_id).await
	{
		tracing::error!("Failed to delete web session {}: {}", session_id, e);
	}
	HttpResponse::Found()
		.cookie(
			actix_web::cookie::Cookie::build("tidal_subsonic_wsid", "")
				.path("/")
				.http_only(true)
				.secure(true)
				.same_site(actix_web::cookie::SameSite::Strict)
				.max_age(actix_web::cookie::time::Duration::ZERO)
				.finish(),
		)
		.append_header(("Location", "/?logged_out=true"))
		.finish()
}

async fn delete_account(
	req: HttpRequest,
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let cookie_value = req
		.cookie("tidal_subsonic_wsid")
		.map(|c| c.value().to_string());

	if let Some(session_id) = cookie_value
		&& let Ok(Some((tidal_user_id, _))) = manager.db.get_web_session(&session_id).await
	{
		if let Err(e) = manager.clear_tokens_for_tidal_user(&tidal_user_id).await {
			tracing::error!("Failed to clear tokens for user {}: {}", tidal_user_id, e);
		}
		let _ = manager.db.delete_web_session(&session_id).await;
	}

	HttpResponse::Found()
		.cookie(
			actix_web::cookie::Cookie::build("tidal_subsonic_wsid", "")
				.path("/")
				.http_only(true)
				.secure(true)
				.same_site(actix_web::cookie::SameSite::Strict)
				.max_age(actix_web::cookie::time::Duration::ZERO)
				.finish(),
		)
		.append_header(("Location", "/?account_deleted=true"))
		.finish()
}

async fn request_data(
	req: HttpRequest,
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let cookie_value = req
		.cookie("tidal_subsonic_wsid")
		.map(|c| c.value().to_string());

	let session_id = match cookie_value {
		Some(id) => id,
		None => return HttpResponse::Unauthorized().finish(),
	};

	let (tidal_user_id, username) = match manager.db.get_web_session(&session_id).await {
		Ok(Some(data)) => data,
		_ => return HttpResponse::Unauthorized().finish(),
	};

	let stored_tokens = match manager.db.get_tokens_by_tidal_id(&tidal_user_id).await {
		Ok(Some(tokens)) => tokens,
		e => {
			tracing::error!(
				"Failed to retrieve tokens for user {}: {:?}",
				tidal_user_id,
				e
			);
			return HttpResponse::InternalServerError().finish();
		}
	};

	if let Some(last_request) = stored_tokens.last_data_request {
		let cooldown = Duration::days(30);
		if Utc::now() < last_request + cooldown {
			let next_available = last_request + cooldown;
			let days_left = (next_available - Utc::now()).num_days();
			return HttpResponse::Found()
				.append_header((
					"Location",
					format!(
						"/?error=You requested your data on {}. You can request it again in {} days.",
						last_request.format("%d/%m/%Y"),
						days_left
					),
				))
				.finish();
		}
	}

	let users_data = match manager.db.get_all_users_export_data(&tidal_user_id).await {
		Ok(data) => data,
		_ => return HttpResponse::InternalServerError().finish(),
	};

	let play_queues = manager
		.db
		.get_play_queues_for_tidal_user(&tidal_user_id)
		.await
		.unwrap_or_default();

	let mut buffer = Vec::new();
	let mut writer = ZipFileWriter::with_tokio(&mut buffer);

	let profile_json = serde_json::to_vec_pretty(&users_data).unwrap_or_default();
	let profile_entry = ZipEntryBuilder::new("profile.json".into(), Compression::Deflate);
	if let Err(e) = writer.write_entry_whole(profile_entry, &profile_json).await {
		tracing::error!("Failed to write profile.json to zip: {}", e);
		return HttpResponse::InternalServerError().finish();
	}

	let play_queues_json = serde_json::to_vec_pretty(&play_queues).unwrap_or_default();
	let play_queues_entry = ZipEntryBuilder::new("play_queues.json".into(), Compression::Deflate);
	if let Err(e) = writer
		.write_entry_whole(play_queues_entry, &play_queues_json)
		.await
	{
		tracing::error!("Failed to write play_queues.json to zip: {}", e);
		return HttpResponse::InternalServerError().finish();
	}

	let host = req.connection_info().host().to_string();

	let readme_content = format!(
		"This ZIP file contains your personal data stored by the Tidal Subsonic Server hosted on {}.\n\n\
                - profile.json: Your account settings, linked Tidal ID, and feature flags.\n\
                - play_queues.json: Your active listening state and queued tracks.\n\n\
                NOTE:\n\
                - Your Tidal credentials (access and refresh tokens) and Subsonic passwords are encrypted in our database and are not included in this export for security reasons.\n",
		host
	);
	let readme_entry = ZipEntryBuilder::new("README.txt".into(), Compression::Deflate);
	if let Err(e) = writer
		.write_entry_whole(readme_entry, readme_content.as_bytes())
		.await
	{
		tracing::error!(error = %e, "Failed to write README.txt to zip");
		return HttpResponse::InternalServerError().finish();
	}

	if let Err(e) = writer.close().await {
		tracing::error!(error = %e, "Failed to close zip writer");
		return HttpResponse::InternalServerError().finish();
	}

	if let Err(e) = manager.db.update_last_data_request(&tidal_user_id).await {
		tracing::error!(
			user = %tidal_user_id,
			error = %e,
			"Failed to update last_data_request for user"
		);
	}

	HttpResponse::Ok()
		.content_type("application/zip")
		.append_header((
			"Content-Disposition",
			format!(
				"attachment; filename=\"tss_data_{}_on_{}.zip\"",
				username, host
			),
		))
		.body(buffer)
}
