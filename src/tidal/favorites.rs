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

pub fn get_favorite_date(user_id: i64, item_id: i64) -> Option<String> {
	if let Some(user_favorites) = FAVORITE_CACHE.get(&user_id)
		&& let Ok(guard) = user_favorites.read()
	{
		return guard.get(&item_id).cloned();
	}
	None
}

pub fn add_favorite(user_id: i64, item_id: i64, created: String) {
	let user_favorites = FAVORITE_CACHE.get_with(user_id, || Arc::new(RwLock::new(HashMap::new())));
	if let Ok(mut guard) = user_favorites.write() {
		guard.insert(item_id, created);
	}
}

pub fn set_favorites_map(user_id: i64, favorites: HashMap<i64, String>) {
	tracing::debug!(
		"Setting favorites for user_id {}: count {}",
		user_id,
		favorites.len()
	);
	FAVORITE_CACHE.insert(user_id, Arc::new(RwLock::new(favorites)));
}

pub fn remove_favorite(user_id: i64, item_id: i64) {
	if let Some(user_favorites) = FAVORITE_CACHE.get(&user_id)
		&& let Ok(mut guard) = user_favorites.write()
	{
		guard.remove(&item_id);
	}
}

pub fn get_favorites_count(user_id: i64) -> usize {
	FAVORITE_CACHE
		.get(&user_id)
		.and_then(|favorites| favorites.read().ok().map(|guard| guard.len()))
		.unwrap_or(0)
}
