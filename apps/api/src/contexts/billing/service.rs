use crate::infrastructure::repositories;
use crate::infrastructure::rows::CheckoutAttempt;
use crate::{
    AppState,
    contexts::identity as security,
    error::{ApiError, Result},
    infrastructure::stripe,
    models::*,
};
use uuid::Uuid;

pub async fn billing(state: AppState, user: User, org: Uuid) -> Result<Billing> {
    security::verified(&user)?;
    let enabled = state.config.billing_enabled();
    repositories::run(state.pool, move |c| {
        crate::contexts::organizations::role(c, user.id, org)?;
        let subscription = c.billing_subscription(org)?;
        let seats_used = crate::contexts::organizations::member_count(c, org)?;
        Ok(Billing {
            subscription,
            seat_limit: crate::contexts::organizations::seat_limit(c, org)?,
            seats_used,
            billing_enabled: enabled,
        })
    })
    .await
}
pub async fn checkout(state: AppState, user: User, org: Uuid) -> Result<RedirectUrl> {
    security::verified(&user)?;
    if !state.config.billing_enabled() {
        return Err(ApiError::bad(
            "Billing hasn't been configured for this installation.",
        ));
    }
    let expires_at = crate::infrastructure::clock::now() + chrono::Duration::hours(23);
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
    let attempt = repositories::run(state.pool.clone(), move |c| {
        security::rate_limit(c, &format!("checkout:{org}"), 10)?;
        c.transaction(|c| {
            security::authorize(c, &user)?;
            crate::contexts::organizations::lock_org(c, org)?;
            crate::contexts::organizations::owner(c, user.id, org)?;
            let sub = c.billing_subscription(org)?;
            appshell_domain::billing::allow_checkout(&sub.plan, &sub.status)?;
            if let Some(attempt) = c.billing_checkout_attempt(org)? {
                return Ok(attempt);
            }
            let request_key = format!("checkout:{}", c.generated_id()?);
            c.billing_save_checkout_attempt(org, &request_key, &parameters, expires_at)?;
            Ok(CheckoutAttempt {
                request_key,
                parameters,
            })
        })
    })
    .await?;
    stripe::checkout(&state, attempt).await
}

pub async fn portal(state: AppState, user: User, org: Uuid) -> Result<RedirectUrl> {
    security::verified(&user)?;
    if !state.config.billing_enabled() {
        return Err(ApiError::bad("Billing hasn't been configured."));
    }
    let customer = repositories::run(state.pool.clone(), move |c| {
        crate::contexts::organizations::owner(c, user.id, org)?;
        Ok(c.billing_customer(org)?.value)
    })
    .await?;
    stripe::portal(&state, customer).await
}

pub async fn webhook(state: AppState, signature: String, body: Vec<u8>) -> Result<Message> {
    let secret = state
        .config
        .stripe_webhook_secret
        .as_deref()
        .ok_or_else(ApiError::forbidden)?;
    stripe::verify_signature(
        &signature,
        &body,
        secret,
        crate::infrastructure::clock::now().timestamp(),
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
        return Ok(crate::models::message("Ignored."));
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
    let lease_id = repositories::run(state.pool.clone(), |c| c.generated_id()).await?;
    let lock_key = provider_id.clone();
    repositories::run(state.pool.clone(), move |c| {
        let acquired = c.billing_acquire_lease(lock_key, lease_id)?;
        if acquired != 1 {
            return Err(ApiError(
                503,
                "billing_busy",
                "Subscription synchronization is in progress. Please retry.",
            ));
        }
        Ok(())
    })
    .await?;
    let result = sync_subscription(&state, provider_id.clone(), event_id, lease_id).await;
    repositories::run(state.pool.clone(), move |c| {
        c.billing_release_lease(provider_id, lease_id)?;
        Ok(())
    })
    .await?;
    result?;
    Ok(crate::models::message("Received."))
}

async fn sync_subscription(
    state: &AppState,
    provider_id: String,
    event_id: String,
    lease_id: Uuid,
) -> Result<()> {
    // The lease serializes both provider reads and writes across replicas. A stale
    // request cannot commit after its lease expires or another request takes over.
    let sub = stripe::subscription(state, &provider_id).await?;
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
    repositories::run(state.pool.clone(), move |c| {
        c.transaction(|c| {
            let lease = c.billing_lock_lease(&provider_id, lease_id)?;
            if lease.is_none() {
                return Err(ApiError(
                    503,
                    "billing_busy",
                    "Subscription synchronization expired. Please retry.",
                ));
            }
            crate::contexts::organizations::lock_org(c, org)?;
            let inserted = c.billing_record_event(event_id)?;
            if inserted == 0 {
                return Ok(());
            }
            // Never let an older, different subscription replace an active subscription.
            c.billing_sync_subscription(status, provider_id, customer, end, org)?;
            Ok(())
        })
    })
    .await?;
    Ok(())
}

pub(crate) fn initialize(c: &mut repositories::UnitOfWork<'_>, org: Uuid) -> Result<()> {
    c.billing_initialize(org)?;
    Ok(())
}
pub(crate) fn seat_limit(c: &mut repositories::UnitOfWork<'_>, org: Uuid) -> Result<i64> {
    let sub = c.billing_subscription(org)?;
    Ok(appshell_domain::billing::seat_limit(&sub.plan, &sub.status))
}
