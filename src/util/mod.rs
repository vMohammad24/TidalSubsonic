pub use http_client::http_client;

pub mod crypto;
pub mod http_client;
pub mod rate_limit;
pub mod session;

use futures_util::StreamExt;
use std::future::Future;

pub async fn fetch_concurrently<T, F, Fut, R>(items: Vec<T>, concurrency: usize, f: F) -> Vec<R>
where
	F: Fn(T) -> Fut,
	Fut: Future<Output = Option<R>>,
{
	futures_util::stream::iter(items)
		.map(f)
		.buffered(concurrency)
		.filter_map(std::future::ready)
		.collect()
		.await
}
