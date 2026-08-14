use actix_web::{HttpRequest, HttpResponse, web};
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry};
use prometheus::{Encoder, TextEncoder};
use tss_tidal::observability::TidalObserver;
use tss_tidal::session::Session;

macro_rules! register_metrics {
	($registry:expr; $($metric:expr),+ $(,)?) => {
		$(
			$registry.register(Box::new($metric.clone()))?;
		)+
	};
}

#[derive(Clone)]
pub struct Metrics {
	pub registry: Registry,
	enabled: bool,
	subsonic_auth_attempts: IntCounterVec,
	cache_accesses: IntCounterVec,
	tidal_clients_loaded: IntGauge,
	tidal_requests: IntCounterVec,
	tidal_request_duration: HistogramVec,
	tidal_errors: IntCounterVec,
	tidal_requests_in_flight: IntGaugeVec,
	tidal_auth_refreshes: IntCounterVec,
	playback_requests: IntCounterVec,
	playback_bytes: IntCounterVec,
	playback_active_streams: IntGauge,
	playback_startup_duration: HistogramVec,
	cache_entries: IntGaugeVec,
	cache_evictions: IntCounterVec,
}

impl Metrics {
	pub fn new(enabled: bool) -> Result<Self, prometheus::Error> {
		let registry = Registry::new();
		let subsonic_auth_attempts = IntCounterVec::new(
			Opts::new(
				"tss_subsonic_auth_attempts_total",
				"Subsonic authentication attempts by outcome",
			),
			&["outcome"],
		)?;
		let cache_accesses = IntCounterVec::new(
			Opts::new(
				"tss_cache_accesses_total",
				"Application cache accesses by cache and result",
			),
			&["cache", "result"],
		)?;
		let tidal_clients_loaded = IntGauge::new(
			"tss_tidal_clients_loaded",
			"Tidal user clients currently loaded in memory",
		)?;
		let tidal_requests = IntCounterVec::new(
			Opts::new("tss_tidal_requests_total", "Logical Tidal API requests by outcome"),
			&["api_version", "method", "outcome"],
		)?;
		let tidal_request_duration = HistogramVec::new(
			HistogramOpts::new(
				"tss_tidal_request_duration_seconds",
				"End-to-end Tidal API request latency including retries",
			),
			&["api_version", "method"],
		)?;
		let tidal_errors = IntCounterVec::new(
			Opts::new("tss_tidal_errors_total", "Tidal API request errors by category"),
			&["api_version", "error"],
		)?;
		let tidal_requests_in_flight = IntGaugeVec::new(
			Opts::new(
				"tss_tidal_requests_in_flight",
				"Tidal API requests currently in flight",
			),
			&["api_version"],
		)?;
		let tidal_auth_refreshes = IntCounterVec::new(
			Opts::new(
				"tss_tidal_auth_refreshes_total",
				"Tidal OAuth token refresh attempts by outcome",
			),
			&["outcome"],
		)?;
		let playback_requests = IntCounterVec::new(
			Opts::new(
				"tss_playback_requests_total",
				"Playback requests by delivery outcome",
			),
			&["outcome"],
		)?;
		let playback_bytes = IntCounterVec::new(
			Opts::new(
				"tss_playback_bytes_total",
				"Audio bytes proxied to clients by delivery type",
			),
			&["delivery"],
		)?;
		let playback_active_streams = IntGauge::new(
			"tss_playback_active_streams",
			"Audio streams currently being proxied",
		)?;
		let playback_startup_duration = HistogramVec::new(
			HistogramOpts::new(
				"tss_playback_startup_duration_seconds",
				"Time from playback request to redirect or first proxy response",
			),
			&["delivery"],
		)?;
		let cache_entries = IntGaugeVec::new(
			Opts::new("tss_cache_entries", "Current entries in application caches"),
			&["cache"],
		)?;
		let cache_evictions = IntCounterVec::new(
			Opts::new("tss_cache_evictions_total", "Cache removals by cache and cause"),
			&["cache", "cause"],
		)?;

		register_metrics!(registry;
			subsonic_auth_attempts,
			cache_accesses,
			tidal_clients_loaded,
			tidal_requests,
			tidal_request_duration,
			tidal_errors,
			tidal_requests_in_flight,
			tidal_auth_refreshes,
			playback_requests,
			playback_bytes,
			playback_active_streams,
			playback_startup_duration,
			cache_entries,
			cache_evictions,
		);
		#[cfg(target_os = "linux")]
		if enabled {
			registry.register(Box::new(prometheus::process_collector::ProcessCollector::new(
				std::process::id() as prometheus::process_collector::pid_t,
				"",
			)))?;
		}

		Ok(Self {
			registry,
			enabled,
			subsonic_auth_attempts,
			cache_accesses,
			tidal_clients_loaded,
			tidal_requests,
			tidal_request_duration,
			tidal_errors,
			tidal_requests_in_flight,
			tidal_auth_refreshes,
			playback_requests,
			playback_bytes,
			playback_active_streams,
			playback_startup_duration,
			cache_entries,
			cache_evictions,
		})
	}

