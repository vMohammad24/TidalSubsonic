use crate::api::subsonic::models::SubsonicResponseWrapper;
use crate::api::subsonic::response::SubsonicResponder;
use actix_web::Responder;

pub async fn ping() -> impl Responder {
	SubsonicResponder(SubsonicResponseWrapper::ok())
}

pub async fn get_license() -> impl Responder {
	SubsonicResponder(SubsonicResponseWrapper::ok())
}

pub async fn get_open_subsonic_extensions() -> impl Responder {
	SubsonicResponder(SubsonicResponseWrapper::ok())
}
