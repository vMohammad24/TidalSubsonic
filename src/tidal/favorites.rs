use moka::sync::Cache;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;

type ItemFavorites = Arc<RwLock<HashMap<i64, String>>>;

pub static FAVORITE_CACHE: LazyLock<Cache<i64, ItemFavorites>> = LazyLock::new(|| {
	Cache::builder()
		.time_to_idle(Duration::from_hours(24))
		.max_capacity(10_000)
		.build()
});

pub static LOCAL_FAVORITE_CACHE: LazyLock<Cache<String, ItemFavorites>> = LazyLock::new(|| {
	Cache::builder()
		.time_to_idle(Duration::from_hours(24))
		.max_capacity(10_000)
		.build()
});

pub fn get_favorite_date(user_id: i64, item_id: i64) -> Option<String> {
	FAVORITE_CACHE
		.get(&user_id)
		.and_then(|favs| favs.read().ok()?.get(&item_id).cloned())
}

pub fn get_local_favorite_date(username: &str, item_id: i64) -> Option<String> {
	LOCAL_FAVORITE_CACHE
		.get(username)
		.and_then(|favs| favs.read().ok()?.get(&item_id).cloned())
}

fn insert_into_cache<K: std::hash::Hash + Eq + Send + Sync + 'static>(
	cache: &Cache<K, ItemFavorites>,
	key: K,
	item_id: i64,
	created: String,
) {
	let user_favorites = cache.get_with(key, || Arc::new(RwLock::new(HashMap::new())));
	if let Ok(mut guard) = user_favorites.write() {
		guard.insert(item_id, created);
	}
}

fn remove_from_cache<K: std::hash::Hash + Eq + Send + Sync + std::borrow::Borrow<Q> + 'static, Q>(
	cache: &Cache<K, ItemFavorites>,
	key: &Q,
	item_id: i64,
) where
	Q: std::hash::Hash + Eq + ?Sized,
{
	if let Some(user_favorites) = cache.get(key)
		&& let Ok(mut guard) = user_favorites.write()
	{
		guard.remove(&item_id);
	}
}

pub fn add_favorite(user_id: i64, item_id: i64, created: String) {
	insert_into_cache(&FAVORITE_CACHE, user_id, item_id, created);
}

pub fn add_local_favorite(username: &str, item_id: i64, created: String) {
	insert_into_cache(
		&LOCAL_FAVORITE_CACHE,
		username.to_string(),
		item_id,
		created,
	);
}

pub fn set_favorites_map(user_id: i64, favorites: HashMap<i64, String>) {
	tracing::debug!(
		"Setting favorites for user_id {}: count {}",
		user_id,
		favorites.len()
	);
	FAVORITE_CACHE.insert(user_id, Arc::new(RwLock::new(favorites)));
}

pub fn set_local_favorites_map(username: &str, favorites: HashMap<i64, String>) {
	tracing::debug!(
		"Setting local favorites for user {}: count {}",
		username,
		favorites.len()
	);
	LOCAL_FAVORITE_CACHE.insert(username.to_string(), Arc::new(RwLock::new(favorites)));
}

pub fn remove_favorite(user_id: i64, item_id: i64) {
	remove_from_cache(&FAVORITE_CACHE, &user_id, item_id);
}

pub fn remove_local_favorite(username: &str, item_id: i64) {
	remove_from_cache(&LOCAL_FAVORITE_CACHE, username, item_id);
}

pub fn get_favorites_count(user_id: i64) -> usize {
	FAVORITE_CACHE
		.get(&user_id)
		.and_then(|favorites| favorites.read().ok().map(|guard| guard.len()))
		.unwrap_or(0)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_user_favorites_cache_operations() {
		let user_id = 998877;
		let item_id = 12345;
		let date = "2023-01-01T00:00:00Z".to_string();

		assert_eq!(get_favorites_count(user_id), 0);
		assert_eq!(get_favorite_date(user_id, item_id), None);

		add_favorite(user_id, item_id, date.clone());
		assert_eq!(get_favorites_count(user_id), 1);
		assert_eq!(get_favorite_date(user_id, item_id), Some(date));

		remove_favorite(user_id, item_id);
		assert_eq!(get_favorites_count(user_id), 0);
		assert_eq!(get_favorite_date(user_id, item_id), None);
	}

	#[test]
	fn test_local_favorites_cache_operations() {
		let username = "test_user_fav";
		let item_id = 54321;
		let date = "2023-06-01T12:00:00Z".to_string();

		assert_eq!(get_local_favorite_date(username, item_id), None);

		add_local_favorite(username, item_id, date.clone());
		assert_eq!(get_local_favorite_date(username, item_id), Some(date));

		remove_local_favorite(username, item_id);
		assert_eq!(get_local_favorite_date(username, item_id), None);
	}

	#[tokio::test]
	async fn test_concurrent_cache_operations() {
		let mut handles = Vec::new();
		for i in 0..20 {
			handles.push(tokio::spawn(async move {
				let user_id = 1000 + i;
				let item_id = 5000 + i;
				let date = format!("2026-01-01T00:00:{:02}Z", i);

				add_favorite(user_id, item_id, date.clone());
				assert_eq!(get_favorite_date(user_id, item_id), Some(date));
				assert!(get_favorites_count(user_id) >= 1);
				remove_favorite(user_id, item_id);
			}));
		}

		for handle in handles {
			handle.await.expect("Task should join successfully");
		}
	}
}
