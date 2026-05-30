use actix_session::Session;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use askama::Template;
use serde::Deserialize;
use std::sync::Arc;

use crate::tidal::manager::TidalClientManager;
use crate::util::crypto;
use crate::util::session::{
	StatusMessage, UserInfo, extract_user_id, get_flash, get_users_info, set_flash,
};

#[derive(Deserialize)]
pub struct HomeQuery {
	pub logged_out: Option<String>,
	pub account_deleted: Option<String>,
	pub error: Option<String>,
	pub success: Option<String>,
}

#[derive(Deserialize)]
struct CreateUserForm {
	username: String,
	password: String,
}

#[derive(Deserialize)]
struct DeleteUserForm {
	username: String,
}

#[derive(Deserialize)]
struct UpdateFeaturesForm {
	username: String,
	#[serde(default)]
	use_playlists: Option<String>,
	#[serde(default)]
	use_favorites: Option<String>,
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
	pub is_authenticated: bool,
	pub tidal_username: String,
	pub status_message: Option<StatusMessage>,
	pub server_url: String,
	pub users: Vec<UserInfo>,
}

#[derive(Template)]
#[template(path = "users_list.html")]
pub struct UsersListTemplate {
	pub users: Vec<UserInfo>,
}

async fn render_user_update(
	req: &HttpRequest,
	session: &Session,
	manager: &TidalClientManager,
	tidal_user_id: &str,
	status: Option<StatusMessage>,
) -> HttpResponse {
	let status_code = match status {
		Some(StatusMessage::Error(_)) => actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
		_ => actix_web::http::StatusCode::OK,
	};

	if req.headers().contains_key("HX-Request") {
		let users = get_users_info(tidal_user_id, &manager.db).await;
		let template = UsersListTemplate { users };
		match template.render() {
			Ok(html) => {
				let mut response_body = html;
				if let Some(s) = status {
					let oob_notification = match s {
						StatusMessage::Success(m) => format!(
							r#"<div id="notification" hx-swap-oob="true" class="notification success">{}</div>"#,
							m
						),
						StatusMessage::Error(m) => format!(
							r#"<div id="notification" hx-swap-oob="true" class="notification error">Error: {}</div>"#,
							m
						),
					};
					response_body.push_str(&oob_notification);
				} else {
					response_body.push_str(r#"<div id="notification" hx-swap-oob="true"></div>"#);
				}

				HttpResponse::build(status_code)
					.content_type("text/html")
					.body(response_body)
			}
			Err(e) => {
				tracing::error!("Template error: {}", e);
				HttpResponse::InternalServerError().body("Internal Server Error")
			}
		}
	} else {
		if let Some(s) = status {
			match s {
				StatusMessage::Success(m) => set_flash(session, "success", &m),
				StatusMessage::Error(m) => set_flash(session, "error", &m),
			}
		}
		HttpResponse::SeeOther()
			.append_header(("Location", "/"))
			.finish()
	}
}

pub async fn home(
	query: web::Query<HomeQuery>,
	req: HttpRequest,
	session: Session,
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let mut is_authenticated = false;
	let mut tidal_username = "".to_string();
	let mut tidal_user_id = None;

	if let Some(cookie) = req.cookie("tidal_subsonic_wsid") {
		let session_id = cookie.value();
		if let Ok(Some((uid, username))) = manager.db.get_web_session(session_id).await {
			is_authenticated = true;
			tidal_username = username;
			tidal_user_id = Some(uid);
		}
	}

	let status_message = if let Some(err) = &query.error {
		Some(StatusMessage::Error(err.clone()))
	} else if let Some(succ) = &query.success {
		Some(StatusMessage::Success(succ.clone()))
	} else if query.logged_out.is_some() {
		Some(StatusMessage::Success(
			"You have been logged out successfully.".to_string(),
		))
	} else if query.account_deleted.is_some() {
		Some(StatusMessage::Success(
			"Your account has been deleted successfully.".to_string(),
		))
	} else {
		get_flash(&session)
	};

	let server_url = format!(
		"{}://{}",
		req.connection_info().scheme(),
		req.connection_info().host()
	);

	let mut users = Vec::new();
	if let Some(ref uid) = tidal_user_id {
		users = get_users_info(uid, &manager.db).await;
	}

	let template = IndexTemplate {
		is_authenticated,
		tidal_username,
		status_message,
		server_url,
		users,
	};
	match template.render() {
		Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
		Err(e) => {
			tracing::error!("Template error: {}", e);
			HttpResponse::InternalServerError().body("Internal Server Error")
		}
	}
}

async fn create_user_form(
	form: web::Form<CreateUserForm>,
	req: HttpRequest,
	session: Session,
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let tidal_user_id = match extract_user_id(&req, &manager).await {
		Some(id) => id,
		None => return HttpResponse::Unauthorized().finish(),
	};

	let status = if form.username.trim().is_empty() || form.password.is_empty() {
		Some(StatusMessage::Error(
			"Username and password are required".into(),
		))
	} else {
		match crypto::encrypt_string(&form.password) {
			Ok(encrypted_password) => {
				if manager
					.db
					.create_user(
						&form.username,
						&tidal_user_id,
						Some(&encrypted_password),
						true,
						true,
					)
					.await
					.is_ok()
				{
					Some(StatusMessage::Success("User created successfully".into()))
				} else {
					Some(StatusMessage::Error("Username already exists".into()))
				}
			}
			Err(e) => {
				tracing::error!("Failed to encrypt user password: {}", e);
				Some(StatusMessage::Error("Failed to create user".into()))
			}
		}
	};

	render_user_update(&req, &session, &manager, &tidal_user_id, status).await
}

async fn delete_user_form(
	form: web::Form<DeleteUserForm>,
	req: HttpRequest,
	session: Session,
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let tidal_user_id = match extract_user_id(&req, &manager).await {
		Some(id) => id,
		None => return HttpResponse::Unauthorized().finish(),
	};

	let user_data = manager.db.get_user_details(&form.username).await;
	let status = match user_data {
		Ok(Some((user_tidal_id, _, _, _))) if user_tidal_id == tidal_user_id => {
			if manager.db.delete_user(&form.username).await.is_ok() {
				Some(StatusMessage::Success("User deleted successfully".into()))
			} else {
				Some(StatusMessage::Error("Failed to delete user".into()))
			}
		}
		_ => Some(StatusMessage::Error("User not found".into())),
	};

	render_user_update(&req, &session, &manager, &tidal_user_id, status).await
}

async fn update_features_form(
	form: web::Form<UpdateFeaturesForm>,
	req: HttpRequest,
	session: Session,
	manager: web::Data<Arc<TidalClientManager>>,
) -> impl Responder {
	let tidal_user_id = match extract_user_id(&req, &manager).await {
		Some(id) => id,
		None => return HttpResponse::Unauthorized().finish(),
	};

	let user_data = manager.db.get_user_details(&form.username).await;
	let status = match user_data {
		Ok(Some((user_tidal_id, _, _, _))) if user_tidal_id == tidal_user_id => {
			let use_playlists = form.use_playlists.as_deref() == Some("true");
			let use_favorites = form.use_favorites.as_deref() == Some("true");

			if manager
				.db
				.update_user_feature_flags(&form.username, use_playlists, use_favorites)
				.await
				.unwrap_or(false)
			{
				Some(StatusMessage::Success(format!(
					"Features updated for \"{}\"",
					form.username
				)))
			} else {
				Some(StatusMessage::Error("Failed to update features".into()))
			}
		}
		_ => Some(StatusMessage::Error("User not found".into())),
	};

	render_user_update(&req, &session, &manager, &tidal_user_id, status).await
}

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.service(web::resource("/").route(web::get().to(home)))
		.route("/users/create", web::post().to(create_user_form))
		.route("/users/delete", web::post().to(delete_user_form))
		.route("/users/features", web::post().to(update_features_form));
}
