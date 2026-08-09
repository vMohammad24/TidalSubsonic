use actix_web::body::BoxBody;
use actix_web::http::header::ContentType;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder};
use quick_xml::se::to_string;
use std::collections::HashMap;

use crate::api::subsonic::middleware::SubsonicContext;
use crate::api::subsonic::models::SubsonicResponseWrapper;

use crate::api::error::AppError;

pub struct SubsonicResponder(pub SubsonicResponseWrapper);

pub struct ApiResult(pub Result<SubsonicResponseWrapper, AppError>);

impl ApiResult {
	pub fn from_result<E: Into<AppError>>(res: Result<SubsonicResponseWrapper, E>) -> Self {
		ApiResult(res.map_err(Into::into))
	}
}

impl<E: Into<AppError>> From<Result<SubsonicResponseWrapper, E>> for ApiResult {
	fn from(res: Result<SubsonicResponseWrapper, E>) -> Self {
		ApiResult(res.map_err(Into::into))
	}
}

impl Responder for ApiResult {
	type Body = BoxBody;

	fn respond_to(self, req: &HttpRequest) -> HttpResponse<Self::Body> {
		match self.0 {
			Ok(wrapper) => SubsonicResponder(wrapper).respond_to(req),
			Err(err) => {
				let (code, msg) = err.into_subsonic_code_and_msg();
				let error_wrapper = SubsonicResponseWrapper::error(code, &msg);
				SubsonicResponder(error_wrapper).respond_to(req)
			}
		}
	}
}

pub(super) fn strip_at_prefix(val: &mut serde_json::Value) {
	match val {
		serde_json::Value::Object(map) => {
			if map.keys().any(|k| k.starts_with('@')) {
				let old_map = std::mem::take(map);
				for (mut k, mut v) in old_map {
					strip_at_prefix(&mut v);
					if k.starts_with('@') {
						k.remove(0);
					}
					map.insert(k, v);
				}
			} else {
				for v in map.values_mut() {
					strip_at_prefix(v);
				}
			}
		}
		serde_json::Value::Array(arr) => {
			for v in arr {
				strip_at_prefix(v);
			}
		}
		_ => {}
	}
}

impl Responder for SubsonicResponder {
	type Body = BoxBody;

	fn respond_to(self, req: &HttpRequest) -> HttpResponse<Self::Body> {
		let extensions = req.extensions();
		let format_owned;
		let format: &str = if let Some(ctx) = extensions.get::<SubsonicContext>() {
			ctx.format.as_str()
		} else {
			#[derive(serde::Deserialize)]
			struct FormatQuery<'a> {
				f: Option<&'a str>,
			}
			format_owned = serde_qs::from_str::<FormatQuery>(req.query_string())
				.ok()
				.and_then(|q| q.f.map(String::from))
				.unwrap_or_else(|| "xml".to_string());
			&format_owned
		};

		if format == "xml" {
			return match to_string(&self.0.response) {
				Ok(xml_str) => HttpResponse::Ok()
					.content_type(ContentType::xml())
					.body(format!(
						"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}",
						xml_str
					)),
				Err(e) => HttpResponse::InternalServerError()
					.content_type(ContentType::plaintext())
					.body(format!("XML Serialization Error: {}", e)),
			};
		}

		let mut val = match serde_json::to_value(&self.0) {
			Ok(v) => v,
			Err(e) => {
				return HttpResponse::InternalServerError()
					.content_type(ContentType::plaintext())
					.body(format!("JSON Serialization Error: {}", e));
			}
		};

		strip_at_prefix(&mut val);

		let json_str = match serde_json::to_string(&val) {
			Ok(s) => s,
			Err(e) => {
				return HttpResponse::InternalServerError()
					.content_type(ContentType::plaintext())
					.body(format!("JSON Stringify Error: {}", e));
			}
		};

