use crate::{
    AppState, db,
    error::{ApiError, Result},
    models::*,
    security::{self, Auth},
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
};
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Nullable, Text, Timestamptz, Uuid as SqlUuid},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

#[derive(QueryableByName)]
struct CheckoutAttempt {
    #[diesel(sql_type = Text)]
    request_key: String,
    #[diesel(sql_type = diesel::sql_types::Jsonb)]
    parameters: serde_json::Value,
}

#[utoipa::path(get,path="/api/organizations/{org}/billing",params(("org"=Uuid,Path)),responses((status=200,body=Billing)))]
pub async fn billing(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path(org): Path<Uuid>,
) -> Result<Json<Billing>> {
    security::verified(&user)?;
    let enabled = state.config.billing_enabled();
    Ok(Json(
        db::run(state.pool, move |c| {
            security::role(c, user.id, org)?;
            let subscription = sql_query(
                "SELECT plan,status,current_period_end FROM subscriptions WHERE organization_id=$1",
            )
            .bind::<SqlUuid, _>(org)
            .get_result::<Subscription>(c)?;
            let seats_used =
                sql_query("SELECT count(*) AS count FROM memberships WHERE organization_id=$1")
                    .bind::<SqlUuid, _>(org)
                    .get_result::<Count>(c)?
                    .count;
            Ok(Billing {
                subscription,
                seat_limit: crate::organizations::seat_limit(c, org)?,
                seats_used,
                billing_enabled: enabled,
            })
        })
        .await?,
    ))
}
#[utoipa::path(post,path="/api/organizations/{org}/checkout",params(("org"=Uuid,Path)),responses((status=200,body=RedirectUrl)))]
pub async fn checkout(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path(org): Path<Uuid>,
) -> Result<Json<RedirectUrl>> {
    security::verified(&user)?;
    if !state.config.billing_enabled() {
        return Err(ApiError::bad(
            "Billing hasn't been configured for this installation.",
        ));
    }
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(23);
    let parameters = vec![
        ("mode", "subscription".to_owned()),
        (
            "line_items[0][price]",
            state.config.stripe_price.clone().unwrap_or_default(),
        ),
        ("line_items[0][quantity]", "1".into()),
        ("customer_email", user.email.clone()),
        ("client_reference_id", org.to_string()),
        (
            "subscription_data[metadata][organization_id]",
            org.to_string(),
        ),
        (
            "success_url",
            format!("{}/app/billing", state.config.app_url),
        ),
        (
            "cancel_url",
            format!("{}/app/billing", state.config.app_url),
        ),
        ("expires_at", expires_at.timestamp().to_string()),
    ];
    let parameters = serde_json::to_value(parameters).map_err(ApiError::internal)?;
    let attempt = db::run(state.pool.clone(), move |c| {
        security::rate_limit(c, &format!("checkout:{org}"), 10)?;
        c.transaction::<_,ApiError,_>(|c| {
        crate::organizations::lock_org(c,org)?;
        security::owner(c, user.id, org)?;
        let sub = sql_query(
            "SELECT plan,status,current_period_end FROM subscriptions WHERE organization_id=$1",
        )
        .bind::<SqlUuid, _>(org)
        .get_result::<Subscription>(c)?;
        if sub.plan == "pro" && !["canceled", "incomplete_expired"].contains(&sub.status.as_str()) {
            return Err(ApiError::bad(
                "Manage your existing subscription in the billing portal.",
            ));
        }
        if let Some(attempt) = sql_query("SELECT request_key,parameters FROM checkout_attempts WHERE organization_id=$1 AND expires_at>now()")
            .bind::<SqlUuid,_>(org).get_result::<CheckoutAttempt>(c).optional()? { return Ok(attempt); }
        let request_key = format!("checkout:{}",Uuid::new_v4());
        sql_query("INSERT INTO checkout_attempts(organization_id,request_key,parameters,expires_at) VALUES($1,$2,$3,$4) ON CONFLICT(organization_id) DO UPDATE SET request_key=excluded.request_key,parameters=excluded.parameters,expires_at=excluded.expires_at")
            .bind::<SqlUuid,_>(org).bind::<Text,_>(&request_key).bind::<diesel::sql_types::Jsonb,_>(&parameters).bind::<Timestamptz,_>(expires_at).execute(c)?;
        Ok(CheckoutAttempt { request_key,parameters })
        })
    })
    .await?;
    let parameters: Vec<(String, String)> =
        serde_json::from_value(attempt.parameters).map_err(ApiError::internal)?;
    let response = state
        .http
        .post("https://api.stripe.com/v1/checkout/sessions")
        .bearer_auth(state.config.stripe_key.as_deref().unwrap_or_default())
        .header("Idempotency-Key", attempt.request_key)
        .form(&parameters)
        .send()
        .await
        .map_err(ApiError::internal)?
        .error_for_status()
        .map_err(ApiError::internal)?
        .json::<serde_json::Value>()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(RedirectUrl {
        url: response["url"]
            .as_str()
            .ok_or_else(|| ApiError::bad("Checkout is temporarily unavailable."))?
            .into(),
    }))
}
#[utoipa::path(post,path="/api/organizations/{org}/billing-portal",params(("org"=Uuid,Path)),responses((status=200,body=RedirectUrl)))]
pub async fn portal(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path(org): Path<Uuid>,
) -> Result<Json<RedirectUrl>> {
    security::verified(&user)?;
    if !state.config.billing_enabled() {
        return Err(ApiError::bad("Billing hasn't been configured."));
    }
    let customer=db::run(state.pool.clone(),move |c| {
        security::owner(c,user.id,org)?;
        Ok(sql_query("SELECT customer_id AS value FROM subscriptions WHERE organization_id=$1 AND customer_id IS NOT NULL").bind::<SqlUuid,_>(org).get_result::<TextValue>(c)?.value)
    }).await?;
    let response = state
        .http
        .post("https://api.stripe.com/v1/billing_portal/sessions")
        .bearer_auth(state.config.stripe_key.as_deref().unwrap_or_default())
        .form(&[
            ("customer", customer),
            (
                "return_url",
                format!("{}/app/billing", state.config.app_url),
            ),
        ])
        .send()
        .await
        .map_err(ApiError::internal)?
        .error_for_status()
        .map_err(ApiError::internal)?
        .json::<serde_json::Value>()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(RedirectUrl {
        url: response["url"]
            .as_str()
            .ok_or_else(|| ApiError::bad("Billing portal is unavailable."))?
            .into(),
    }))
}
pub fn verify_signature(header: &str, body: &[u8], secret: &str, now: i64) -> Result<()> {
    let timestamp = header
        .split(',')
        .find_map(|p| p.strip_prefix("t="))
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(ApiError::forbidden)?;
    if now.abs_diff(timestamp) > 300 {
        return Err(ApiError::forbidden());
    }
    for sig in header.split(',').filter_map(|p| p.strip_prefix("v1=")) {
        let Ok(sig) = hex::decode(sig) else { continue };
        let mut mac =
            Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(ApiError::internal)?;
        mac.update(format!("{timestamp}.").as_bytes());
        mac.update(body);
        if mac.verify_slice(&sig).is_ok() {
            return Ok(());
        }
    }
    Err(ApiError::forbidden())
}
pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Message>> {
    let secret = state
        .config
        .stripe_webhook_secret
        .as_deref()
        .ok_or_else(ApiError::forbidden)?;
    verify_signature(
        headers
            .get("stripe-signature")
            .and_then(|s| s.to_str().ok())
            .unwrap_or_default(),
        &body,
        secret,
        chrono::Utc::now().timestamp(),
    )?;
    let event: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| ApiError::bad("Invalid event."))?;
    let event_type = event["type"].as_str().unwrap_or_default();
    if ![
        "customer.subscription.created",
        "customer.subscription.updated",
        "customer.subscription.deleted",
    ]
    .contains(&event_type)
    {
        return Ok(crate::auth::message("Ignored."));
    }
    let provider_id = event["data"]["object"]["id"]
        .as_str()
        .filter(|id| {
            id.starts_with("sub_") && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        })
        .ok_or_else(|| ApiError::bad("Invalid subscription."))?
        .to_owned();
    let event_id = event["id"]
        .as_str()
        .ok_or_else(|| ApiError::bad("Missing event id."))?
        .to_owned();
    let lease_id = Uuid::new_v4();
    let lock_key = provider_id.clone();
    db::run(state.pool.clone(), move |c| {
        let acquired = sql_query("INSERT INTO billing_sync_locks(provider_id,lease_id,expires_at) VALUES($1,$2,now()+interval '60 seconds') ON CONFLICT(provider_id) DO UPDATE SET lease_id=excluded.lease_id,expires_at=excluded.expires_at WHERE billing_sync_locks.expires_at<now()")
            .bind::<Text,_>(lock_key).bind::<SqlUuid,_>(lease_id).execute(c)?;
        if acquired != 1 { return Err(ApiError(axum::http::StatusCode::SERVICE_UNAVAILABLE,"billing_busy","Subscription synchronization is in progress. Please retry.")); }
        Ok(())
    }).await?;
    let result = sync_subscription(&state, provider_id.clone(), event_id, lease_id).await;
    db::run(state.pool.clone(), move |c| {
        sql_query("DELETE FROM billing_sync_locks WHERE provider_id=$1 AND lease_id=$2")
            .bind::<Text, _>(provider_id)
            .bind::<SqlUuid, _>(lease_id)
            .execute(c)?;
        Ok(())
    })
    .await?;
    result?;
    Ok(crate::auth::message("Received."))
}

