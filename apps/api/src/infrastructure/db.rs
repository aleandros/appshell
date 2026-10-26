use crate::error::{ApiError, Result};
use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool},
};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
pub type DbPool = Pool<ConnectionManager<PgConnection>>;
/// # Errors
/// Returns an internal error when the connection pool cannot be established.
///
/// # Panics
/// Panics on an invalid or zero `DB_POOL_SIZE`; this is a fatal startup configuration error.
#[expect(
    clippy::panic_in_result_fn,
    reason = "Invalid startup configuration is fatal; connection failures remain recoverable errors."
)]
pub fn connect(url: &str) -> Result<DbPool> {
    let max_size = std::env::var("DB_POOL_SIZE").map_or(8, |value| {
        value
            .parse::<u32>()
            .expect("DB_POOL_SIZE must be a positive integer")
    });
    assert!(max_size > 0, "DB_POOL_SIZE must be a positive integer");
    Pool::builder()
        .max_size(max_size)
        .min_idle(Some(0))
        .idle_timeout(Some(std::time::Duration::from_secs(60)))
        .connection_timeout(std::time::Duration::from_secs(5))
        .build(ConnectionManager::new(url))
        .map_err(ApiError::internal)
}
/// # Errors
/// Propagates connection acquisition, blocking task, and operation errors.
pub async fn run<T, F>(pool: DbPool, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(&mut PgConnection) -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(ApiError::internal)?;
        f(&mut conn)
    })
    .await
    .map_err(ApiError::internal)?
}
/// # Errors
/// Returns an error if locking, migrations, or model validation fail; the transaction rolls back.
pub async fn migrate(pool: DbPool) -> Result<()> {
    run(pool, |c| {
        use diesel::{Connection, RunQueryDsl};
        c.transaction::<_, ApiError, _>(|c| {
            // Transaction-scoped locks also work behind transaction-mode poolers.
            diesel::sql_query("SELECT pg_advisory_xact_lock(72184612)").execute(c)?;
            c.run_pending_migrations(MIGRATIONS)
                .map(|_| ())
                .map_err(ApiError::internal)?;
            diesel::sql_query("SELECT validate_model_conventions()").execute(c)?;
            Ok(())
        })
    })
    .await
}

/// # Errors
/// Returns an error for a missing `DATABASE_URL`, connection failure, or failed blocking task.
///
/// # Panics
/// The blocking initializer panics on invalid `DB_POOL_SIZE`; this is returned as a task error.
pub async fn from_env() -> Result<DbPool> {
    tokio::task::spawn_blocking(|| {
        let url = std::env::var("DATABASE_URL").map_err(ApiError::internal)?;
        connect(&url)
    })
    .await
    .map_err(ApiError::internal)?
}
