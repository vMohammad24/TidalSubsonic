use reqwest::Client;
use serde::{Deserialize, Deserializer};
use std::sync::LazyLock;
use std::time::Duration;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
	if cfg!(debug_assertions) {
		Client::builder()
			.timeout(Duration::from_secs(30))
			.danger_accept_invalid_certs(true) // used for debugging
			.build()
			.unwrap_or_default()
	} else {
		Client::builder()
			.timeout(Duration::from_secs(30))
			.build()
			.unwrap_or_default()
	}
});

pub fn http_client() -> Client {
	HTTP_CLIENT.clone()
}

pub fn deserialize_list<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
	D: Deserializer<'de>,
{
	let s = Option::<String>::deserialize(deserializer)?;

	Ok(s.filter(|s| !s.is_empty())
		.map(|s| s.split(',').map(|item| item.trim().to_string()).collect()))
}