	pub fn start_playback_stream(&self) -> ActivePlaybackStream {
		let gauge = self.enabled.then(|| {
			self.playback_active_streams.inc();
			self.playback_active_streams.clone()
		});
		ActivePlaybackStream {
			gauge,
		}
	}

	pub fn observe_tidal_session(&self, session: Session) -> Session {
		if self.enabled {
			session.with_observer(std::sync::Arc::new(self.clone()))
		} else {
			session
		}
	}

	pub fn record_auth_attempt(&self, outcome: &'static str) {
		if !self.enabled {
			return;
		}
		self.subsonic_auth_attempts
			.with_label_values(&[outcome])
			.inc();
	}

	pub fn record_playback_outcome(&self, outcome: &'static str) {
		if !self.enabled {
			return;
		}
		self.playback_requests
			.with_label_values(&[outcome])
			.inc();
	}

	pub fn record_playback_started(
		&self,
		delivery: &'static str,
		started_at: std::time::Instant,
	) {
		if !self.enabled {
			return;
		}
		self.record_playback_outcome(delivery);
		self.playback_startup_duration
			.with_label_values(&[delivery])
			.observe(started_at.elapsed().as_secs_f64());
	}

	pub fn record_playback_bytes(&self, delivery: &'static str, bytes: usize) {
		if !self.enabled {
			return;
		}
		self.playback_bytes
			.with_label_values(&[delivery])
			.inc_by(bytes as u64);
	}

	pub fn record_cache_access(&self, cache: &'static str, result: &'static str, entries: u64) {
		if !self.enabled {
			return;
		}
		self.cache_accesses
			.with_label_values(&[cache, result])
			.inc();
		self.set_cache_entries(cache, entries);
	}

	pub fn set_cache_entries(&self, cache: &'static str, entries: u64) {
		if !self.enabled {
			return;
		}
		self.cache_entries
			.with_label_values(&[cache])
			.set(entries as i64);
	}

	pub fn record_cache_eviction(&self, cache: &'static str, cause: &'static str) {
		if !self.enabled {
			return;
		}
		self.cache_evictions
			.with_label_values(&[cache, cause])
			.inc();
		if cause != "replaced" {
			self.cache_entries.with_label_values(&[cache]).dec();
		}
	}

	pub fn set_tidal_clients_loaded(&self, clients: usize) {
		if !self.enabled {
			return;
		}
		self.tidal_clients_loaded.set(clients as i64);
		self.set_cache_entries("tidal_user", clients as u64);
	}
}

pub struct ActivePlaybackStream {
	gauge: Option<IntGauge>,
}

impl Drop for ActivePlaybackStream {
	fn drop(&mut self) {
		if let Some(gauge) = &self.gauge {
			gauge.dec();
		}
	}
}

