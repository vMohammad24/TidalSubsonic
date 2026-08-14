use reqwest::Client;
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
