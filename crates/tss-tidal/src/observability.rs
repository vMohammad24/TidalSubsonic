pub trait TidalObserver: Send + Sync {
	fn request_started(&self, _api_version: &'static str, _method: &'static str) {}

	fn request_finished(
		&self,
		_api_version: &'static str,
		_method: &'static str,
		_outcome: &'static str,
		_duration_seconds: f64,
	) {
	}

	fn auth_refresh_finished(&self, _outcome: &'static str) {}

	fn cache_access(&self, _cache: &'static str, _result: &'static str, _entries: u64) {}
}

pub(crate) struct NoopTidalObserver;

impl TidalObserver for NoopTidalObserver {}