impl TidalObserver for Metrics {
	fn request_started(&self, api_version: &'static str, _method: &'static str) {
		if !self.enabled {
			return;
		}
		self.tidal_requests_in_flight
			.with_label_values(&[api_version])
			.inc();
	}

	fn request_finished(
		&self,
		api_version: &'static str,
		method: &'static str,
		outcome: &'static str,
		duration_seconds: f64,
	) {
		if !self.enabled {
			return;
		}
		self.tidal_requests_in_flight
			.with_label_values(&[api_version])
			.dec();
		self.tidal_requests
			.with_label_values(&[api_version, method, outcome])
			.inc();
		self.tidal_request_duration
			.with_label_values(&[api_version, method])
			.observe(duration_seconds);
		if outcome != "success" {
			self.tidal_errors
				.with_label_values(&[api_version, outcome])
				.inc();
		}
	}

	fn auth_refresh_finished(&self, outcome: &'static str) {
		if !self.enabled {
			return;
		}
		self.tidal_auth_refreshes
			.with_label_values(&[outcome])
			.inc();
	}

	fn cache_access(&self, cache: &'static str, result: &'static str, entries: u64) {
		if !self.enabled {
			return;
		}
		if matches!(result, "hit" | "miss") {
			self.record_cache_access(cache, result, entries);
		} else {
			self.set_cache_entries(cache, entries);
		}
	}
}

pub async fn endpoint(req: HttpRequest, metrics: web::Data<Metrics>) -> HttpResponse {
	if !metrics.enabled {
		return HttpResponse::NotFound().finish();
	}

	let is_localhost = req
		.peer_addr()
		.is_some_and(|addr| addr.ip().is_loopback());
	if !is_localhost {
		return HttpResponse::Forbidden().finish();
	}

	let mut body = Vec::new();
	if let Err(error) = TextEncoder::new().encode(&metrics.registry.gather(), &mut body) {
		tracing::error!(%error, "Failed to encode Prometheus metrics");
		return HttpResponse::InternalServerError().finish();
	}

	HttpResponse::Ok()
		.content_type("text/plain; version=0.0.4; charset=utf-8")
		.body(body)
}

#[cfg(test)]
mod tests {
	use super::*;
	use actix_web::{App, http::StatusCode, test};
	use std::net::{IpAddr, Ipv4Addr, SocketAddr};

	#[actix_web::test]
	async fn test_metrics_endpoint_allows_ipv4_localhost() {
		let app = test::init_service(
			App::new()
				.app_data(web::Data::new(Metrics::new(true).expect("metrics")))
				.route("/metrics", web::get().to(endpoint)),
		)
		.await;
		let request = test::TestRequest::get()
			.uri("/metrics")
			.peer_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1234))
			.to_request();

		let response = test::call_service(&app, request).await;
		assert_eq!(response.status(), StatusCode::OK);
	}

	#[actix_web::test]
	async fn test_metrics_endpoint_allows_ipv6_localhost() {
		let app = test::init_service(
			App::new()
				.app_data(web::Data::new(Metrics::new(true).expect("metrics")))
				.route("/metrics", web::get().to(endpoint)),
		)
		.await;
		let request = test::TestRequest::get()
			.uri("/metrics")
			.peer_addr(SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 1234))
			.to_request();

		let response = test::call_service(&app, request).await;
		assert_eq!(response.status(), StatusCode::OK);
	}

	#[actix_web::test]
	async fn test_metrics_endpoint_rejects_non_localhost() {
		let app = test::init_service(
			App::new()
				.app_data(web::Data::new(Metrics::new(true).expect("metrics")))
				.route("/metrics", web::get().to(endpoint)),
		)
		.await;
		let request = test::TestRequest::get()
			.uri("/metrics")
			.peer_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 1234))
			.to_request();

		let response = test::call_service(&app, request).await;
		assert_eq!(response.status(), StatusCode::FORBIDDEN);
	}
}
