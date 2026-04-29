use crate::tidal::manager::TidalClientManager;
use crate::util::crypto;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
}

#[derive(Deserialize)]
struct CreateUserReq {
	username: Option<String>,
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

async fn extract_user_id(req: &HttpRequest, manager: &TidalClientManager) -> Option<String> {
	if let Some(cookie) = req.cookie("tidal_subsonic_wsid") {
		let session_id = cookie.value();
		if let Ok(Some((tidal_user_id, _username))) = manager.db.get_web_session(session_id).await {
			return Some(tidal_user_id);
		}
	}
	None
}

async fn get_users(
	req: HttpRequest,
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let tidal_user_id = match extract_user_id(&req, &manager).await {
		Some(id) => id,
		None => {
			return HttpResponse::Unauthorized()
				.json(serde_json::json!({ "error": "User ID not found" }));
		}
	};

	let usernames = match manager
		.db
		.list_users_for_tidal_account(&tidal_user_id)
		.await
	{
		Ok(u) => u,
		Err(_) => {
			return HttpResponse::InternalServerError()
				.json(serde_json::json!({"error": "DB Error"}));
		}
	};

	let mut users_with_details = Vec::new();
	for u in usernames {
		if let Ok(Some((_, _, use_playlists, use_favorites))) =
			manager.db.get_user_details(&u).await
		{
			let lastfm_username = manager
				.db
				.get_lastfm_details(&u)
				.await
				.ok()
				.flatten()
				.map(|(_, name)| name);
			users_with_details.push(UserResponse {
				username: u,
				lastfm_username,
				use_playlists,
				use_favorites,
			});
		}
	}

	HttpResponse::Ok().json(serde_json::json!({ "users": users_with_details }))
}

async fn create_user(
	req_body: web::Json<CreateUserReq>,
	req: HttpRequest,
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let tidal_user_id = match extract_user_id(&req, &manager).await {
		Some(id) => id,
		None => {
			return HttpResponse::Unauthorized()
				.json(serde_json::json!({ "error": "User ID not found" }));
		}
	};

	let username = match &req_body.username {
		Some(u) => u,
		None => {
			return HttpResponse::BadRequest()
				.json(serde_json::json!({ "error": "Username and password are required" }));
		}
	};

	let password = match &req_body.password {
		Some(p) => p,
		None => {
			return HttpResponse::BadRequest()
				.json(serde_json::json!({ "error": "Username and password are required" }));
		}
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
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let tidal_user_id = match extract_user_id(&req, &manager).await {
		Some(id) => id,
		None => {
			return HttpResponse::Unauthorized()
				.json(serde_json::json!({ "error": "User ID not found" }));
		}
	};

	let (username, feature, enabled) =
		match (&req_body.username, &req_body.feature, req_body.enabled) {
			(Some(u), Some(f), Some(e)) => (u, f, e),
			_ => {
				return HttpResponse::BadRequest().json(
					serde_json::json!({ "error": "Missing username, feature, or enabled flag" }),
				);
			}
		};

	let (user_tidal_id, _, mut use_playlists, mut use_favorites) =
		match manager.db.get_user_details(username).await {
			Ok(Some(d)) => d,
			_ => {
				return HttpResponse::Forbidden().json(
					serde_json::json!({ "error": "User not found or does not belong to this Tidal account" }),
				);
			}
		};

	if user_tidal_id != tidal_user_id {
		return HttpResponse::Forbidden().json(
			serde_json::json!({ "error": "User not found or does not belong to this Tidal account" }),
		);
	}

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
		.update_user_feature_flags(username, use_playlists, use_favorites)
		.await
		.unwrap_or(false);
	HttpResponse::Ok().json(serde_json::json!({ "success": success }))
}

async fn delete_user(
	req_body: web::Json<DeleteUserReq>,
	req: HttpRequest,
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let tidal_user_id = match extract_user_id(&req, &manager).await {
		Some(id) => id,
		None => {
			return HttpResponse::Unauthorized()
				.json(serde_json::json!({ "error": "User ID not found" }));
		}
	};

	let username = match &req_body.username {
		Some(u) => u,
		None => {
			return HttpResponse::BadRequest()
				.json(serde_json::json!({ "error": "Username is required" }));
		}
	};

	let (user_tidal_id, _, _, _) = match manager.db.get_user_details(username).await {
		Ok(Some(d)) => d,
		_ => {
			return HttpResponse::Forbidden().json(
				serde_json::json!({ "error": "User not found or does not belong to this Tidal account" }),
			);
		}
	};

	if user_tidal_id != tidal_user_id {
		return HttpResponse::Forbidden().json(
			serde_json::json!({ "error": "User not found or does not belong to this Tidal account" }),
		);
	}

	let success = manager.db.delete_user(username).await.is_ok();
	HttpResponse::Ok().json(serde_json::json!({ "success": success }))
}
