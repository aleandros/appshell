use crate::{
    AppState,
    contexts::admin as service,
    error::{ApiError, Result},
    http::{error::ApiJson, security},
    models::{
        AdminAccount, AdminSearch, CreateAdmin, HistoryEntry, Login, ManagedUser, Message,
        UpdateAdmin, UpdateUser,
    },
};
use axum::{
    Json,
    extract::{FromRequestParts, Path, Query, State},
    http::{HeaderMap, header::SET_COOKIE, request::Parts},
};
use uuid::Uuid;

pub(crate) struct AdminAuth(pub AdminAccount);
impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        let token = security::named_token(&parts.headers, "appshell_admin_session")
            .ok_or_else(ApiError::unauthorized)?;
        Ok(Self(service::authenticate(state, token).await?))
    }
}
#[expect(
    clippy::fn_params_excessive_bools,
    reason = "Cookie flags directly encode the existing production and expiry policy."
)]
fn cookie(token: &str, production: bool, clear: bool) -> String {
    format!(
        "appshell_admin_session={token}; HttpOnly; Path=/api/admin; SameSite=Strict; Max-Age={}{}",
        if clear { 0 } else { 8 * 60 * 60 },
        if production { "; Secure" } else { "" }
    )
}
#[utoipa::path(operation_id="admin_login",post,path="/api/admin/login",request_body=Login,responses((status=200,body=AdminAccount)))]
pub(crate) async fn login(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<Login>,
) -> Result<([(axum::http::HeaderName, String); 1], Json<AdminAccount>)> {
    let production = state.config.production;
    let (token, account) = service::login(state, input).await?;
    Ok((
        [(SET_COOKIE, cookie(&token, production, false))],
        Json(account),
    ))
}
#[utoipa::path(operation_id="admin_logout",post,path="/api/admin/logout",responses((status=200,body=Message)))]
pub(crate) async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<([(axum::http::HeaderName, String); 1], Json<Message>)> {
    let production = state.config.production;
    let result = service::logout(
        state,
        security::named_token(&headers, "appshell_admin_session"),
    )
    .await?;
    Ok(([(SET_COOKIE, cookie("", production, true))], Json(result)))
}
#[utoipa::path(operation_id="admin_session",get,path="/api/admin/session",responses((status=200,body=AdminAccount)))]
pub(crate) async fn session(AdminAuth(actor): AdminAuth) -> Json<AdminAccount> {
    Json(actor)
}
#[utoipa::path(operation_id="admin_users",get,path="/api/admin/users",params(("search"=Option<String>,Query),("offset"=Option<i64>,Query)),responses((status=200,body=Vec<ManagedUser>)))]
pub(crate) async fn users(
    State(state): State<AppState>,
    AdminAuth(actor): AdminAuth,
    Query(input): Query<AdminSearch>,
) -> Result<Json<Vec<ManagedUser>>> {
    Ok(Json(service::users(state, actor, input).await?))
}
#[utoipa::path(operation_id="admin_accounts",get,path="/api/admin/accounts",params(("search"=Option<String>,Query),("offset"=Option<i64>,Query)),responses((status=200,body=Vec<AdminAccount>)))]
pub(crate) async fn accounts(
    State(state): State<AppState>,
    AdminAuth(actor): AdminAuth,
    Query(input): Query<AdminSearch>,
) -> Result<Json<Vec<AdminAccount>>> {
    Ok(Json(service::accounts(state, actor, input).await?))
}
#[utoipa::path(operation_id="admin_create",post,path="/api/admin/accounts",request_body=CreateAdmin,responses((status=200,body=AdminAccount)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    AdminAuth(actor): AdminAuth,
    ApiJson(input): ApiJson<CreateAdmin>,
) -> Result<Json<AdminAccount>> {
    Ok(Json(service::create(state, actor, input).await?))
}
#[utoipa::path(operation_id="admin_update",post,path="/api/admin/accounts/{id}",params(("id"=Uuid,Path)),request_body=UpdateAdmin,responses((status=200,body=Message)))]
pub(crate) async fn update(
    State(state): State<AppState>,
    AdminAuth(actor): AdminAuth,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<UpdateAdmin>,
) -> Result<Json<Message>> {
    Ok(Json(service::update(state, actor, id, input).await?))
}
#[utoipa::path(operation_id="admin_update_user",post,path="/api/admin/users/{id}",params(("id"=Uuid,Path)),request_body=UpdateUser,responses((status=200,body=Message)))]
pub(crate) async fn update_user(
    State(state): State<AppState>,
    AdminAuth(actor): AdminAuth,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<UpdateUser>,
) -> Result<Json<Message>> {
    Ok(Json(service::update_user(state, actor, id, input).await?))
}
#[utoipa::path(operation_id="admin_reset_user",post,path="/api/admin/users/{id}/reset-password",params(("id"=Uuid,Path)),responses((status=200,body=Message)))]
pub(crate) async fn reset_user(
    State(state): State<AppState>,
    AdminAuth(actor): AdminAuth,
    Path(id): Path<Uuid>,
) -> Result<Json<Message>> {
    Ok(Json(service::reset_user(state, actor, id).await?))
}
#[utoipa::path(operation_id="admin_history",get,path="/api/admin/users/{id}/history",params(("id"=Uuid,Path),("offset"=Option<i64>,Query)),responses((status=200,body=Vec<HistoryEntry>)))]
pub(crate) async fn history(
    State(state): State<AppState>,
    AdminAuth(actor): AdminAuth,
    Path(id): Path<Uuid>,
    Query(input): Query<AdminSearch>,
) -> Result<Json<Vec<HistoryEntry>>> {
    Ok(Json(service::history(state, actor, id, input).await?))
}
