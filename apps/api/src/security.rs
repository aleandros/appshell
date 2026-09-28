use crate::{
    AppState, db,
    error::{ApiError, Result},
    models::{Count, TextValue, User},
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, StatusCode, request::Parts},
    middleware::Next,
    response::Response,
};
use diesel::{prelude::*, sql_query, sql_types::Text};
use sha2::{Digest, Sha256};
use validator::ValidateEmail;

pub fn token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
pub fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
pub fn email(value: &str) -> Result<String> {
    let value = value.trim().to_lowercase();
    if value.len() > 254 || !value.validate_email() {
        return Err(ApiError::bad("Enter a valid email address."));
    }
    Ok(value)
}
pub fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 {
        return Err(ApiError::bad("Use between 1 and 80 characters for names."));
    }
    Ok(value.into())
}
pub fn password(value: &str) -> Result<()> {
    if value.chars().count() < 12 || value.len() > 128 {
        return Err(ApiError::bad(
            "Use a password with at least 12 characters and at most 128 bytes.",
        ));
    }
    Ok(())
}
pub fn hash_password(value: &str) -> Result<String> {
    password(value)?;
    Argon2::default()
        .hash_password(
            value.as_bytes(),
            &SaltString::generate(&mut rand_core::OsRng),
        )
        .map(|v| v.to_string())
        .map_err(ApiError::internal)
}
pub fn verify_password(value: &str, hash: &str) -> bool {
    value.len() <= 128
        && PasswordHash::new(hash).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(value.as_bytes(), &hash)
                .is_ok()
        })
}
pub fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("cookie")?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == "appshell_session" && value.len() == 64).then(|| value.to_owned())
        })
}
pub fn cookie(value: &str, production: bool, clear: bool) -> String {
    format!(
        "appshell_session={value}; HttpOnly; Path=/; SameSite=Lax; Max-Age={}{}",
        if clear { 0 } else { 60 * 60 * 24 * 14 },
        if production { "; Secure" } else { "" }
    )
}

pub struct Auth(pub User);
impl FromRequestParts<AppState> for Auth {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        let hash = digest(&session_token(&parts.headers).ok_or_else(ApiError::unauthorized)?);
        db::run(state.pool.clone(), move |c| {
            sql_query("SELECT u.id,u.email,u.name,u.email_verified_at FROM users u JOIN sessions s ON s.user_id=u.id WHERE s.token_hash=$1 AND s.expires_at>now()")
                .bind::<Text,_>(hash).get_result::<User>(c).optional()?.map(Self).ok_or_else(ApiError::unauthorized)
        }).await
    }
}
pub fn verified(user: &User) -> Result<()> {
    if user.email_verified_at.is_none() {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "email_unverified",
            "Verify your email before continuing.",
        ));
    }
    Ok(())
}
pub fn role(c: &mut PgConnection, user: uuid::Uuid, org: uuid::Uuid) -> Result<String> {
    sql_query("SELECT role AS value FROM memberships WHERE user_id=$1 AND organization_id=$2")
        .bind::<diesel::sql_types::Uuid, _>(user)
        .bind::<diesel::sql_types::Uuid, _>(org)
        .get_result::<TextValue>(c)
        .optional()?
        .map(|r| r.value)
        .ok_or_else(ApiError::forbidden)
}
pub fn admin(c: &mut PgConnection, user: uuid::Uuid, org: uuid::Uuid) -> Result<()> {
    if !["owner", "admin"].contains(&role(c, user, org)?.as_str()) {
        return Err(ApiError::forbidden());
    }
    Ok(())
}
pub fn owner(c: &mut PgConnection, user: uuid::Uuid, org: uuid::Uuid) -> Result<()> {
    if role(c, user, org)? != "owner" {
        return Err(ApiError::forbidden());
    }
    Ok(())
}
pub fn rate_limit(c: &mut PgConnection, key: &str, limit: i32) -> Result<()> {
    let count = sql_query("INSERT INTO rate_limits(key,hits,expires_at) VALUES($1,1,now()+interval '15 minutes') ON CONFLICT(key) DO UPDATE SET hits=CASE WHEN rate_limits.expires_at<now() THEN 1 ELSE rate_limits.hits+1 END, expires_at=CASE WHEN rate_limits.expires_at<now() THEN now()+interval '15 minutes' ELSE rate_limits.expires_at END RETURNING hits::bigint AS count")
        .bind::<Text,_>(digest(key)).get_result::<Count>(c)?.count;
    if count > i64::from(limit) {
        return Err(ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many attempts. Please try again in 15 minutes.",
        ));
    }
    Ok(())
}
pub async fn guard(State(state): State<AppState>, req: Request, next: Next) -> Result<Response> {
    let unsafe_method = !matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    );
    let path = req
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .map_or(req.uri().path(), |original| original.0.path());
    if unsafe_method && path != "/api/webhooks/stripe" {
        if req.headers().get("origin").and_then(|v| v.to_str().ok()) != Some(&state.config.app_url)
        {
            return Err(ApiError::forbidden());
        }
        // Shared across replicas, and independent of attacker-controlled forwarded headers.
        db::run(state.pool.clone(), |c| {
            rate_limit(c, "global:mutations", 2000)
        })
        .await?;
    }
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
        .headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    response
        .headers_mut()
        .insert("x-frame-options", "DENY".parse().unwrap());
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_are_validated() {
        assert_eq!(
            email("  PERSON@Example.com ").unwrap(),
            "person@example.com"
        );
        assert!(email("oops").is_err());
        assert!(password("short").is_err());
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("different", &hash));
    }
    #[test]
    fn secrets_and_cookies() {
        let value = token();
        assert_eq!(value.len(), 64);
        assert_ne!(value, token());
        assert_ne!(digest(&value), value);
        assert!(cookie(&value, true, false).contains("Secure"));
        assert!(cookie("", true, true).contains("Max-Age=0"));
    }
}
