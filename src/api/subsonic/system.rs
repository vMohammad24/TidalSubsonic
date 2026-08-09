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

#[cfg(test)]
mod tests {
	use super::*;
	use actix_web::body::to_bytes;
	use actix_web::test;

	#[actix_web::test]
	async fn test_ping_endpoint_response() {
		let req = test::TestRequest::get().uri("/rest/ping").to_http_request();
		let resp = ping().await.respond_to(&req);
		assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

		let body_bytes = to_bytes(resp.into_body()).await.unwrap_or_default();
		let body_str = String::from_utf8_lossy(&body_bytes);
		assert!(body_str.contains("<subsonic-response"));
		assert!(body_str.contains("status=\"ok\""));
	}

	#[actix_web::test]
	async fn test_get_license_endpoint_response() {
		let req = test::TestRequest::get()
			.uri("/rest/getLicense")
			.to_http_request();
		let resp = get_license().await.respond_to(&req);
		assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

		let body_bytes = to_bytes(resp.into_body()).await.unwrap_or_default();
		let body_str = String::from_utf8_lossy(&body_bytes);
		assert!(body_str.contains("status=\"ok\""));
	}

	#[actix_web::test]
	async fn test_get_open_subsonic_extensions_response() {
		let req = test::TestRequest::get()
			.uri("/rest/getOpenSubsonicExtensions")
			.to_http_request();
		let resp = get_open_subsonic_extensions().await.respond_to(&req);
		assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

		let body_bytes = to_bytes(resp.into_body()).await.unwrap_or_default();
		let body_str = String::from_utf8_lossy(&body_bytes);
		assert!(body_str.contains("status=\"ok\""));
	}
}
