use actix_web::{FromRequest, HttpRequest, dev::Payload, error::ErrorBadRequest};
use reqwest::Client;
use serde::Deserializer;
use serde::de::{self, DeserializeOwned, Visitor};
use std::fmt;
use std::future::{Ready, ready};
use std::ops::Deref;
use std::sync::LazyLock;
use std::time::Duration;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
	let builder = Client::builder().timeout(Duration::from_secs(30));

	#[cfg(debug_assertions)]
	let builder = builder.danger_accept_invalid_certs(true); // used for debugging

	builder.build().unwrap_or_default()
});

pub fn http_client() -> Client {
	HTTP_CLIENT.clone()
}

pub fn deserialize_list<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
	D: Deserializer<'de>,
{
	struct ListVisitor;

	impl<'de> Visitor<'de> for ListVisitor {
		type Value = Option<Vec<String>>;

		fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
			formatter.write_str("a string or a sequence of strings")
		}

		fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
		where
			E: de::Error,
		{
			let res: Vec<String> = v
				.split(',')
				.map(str::trim)
				.filter(|s| !s.is_empty())
				.map(String::from)
				.collect();

			Ok(if res.is_empty() { None } else { Some(res) })
		}

		fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
		where
			A: de::SeqAccess<'de>,
		{
			let mut res = Vec::with_capacity(seq.size_hint().unwrap_or(0));

			while let Some(value) = seq.next_element::<String>()? {
				res.extend(
					value
						.split(',')
						.map(str::trim)
						.filter(|s| !s.is_empty())
						.map(String::from),
				);
			}

			Ok(if res.is_empty() { None } else { Some(res) })
		}

		fn visit_none<E>(self) -> Result<Self::Value, E>
		where
			E: de::Error,
		{
			Ok(None)
		}

		fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
		where
			D: Deserializer<'de>,
		{
			deserializer.deserialize_any(self)
		}
	}

	deserializer.deserialize_any(ListVisitor)
}

pub struct QsQuery<T>(pub T);

impl<T> Deref for QsQuery<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> FromRequest for QsQuery<T>
where
	T: DeserializeOwned,
{
	type Error = actix_web::Error;
	type Future = Ready<Result<Self, Self::Error>>;

	fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
		let query = req.query_string();

		match serde_qs::from_str::<T>(query) {
			Ok(val) => ready(Ok(QsQuery(val))),
			Err(e) => {
				tracing::error!(%e, %query, "Query deserialize error");
				ready(Err(ErrorBadRequest(format!(
					"Query deserialize error: {e}"
				))))
			}
		}
	}
}
