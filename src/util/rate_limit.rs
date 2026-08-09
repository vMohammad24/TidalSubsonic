use moka::sync::Cache;
use std::time::Duration;

#[derive(Clone)]
pub struct RateLimiter {
	cache: Cache<String, u32>,
	max_requests: u32,
}

impl RateLimiter {
	pub fn new(max_requests: u32, window_secs: u64) -> Self {
		Self {
			cache: Cache::builder()
				.time_to_live(Duration::from_secs(window_secs))
				.build(),
			max_requests,
		}
	}

	pub fn check_and_increment(&self, key: &str) -> bool {
		let current = self.cache.get(key).unwrap_or(0);
		if current >= self.max_requests {
			false
		} else {
			self.cache.insert(key.to_string(), current + 1);
			true
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_rate_limiter_allows_under_limit() {
		let limiter = RateLimiter::new(3, 60);
		assert!(limiter.check_and_increment("user1"));
		assert!(limiter.check_and_increment("user1"));
		assert!(limiter.check_and_increment("user1"));
		assert!(!limiter.check_and_increment("user1"));
	}

	#[test]
	fn test_rate_limiter_independent_keys() {
		let limiter = RateLimiter::new(1, 60);
		assert!(limiter.check_and_increment("user1"));
		assert!(!limiter.check_and_increment("user1"));
		assert!(limiter.check_and_increment("user2"));
	}
}
