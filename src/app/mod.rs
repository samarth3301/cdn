use axum::{
    routing::{get, post},
    Router,
    middleware::Next,
    http::Request,
    body::Body,
};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use std::time::Instant;

use crate::handlers::{get_object, health_check, register_bucket, reset_cache};
use crate::models::AppState;

async fn request_logger_middleware(
    req: Request<Body>,
    next: Next,
) -> axum::response::Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status();

    info!(
        method = %method,
        path = %uri.path(),
        query = ?uri.query(),
        status = status.as_u16(),
        duration_ms = duration.as_millis(),
        "HTTP Request"
    );

    response
}

/// Create the Axum application with all routes and middleware
pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/reset-cache", post(reset_cache))
        .route("/buckets/register", post(register_bucket))
        .route("/:instance/*key", get(get_object))
        .layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn(request_logger_middleware))
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
                .layer(tower::limit::ConcurrencyLimitLayer::new(100)),
        )
        .with_state(state)
}
