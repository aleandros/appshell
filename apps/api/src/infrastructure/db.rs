use crate::error::{ApiError, Result};
use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool},
};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
pub type DbPool = Pool<ConnectionManager<PgConnection>>;
pub fn connect(url: &str) -> Result<DbPool> {
    Pool::builder()
        .max_size(8)
        .min_idle(Some(0))
        .idle_timeout(Some(std::time::Duration::from_secs(60)))
        .connection_timeout(std::time::Duration::from_secs(5))
        .build(ConnectionManager::new(url))
        .map_err(ApiError::internal)
}
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
pub async fn migrate(pool: DbPool) -> Result<()> {
    run(pool, |c| {
        use diesel::{Connection, RunQueryDsl};
        c.transaction::<_, ApiError, _>(|c| {
            // Transaction-scoped locks also work behind transaction-mode poolers.
            diesel::sql_query("SELECT pg_advisory_xact_lock(72184612)").execute(c)?;
            c.run_pending_migrations(MIGRATIONS)
                .map(|_| ())
                .map_err(ApiError::internal)
        })
    })
    .await
}
