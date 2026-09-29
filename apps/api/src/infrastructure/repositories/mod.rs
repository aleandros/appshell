use crate::{
    db,
    error::{ApiError, Result},
};
use diesel::{Connection, PgConnection, RunQueryDsl};
use uuid::Uuid;
mod admin;
mod billing;
mod health;
mod identity;
mod mail;
mod organizations;
/// One connection, shared by every repository participating in a use case.
/// Transactions keep locks, single-use tokens, and outbox writes atomic.
pub(crate) struct UnitOfWork<'a> {
    connection: &'a mut PgConnection,
    in_transaction: bool,
}
impl UnitOfWork<'_> {
    /// Transaction-local attribution; never persists on a pooled connection.
    pub(crate) fn actor(&mut self, kind: &str, id: Uuid) -> Result<()> {
        if !self.in_transaction {
            return Err(ApiError::internal(
                "Audit attribution requires a unit-of-work transaction",
            ));
        }
        diesel::sql_query(
            "SELECT set_config('app.actor_kind',$1,true),set_config('app.actor_id',$2,true)",
        )
        .bind::<diesel::sql_types::Text, _>(kind)
        .bind::<diesel::sql_types::Text, _>(id.to_string())
        .execute(self.connection)?;
        Ok(())
    }
    pub(crate) fn generated_id(&mut self) -> Result<Uuid> {
        Ok(diesel::sql_query("SELECT gen_random_uuid() AS id")
            .get_result::<super::rows::Id>(self.connection)?
            .id)
    }

    pub(crate) fn transaction<T>(
        &mut self,
        f: impl FnOnce(&mut UnitOfWork<'_>) -> Result<T>,
    ) -> Result<T> {
        self.connection.transaction::<T, ApiError, _>(|connection| {
            f(&mut UnitOfWork {
                connection,
                in_transaction: true,
            })
        })
    }
}
pub(crate) async fn run<T: Send + 'static>(
    pool: db::DbPool,
    f: impl FnOnce(&mut UnitOfWork<'_>) -> Result<T> + Send + 'static,
) -> Result<T> {
    db::run(pool, move |connection| {
        f(&mut UnitOfWork {
            connection,
            in_transaction: false,
        })
    })
    .await
}

pub(crate) use crate::db::DbPool;
