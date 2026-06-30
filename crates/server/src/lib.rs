pub mod auth;
pub mod error;
pub mod routes;
pub mod state;

use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use state::AppState;

/// `static_dir`, when set, serves the built frontend (e.g. `web/dist`) from
/// this same binary -- anything not matched by `/api/*` falls back to
/// `index.html` so client-side routing (React Router) still works on a
/// hard refresh of a deep link. This is what makes "one container, one
/// process" deploys (Docker Compose, Fly.io, Render) possible without a
/// separate static host or CDN.
pub fn build_router(state: AppState, static_dir: Option<&str>) -> Router {
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    let api = Router::new()
        .route("/auth/register", post(routes::auth::register))
        .route("/auth/login", post(routes::auth::login))
        .route("/me", get(routes::auth::me))
        .route("/ranges", get(routes::ranges::list).post(routes::ranges::create))
        .route("/ranges/:id", delete(routes::ranges::delete))
        .route("/solve", post(routes::solve::enqueue))
        .route("/solve/jobs/:id", get(routes::solve::job_status))
        .route("/solve/preflop", get(routes::solve::preflop_library))
        .route("/drills/generate", post(routes::drills::generate))
        .route("/drills/:id/answer", post(routes::drills::answer))
        .route("/drills/attempts", get(routes::drills::recent_attempts))
        .route("/stats/weakness", get(routes::drills::weakness_profile))
        .route("/hands/upload", post(routes::hands::upload))
        .route("/hands", get(routes::hands::list))
        .route("/hands/parsed", get(routes::hands::parsed_hands))
        .route("/stats/overview", get(routes::hands::stats_overview))
        .route("/health", get(health));

    let mut router = Router::new()
        .nest("/api", api)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    if let Some(dir) = static_dir {
        let index_path = format!("{dir}/index.html");
        let serve_dir = ServeDir::new(dir).not_found_service(ServeFile::new(index_path));
        router = router.fallback_service(serve_dir);
    }

    router
}

async fn health() -> &'static str {
    "ok"
}
