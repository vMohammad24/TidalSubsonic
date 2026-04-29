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

#[derive(Clone, Deserialize, Debug)]
pub struct SubsonicQuery {
	pub u: String,
	pub p: Option<String>,
	pub t: Option<String>,
	pub s: Option<String>,
	pub f: Option<String>,
}

#[derive(Clone)]
pub struct SubsonicContext {
	pub user: String,
	pub format: String,
	pub tidal_api: TidalApi,
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
		let auth_res = serde_urlencoded::from_str::<SubsonicQuery>(query_string);

		match auth_res {
			Ok(query) => {
				let mut format = query
					.f
					.clone()
					.unwrap_or_else(|| "xml".to_string())
					.to_lowercase();
				if format != "json" && format != "jsonp" {
					format = "xml".to_string();
				}

				let db = req
					.app_data::<web::Data<Arc<DbManager>>>()
					.map(|d| d.as_ref().clone());
				let manager = req
					.app_data::<web::Data<Arc<TidalClientManager>>>()
					.map(|d| d.as_ref().clone());
				let username = query.u.clone();
				let t = query.t.clone();
				let s = query.s.clone();
				let p = query.p.clone();
				let format = format.clone();

				if path.ends_with("/getCoverArt.view")
					|| path.ends_with("/getCoverArt")
					|| path.ends_with("/ping.view")
					|| path.ends_with("/ping")
				{
					return Box::pin(async move { service.call(req).await });
				}

				Box::pin(async move {
					let mut authenticated = false;

					if let Some(db) = (db)
						&& let Ok(Some((_, enc_password, _, _))) =
							db.get_user_details(&username).await
						&& let Ok(plain_password) = crypto::decrypt_string(&enc_password)
					{
						if let (Some(t), Some(s)) = (&t, &s) {
							let token_expected =
								format!("{:x}", md5::compute(format!("{}{}", plain_password, s)));
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
						let err = ErrorUnauthorized("Authentication required");
						return Err(err);
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
					});

					service.call(req).await
				})
			}
			Err(_) => Box::pin(async move {
				let err = ErrorUnauthorized("Invalid request parameters");
				Err(err)
			}),
		}
	}
}
