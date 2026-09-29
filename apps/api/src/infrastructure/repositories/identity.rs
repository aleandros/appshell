use super::UnitOfWork;
use crate::error::Result;
use crate::infrastructure::rows::*;
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Nullable, Text, Uuid as SqlUuid},
};
use uuid::Uuid;
impl UnitOfWork<'_> {
    pub(crate) fn identity_session_user(
        &mut self,
        token_hash: impl AsRef<str>,
    ) -> Result<Option<crate::models::User>> {
        Ok(sql_query("SELECT u.id,u.email,u.name,u.email_verified_at FROM users u JOIN sessions s ON s.user_id=u.id WHERE s.token_hash=$1 AND s.expires_at>now() AND s.deleted_at IS NULL AND u.deleted_at IS NULL AND u.status='active'").bind::<Text, _>(token_hash.as_ref()).get_result::<User>(self.connection).optional()?.map(Into::into))
    }
    pub(crate) fn identity_increment_rate_limit(
        &mut self,
        key_hash: impl AsRef<str>,
    ) -> Result<super::super::rows::Count> {
        Ok(sql_query("INSERT INTO rate_limits(key,hits,expires_at) VALUES($1,1,now()+interval '15 minutes') ON CONFLICT(key) DO UPDATE SET deleted_at=NULL,hits=CASE WHEN rate_limits.deleted_at IS NOT NULL OR rate_limits.expires_at<now() THEN 1 ELSE rate_limits.hits+1 END, expires_at=CASE WHEN rate_limits.deleted_at IS NOT NULL OR rate_limits.expires_at<now() THEN now()+interval '15 minutes' ELSE rate_limits.expires_at END RETURNING hits::bigint AS count").bind::<Text, _>(key_hash.as_ref()).get_result::<Count>(self.connection)?)
    }
    pub(crate) fn identity_issue_action(
        &mut self,
        token_hash: impl AsRef<str>,
        user_id: Uuid,
        purpose: impl AsRef<str>,
        payload: Option<&str>,
    ) -> Result<usize> {
        sql_query("UPDATE action_tokens SET deleted_at=now() WHERE user_id=$1 AND purpose=$2 AND deleted_at IS NULL")
            .bind::<SqlUuid,_>(user_id).bind::<Text,_>(purpose.as_ref()).execute(self.connection)?;
        Ok(sql_query("INSERT INTO action_tokens(token_hash,user_id,purpose,payload,expires_at) VALUES($1,$2,$3,$4,now()+interval '1 hour')").bind::<Text, _>(token_hash.as_ref()).bind::<SqlUuid, _>(user_id).bind::<Text, _>(purpose.as_ref()).bind::<Nullable<Text>, _>(payload).execute(self.connection)?)
    }
    pub(crate) fn identity_create_session(
        &mut self,
        token_hash: impl AsRef<str>,
        user_id: Uuid,
    ) -> Result<usize> {
        Ok(sql_query("INSERT INTO sessions(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '14 days')").bind::<Text, _>(token_hash.as_ref()).bind::<SqlUuid, _>(user_id).execute(self.connection)?)
    }
    pub(crate) fn identity_create_user(
        &mut self,
        email: impl AsRef<str>,
        name: impl AsRef<str>,
        password_hash: impl AsRef<str>,
    ) -> Result<crate::models::User> {
        Ok(sql_query("INSERT INTO users(email,name,password_hash) VALUES($1,$2,$3) RETURNING id,email,name,email_verified_at").bind::<Text, _>(email.as_ref()).bind::<Text, _>(name.as_ref()).bind::<Text, _>(password_hash.as_ref()).get_result::<User>(self.connection)?.into())
    }
    pub(crate) fn identity_password_hash(
        &mut self,
        user_id: Uuid,
    ) -> Result<Option<super::super::rows::TextValue>> {
        Ok(sql_query(
            "SELECT password_hash AS value FROM users WHERE deleted_at IS NULL AND status='active' AND id=$1 AND password_hash IS NOT NULL",
        )
        .bind::<SqlUuid, _>(user_id)
        .get_result::<TextValue>(self.connection)
        .optional()?)
    }
    pub(crate) fn identity_credentials(
        &mut self,
        email: impl AsRef<str>,
    ) -> Result<Option<super::super::rows::Credentials>> {
        Ok(
            sql_query("SELECT id,password_hash FROM users WHERE deleted_at IS NULL AND status='active' AND email=$1 FOR UPDATE")
                .bind::<Text, _>(email.as_ref())
                .get_result::<Credentials>(self.connection)
                .optional()?,
        )
    }
    pub(crate) fn identity_user(&mut self, user_id: Uuid) -> Result<crate::models::User> {
        Ok(
            sql_query("SELECT id,email,name,email_verified_at FROM users WHERE deleted_at IS NULL AND status='active' AND id=$1")
                .bind::<SqlUuid, _>(user_id)
                .get_result::<User>(self.connection)?
                .into(),
        )
    }
    pub(crate) fn identity_delete_session(&mut self, token_hash: impl AsRef<str>) -> Result<usize> {
        Ok(sql_query(
            "UPDATE sessions SET deleted_at=now() WHERE deleted_at IS NULL AND token_hash=$1",
        )
        .bind::<Text, _>(token_hash.as_ref())
        .execute(self.connection)?)
    }
    pub(crate) fn identity_password_user(
        &mut self,
        email: impl AsRef<str>,
    ) -> Result<Option<crate::models::User>> {
        Ok(sql_query("SELECT id,email,name,email_verified_at FROM users WHERE deleted_at IS NULL AND status='active' AND email=$1 AND password_hash IS NOT NULL FOR UPDATE").bind::<Text, _>(email.as_ref()).get_result::<User>(self.connection).optional()?.map(Into::into))
    }
    pub(crate) fn identity_consume_action(
        &mut self,
        token_hash: impl AsRef<str>,
        purpose: impl AsRef<str>,
    ) -> Result<Option<super::super::rows::Action>> {
        let owner = sql_query("SELECT u.id FROM users u WHERE u.deleted_at IS NULL AND u.status='active' AND u.id=(SELECT user_id FROM action_tokens WHERE token_hash=$1 AND purpose=$2 AND deleted_at IS NULL AND expires_at>now()) FOR UPDATE")
            .bind::<Text,_>(token_hash.as_ref()).bind::<Text,_>(purpose.as_ref()).get_result::<Id>(self.connection).optional()?;
        let Some(owner) = owner else {
            return Ok(None);
        };
        self.actor("user", owner.id)?;
        Ok(sql_query("UPDATE action_tokens SET deleted_at=now() WHERE deleted_at IS NULL AND token_hash=$1 AND purpose=$2 AND expires_at>now() RETURNING user_id,payload").bind::<Text, _>(token_hash.as_ref()).bind::<Text, _>(purpose.as_ref()).get_result::<Action>(self.connection).optional()?)
    }
    pub(crate) fn identity_update_password(
        &mut self,
        password_hash: impl AsRef<str>,
        user_id: Uuid,
    ) -> Result<usize> {
        Ok(sql_query("UPDATE users SET password_hash=$1 WHERE deleted_at IS NULL AND status='active' AND id=$2")
            .bind::<Text, _>(password_hash.as_ref())
            .bind::<SqlUuid, _>(user_id)
            .execute(self.connection)?)
    }
    pub(crate) fn identity_verify_email(&mut self, user_id: Uuid) -> Result<usize> {
        Ok(
            sql_query("UPDATE users SET email_verified_at=now() WHERE deleted_at IS NULL AND status='active' AND id=$1")
                .bind::<SqlUuid, _>(user_id)
                .execute(self.connection)?,
        )
    }
    pub(crate) fn identity_revoke_sessions(&mut self, user_id: Uuid) -> Result<usize> {
        Ok(sql_query(
            "UPDATE sessions SET deleted_at=now() WHERE deleted_at IS NULL AND user_id=$1",
        )
        .bind::<SqlUuid, _>(user_id)
        .execute(self.connection)?)
    }
    pub(crate) fn identity_revoke_actions(&mut self, user_id: Uuid) -> Result<usize> {
        Ok(sql_query(
            "UPDATE action_tokens SET deleted_at=now() WHERE deleted_at IS NULL AND user_id=$1",
        )
        .bind::<SqlUuid, _>(user_id)
        .execute(self.connection)?)
    }
    pub(crate) fn identity_lock_user(&mut self, user_id: Uuid) -> Result<usize> {
        Ok(sql_query("SELECT id FROM users WHERE deleted_at IS NULL AND status='active' AND id=$1 FOR UPDATE")
            .bind::<SqlUuid, _>(user_id)
            .execute(self.connection)?)
    }
    pub(crate) fn identity_update_email(
        &mut self,
        email: impl AsRef<str>,
        user_id: Uuid,
    ) -> Result<usize> {
        Ok(
            sql_query("UPDATE users SET email=$1,email_verified_at=now() WHERE deleted_at IS NULL AND status='active' AND id=$2")
                .bind::<Text, _>(email.as_ref())
                .bind::<SqlUuid, _>(user_id)
                .execute(self.connection)?,
        )
    }
}

