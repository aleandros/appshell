use crate::{
    AppState,
    contexts::identity as service,
    error::Result,
    http::error::ApiJson,
    http::security::{self, Auth},
    models::*,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header::SET_COOKIE},
};
#[utoipa::path(post,path="/api/auth/signup",request_body=Signup,responses((status=200,body=Session)))]
pub async fn signup(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<Signup>,
) -> Result<([(axum::http::HeaderName, String); 1], Json<Session>)> {
    let production = state.config.production;
    let (token, result) = service::signup(state, input).await?;
    Ok((
        [(SET_COOKIE, security::cookie(&token, production, false))],
        Json(result),
    ))
}
#[utoipa::path(post,path="/api/auth/login",request_body=Login,responses((status=200,body=Session)))]
pub async fn login(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<Login>,
) -> Result<([(axum::http::HeaderName, String); 1], Json<Session>)> {
    let production = state.config.production;
    let (token, result) = service::login(state, input).await?;
    Ok((
        [(SET_COOKIE, security::cookie(&token, production, false))],
        Json(result),
    ))
}
#[utoipa::path(get,path="/api/auth/session",responses((status=200,body=Session)))]
pub async fn me(State(state): State<AppState>, Auth(user): Auth) -> Result<Json<Session>> {
    Ok(Json(service::me(state, user).await?))
}
#[utoipa::path(post,path="/api/auth/logout",responses((status=200,body=Message)))]
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<([(axum::http::HeaderName, String); 1], Json<Message>)> {
    let production = state.config.production;
    let (token, result) = service::logout(state, security::session_token(&headers)).await?;
    Ok((
        [(SET_COOKIE, security::cookie(&token, production, true))],
        Json(result),
    ))
}
#[utoipa::path(post,path="/api/auth/forgot-password",request_body=EmailInput,responses((status=200,body=Message)))]
pub async fn forgot(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<EmailInput>,
) -> Result<Json<Message>> {
    Ok(Json(service::forgot(state, input).await?))
}
#[utoipa::path(post,path="/api/auth/reset-password",request_body=ResetPassword,responses((status=200,body=Message)))]
pub async fn reset(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<ResetPassword>,
) -> Result<Json<Message>> {
    Ok(Json(service::reset(state, input).await?))
}
#[utoipa::path(post,path="/api/auth/verify-email",request_body=TokenInput,responses((status=200,body=Message)))]
pub async fn verify(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<TokenInput>,
) -> Result<Json<Message>> {
    Ok(Json(service::verify(state, input).await?))
}
#[utoipa::path(post,path="/api/auth/resend-verification",responses((status=200,body=Message)))]
pub async fn resend(State(state): State<AppState>, Auth(user): Auth) -> Result<Json<Message>> {
    Ok(Json(service::resend(state, user).await?))
}
#[utoipa::path(post,path="/api/auth/change-password",request_body=ChangePassword,responses((status=200,body=Message)))]
pub async fn change_password(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(input): ApiJson<ChangePassword>,
) -> Result<Json<Message>> {
    Ok(Json(service::change_password(state, user, input).await?))
}
#[utoipa::path(post,path="/api/auth/change-email",request_body=ChangeEmail,responses((status=200,body=Message)))]
pub async fn change_email(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(input): ApiJson<ChangeEmail>,
) -> Result<Json<Message>> {
    Ok(Json(service::change_email(state, user, input).await?))
}
#[utoipa::path(post,path="/api/auth/confirm-email",request_body=TokenInput,responses((status=200,body=Message)))]
pub async fn confirm_email(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<TokenInput>,
) -> Result<Json<Message>> {
    Ok(Json(service::confirm_email(state, input).await?))
}
