mod infrastructure;
use crate::infrastructure::repositories;
pub mod config;
mod contexts;
pub use infrastructure::db;
pub mod error;
mod http;
pub use infrastructure::mail;
pub mod models;
use axum::{
    Json, Router, middleware,
    routing::{delete, get, post},
};
use http::{auth, billing, organizations};
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use utoipa::OpenApi;

#[derive(Clone)]
pub struct AppState {
    pub pool: db::DbPool,
    pub config: config::Config,
    pub http: reqwest::Client,
    pub dummy_password_hash: Arc<String>,
}
impl AppState {
    pub fn new(pool: db::DbPool, config: config::Config) -> Self {
        Self {
            pool,
            config,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .expect("HTTP client"),
            dummy_password_hash: Arc::new(
                crate::infrastructure::crypto::hash_password(
                    &crate::infrastructure::crypto::token(),
                )
                .expect("password hasher"),
            ),
        }
    }
}
#[derive(OpenApi)]
#[openapi(
    paths(
        auth::signup,
        auth::login,
        auth::me,
        auth::logout,
        auth::forgot,
        auth::reset,
        auth::verify,
        auth::resend,
        auth::change_password,
        auth::change_email,
        auth::confirm_email,
        organizations::create,
        organizations::team,
        organizations::invite,
        organizations::accept,
        organizations::revoke_invite,
        organizations::remove_member,
        billing::billing,
        billing::checkout,
        billing::portal
    ),
    components(schemas(error::ErrorBody))
)]
pub struct ApiDoc;

pub fn router(state: AppState, static_dir: Option<&str>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            state
                .config
                .app_url
                .parse::<axum::http::HeaderValue>()
                .expect("APP_URL header"),
        )
        .allow_credentials(true)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([axum::http::header::CONTENT_TYPE]);
    let api = Router::new()
        .route("/auth/signup", post(auth::signup))
        .route("/auth/login", post(auth::login))
        .route("/auth/session", get(auth::me))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/forgot-password", post(auth::forgot))
        .route("/auth/reset-password", post(auth::reset))
        .route("/auth/verify-email", post(auth::verify))
        .route("/auth/resend-verification", post(auth::resend))
        .route("/auth/change-password", post(auth::change_password))
        .route("/auth/change-email", post(auth::change_email))
        .route("/auth/confirm-email", post(auth::confirm_email))
        .route("/organizations", post(organizations::create))
        .route("/organizations/{org}/team", get(organizations::team))
        .route(
            "/organizations/{org}/invitations",
            post(organizations::invite),
        )
        .route(
            "/organizations/{org}/invitations/{id}",
            delete(organizations::revoke_invite),
        )
        .route(
            "/organizations/{org}/members/{id}",
            delete(organizations::remove_member),
        )
        .route("/invitations/accept", post(organizations::accept))
        .route("/organizations/{org}/billing", get(billing::billing))
        .route("/organizations/{org}/checkout", post(billing::checkout))
        .route("/organizations/{org}/billing-portal", post(billing::portal))
        .route("/webhooks/stripe", post(billing::webhook))
        .route("/openapi.json", get(|| async { Json(ApiDoc::openapi()) }))
        .fallback(|| async { error::ApiError(404, "not_found", "Unknown API endpoint.") })
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            http::security::guard,
        ));
    let mut app = Router::new()
        .nest("/api", api)
        .route("/health", get(|| async { "ok" }))
        .route(
            "/ready",
            get(
                |axum::extract::State(s): axum::extract::State<AppState>| async move {
                    repositories::run(s.pool, |c| {
                        c.health_check()?;
                        Ok(())
                    })
                    .await
                    .map(|_| "ready")
                },
            ),
        );
    if let Some(dir) = static_dir {
        app = app.fallback_service(
            ServeDir::new(dir).fallback(ServeFile::new(format!("{dir}/index.html"))),
        );
    }
    app.layer(cors)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
