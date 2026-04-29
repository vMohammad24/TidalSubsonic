use actix_web::{HttpRequest, HttpResponse, Responder, web};
use askama::Template;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct HomeQuery {
	pub logged_out: Option<String>,
	pub account_deleted: Option<String>,
	pub error: Option<String>,
	pub success: Option<String>,
}

pub enum StatusMessage {
	Success(String),
	Error(String),
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
	pub is_authenticated: bool,
	pub tidal_username: String,
	pub status_message: Option<StatusMessage>,
}

pub async fn home(
	query: web::Query<HomeQuery>,
	req: HttpRequest,
	manager: web::Data<std::sync::Arc<crate::tidal::manager::TidalClientManager>>,
) -> impl Responder {
	let mut is_authenticated = false;
	let mut tidal_username = "".to_string();

	if let Some(cookie) = req.cookie("tidal_subsonic_wsid") {
		let session_id = cookie.value();
		if let Ok(Some((_tidal_user_id, username))) = manager.db.get_web_session(session_id).await {
			is_authenticated = true;
			tidal_username = username;
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
		None
	};

	let template = IndexTemplate {
		is_authenticated,
		tidal_username,
		status_message,
	};
	match template.render() {
		Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
		Err(e) => {
			tracing::error!("Template error: {}", e);
			HttpResponse::InternalServerError().body("Internal Server Error")
		}
	}
}

pub async fn login_js() -> impl Responder {
	let js = include_str!("../../templates/login.js");
	HttpResponse::Ok()
		.content_type("application/javascript")
		.insert_header(("Cache-Control", "max-age=3600"))
		.body(js)
}

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.service(web::resource("/").route(web::get().to(home)))
		.service(web::resource("/views/login.js").route(web::get().to(login_js)));
}
