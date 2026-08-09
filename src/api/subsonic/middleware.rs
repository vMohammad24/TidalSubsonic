use crate::db::DbManager;
use crate::tidal::{api::TidalApi, manager::TidalClientManager};
use crate::util::crypto;
use actix_web::error::ErrorUnauthorized;
use actix_web::{
	HttpMessage,
	dev::{Service, ServiceRequest, Transform, forward_ready},
	web,
};
use futures_util::future::LocalBoxFuture;
use md5;
use serde::Deserialize;
use std::future::{Ready, ready};
use std::sync::Arc;
use tracing::Instrument;
use validator::Validate;

#[derive(Clone, Deserialize, Debug, Validate)]
pub struct SubsonicQuery {
	#[validate(length(min = 1, max = 128))]
	pub u: String,
	#[validate(length(min = 1, max = 256))]
	pub p: Option<String>,
	#[validate(length(min = 1, max = 128))]
	pub t: Option<String>,
	#[validate(length(min = 1, max = 128))]
	pub s: Option<String>,
	pub f: Option<String>,
}

#[derive(Clone)]
pub struct SubsonicContext {
	pub user: String,
	pub format: String,
	pub tidal_api: TidalApi,
	pub use_playlists: bool,
	pub use_favorites: bool,
}

impl SubsonicContext {
	pub fn get_starred_date(&self, item_id: i64) -> Option<String> {
		if self.use_favorites {
			crate::tidal::favorites::get_local_favorite_date(&self.user, item_id)
		} else {
			self.tidal_api
				.user_id()
				.and_then(|uid| crate::tidal::favorites::get_favorite_date(uid, item_id))
		}
	}
}

pub struct SubsonicAuth;

impl<S, B> Transform<S, ServiceRequest> for SubsonicAuth
where
	S: Service<
			ServiceRequest,
			Response = actix_web::dev::ServiceResponse<B>,
			Error = actix_web::Error,
		> + 'static,
	S::Future: 'static,
	B: 'static,
{
	type Response = actix_web::dev::ServiceResponse<B>;
	type Error = actix_web::Error;
	type InitError = ();
	type Transform = SubsonicAuthMiddleware<S>;
	type Future = Ready<Result<Self::Transform, Self::InitError>>;

	fn new_transform(&self, service: S) -> Self::Future {
		ready(Ok(SubsonicAuthMiddleware {
			service: Arc::new(service),
		}))
	}
}

pub struct SubsonicAuthMiddleware<S> {
	service: Arc<S>,
}

