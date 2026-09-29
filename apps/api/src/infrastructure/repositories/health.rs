use super::UnitOfWork;
use crate::error::Result;
use diesel::{prelude::*, sql_query};
impl UnitOfWork<'_> {
    pub(crate) fn health_check(&mut self) -> Result<usize> {
        Ok(sql_query("SELECT 1").execute(self.connection)?)
    }
}
