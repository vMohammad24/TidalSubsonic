use actix_web::body::BoxBody;
use actix_web::http::header::ContentType;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder};
use quick_xml::se::to_string;
use std::collections::HashMap;

use crate::api::subsonic::middleware::SubsonicContext;
use crate::api::subsonic::models::SubsonicResponseWrapper;

pub struct SubsonicResponder(pub SubsonicResponseWrapper);

fn strip_at_prefix(val: &mut serde_json::Value) {
	match val {
		serde_json::Value::Object(map) => {
			let needs_stripping = map.keys().any(|k| k.starts_with('@'));
			if needs_stripping {
				let old_map = std::mem::take(map);
				for (k, mut v) in old_map {
					strip_at_prefix(&mut v);
					let new_key = if let Some(stripped) = k.strip_prefix('@') {
						stripped.to_string()
					} else {
						k
					};
					map.insert(new_key, v);
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
		let format = req
			.extensions()
			.get::<SubsonicContext>()
			.map(|ctx| ctx.format.clone())
			.unwrap_or_else(|| {
				serde_urlencoded::from_str::<HashMap<String, String>>(req.query_string())
					.ok()
					.and_then(|q| q.get("f").cloned())
					.unwrap_or_else(|| "xml".to_string())
			});

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
			let callback =
				serde_urlencoded::from_str::<HashMap<String, String>>(req.query_string())
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
