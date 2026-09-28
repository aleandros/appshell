use crate::{AppState, db, error::Result};
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Text, Uuid as SqlUuid},
};
use uuid::Uuid;

pub fn enqueue(c: &mut PgConnection, recipient: &str, subject: &str, body: &str) -> Result<()> {
    sql_query("INSERT INTO mail_outbox(id,recipient,subject,body) VALUES($1,$2,$3,$4)")
        .bind::<SqlUuid, _>(Uuid::new_v4())
        .bind::<Text, _>(recipient)
        .bind::<Text, _>(subject)
        .bind::<Text, _>(body)
        .execute(c)?;
    Ok(())
}
pub fn action(
    c: &mut PgConnection,
    user: Uuid,
    recipient: &str,
    purpose: &str,
    payload: Option<&str>,
    app_url: &str,
) -> Result<()> {
    let token = crate::security::token();
    sql_query("INSERT INTO action_tokens(token_hash,user_id,purpose,payload,expires_at) VALUES($1,$2,$3,$4,now()+interval '1 hour') ON CONFLICT(user_id,purpose) DO UPDATE SET token_hash=excluded.token_hash,payload=excluded.payload,expires_at=excluded.expires_at")
        .bind::<Text,_>(crate::security::digest(&token)).bind::<SqlUuid,_>(user).bind::<Text,_>(purpose)
        .bind::<diesel::sql_types::Nullable<Text>,_>(payload).execute(c)?;
    let (path, subject) = match purpose {
        "verify" => ("verify-email", "Verify your email"),
        "reset" => ("reset-password", "Reset your password"),
        _ => ("confirm-email", "Confirm your new email"),
    };
    enqueue(
        c,
        recipient,
        subject,
        &format!(
            "{subject}\n\nOpen this link to continue:\n{app_url}/{path}#token={token}\n\nThis link expires in one hour. If you did not request this, ignore this email."
        ),
    )
}
#[derive(QueryableByName)]
struct Mail {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    recipient: String,
    #[diesel(sql_type = Text)]
    subject: String,
    #[diesel(sql_type = Text)]
    body: String,
}
pub async fn tick(state: &AppState) -> Result<()> {
    let mails = db::run(state.pool.clone(), |c| {
        Ok(sql_query("UPDATE mail_outbox SET attempts=attempts+1,available_at=now()+interval '5 minutes' WHERE id IN (SELECT id FROM mail_outbox WHERE sent_at IS NULL AND attempts<10 AND available_at<=now() ORDER BY created_at LIMIT 10 FOR UPDATE SKIP LOCKED) RETURNING id,recipient,subject,body").load::<Mail>(c)?)
    }).await?;
    for mail in mails {
        let delivered = if state.config.mail_mode == "console" {
            tracing::info!(to=%mail.recipient, body=%mail.body, "development email");
            true
        } else {
            match state.http.post("https://api.resend.com/emails")
                .bearer_auth(state.config.resend_key.as_deref().unwrap_or_default())
                .header("Idempotency-Key",mail.id.to_string())
                .json(&serde_json::json!({"from":state.config.mail_from,"to":[mail.recipient],"subject":mail.subject,"text":mail.body}))
                .send().await {
                    Ok(response) => response.status().is_success(),
                    Err(error) => { tracing::warn!(%error,"mail delivery failed"); false }
                }
        };
        if delivered {
            db::run(state.pool.clone(), move |c| {
                sql_query("UPDATE mail_outbox SET sent_at=now(),body='[delivered]' WHERE id=$1")
                    .bind::<SqlUuid, _>(mail.id)
                    .execute(c)?;
                Ok(())
            })
            .await?;
        } else {
            tracing::warn!(mail_id=%mail.id,"mail queued for retry; check provider configuration");
        }
    }
    Ok(())
}
pub async fn worker(state: AppState) {
    loop {
        if let Err(error) = tick(&state).await {
            tracing::error!(?error, "outbox worker failed");
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}
