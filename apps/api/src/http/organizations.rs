use crate::{
    AppState, contexts::organizations as service, error::Result, http::error::ApiJson,
    http::security::Auth, models::*,
};
use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;
#[utoipa::path(post,path="/api/organizations",request_body=NameInput,responses((status=200,body=Organization)))]
pub async fn create(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(input): ApiJson<NameInput>,
) -> Result<Json<Organization>> {
    Ok(Json(service::create(state, user, input).await?))
}
#[utoipa::path(get,path="/api/organizations/{org}/team",params(("org"=Uuid,Path)),responses((status=200,body=Team)))]
pub async fn team(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path(org): Path<Uuid>,
) -> Result<Json<Team>> {
    Ok(Json(service::team(state, user, org).await?))
}
#[utoipa::path(post,path="/api/organizations/{org}/invitations",params(("org"=Uuid,Path)),request_body=InviteInput,responses((status=200,body=Message)))]
pub async fn invite(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path(org): Path<Uuid>,
    ApiJson(input): ApiJson<InviteInput>,
) -> Result<Json<Message>> {
    Ok(Json(service::invite(state, user, org, input).await?))
}
#[utoipa::path(post,path="/api/invitations/accept",request_body=TokenInput,responses((status=200,body=Message)))]
pub async fn accept(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(input): ApiJson<TokenInput>,
) -> Result<Json<Message>> {
    Ok(Json(service::accept(state, user, input).await?))
}
#[utoipa::path(delete,path="/api/organizations/{org}/invitations/{id}",params(("org"=Uuid,Path),("id"=Uuid,Path)),responses((status=200,body=Message)))]
pub async fn revoke_invite(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path((org, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Message>> {
    Ok(Json(service::revoke_invite(state, user, org, id).await?))
}
#[utoipa::path(delete,path="/api/organizations/{org}/members/{id}",params(("org"=Uuid,Path),("id"=Uuid,Path)),responses((status=200,body=Message)))]
pub async fn remove_member(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path((org, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Message>> {
    Ok(Json(service::remove_member(state, user, org, id).await?))
}
