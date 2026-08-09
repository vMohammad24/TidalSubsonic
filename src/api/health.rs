use crate::db::DbManager;
use actix_web::{HttpResponse, Responder, web};
use serde_json::json;

pub async fn health_check(db: web::Data<DbManager>) -> impl Responder {
	match sqlx::query("SELECT 1").execute(&db.pool).await {
		Ok(_) => HttpResponse::Ok().json(json!({
			"status": "healthy",
			"database": "up"
		})),
		Err(e) => {
			tracing::error!("Health check database failure: {}", e);
			HttpResponse::ServiceUnavailable().json(json!({
				"status": "unhealthy",
				"database": "down",
				"error": e.to_string()
			}))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use actix_web::test;

	#[actix_web::test]
	async fn test_health_check_endpoint() {
		dotenvy::dotenv().ok();
		let Ok(db_url) = std::env::var("DATABASE_URL") else {
			return;
		};
		let Ok(db) = DbManager::new(&db_url).await else {
			return;
		};

		let app_data = web::Data::new(db);
		let resp = health_check(app_data)
			.await
			.respond_to(&test::TestRequest::get().uri("/health").to_http_request());

		assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
	}
}
