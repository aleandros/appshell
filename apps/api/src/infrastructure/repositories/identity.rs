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
        Ok(sql_query("SELECT u.id,u.email,u.name,u.email_verified_at FROM users u JOIN sessions s ON s.user_id=u.id WHERE s.token_hash=$1 AND s.expires_at>now()").bind::<Text, _>(token_hash.as_ref()).get_result::<User>(self.connection).optional()?.map(Into::into))
    }
    pub(crate) fn identity_increment_rate_limit(
        &mut self,
        key_hash: impl AsRef<str>,
    ) -> Result<super::super::rows::Count> {
        Ok(sql_query("INSERT INTO rate_limits(key,hits,expires_at) VALUES($1,1,now()+interval '15 minutes') ON CONFLICT(key) DO UPDATE SET hits=CASE WHEN rate_limits.expires_at<now() THEN 1 ELSE rate_limits.hits+1 END, expires_at=CASE WHEN rate_limits.expires_at<now() THEN now()+interval '15 minutes' ELSE rate_limits.expires_at END RETURNING hits::bigint AS count").bind::<Text, _>(key_hash.as_ref()).get_result::<Count>(self.connection)?)
    }
    pub(crate) fn identity_issue_action(
        &mut self,
        token_hash: impl AsRef<str>,
        user_id: Uuid,
        purpose: impl AsRef<str>,
        payload: Option<&str>,
    ) -> Result<usize> {
        Ok(sql_query("INSERT INTO action_tokens(token_hash,user_id,purpose,payload,expires_at) VALUES($1,$2,$3,$4,now()+interval '1 hour') ON CONFLICT(user_id,purpose) DO UPDATE SET token_hash=excluded.token_hash,payload=excluded.payload,expires_at=excluded.expires_at").bind::<Text, _>(token_hash.as_ref()).bind::<SqlUuid, _>(user_id).bind::<Text, _>(purpose.as_ref()).bind::<Nullable<Text>, _>(payload).execute(self.connection)?)
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
        id: Uuid,
        email: impl AsRef<str>,
        name: impl AsRef<str>,
        password_hash: impl AsRef<str>,
    ) -> Result<crate::models::User> {
        Ok(sql_query("INSERT INTO users(id,email,name,password_hash) VALUES($1,$2,$3,$4) RETURNING id,email,name,email_verified_at").bind::<SqlUuid, _>(id).bind::<Text, _>(email.as_ref()).bind::<Text, _>(name.as_ref()).bind::<Text, _>(password_hash.as_ref()).get_result::<User>(self.connection)?.into())
    }
    pub(crate) fn identity_password_hash(
        &mut self,
        user_id: Uuid,
    ) -> Result<Option<super::super::rows::TextValue>> {
        Ok(sql_query(
            "SELECT password_hash AS value FROM users WHERE id=$1 AND password_hash IS NOT NULL",
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
            sql_query("SELECT id,password_hash FROM users WHERE email=$1")
                .bind::<Text, _>(email.as_ref())
                .get_result::<Credentials>(self.connection)
                .optional()?,
        )
    }
    pub(crate) fn identity_user(&mut self, user_id: Uuid) -> Result<crate::models::User> {
        Ok(
            sql_query("SELECT id,email,name,email_verified_at FROM users WHERE id=$1")
                .bind::<SqlUuid, _>(user_id)
                .get_result::<User>(self.connection)?
                .into(),
        )
    }
    pub(crate) fn identity_delete_session(&mut self, token_hash: impl AsRef<str>) -> Result<usize> {
        Ok(sql_query("DELETE FROM sessions WHERE token_hash=$1")
            .bind::<Text, _>(token_hash.as_ref())
            .execute(self.connection)?)
    }
    pub(crate) fn identity_password_user(
        &mut self,
        email: impl AsRef<str>,
    ) -> Result<Option<crate::models::User>> {
        Ok(sql_query("SELECT id,email,name,email_verified_at FROM users WHERE email=$1 AND password_hash IS NOT NULL").bind::<Text, _>(email.as_ref()).get_result::<User>(self.connection).optional()?.map(Into::into))
    }
    pub(crate) fn identity_consume_action(
        &mut self,
        token_hash: impl AsRef<str>,
        purpose: impl AsRef<str>,
    ) -> Result<Option<super::super::rows::Action>> {
        Ok(sql_query("DELETE FROM action_tokens WHERE token_hash=$1 AND purpose=$2 AND expires_at>now() RETURNING user_id,payload").bind::<Text, _>(token_hash.as_ref()).bind::<Text, _>(purpose.as_ref()).get_result::<Action>(self.connection).optional()?)
    }
    pub(crate) fn identity_update_password(
        &mut self,
        password_hash: impl AsRef<str>,
        user_id: Uuid,
    ) -> Result<usize> {
        Ok(sql_query("UPDATE users SET password_hash=$1 WHERE id=$2")
            .bind::<Text, _>(password_hash.as_ref())
            .bind::<SqlUuid, _>(user_id)
            .execute(self.connection)?)
    }
    pub(crate) fn identity_verify_email(&mut self, user_id: Uuid) -> Result<usize> {
        Ok(
            sql_query("UPDATE users SET email_verified_at=now() WHERE id=$1")
                .bind::<SqlUuid, _>(user_id)
                .execute(self.connection)?,
        )
    }
    pub(crate) fn identity_revoke_sessions(&mut self, user_id: Uuid) -> Result<usize> {
        Ok(sql_query("DELETE FROM sessions WHERE user_id=$1")
            .bind::<SqlUuid, _>(user_id)
            .execute(self.connection)?)
    }
    pub(crate) fn identity_revoke_actions(&mut self, user_id: Uuid) -> Result<usize> {
        Ok(sql_query("DELETE FROM action_tokens WHERE user_id=$1")
            .bind::<SqlUuid, _>(user_id)
            .execute(self.connection)?)
    }
    pub(crate) fn identity_lock_user(&mut self, user_id: Uuid) -> Result<usize> {
        Ok(sql_query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
            .bind::<SqlUuid, _>(user_id)
            .execute(self.connection)?)
    }
    pub(crate) fn identity_update_email(
        &mut self,
        email: impl AsRef<str>,
        user_id: Uuid,
    ) -> Result<usize> {
        Ok(
            sql_query("UPDATE users SET email=$1,email_verified_at=now() WHERE id=$2")
                .bind::<Text, _>(email.as_ref())
                .bind::<SqlUuid, _>(user_id)
                .execute(self.connection)?,
        )
    }
}
