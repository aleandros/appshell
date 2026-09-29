use crate::infrastructure::repositories;
use crate::infrastructure::repositories::UnitOfWork;
use crate::infrastructure::rows::Action;
use crate::{
    AppState,
    contexts::identity as security,
    error::{ApiError, Result},
    infrastructure::crypto,
    mail,
    models::*,
};
use uuid::Uuid;

pub fn session(c: &mut UnitOfWork<'_>, user: User) -> Result<Session> {
    let organizations = crate::contexts::organizations::for_user(c, user.id)?;
    Ok(Session {
        user,
        organizations,
    })
}
fn new_session(c: &mut UnitOfWork<'_>, id: Uuid, token: &str) -> Result<()> {
    c.identity_create_session(crypto::digest(token), id)?;
    Ok(())
}
use crate::models::message;
pub async fn signup(state: AppState, input: Signup) -> Result<(String, Session)> {
    let email = appshell_domain::identity::email(&input.email)?;
    let name = appshell_domain::identity::name(&input.name)?;
    let organization = appshell_domain::identity::name(&input.organization)?;
    appshell_domain::identity::password(&input.password)?;
    let token = crypto::token();
    let session_token = token.clone();

    let result = repositories::run(state.pool, move |c| {
        security::rate_limit(c, &format!("signup:{email}"), 5)?;
        let hash = crypto::hash_password(&input.password)?;
        c.transaction(|c| {
            let id = crate::infrastructure::crypto::id();

            let user = c.identity_create_user(id, &email, name, hash)?;
            crate::contexts::organizations::create_for_owner(c, id, organization)?;
            mail::action(c, id, &email, "verify", None, &state.config)?;
            new_session(c, id, &session_token)?;
            session(c, user)
        })
    })
    .await?;
    Ok((token, result))
}

fn check_password(c: &mut UnitOfWork<'_>, id: Uuid, password: &str) -> Result<()> {
    let hash = c.identity_password_hash(id)?;
    if !hash.is_some_and(|h| crypto::verify_password(password, &h.value)) {
        return Err(ApiError::bad("The current password is incorrect."));
    }
    Ok(())
}
pub async fn login(state: AppState, input: Login) -> Result<(String, Session)> {
    let email = appshell_domain::identity::email(&input.email)?;
    if input.password.len() > 128 {
        return Err(ApiError::unauthorized());
    }
    let token = crypto::token();
    let session_token = token.clone();

    let result = repositories::run(state.pool, move |c| {
        security::rate_limit(c, &format!("login:{email}"), 20)?;
        let credentials = c.identity_credentials(email)?;
        // Perform the same expensive operation for unknown accounts.
        let dummy = state.dummy_password_hash.as_str();
        let valid = crypto::verify_password(
            &input.password,
            credentials
                .as_ref()
                .and_then(|v| v.password_hash.as_deref())
                .unwrap_or(dummy),
        );
        let id = credentials
            .filter(|_| valid)
            .ok_or_else(ApiError::unauthorized)?
            .id;
        c.transaction(|c| {
            new_session(c, id, &session_token)?;
            let user = c.identity_user(id)?;
            session(c, user)
        })
    })
    .await?;
    Ok((token, result))
}
pub async fn me(state: AppState, user: User) -> Result<Session> {
    repositories::run(state.pool, move |c| session(c, user)).await
}
pub async fn logout(state: AppState, token: Option<String>) -> Result<(String, Message)> {
    if let Some(token) = token {
        repositories::run(state.pool, move |c| {
            c.identity_delete_session(crypto::digest(&token))?;
            Ok(())
        })
        .await?;
    }
    Ok((String::new(), message("Signed out.")))
}
pub async fn forgot(state: AppState, input: EmailInput) -> Result<Message> {
    let email = appshell_domain::identity::email(&input.email)?;
    repositories::run(state.pool, move |c| {
        security::rate_limit(c, &format!("forgot:{email}"), 5)?;
        c.transaction(|c| {
            let user = c.identity_password_user(&email)?;
            if let Some(user) = user {
                mail::action(c, user.id, &email, "reset", None, &state.config)?;
            }
            Ok(())
        })
    })
    .await?;
    Ok(message("If an account exists, a reset link is on its way."))
}

