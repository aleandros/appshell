use crate::infrastructure::repositories;
use crate::infrastructure::repositories::UnitOfWork;
use crate::{AppState, error::Result};
use uuid::Uuid;

pub(crate) fn enqueue(
    c: &mut UnitOfWork<'_>,
    recipient: &str,
    subject: &str,
    body: &str,
) -> Result<()> {
    c.mail_enqueue(Uuid::new_v4(), recipient, subject, body)?;
    Ok(())
}
pub(crate) fn action(
    c: &mut UnitOfWork<'_>,
    user: Uuid,
    recipient: &str,
    purpose: &str,
    payload: Option<&str>,
    app_url: &str,
) -> Result<()> {
    let token = crate::infrastructure::crypto::token();
    c.identity_issue_action(
        crate::infrastructure::crypto::digest(&token),
        user,
        purpose,
        payload,
    )?;
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

pub async fn tick(state: &AppState) -> Result<()> {
    let mails = repositories::run(state.pool.clone(), |c| c.mail_claim()).await?;
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
            repositories::run(state.pool.clone(), move |c| {
                c.mail_mark_delivered(mail.id)?;
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
