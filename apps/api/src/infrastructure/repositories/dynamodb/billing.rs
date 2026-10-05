use super::*;
use crate::{
    infrastructure::rows::{self, CheckoutAttempt, TextValue},
    models,
};
impl UnitOfWork<'_> {
    pub(crate) fn billing_subscription(&mut self, org: Uuid) -> Result<models::Subscription> {
        Ok(decode::<rows::Subscription>(self.require(&key("subscription", org))?)?.into())
    }
    pub(crate) fn billing_checkout_attempt(
        &mut self,
        org: Uuid,
    ) -> Result<Option<CheckoutAttempt>> {
        let v = self.get(&key("checkout", org))?;
        if active(&v) && expires(&v, "expires_at") {
            Ok(Some(decode(v)?))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn billing_save_checkout_attempt(
        &mut self,
        org: Uuid,
        request: impl AsRef<str>,
        parameters: &Value,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<usize> {
        let k = key("checkout", org);
        let mut v = self.get(&k)?;
        if v.is_null() {
            v = fresh(json!({}));
        }
        v["request_key"] = json!(request.as_ref());
        v["parameters"] = parameters.clone();
        v["expires_at"] = json!(expires_at);
        v["deleted_at"] = Value::Null;
        self.save(k, v)?;
        Ok(1)
    }
    pub(crate) fn billing_customer(&mut self, org: Uuid) -> Result<TextValue> {
        let v = self.require(&key("subscription", org))?;
        Ok(TextValue {
            value: v["customer_id"].as_str().ok_or_else(missing)?.into(),
        })
    }
    pub(crate) fn billing_acquire_lease(
        &mut self,
        provider: impl AsRef<str>,
        lease: Uuid,
    ) -> Result<usize> {
        let k = key("billing_lease", provider.as_ref());
        let mut v = self.get(&k)?;
        if active(&v) && expires(&v, "expires_at") {
            return Ok(0);
        }
        if v.is_null() {
            v = fresh(json!({}));
        }
        v["lease_id"] = json!(lease);
        v["expires_at"] = json!(now() + chrono::Duration::seconds(60));
        v["deleted_at"] = Value::Null;
        self.save(k, v)?;
        Ok(1)
    }
    pub(crate) fn billing_release_lease(
        &mut self,
        provider: impl AsRef<str>,
        lease: Uuid,
    ) -> Result<usize> {
        let k = key("billing_lease", provider.as_ref());
        let v = self.get(&k)?;
        if v["lease_id"] != json!(lease) {
            return Ok(0);
        }
        self.remove(k)
    }
    pub(crate) fn billing_lock_lease(
        &mut self,
        provider: impl AsRef<str>,
        lease: Uuid,
    ) -> Result<Option<TextValue>> {
        let v = self.get(&key("billing_lease", provider.as_ref()))?;
        Ok(
            if active(&v) && expires(&v, "expires_at") && v["lease_id"] == json!(lease) {
                Some(TextValue {
                    value: provider.as_ref().into(),
                })
            } else {
                None
            },
        )
    }
    pub(crate) fn billing_record_event(&mut self, event: impl AsRef<str>) -> Result<usize> {
        let k = key("billing_event", event.as_ref());
        if !self.get(&k)?.is_null() {
            return Ok(0);
        }
        self.save(k, fresh(json!({"provider_event_id":event.as_ref()})))?;
        Ok(1)
    }
    pub(crate) fn billing_sync_subscription(
        &mut self,
        status: impl AsRef<str>,
        provider: impl AsRef<str>,
        customer: impl AsRef<str>,
        end: Option<chrono::DateTime<chrono::Utc>>,
        org: Uuid,
    ) -> Result<usize> {
        let k = key("subscription", org);
        let mut v = self.require(&k)?;
        if !v["provider_id"].is_null()
            && v["provider_id"] != provider.as_ref()
            && v["status"] != "canceled"
            && v["status"] != "incomplete_expired"
        {
            return Ok(0);
        }
        v["plan"] = json!("pro");
        v["status"] = json!(status.as_ref());
        v["provider_id"] = json!(provider.as_ref());
        v["customer_id"] = json!(customer.as_ref());
        v["current_period_end"] = json!(end);
        self.save(k, v)?;
        Ok(1)
    }
    pub(crate) fn billing_initialize(&mut self, org: Uuid) -> Result<usize> {
        self.save(key("subscription",org),fresh(json!({"organization_id":org,"plan":"free","status":"active","current_period_end":null})))?;
        Ok(1)
    }
}
