use crate::{
    AppState,
    error::{ApiError, Result},
    models::RedirectUrl,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
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

pub(crate) async fn checkout(
    state: &AppState,
    attempt: crate::infrastructure::rows::CheckoutAttempt,
) -> Result<RedirectUrl> {
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
    Ok(RedirectUrl {
        url: response["url"]
            .as_str()
            .ok_or_else(|| ApiError::bad("Checkout is temporarily unavailable."))?
            .into(),
    })
}

pub(crate) async fn portal(state: &AppState, customer: String) -> Result<RedirectUrl> {
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
    Ok(RedirectUrl {
        url: response["url"]
            .as_str()
            .ok_or_else(|| ApiError::bad("Billing portal is unavailable."))?
            .into(),
    })
}

pub(crate) async fn subscription(state: &AppState, provider_id: &str) -> Result<serde_json::Value> {
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
    Ok(sub)
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
