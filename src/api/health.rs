use crate::db::DbManager;
use actix_web::{HttpResponse, Responder, web};
use serde_json::json;
use std::sync::Arc;

pub async fn health_check(db: web::Data<Arc<DbManager>>) -> impl Responder {
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