		if format == "jsonp" {
			let callback = serde_qs::from_str::<HashMap<String, String>>(req.query_string())
				.ok()
				.and_then(|mut q| q.remove("callback"))
				.unwrap_or_else(|| "subsonicCallback".to_string());

			HttpResponse::Ok()
				.content_type("application/javascript")
				.body(format!("{}({});", callback, json_str))
		} else {
			HttpResponse::Ok()
				.content_type(ContentType::json())
				.body(json_str)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use actix_web::body::to_bytes;
	use actix_web::test;

	#[actix_web::test]
	async fn test_strip_at_prefix_nested() {
		let mut json_val = serde_json::json!({
			"@status": "ok",
			"@version": "1.16.1",
			"nested": {
				"@id": "123",
				"name": "test",
				"items": [
					{ "@item_id": "456" }
				]
			}
		});

		strip_at_prefix(&mut json_val);

		let expected = serde_json::json!({
			"status": "ok",
			"version": "1.16.1",
			"nested": {
				"id": "123",
				"name": "test",
				"items": [
					{ "item_id": "456" }
				]
			}
		});

		assert_eq!(json_val, expected);
	}

	#[actix_web::test]
	async fn test_subsonic_responder_xml_output() {
		let wrapper = SubsonicResponseWrapper::ok();
		let req = test::TestRequest::get()
			.uri("/rest/ping?f=xml")
			.to_http_request();

		let resp = SubsonicResponder(wrapper).respond_to(&req);
		assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

		let body_bytes = to_bytes(resp.into_body()).await.unwrap();
		let body_str = String::from_utf8_lossy(&body_bytes);
		assert!(body_str.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
		assert!(body_str.contains("<subsonic-response"));
		assert!(body_str.contains("status=\"ok\""));
	}

	#[actix_web::test]
	async fn test_subsonic_responder_json_output() {
		let wrapper = SubsonicResponseWrapper::ok();
		let req = test::TestRequest::get()
			.uri("/rest/ping?f=json")
			.to_http_request();

		let resp = SubsonicResponder(wrapper).respond_to(&req);
		assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

		let body_bytes = to_bytes(resp.into_body()).await.unwrap();
		let body_str = String::from_utf8_lossy(&body_bytes);
		assert!(body_str.contains("\"subsonic-response\""));
		assert!(body_str.contains("\"status\":\"ok\""));
		// verify @ was stripped
		assert!(!body_str.contains("\"@status\""));
	}

	#[actix_web::test]
	async fn test_subsonic_responder_jsonp_output() {
		let wrapper = SubsonicResponseWrapper::ok();
		let req = test::TestRequest::get()
			.uri("/rest/ping?f=jsonp&callback=myCallback")
			.to_http_request();

		let resp = SubsonicResponder(wrapper).respond_to(&req);
		assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

		let body_bytes = to_bytes(resp.into_body()).await.unwrap();
		let body_str = String::from_utf8_lossy(&body_bytes);
		assert!(body_str.starts_with("myCallback("));
		assert!(body_str.ends_with(");"));
		assert!(body_str.contains("\"subsonic-response\""));
	}

	#[actix_web::test]
	async fn test_subsonic_error_response_wrapper() {
		let err_wrapper = SubsonicResponseWrapper::error(70, "Resource not found");
		assert_eq!(err_wrapper.response.status, "failed");
		assert!(err_wrapper.response.error.is_some());

		let err = err_wrapper.response.error.unwrap();
		assert_eq!(err.code, 70);
		assert_eq!(err.message, "Resource not found");
	}

	#[actix_web::test]
	async fn test_subsonic_indexes_and_search_xml_attribute_serialization() {
		use crate::api::subsonic::models::{Index, Indexes, SearchResult3};

		let mut wrapper = SubsonicResponseWrapper::ok();
		wrapper.response.indexes = Some(Indexes {
			last_modified: 1600000000000,
			ignored_articles: Some("The A An".to_string()),
			index: vec![Index {
				name: "D".to_string(),
				artist: vec![],
			}],
		});
		wrapper.response.search_result3 = Some(SearchResult3 {
			artist: None,
			album: None,
			song: None,
		});

		let req = test::TestRequest::get()
			.uri("/rest/getIndexes?f=xml")
			.to_http_request();
		let resp = SubsonicResponder(wrapper).respond_to(&req);
		assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

		let body_bytes = to_bytes(resp.into_body()).await.unwrap();
		let body_str = String::from_utf8_lossy(&body_bytes);
		assert!(body_str.contains("<indexes lastModified=\"1600000000000\""));
		assert!(body_str.contains("<searchResult3"));
	}
}