impl<S, B> Service<ServiceRequest> for SubsonicAuthMiddleware<S>
where
	S: Service<
			ServiceRequest,
			Response = actix_web::dev::ServiceResponse<B>,
			Error = actix_web::Error,
		> + 'static,
	S::Future: 'static,
	B: 'static,
{
	type Response = actix_web::dev::ServiceResponse<B>;
	type Error = actix_web::Error;
	type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

	forward_ready!(service);

	fn call(&self, req: ServiceRequest) -> Self::Future {
		let service = self.service.clone();

		let path = req.path();
		let query_string = req.query_string();
		let auth_res = serde_qs::from_str::<SubsonicQuery>(query_string);

		match auth_res {
			Ok(query) => {
				if query.validate().is_err() {
					return Box::pin(async move {
						Err(ErrorUnauthorized("Invalid request parameters"))
					});
				}
				let SubsonicQuery {
					u: username,
					p,
					t,
					s,
					f,
				} = query;
				let format = match f.as_deref() {
					Some(fmt) if fmt.eq_ignore_ascii_case("json") => "json".to_string(),
					Some(fmt) if fmt.eq_ignore_ascii_case("jsonp") => "jsonp".to_string(),
					_ => "xml".to_string(),
				};

				let db = req
					.app_data::<web::Data<DbManager>>()
					.map(|d| d.get_ref().clone());
				let manager = req
					.app_data::<web::Data<TidalClientManager>>()
					.map(|d| d.get_ref().clone());

				if path.ends_with("/getCoverArt.view")
					|| path.ends_with("/getCoverArt")
					|| path.ends_with("/ping.view")
					|| path.ends_with("/ping")
				{
					return Box::pin(async move { service.call(req).await });
				}

				let span = tracing::info_span!(
					"subsonic_request",
					username = %username,
					format = %format,
					path = %path
				);

				Box::pin(
					async move {
						let mut authenticated = false;
						let mut use_playlists = true;
						let mut use_favorites = true;

						if let Some(ref db) = (db)
							&& let Ok(Some((_, enc_password, up, uf, _))) =
								db.get_user_details(&username).await
							&& let Ok(plain_password) = crypto::decrypt_string(&enc_password)
						{
							use_playlists = up;
							use_favorites = uf;
							if let (Some(t), Some(s)) = (&t, &s) {
								let token_expected = format!(
									"{:x}",
									md5::compute(format!("{}{}", plain_password, s))
								);
								if token_expected == *t {
									authenticated = true;
								}
							} else if let Some(p) = &p
								&& plain_password == *p
							{
								authenticated = true;
							}
						}

						if !authenticated {
							tracing::warn!(username = %username, "Subsonic authentication failure");
							let err = ErrorUnauthorized("Authentication required");
							return Err(err);
						}

						if use_favorites
							&& let Some(db) = &db && crate::tidal::favorites::LOCAL_FAVORITE_CACHE
							.get(&username)
							.is_none() && let Ok(favs) =
							db.get_all_local_favorites_map(&username).await
						{
							crate::tidal::favorites::set_local_favorites_map(&username, favs);
						}

						let tidal_api = if let Some(manager) = manager {
							match manager.get_client_for_subsonic_user(&username).await {
								Ok(session) => TidalApi::new(session),
								Err(e) => {
									let err = ErrorUnauthorized(e.to_string());
									return Err(err);
								}
							}
						} else {
							let err = ErrorUnauthorized("Tidal client manager not available");
							return Err(err);
						};

						req.extensions_mut().insert(SubsonicContext {
							user: username,
							format,
							tidal_api,
							use_playlists,
							use_favorites,
						});

						service.call(req).await
					}
					.instrument(span),
				)
			}
			Err(_) => Box::pin(async move {
				let err = ErrorUnauthorized("Invalid request parameters");
				Err(err)
			}),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use actix_web::{App, HttpResponse, test, web};

	async fn dummy_handler() -> HttpResponse {
		HttpResponse::Ok().body("OK")
	}

	#[actix_web::test]
	async fn test_unauthenticated_request_missing_credentials() {
		let app = test::init_service(
			App::new()
				.wrap(SubsonicAuth)
				.route("/rest/getMusicFolders", web::get().to(dummy_handler)),
		)
		.await;

		let req = test::TestRequest::get()
			.uri("/rest/getMusicFolders")
			.to_request();
		let res = test::try_call_service(&app, req).await;
		assert!(res.is_err());
		let err = res.unwrap_err();
		assert_eq!(
			err.error_response().status(),
			actix_web::http::StatusCode::UNAUTHORIZED
		);
	}

	#[actix_web::test]
	async fn test_malformed_credentials_does_not_leak_errors() {
		let app = test::init_service(
			App::new()
				.wrap(SubsonicAuth)
				.route("/rest/getMusicFolders", web::get().to(dummy_handler)),
		)
		.await;

		let req = test::TestRequest::get()
			.uri("/rest/getMusicFolders?u=invalid_user&p=invalid_pass&v=1.16.1&c=test")
			.to_request();
		let res = test::try_call_service(&app, req).await;
		assert!(res.is_err());
		let err = res.unwrap_err();
		assert_eq!(
			err.error_response().status(),
			actix_web::http::StatusCode::UNAUTHORIZED
		);

		let err_str = err.to_string();
		assert!(!err_str.contains("postgres"));
		assert!(!err_str.contains("SELECT"));
		assert!(!err_str.contains("sqlx"));
	}

	#[actix_web::test]
	async fn test_ping_and_cover_art_bypass_authentication() {
		let app = test::init_service(
			App::new()
				.wrap(SubsonicAuth)
				.route("/rest/ping.view", web::get().to(dummy_handler))
				.route("/rest/getCoverArt.view", web::get().to(dummy_handler)),
		)
		.await;

		let req = test::TestRequest::get()
			.uri("/rest/ping.view?u=guest&p=pass&v=1.16.1&c=test")
			.to_request();
		let res = test::call_service(&app, req).await;
		assert_eq!(res.status(), actix_web::http::StatusCode::OK);

		let req_cover = test::TestRequest::get()
			.uri("/rest/getCoverArt.view?u=guest&p=pass&v=1.16.1&c=test")
			.to_request();
		let res_cover = test::call_service(&app, req_cover).await;
		assert_eq!(res_cover.status(), actix_web::http::StatusCode::OK);
	}

	#[actix_web::test]
	async fn test_subsonic_md5_token_calculation() {
		let password = "my_subsonic_password";
		let salt = "c1b2a3";
		let expected_token = format!("{:x}", md5::compute(format!("{}{}", password, salt)));

		let test_token = format!("{:x}", md5::compute("my_subsonic_passwordc1b2a3"));
		assert_eq!(expected_token, test_token);
	}

	#[actix_web::test]
	async fn test_subsonic_auth_rejects_invalid_token() {
		let app = test::init_service(
			App::new()
				.wrap(SubsonicAuth)
				.route("/rest/getArtists", web::get().to(dummy_handler)),
		)
		.await;

		let req = test::TestRequest::get()
			.uri("/rest/getArtists?u=user1&t=invalid_token_12345&s=salt123&v=1.16.1&c=test")
			.to_request();

		let res = test::try_call_service(&app, req).await;
		assert!(res.is_err());
		let err = res.unwrap_err();
		assert_eq!(
			err.error_response().status(),
			actix_web::http::StatusCode::UNAUTHORIZED
		);
	}
}
