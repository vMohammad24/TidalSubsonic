use crate::tidal::manager::TidalClientManager;
use crate::util::crypto;
use crate::util::session::extract_user_id;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use serde::{Deserialize, Serialize};
use validator::Validate;

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.route("/api/users", web::get().to(get_users))
		.route("/api/users", web::post().to(create_user))
		.route("/api/users/features", web::post().to(update_features))
		.route("/api/users/delete", web::post().to(delete_user));
}

#[derive(Serialize)]
struct UserResponse {
	username: String,
	#[serde(rename = "lastFmUsername")]
	lastfm_username: Option<String>,
	#[serde(rename = "usePlaylists")]
	use_playlists: bool,
	#[serde(rename = "useFavorites")]
	use_favorites: bool,
	scrobbles: i64,
}

#[derive(Deserialize, Validate)]
struct CreateUserReq {
	#[validate(length(min = 1, max = 64))]
	username: Option<String>,
	#[validate(length(min = 1, max = 128))]
	password: Option<String>,
}

#[derive(Deserialize)]
struct UpdateFeaturesReq {
	username: Option<String>,
	feature: Option<String>,
	enabled: Option<bool>,
}

#[derive(Deserialize)]
struct DeleteUserReq {
	username: Option<String>,
}

async fn get_users(req: HttpRequest, manager: web::Data<TidalClientManager>) -> impl Responder {
	let Some(tidal_user_id) = extract_user_id(&req, &manager).await else {
		return HttpResponse::Unauthorized()
			.json(serde_json::json!({ "error": "User ID not found" }));
	};

	let users = crate::util::session::get_users_info(&tidal_user_id, &manager.db).await;
	let users_with_details: Vec<_> = users
		.into_iter()
		.map(|u| UserResponse {
			username: u.username,
			lastfm_username: u.lastfm_username,
			use_playlists: u.use_playlists,
			use_favorites: u.use_favorites,
			scrobbles: u.scrobbles,
		})
		.collect();

	HttpResponse::Ok().json(serde_json::json!({ "users": users_with_details }))
}

use crate::util::rate_limit::RateLimiter;
use std::sync::LazyLock;

static CREATE_USER_LIMITER: LazyLock<RateLimiter> = LazyLock::new(|| RateLimiter::new(10, 60));

async fn create_user(
	req_body: web::Json<CreateUserReq>,
	req: HttpRequest,
	manager: web::Data<TidalClientManager>,
) -> impl Responder {
	let client_ip = req
		.connection_info()
		.realip_remote_addr()
		.unwrap_or("unknown")
		.to_string();
	if !CREATE_USER_LIMITER.check_and_increment(&client_ip) {
		return HttpResponse::TooManyRequests().json(serde_json::json!({
			"error": "Rate limit exceeded"
		}));
	}
	if let Err(errors) = req_body.validate() {
		return HttpResponse::BadRequest().json(serde_json::json!({
			"error": "Input validation failed",
			"details": errors.to_string()
		}));
	}
	let Some(tidal_user_id) = extract_user_id(&req, &manager).await else {
		return HttpResponse::Unauthorized()
			.json(serde_json::json!({ "error": "User ID not found" }));
	};

	let Some(username) = &req_body.username else {
		return HttpResponse::BadRequest()
			.json(serde_json::json!({ "error": "Username and password are required" }));
	};

	let Some(password) = &req_body.password else {
		return HttpResponse::BadRequest()
			.json(serde_json::json!({ "error": "Username and password are required" }));
	};

	let encrypted_password = match crypto::encrypt_string(password) {
		Ok(enc) => Some(enc),
		Err(e) => {
			tracing::error!(error = %e, "Failed to encrypt password");
			return HttpResponse::InternalServerError()
				.json(serde_json::json!({ "error": "Failed to encrypt password" }));
		}
	};

	if manager
		.db
		.create_user(
			username,
			&tidal_user_id,
			encrypted_password.as_deref(),
			true,
			true,
			true,
		)
		.await
		.is_err()
	{
		return HttpResponse::Conflict()
			.json(serde_json::json!({ "error": "Username already exists" }));
	}

	HttpResponse::Created().json(serde_json::json!({
		"success": true,
		"user": {
			"username": username,
			"tidalUserId": tidal_user_id,
			"usePlaylists": true,
			"useFavorites": true
		}
	}))
}

async fn update_features(
	req_body: web::Json<UpdateFeaturesReq>,
	req: HttpRequest,
	manager: web::Data<TidalClientManager>,
) -> impl Responder {
	let Some(tidal_user_id) = extract_user_id(&req, &manager).await else {
		return HttpResponse::Unauthorized()
			.json(serde_json::json!({ "error": "User ID not found" }));
	};

	let (Some(username), Some(feature), Some(enabled)) =
		(&req_body.username, &req_body.feature, req_body.enabled)
	else {
		return HttpResponse::BadRequest()
			.json(serde_json::json!({ "error": "Missing username, feature, or enabled flag" }));
	};

	if !manager
		.db
		.verify_user_ownership(username, &tidal_user_id)
		.await
		.unwrap_or(false)
	{
		return HttpResponse::Forbidden().json(
			serde_json::json!({ "error": "User not found or does not belong to this Tidal account" }),
		);
	}

	let (_, _, mut use_playlists, mut use_favorites, use_event_batch) =
		match manager.db.get_user_details(username).await {
			Ok(Some(d)) => d,
			_ => {
				return HttpResponse::Forbidden().json(
					serde_json::json!({ "error": "User not found or does not belong to this Tidal account" }),
				);
			}
		};

	if feature == "usePlaylists" {
		use_playlists = enabled;
	} else if feature == "useFavorites" {
		use_favorites = enabled;
	} else {
		return HttpResponse::BadRequest()
			.json(serde_json::json!({ "error": "Invalid feature flag" }));
	}

	let success = manager
		.db
		.update_user_feature_flags(username, use_playlists, use_favorites, use_event_batch)
		.await
		.unwrap_or(false);
	HttpResponse::Ok().json(serde_json::json!({ "success": success }))
}

async fn delete_user(
	req_body: web::Json<DeleteUserReq>,
	req: HttpRequest,
	manager: web::Data<TidalClientManager>,
) -> impl Responder {
	let Some(tidal_user_id) = extract_user_id(&req, &manager).await else {
		return HttpResponse::Unauthorized()
			.json(serde_json::json!({ "error": "User ID not found" }));
	};

	let Some(username) = &req_body.username else {
		return HttpResponse::BadRequest()
			.json(serde_json::json!({ "error": "Username is required" }));
	};

	if !manager
		.db
		.verify_user_ownership(username, &tidal_user_id)
		.await
		.unwrap_or(false)
	{
		return HttpResponse::Forbidden().json(
			serde_json::json!({ "error": "User not found or does not belong to this Tidal account" }),
		);
	}

	let success = manager.db.delete_user(username).await.is_ok();
	HttpResponse::Ok().json(serde_json::json!({ "success": success }))
}