async fn sync_subscription(
    state: &AppState,
    provider_id: String,
    event_id: String,
    lease_id: Uuid,
) -> Result<()> {
    // The lease serializes both provider reads and writes across replicas. A stale
    // request cannot commit after its lease expires or another request takes over.
    let sub = state
        .http
        .get(format!(
            "https://api.stripe.com/v1/subscriptions/{provider_id}"
        ))
        .bearer_auth(state.config.stripe_key.as_deref().unwrap_or_default())
        .send()
        .await
        .map_err(ApiError::internal)?
        .error_for_status()
        .map_err(ApiError::internal)?
        .json::<serde_json::Value>()
        .await
        .map_err(ApiError::internal)?;
    let org = sub["metadata"]["organization_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| ApiError::bad("Missing organization metadata."))?;
    let customer = sub["customer"]
        .as_str()
        .ok_or_else(|| ApiError::bad("Missing customer."))?
        .to_owned();
    let status = sub["status"]
        .as_str()
        .ok_or_else(|| ApiError::bad("Missing status."))?
        .to_owned();
    let configured_price = state.config.stripe_price.as_deref().unwrap_or_default();
    if sub["items"]["data"][0]["price"]["id"].as_str() != Some(configured_price) {
        return Err(ApiError::bad("Unrecognized subscription price."));
    }
    let end = sub["items"]["data"][0]["current_period_end"]
        .as_i64()
        .or_else(|| sub["current_period_end"].as_i64())
        .and_then(|s| chrono::DateTime::from_timestamp(s, 0));
    db::run(state.pool.clone(),move |c| c.transaction::<_,ApiError,_>(|c| {
        let lease = sql_query("SELECT provider_id AS value FROM billing_sync_locks WHERE provider_id=$1 AND lease_id=$2 AND expires_at>now() FOR UPDATE")
            .bind::<Text,_>(&provider_id).bind::<SqlUuid,_>(lease_id).get_result::<TextValue>(c).optional()?;
        if lease.is_none() { return Err(ApiError(axum::http::StatusCode::SERVICE_UNAVAILABLE,"billing_busy","Subscription synchronization expired. Please retry.")); }
        crate::organizations::lock_org(c,org)?;
        let inserted=sql_query("INSERT INTO billing_events(id) VALUES($1) ON CONFLICT DO NOTHING").bind::<Text,_>(event_id).execute(c)?;
        if inserted==0 { return Ok(()); }
        // Never let an older, different subscription replace an active subscription.
        sql_query("UPDATE subscriptions SET plan='pro',status=$1,provider_id=$2,customer_id=$3,current_period_end=$4,updated_at=now() WHERE organization_id=$5 AND (provider_id IS NULL OR provider_id=$2 OR status IN ('canceled','incomplete_expired'))")
            .bind::<Text,_>(status).bind::<Text,_>(provider_id).bind::<Text,_>(customer).bind::<Nullable<Timestamptz>,_>(end).bind::<SqlUuid,_>(org).execute(c)?; Ok(())
    })).await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn webhook_rejects_tampering_and_expired_signatures() {
        let body = b"event";
        let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
        mac.update(b"1000.event");
        let header = format!("t=1000,v1={}", hex::encode(mac.finalize().into_bytes()));
        assert!(verify_signature(&header, body, "secret", 1001).is_ok());
        assert!(verify_signature(&header, b"tampered", "secret", 1001).is_err());
        assert!(verify_signature(&header, body, "secret", 1400).is_err());
    }
}
