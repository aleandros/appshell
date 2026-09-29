use super::UnitOfWork;
use crate::{
    error::{ApiError, Result},
    infrastructure::rows::*,
    models,
};
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{BigInt, Nullable, Text, Uuid as SqlUuid},
};
use uuid::Uuid;
impl UnitOfWork<'_> {
    pub(crate) fn admin_management_lock(&mut self) -> Result<()> {
        sql_query("SELECT pg_advisory_xact_lock(72184613)").execute(self.connection)?;
        Ok(())
    }
    pub(crate) fn admin_count(&mut self) -> Result<i64> {
        Ok(sql_query("SELECT count(*) AS count FROM admin_accounts")
            .get_result::<Count>(self.connection)?
            .count)
    }
    pub(crate) fn admin_require_actor(&mut self, id: Uuid) -> Result<()> {
        sql_query("SELECT id FROM admin_accounts WHERE id=$1 AND status='active' AND deleted_at IS NULL FOR UPDATE")
            .bind::<SqlUuid,_>(id).get_result::<Id>(self.connection).optional()?.ok_or_else(ApiError::unauthorized)?;
        self.actor("admin", id)
    }
    pub(crate) fn admin_create(
        &mut self,
        email: &str,
        name: &str,
        hash: &str,
    ) -> Result<models::AdminAccount> {
        Ok(sql_query("INSERT INTO admin_accounts(email,name,password_hash) VALUES($1,$2,$3) RETURNING id,email,name,status,deleted_at")
            .bind::<Text,_>(email).bind::<Text,_>(name).bind::<Text,_>(hash).get_result::<AdminAccount>(self.connection)?.into())
    }
    pub(crate) fn admin_credentials(&mut self, email: &str) -> Result<Option<Credentials>> {
        Ok(sql_query("SELECT id,password_hash FROM admin_accounts WHERE email=$1 AND status='active' AND deleted_at IS NULL")
            .bind::<Text,_>(email).get_result(self.connection).optional()?)
    }
    pub(crate) fn admin_session(&mut self, hash: &str) -> Result<Option<models::AdminAccount>> {
        Ok(sql_query("SELECT a.id,a.email,a.name,a.status,a.deleted_at FROM admin_accounts a JOIN admin_sessions s ON s.admin_id=a.id WHERE s.token_hash=$1 AND s.expires_at>now() AND s.deleted_at IS NULL AND a.deleted_at IS NULL AND a.status='active'")
            .bind::<Text,_>(hash).get_result::<AdminAccount>(self.connection).optional()?.map(Into::into))
    }
    pub(crate) fn admin_create_session(&mut self, id: Uuid, hash: &str) -> Result<()> {
        sql_query("INSERT INTO admin_sessions(admin_id,token_hash,expires_at) VALUES($1,$2,now()+interval '8 hours')").bind::<SqlUuid,_>(id).bind::<Text,_>(hash).execute(self.connection)?;
        Ok(())
    }
    pub(crate) fn admin_logout(&mut self, hash: &str) -> Result<()> {
        sql_query(
            "UPDATE admin_sessions SET deleted_at=now() WHERE token_hash=$1 AND deleted_at IS NULL",
        )
        .bind::<Text, _>(hash)
        .execute(self.connection)?;
        Ok(())
    }
    pub(crate) fn admin_list(
        &mut self,
        search: &str,
        offset: i64,
    ) -> Result<Vec<models::AdminAccount>> {
        Ok(sql_query("SELECT id,email,name,status,deleted_at FROM admin_accounts WHERE strpos(lower(email||' '||name),lower($1))>0 ORDER BY created_at,id LIMIT 50 OFFSET $2")
            .bind::<Text,_>(search).bind::<BigInt,_>(offset).load::<AdminAccount>(self.connection)?.into_iter().map(Into::into).collect())
    }
    pub(crate) fn admin_update(
        &mut self,
        id: Uuid,
        email: &str,
        name: &str,
        status: &str,
        hash: Option<&str>,
    ) -> Result<()> {
        let changed=sql_query("UPDATE admin_accounts SET email=$2,name=$3,status=CASE WHEN $4='deleted' THEN 'suspended' ELSE $4 END,deleted_at=CASE WHEN $4='deleted' THEN coalesce(deleted_at,now()) ELSE NULL END,password_hash=coalesce($5,password_hash) WHERE id=$1")
            .bind::<SqlUuid,_>(id).bind::<Text,_>(email).bind::<Text,_>(name).bind::<Text,_>(status).bind::<Nullable<Text>,_>(hash).execute(self.connection)?;
        if changed == 0 {
            return Err(ApiError(404, "not_found", "Administrator not found."));
        }
        sql_query(
            "UPDATE admin_sessions SET deleted_at=now() WHERE admin_id=$1 AND deleted_at IS NULL",
        )
        .bind::<SqlUuid, _>(id)
        .execute(self.connection)?;
        Ok(())
    }
}
