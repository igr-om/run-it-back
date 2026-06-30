use std::net::SocketAddr;
use std::sync::Arc;

use tracing_subscriber::EnvFilter;

use rib_server::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt().with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))).init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://runitback:runitback@localhost:5432/runitback".to_string());
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        tracing::warn!("JWT_SECRET not set -- using an insecure default. Set this in production!");
        "dev-only-insecure-secret-change-me".to_string()
    });
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let n_workers: usize = std::env::var("RIB_WORKERS").ok().and_then(|p| p.parse().ok()).unwrap_or(0);
    let warm_on_start = std::env::var("RIB_WARM_LIBRARY_ON_START").map(|v| v != "0").unwrap_or(true);

    tracing::info!("connecting to database");
    let db = rib_db::connect(&database_url).await?;
    tracing::info!("running migrations");
    rib_db::migrate(&db).await?;

    let workers = Arc::new(rib_worker::WorkerPool::new(n_workers, db.clone()));

    if warm_on_start {
        let n = rib_worker::enqueue_curated_warm_jobs(&workers);
        tracing::info!(n, "enqueued curated preflop spots for background warming");
    }

    let state = AppState { db, workers, jwt_secret: Arc::new(jwt_secret) };
    let static_dir = std::env::var("STATIC_DIR").ok();
    let app = rib_server::build_router(state, static_dir.as_deref());

    let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;
    tracing::info!(%addr, "Run It Back server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