impl UnitOfWork<'_> {
    pub(crate) fn identity_managed_users(
        &mut self,
        search: &str,
        offset: i64,
    ) -> Result<Vec<crate::models::ManagedUser>> {
        Ok(sql_query("SELECT id,email,name,status,email_verified_at,deleted_at FROM users WHERE strpos(lower(email||' '||name),lower($1))>0 ORDER BY created_at,id LIMIT 50 OFFSET $2")
            .bind::<Text,_>(search).bind::<diesel::sql_types::BigInt,_>(offset).load::<ManagedUser>(self.connection)?.into_iter().map(Into::into).collect())
    }
    pub(crate) fn identity_managed_user(&mut self, id: Uuid) -> Result<crate::models::ManagedUser> {
        Ok(sql_query("SELECT id,email,name,status,email_verified_at,deleted_at FROM users WHERE id=$1 FOR UPDATE")
            .bind::<SqlUuid,_>(id).get_result::<ManagedUser>(self.connection)?.into())
    }
    pub(crate) fn identity_manage_user(
        &mut self,
        id: Uuid,
        email: &str,
        name: &str,
        status: &str,
    ) -> Result<()> {
        sql_query("UPDATE users SET email_verified_at=CASE WHEN email=$2 THEN email_verified_at ELSE NULL END,email=$2,name=$3,status=CASE WHEN $4='deleted' THEN 'suspended' ELSE $4 END,deleted_at=CASE WHEN $4='deleted' THEN coalesce(deleted_at,now()) ELSE NULL END WHERE id=$1")
            .bind::<SqlUuid,_>(id).bind::<Text,_>(email).bind::<Text,_>(name).bind::<Text,_>(status).execute(self.connection)?;
        Ok(())
    }
    pub(crate) fn identity_user_history(
        &mut self,
        id: Uuid,
        offset: i64,
    ) -> Result<Vec<crate::models::HistoryEntry>> {
        Ok(sql_query("SELECT id,operation,changes,actor_id,actor_kind,changed_at FROM users_history WHERE record_id=$1 ORDER BY changed_at DESC,id LIMIT 50 OFFSET $2")
            .bind::<SqlUuid,_>(id).bind::<diesel::sql_types::BigInt,_>(offset).load::<HistoryEntry>(self.connection)?.into_iter().map(Into::into).collect())
    }
}
