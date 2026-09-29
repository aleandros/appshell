use super::UnitOfWork;
use crate::error::Result;
use crate::infrastructure::rows::*;
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Text, Uuid as SqlUuid},
};
use uuid::Uuid;
impl UnitOfWork<'_> {
    pub(crate) fn mail_enqueue(
        &mut self,
        recipient: impl AsRef<str>,
        subject: impl AsRef<str>,
        body: impl AsRef<str>,
        html_body: impl AsRef<str>,
    ) -> Result<usize> {
        Ok(sql_query(
            "INSERT INTO mail_outbox(recipient,subject,body,html_body) VALUES($1,$2,$3,$4)",
        )
        .bind::<Text, _>(recipient.as_ref())
        .bind::<Text, _>(subject.as_ref())
        .bind::<Text, _>(body.as_ref())
        .bind::<Text, _>(html_body.as_ref())
        .execute(self.connection)?)
    }
    pub(crate) fn mail_claim(&mut self) -> Result<Vec<super::super::rows::Mail>> {
        Ok(sql_query("UPDATE mail_outbox SET attempts=attempts+1,available_at=now()+interval '5 minutes' WHERE id IN (SELECT id FROM mail_outbox WHERE deleted_at IS NULL AND sent_at IS NULL AND attempts<10 AND available_at<=now() ORDER BY created_at LIMIT 10 FOR UPDATE SKIP LOCKED) RETURNING id,recipient,subject,body,html_body").load::<Mail>(self.connection)?)
    }
    pub(crate) fn mail_mark_delivered(&mut self, id: Uuid) -> Result<usize> {
        Ok(sql_query(
            "UPDATE mail_outbox SET sent_at=now(),body='[delivered]',html_body=NULL WHERE deleted_at IS NULL AND id=$1",
        )
        .bind::<SqlUuid, _>(id)
        .execute(self.connection)?)
    }
}
