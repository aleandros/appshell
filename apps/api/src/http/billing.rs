use crate::{
    AppState, contexts::billing as service, error::Result, http::security::Auth, models::*,
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
};
use uuid::Uuid;
#[utoipa::path(get,path="/api/organizations/{org}/billing",params(("org"=Uuid,Path)),responses((status=200,body=Billing)))]
pub async fn billing(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path(org): Path<Uuid>,
) -> Result<Json<Billing>> {
    Ok(Json(service::billing(state, user, org).await?))
}
#[utoipa::path(post,path="/api/organizations/{org}/checkout",params(("org"=Uuid,Path)),responses((status=200,body=RedirectUrl)))]
pub async fn checkout(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path(org): Path<Uuid>,
) -> Result<Json<RedirectUrl>> {
    Ok(Json(service::checkout(state, user, org).await?))
}
#[utoipa::path(post,path="/api/organizations/{org}/billing-portal",params(("org"=Uuid,Path)),responses((status=200,body=RedirectUrl)))]
pub async fn portal(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path(org): Path<Uuid>,
) -> Result<Json<RedirectUrl>> {
    Ok(Json(service::portal(state, user, org).await?))
}
pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Message>> {
    Ok(Json(
        service::webhook(
            state,
            headers
                .get("stripe-signature")
                .and_then(|s| s.to_str().ok())
                .unwrap_or_default()
                .to_owned(),
            body.to_vec(),
        )
        .await?,
    ))
}
