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

fn strip_at_prefix(val: &mut serde_json::Value) {
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
