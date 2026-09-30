use crate::infrastructure::{
    email_template::{self, Action},
    repositories::{self, UnitOfWork},
    rows::Mail,
};
use crate::{
    AppState,
    config::Config,
    error::{ApiError, Result},
};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::MultiPart};
use uuid::Uuid;

fn enqueue(
    c: &mut UnitOfWork<'_>,
    recipient: &str,
    subject: &str,
    body: &str,
    action: Option<Action<'_>>,
    config: &Config,
) -> Result<()> {
    let email = email_template::render(&config.mail_brand, subject, body, action)?;
    let id = c.mail_enqueue(recipient, subject, email.text, email.html)?;
    if config.job_backend != crate::config::JobBackend::Disabled {
        let job_id = super::job_queue::enqueue(
            c,
            config,
            super::job_queue::Job::DeliverMail { mail_id: id },
            0,
        )?;
        c.mail_set_job(id, job_id)?;
    }
    Ok(())
}
pub(crate) fn action(
    c: &mut UnitOfWork<'_>,
    user: Uuid,
    recipient: &str,
    purpose: &str,
    payload: Option<&str>,
    config: &Config,
) -> Result<()> {
    let token = crate::infrastructure::crypto::token();
    c.identity_issue_action(
        crate::infrastructure::crypto::digest(&token),
        user,
        purpose,
        payload,
    )?;
    let (path, subject, introduction) = match purpose {
        "verify" => (
            "verify-email",
            "Verify your email",
            "Welcome! Confirm your email address to activate your workspace.",
        ),
        "reset" => (
            "reset-password",
            "Reset your password",
            "We received a request to reset your password.",
        ),
        "email" => (
            "confirm-email",
            "Confirm your new email",
            "Confirm this address to finish changing your account email.",
        ),
        _ => return Err(ApiError::bad("Unknown email action.")),
    };
    enqueue(
        c,
        recipient,
        subject,
        &format!(
            "{introduction}\n\nThis link expires in one hour. If you did not request this, ignore this email."
        ),
        Some(Action {
            label: subject,
            url: &format!("{}/{path}#token={token}", config.app_url),
        }),
        config,
    )
}
pub(crate) fn invitation(
    c: &mut UnitOfWork<'_>,
    recipient: &str,
    inviter: &str,
    token: &str,
    config: &Config,
) -> Result<()> {
    enqueue(
        c,
        recipient,
        "You're invited to a workspace",
        &format!(
            "{inviter} invited you to their workspace.\n\nSign in or create an account with {recipient} to accept. This invitation expires in 7 days."
        ),
        Some(Action {
            label: "Accept invitation",
            url: &format!("{}/accept-invite#token={token}", config.app_url),
        }),
        config,
    )
}
pub(crate) fn email_change_notice(
    c: &mut UnitOfWork<'_>,
    recipient: &str,
    config: &Config,
) -> Result<()> {
    enqueue(
        c,
        recipient,
        "Email change requested",
        &format!(
            "A request was made to change your {} email address. If this was not you, reset your password to invalidate the request.",
            config.mail_brand
        ),
        Some(Action {
            label: "Secure your account",
            url: &format!("{}/forgot-password", config.app_url),
        }),
        config,
    )
}

fn smtp_message(config: &Config, mail: &Mail, html: String) -> Result<Message> {
    Message::builder()
        .from(config.mail_from.parse().map_err(ApiError::internal)?)
        .to(mail.recipient.parse().map_err(ApiError::internal)?)
        .subject(&mail.subject)
        .message_id(Some(format!("{}@appshell.local", mail.id)))
        .multipart(MultiPart::alternative_plain_html(mail.body.clone(), html))
        .map_err(ApiError::internal)
}
async fn deliver(state: &AppState, mail: &Mail) -> Result<()> {
    let html = match &mail.html_body {
        Some(html) => html.clone(),
        None => {
            email_template::render(&state.config.mail_brand, &mail.subject, &mail.body, None)?.html
        }
    };
    match state.config.mail_mode.as_str() {
        "console" if !state.config.production => {
            tracing::info!(to=%mail.recipient, body=%mail.body, "development email");
            Ok(())
        }
        "smtp" if !state.config.production => {
            // Deliberately local/dev only. Production remains on authenticated HTTPS Resend.
            let transport =
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&state.config.smtp_host)
                    .port(state.config.smtp_port)
                    .timeout(Some(std::time::Duration::from_secs(20)))
                    .build();
            transport
                .send(smtp_message(&state.config, mail, html)?)
                .await
                .map_err(ApiError::internal)?;
            Ok(())
        }
        "resend" => {
            state.http.post("https://api.resend.com/emails")
                .bearer_auth(state.config.resend_key.as_deref().unwrap_or_default())
                .header("Idempotency-Key", mail.id.to_string())
                .json(&serde_json::json!({"from":state.config.mail_from,"to":[mail.recipient],"subject":mail.subject,"text":mail.body,"html":html}))
                .send().await.map_err(ApiError::internal)?
                .error_for_status().map_err(ApiError::internal)?;
            Ok(())
        }
        _ => Err(ApiError::bad(
            "Invalid email transport for this environment.",
        )),
    }
}
pub(crate) async fn deliver_queued(state: &AppState, id: Uuid) -> Result<()> {
    // Completed/deleted mail is a no-op on redelivery. Resend additionally receives
    // a stable idempotency key for a crash between provider acceptance and this write.
    let Some(mail) = repositories::run(state.pool.clone(), move |c| c.mail_pending(id)).await?
    else {
        return Ok(());
    };
    deliver(state, &mail).await?;
    repositories::run(state.pool.clone(), move |c| c.mail_mark_delivered(id)).await?;
    Ok(())
}

pub async fn tick(state: &AppState) -> Result<()> {
    let mails = repositories::run(state.pool.clone(), |c| c.mail_claim()).await?;
    for mail in mails {
        // One invalid message must not prevent other jobs from being delivered.
        match deliver(state, &mail).await {
            Ok(()) => {
                repositories::run(state.pool.clone(), move |c| {
                    c.mail_mark_delivered(mail.id)?;
                    Ok(())
                })
                .await?;
            }
            Err(error) => {
                tracing::warn!(mail_id=%mail.id, ?error,"mail queued for retry; check provider configuration")
            }
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
