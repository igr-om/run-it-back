use sqlx::postgres::{PgPool, PgPoolOptions};

pub type Pool = PgPool;

pub async fn connect(database_url: &str) -> anyhow::Result<Pool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Runs every migration in `crates/db/migrations`. Safe to call on every
/// startup -- sqlx tracks applied versions in its own `_sqlx_migrations`
/// table and skips anything already applied.
pub async fn migrate(pool: &Pool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
