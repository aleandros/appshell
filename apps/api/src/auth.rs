use crate::{
    AppState, db,
    error::{ApiError, ApiJson, Result},
    mail,
    models::*,
    security::{self, Auth},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header::SET_COOKIE},
};
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Nullable, Text, Uuid as SqlUuid},
};
use uuid::Uuid;

pub fn session(c: &mut PgConnection, user: User) -> Result<Session> {
    let organizations = sql_query("SELECT o.id,o.name,m.role FROM organizations o JOIN memberships m ON m.organization_id=o.id WHERE m.user_id=$1 ORDER BY o.created_at")
        .bind::<SqlUuid,_>(user.id).load::<Organization>(c)?;
    Ok(Session {
        user,
        organizations,
    })
}
fn new_session(c: &mut PgConnection, id: Uuid, token: &str) -> Result<()> {
    sql_query("INSERT INTO sessions(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '14 days')")
        .bind::<Text,_>(security::digest(token)).bind::<SqlUuid,_>(id).execute(c)?;
    Ok(())
}
pub fn message(text: &str) -> Json<Message> {
    Json(Message {
        message: text.into(),
    })
}

#[utoipa::path(post,path="/api/auth/signup",request_body=Signup,responses((status=200,body=Session)))]
pub async fn signup(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<Signup>,
) -> Result<([(axum::http::HeaderName, String); 1], Json<Session>)> {
    let email = security::email(&input.email)?;
    let name = security::name(&input.name)?;
    let organization = security::name(&input.organization)?;
    security::password(&input.password)?;
    let token = security::token();
    let cookie = security::cookie(&token, state.config.production, false);
    let result = db::run(state.pool,move |c| {
        security::rate_limit(c,&format!("signup:{email}"),5)?;
        let hash = security::hash_password(&input.password)?;
        c.transaction::<_,ApiError,_>(|c| {
            let id = Uuid::new_v4(); let org = Uuid::new_v4();
            let user = sql_query("INSERT INTO users(id,email,name,password_hash) VALUES($1,$2,$3,$4) RETURNING id,email,name,email_verified_at")
                .bind::<SqlUuid,_>(id).bind::<Text,_>(&email).bind::<Text,_>(name).bind::<Text,_>(hash).get_result::<User>(c)?;
            sql_query("INSERT INTO organizations(id,name) VALUES($1,$2)").bind::<SqlUuid,_>(org).bind::<Text,_>(organization).execute(c)?;
            sql_query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'owner')").bind::<SqlUuid,_>(org).bind::<SqlUuid,_>(id).execute(c)?;
            sql_query("INSERT INTO subscriptions(organization_id) VALUES($1)").bind::<SqlUuid,_>(org).execute(c)?;
            mail::action(c,id,&email,"verify",None,&state.config.app_url)?;
            new_session(c,id,&token)?; session(c,user)
        })
    }).await?;
    Ok(([(SET_COOKIE, cookie)], Json(result)))
}
#[derive(QueryableByName)]
struct Credentials {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=Nullable<Text>)]
    password_hash: Option<String>,
}
fn check_password(c: &mut PgConnection, id: Uuid, password: &str) -> Result<()> {
    let hash = sql_query(
        "SELECT password_hash AS value FROM users WHERE id=$1 AND password_hash IS NOT NULL",
    )
    .bind::<SqlUuid, _>(id)
    .get_result::<TextValue>(c)
    .optional()?;
    if !hash.is_some_and(|h| security::verify_password(password, &h.value)) {
        return Err(ApiError::bad("The current password is incorrect."));
    }
    Ok(())
}
#[utoipa::path(post,path="/api/auth/login",request_body=Login,responses((status=200,body=Session)))]
pub async fn login(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<Login>,
) -> Result<([(axum::http::HeaderName, String); 1], Json<Session>)> {
    let email = security::email(&input.email)?;
    if input.password.len() > 128 {
        return Err(ApiError::unauthorized());
    }
    let token = security::token();
    let cookie = security::cookie(&token, state.config.production, false);
    let result = db::run(state.pool, move |c| {
        security::rate_limit(c, &format!("login:{email}"), 20)?;
        let credentials = sql_query("SELECT id,password_hash FROM users WHERE email=$1")
            .bind::<Text, _>(email)
            .get_result::<Credentials>(c)
            .optional()?;
        // Perform the same expensive operation for unknown accounts.
        let dummy = state.dummy_password_hash.as_str();
        let valid = security::verify_password(
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
        c.transaction::<_, ApiError, _>(|c| {
            new_session(c, id, &token)?;
            let user = sql_query("SELECT id,email,name,email_verified_at FROM users WHERE id=$1")
                .bind::<SqlUuid, _>(id)
                .get_result::<User>(c)?;
            session(c, user)
        })
    })
    .await?;
    Ok(([(SET_COOKIE, cookie)], Json(result)))
}
#[utoipa::path(get,path="/api/auth/session",responses((status=200,body=Session)))]
pub async fn me(State(state): State<AppState>, Auth(user): Auth) -> Result<Json<Session>> {
    Ok(Json(db::run(state.pool, move |c| session(c, user)).await?))
}
#[utoipa::path(post,path="/api/auth/logout",responses((status=200,body=Message)))]
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<([(axum::http::HeaderName, String); 1], Json<Message>)> {
    if let Some(token) = security::session_token(&headers) {
        db::run(state.pool, move |c| {
            sql_query("DELETE FROM sessions WHERE token_hash=$1")
                .bind::<Text, _>(security::digest(&token))
                .execute(c)?;
            Ok(())
        })
        .await?;
    }
    Ok((
        [(
            SET_COOKIE,
            security::cookie("", state.config.production, true),
        )],
        message("Signed out."),
    ))
}
#[utoipa::path(post,path="/api/auth/forgot-password",request_body=EmailInput,responses((status=200,body=Message)))]
pub async fn forgot(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<EmailInput>,
) -> Result<Json<Message>> {
    let email = security::email(&input.email)?;
    db::run(state.pool,move |c| {
        security::rate_limit(c,&format!("forgot:{email}"),5)?;
        c.transaction::<_,ApiError,_>(|c| {
            let user=sql_query("SELECT id,email,name,email_verified_at FROM users WHERE email=$1 AND password_hash IS NOT NULL").bind::<Text,_>(&email).get_result::<User>(c).optional()?;
            if let Some(user)=user { mail::action(c,user.id,&email,"reset",None,&state.config.app_url)?; } Ok(())
        })
    }).await?;
    Ok(message("If an account exists, a reset link is on its way."))
}
#[derive(QueryableByName)]
struct Action {
    #[diesel(sql_type=SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type=Nullable<Text>)]
    payload: Option<String>,
}
fn consume(c: &mut PgConnection, token: &str, purpose: &str) -> Result<Action> {
    if token.len() != 64 {
        return Err(ApiError::bad("This link is invalid or has expired."));
    }
    sql_query("DELETE FROM action_tokens WHERE token_hash=$1 AND purpose=$2 AND expires_at>now() RETURNING user_id,payload")
        .bind::<Text,_>(security::digest(token)).bind::<Text,_>(purpose).get_result::<Action>(c).optional()?.ok_or_else(|| ApiError::bad("This link is invalid or has expired."))
}
#[utoipa::path(post,path="/api/auth/reset-password",request_body=ResetPassword,responses((status=200,body=Message)))]
pub async fn reset(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<ResetPassword>,
) -> Result<Json<Message>> {
    security::password(&input.password)?;
    db::run(state.pool, move |c| {
        security::rate_limit(c, "reset:global", 100)?;
        c.transaction::<_, ApiError, _>(|c| {
            let action = consume(c, &input.token, "reset")?;
            let hash = security::hash_password(&input.password)?;
            sql_query("UPDATE users SET password_hash=$1 WHERE id=$2")
                .bind::<Text, _>(hash)
                .bind::<SqlUuid, _>(action.user_id)
                .execute(c)?;
            revoke(c, action.user_id)?;
            Ok(())
        })
    })
    .await?;
    Ok(message("Password updated. Sign in with your new password."))
}
#[utoipa::path(post,path="/api/auth/verify-email",request_body=TokenInput,responses((status=200,body=Message)))]
pub async fn verify(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<TokenInput>,
) -> Result<Json<Message>> {
    db::run(state.pool, move |c| {
        c.transaction::<_, ApiError, _>(|c| {
            let action = consume(c, &input.token, "verify")?;
            sql_query("UPDATE users SET email_verified_at=now() WHERE id=$1")
                .bind::<SqlUuid, _>(action.user_id)
                .execute(c)?;
            Ok(())
        })
    })
    .await?;
    Ok(message("Email verified. Your workspace is ready."))
}
#[utoipa::path(post,path="/api/auth/resend-verification",responses((status=200,body=Message)))]
pub async fn resend(State(state): State<AppState>, Auth(user): Auth) -> Result<Json<Message>> {
    db::run(state.pool, move |c| {
        security::rate_limit(c, &format!("verify:{}", user.id), 5)?;
        if user.email_verified_at.is_none() {
            c.transaction::<_, ApiError, _>(|c| {
                mail::action(
                    c,
                    user.id,
                    &user.email,
                    "verify",
                    None,
                    &state.config.app_url,
                )
            })?;
        }
        Ok(())
    })
    .await?;
    Ok(message("Check your inbox for a verification link."))
}
fn revoke(c: &mut PgConnection, id: Uuid) -> Result<()> {
    sql_query("DELETE FROM sessions WHERE user_id=$1")
        .bind::<SqlUuid, _>(id)
        .execute(c)?;
    sql_query("DELETE FROM action_tokens WHERE user_id=$1")
        .bind::<SqlUuid, _>(id)
        .execute(c)?;
    Ok(())
}
#[utoipa::path(post,path="/api/auth/change-password",request_body=ChangePassword,responses((status=200,body=Message)))]
pub async fn change_password(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(input): ApiJson<ChangePassword>,
) -> Result<Json<Message>> {
    security::password(&input.password)?;
    db::run(state.pool, move |c| {
        security::rate_limit(c, &format!("credentials:{}", user.id), 10)?;
        c.transaction::<_, ApiError, _>(|c| {
            // Lock the account so concurrent password changes cannot both verify an old password.
            sql_query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
                .bind::<SqlUuid, _>(user.id)
                .execute(c)?;
            check_password(c, user.id, &input.current_password)?;
            sql_query("UPDATE users SET password_hash=$1 WHERE id=$2")
                .bind::<Text, _>(security::hash_password(&input.password)?)
                .bind::<SqlUuid, _>(user.id)
                .execute(c)?;
            revoke(c, user.id)
        })
    })
    .await?;
    Ok(message("Password changed. Sign in again on your devices."))
}
#[utoipa::path(post,path="/api/auth/change-email",request_body=ChangeEmail,responses((status=200,body=Message)))]
pub async fn change_email(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(input): ApiJson<ChangeEmail>,
) -> Result<Json<Message>> {
    security::verified(&user)?;
    let email = security::email(&input.email)?;
    db::run(state.pool,move |c| {
        security::rate_limit(c,&format!("credentials:{}",user.id),10)?;
        c.transaction::<_,ApiError,_>(|c| {
            check_password(c,user.id,&input.password)?;
            mail::action(c,user.id,&email,"email",Some(&email),&state.config.app_url)?;
            mail::enqueue(c,&user.email,"Email change requested","A request was made to change your AppShell email address. If this was not you, reset your password to invalidate the request.")
        })
    }).await?;
    Ok(message("Confirm the link sent to your new email address."))
}
#[utoipa::path(post,path="/api/auth/confirm-email",request_body=TokenInput,responses((status=200,body=Message)))]
pub async fn confirm_email(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<TokenInput>,
) -> Result<Json<Message>> {
    db::run(state.pool, move |c| {
        c.transaction::<_, ApiError, _>(|c| {
            let action = consume(c, &input.token, "email")?;
            sql_query("UPDATE users SET email=$1,email_verified_at=now() WHERE id=$2")
                .bind::<Text, _>(
                    action
                        .payload
                        .ok_or_else(|| ApiError::bad("Invalid email change request."))?,
                )
                .bind::<SqlUuid, _>(action.user_id)
                .execute(c)?;
            revoke(c, action.user_id)
        })
    })
    .await?;
    Ok(message("Email updated. Please sign in again."))
}