fn consume(c: &mut UnitOfWork<'_>, token: &str, purpose: &str) -> Result<Action> {
    appshell_domain::identity::action_token(token)?;
    c.identity_consume_action(crypto::digest(token), purpose)?
        .ok_or_else(|| ApiError::bad("This link is invalid or has expired."))
}
pub async fn reset(state: AppState, input: ResetPassword) -> Result<Message> {
    appshell_domain::identity::password(&input.password)?;
    repositories::run(state.pool, move |c| {
        security::rate_limit(c, "reset:global", 100)?;
        c.transaction(|c| {
            let action = consume(c, &input.token, "reset")?;
            let hash = crypto::hash_password(&input.password)?;
            c.identity_update_password(hash, action.user_id)?;
            revoke(c, action.user_id)?;
            Ok(())
        })
    })
    .await?;
    Ok(message("Password updated. Sign in with your new password."))
}
pub async fn verify(state: AppState, input: TokenInput) -> Result<Message> {
    repositories::run(state.pool, move |c| {
        c.transaction(|c| {
            let action = consume(c, &input.token, "verify")?;
            c.identity_verify_email(action.user_id)?;
            Ok(())
        })
    })
    .await?;
    Ok(message("Email verified. Your workspace is ready."))
}
pub async fn resend(state: AppState, user: User) -> Result<Message> {
    repositories::run(state.pool, move |c| {
        security::rate_limit(c, &format!("verify:{}", user.id), 5)?;
        if user.email_verified_at.is_none() {
            c.transaction(|c| {
                mail::action(c, user.id, &user.email, "verify", None, &state.config)
            })?;
        }
        Ok(())
    })
    .await?;
    Ok(message("Check your inbox for a verification link."))
}
fn revoke(c: &mut UnitOfWork<'_>, id: Uuid) -> Result<()> {
    c.identity_revoke_sessions(id)?;
    c.identity_revoke_actions(id)?;
    Ok(())
}
pub async fn change_password(
    state: AppState,
    user: User,
    input: ChangePassword,
) -> Result<Message> {
    appshell_domain::identity::password(&input.password)?;
    repositories::run(state.pool, move |c| {
        security::rate_limit(c, &format!("credentials:{}", user.id), 10)?;
        c.transaction(|c| {
            // Lock the account so concurrent password changes cannot both verify an old password.
            c.identity_lock_user(user.id)?;
            check_password(c, user.id, &input.current_password)?;
            c.identity_update_password(crypto::hash_password(&input.password)?, user.id)?;
            revoke(c, user.id)
        })
    })
    .await?;
    Ok(message("Password changed. Sign in again on your devices."))
}
pub async fn change_email(state: AppState, user: User, input: ChangeEmail) -> Result<Message> {
    security::verified(&user)?;
    let email = appshell_domain::identity::email(&input.email)?;
    repositories::run(state.pool, move |c| {
        security::rate_limit(c, &format!("credentials:{}", user.id), 10)?;
        c.transaction(|c| {
            check_password(c, user.id, &input.password)?;
            mail::action(c, user.id, &email, "email", Some(&email), &state.config)?;
            mail::email_change_notice(c, &user.email, &state.config)
        })
    })
    .await?;
    Ok(message("Confirm the link sent to your new email address."))
}
pub async fn confirm_email(state: AppState, input: TokenInput) -> Result<Message> {
    repositories::run(state.pool, move |c| {
        c.transaction(|c| {
            let action = consume(c, &input.token, "email")?;
            c.identity_update_email(
                action
                    .payload
                    .ok_or_else(|| ApiError::bad("Invalid email change request."))?,
                action.user_id,
            )?;
            revoke(c, action.user_id)
        })
    })
    .await?;
    Ok(message("Email updated. Please sign in again."))
}
