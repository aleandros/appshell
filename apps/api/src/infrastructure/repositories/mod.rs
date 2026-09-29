use crate::{
    db,
    error::{ApiError, Result},
};
use diesel::{Connection, PgConnection};
mod billing;
mod health;
mod identity;
mod mail;
mod organizations;
/// One connection, shared by every repository participating in a use case.
/// Transactions keep locks, single-use tokens, and outbox writes atomic.
pub(crate) struct UnitOfWork<'a> {
    connection: &'a mut PgConnection,
}
impl UnitOfWork<'_> {
    pub(crate) fn transaction<T>(
        &mut self,
        f: impl FnOnce(&mut UnitOfWork<'_>) -> Result<T>,
    ) -> Result<T> {
        self.connection
            .transaction::<T, ApiError, _>(|connection| f(&mut UnitOfWork { connection }))
    }
}
pub(crate) async fn run<T: Send + 'static>(
    pool: db::DbPool,
    f: impl FnOnce(&mut UnitOfWork<'_>) -> Result<T> + Send + 'static,
) -> Result<T> {
    db::run(pool, move |connection| f(&mut UnitOfWork { connection })).await
}
