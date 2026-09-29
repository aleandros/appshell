use crate::{
    AppState,
    contexts::identity,
    error::{ApiError, Result},
    models::User,
};
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, request::Parts},
    middleware::Next,
    response::Response,
};
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
        identity::throttle_requests(&state).await?;
    }
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        "x-content-type-options",
        axum::http::HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        "referrer-policy",
        axum::http::HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        "x-frame-options",
        axum::http::HeaderValue::from_static("DENY"),
    );
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}
pub struct Auth(pub User);
impl FromRequestParts<AppState> for Auth {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        let token = session_token(&parts.headers).ok_or_else(ApiError::unauthorized)?;
        Ok(Self(identity::authenticate(state, token).await?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cookies_remain_secure_and_tokens_are_bounded() {
        assert!(cookie("token", true, false).contains("; Secure"));
        assert!(cookie("token", true, false).contains("HttpOnly; Path=/; SameSite=Lax"));
        assert!(cookie("", true, true).contains("Max-Age=0"));
        let mut headers = HeaderMap::new();
        headers.insert("cookie", "appshell_session=short".parse().unwrap());
        assert!(session_token(&headers).is_none());
        let token = "a".repeat(64);
        headers.insert(
            "cookie",
            format!("other=x; appshell_session={token}")
                .parse()
                .unwrap(),
        );
        assert_eq!(session_token(&headers), Some(token));
    }
}
