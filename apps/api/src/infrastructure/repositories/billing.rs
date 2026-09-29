use super::UnitOfWork;
use crate::error::Result;
use crate::infrastructure::rows::*;
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Jsonb, Nullable, Text, Timestamptz, Uuid as SqlUuid},
};
use uuid::Uuid;
impl UnitOfWork<'_> {
    pub(crate) fn billing_subscription(
        &mut self,
        organization_id: Uuid,
    ) -> Result<crate::models::Subscription> {
        Ok(sql_query(
            "SELECT plan,status,current_period_end FROM subscriptions WHERE organization_id=$1",
        )
        .bind::<SqlUuid, _>(organization_id)
        .get_result::<Subscription>(self.connection)?
        .into())
    }
    pub(crate) fn billing_checkout_attempt(
        &mut self,
        organization_id: Uuid,
    ) -> Result<Option<super::super::rows::CheckoutAttempt>> {
        Ok(sql_query("SELECT request_key,parameters FROM checkout_attempts WHERE organization_id=$1 AND expires_at>now()").bind::<SqlUuid, _>(organization_id).get_result::<CheckoutAttempt>(self.connection).optional()?)
    }
    pub(crate) fn billing_save_checkout_attempt(
        &mut self,
        organization_id: Uuid,
        request_key: impl AsRef<str>,
        parameters: &serde_json::Value,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<usize> {
        Ok(sql_query("INSERT INTO checkout_attempts(organization_id,request_key,parameters,expires_at) VALUES($1,$2,$3,$4) ON CONFLICT(organization_id) DO UPDATE SET request_key=excluded.request_key,parameters=excluded.parameters,expires_at=excluded.expires_at").bind::<SqlUuid, _>(organization_id).bind::<Text, _>(request_key.as_ref()).bind::<Jsonb, _>(parameters).bind::<Timestamptz, _>(expires_at).execute(self.connection)?)
    }
    pub(crate) fn billing_customer(
        &mut self,
        organization_id: Uuid,
    ) -> Result<super::super::rows::TextValue> {
        Ok(sql_query("SELECT customer_id AS value FROM subscriptions WHERE organization_id=$1 AND customer_id IS NOT NULL").bind::<SqlUuid, _>(organization_id).get_result::<TextValue>(self.connection)?)
    }
    pub(crate) fn billing_acquire_lease(
        &mut self,
        provider_id: impl AsRef<str>,
        lease_id: Uuid,
    ) -> Result<usize> {
        Ok(sql_query("INSERT INTO billing_sync_locks(provider_id,lease_id,expires_at) VALUES($1,$2,now()+interval '60 seconds') ON CONFLICT(provider_id) DO UPDATE SET lease_id=excluded.lease_id,expires_at=excluded.expires_at WHERE billing_sync_locks.expires_at<now()").bind::<Text, _>(provider_id.as_ref()).bind::<SqlUuid, _>(lease_id).execute(self.connection)?)
    }
    pub(crate) fn billing_release_lease(
        &mut self,
        provider_id: impl AsRef<str>,
        lease_id: Uuid,
    ) -> Result<usize> {
        Ok(
            sql_query("DELETE FROM billing_sync_locks WHERE provider_id=$1 AND lease_id=$2")
                .bind::<Text, _>(provider_id.as_ref())
                .bind::<SqlUuid, _>(lease_id)
                .execute(self.connection)?,
        )
    }
    pub(crate) fn billing_lock_lease(
        &mut self,
        provider_id: impl AsRef<str>,
        lease_id: Uuid,
    ) -> Result<Option<super::super::rows::TextValue>> {
        Ok(sql_query("SELECT provider_id AS value FROM billing_sync_locks WHERE provider_id=$1 AND lease_id=$2 AND expires_at>now() FOR UPDATE").bind::<Text, _>(provider_id.as_ref()).bind::<SqlUuid, _>(lease_id).get_result::<TextValue>(self.connection).optional()?)
    }
    pub(crate) fn billing_record_event(&mut self, event_id: impl AsRef<str>) -> Result<usize> {
        Ok(
            sql_query("INSERT INTO billing_events(id) VALUES($1) ON CONFLICT DO NOTHING")
                .bind::<Text, _>(event_id.as_ref())
                .execute(self.connection)?,
        )
    }
    pub(crate) fn billing_sync_subscription(
        &mut self,
        status: impl AsRef<str>,
        provider_id: impl AsRef<str>,
        customer_id: impl AsRef<str>,
        period_end: Option<chrono::DateTime<chrono::Utc>>,
        organization_id: Uuid,
    ) -> Result<usize> {
        Ok(sql_query("UPDATE subscriptions SET plan='pro',status=$1,provider_id=$2,customer_id=$3,current_period_end=$4,updated_at=now() WHERE organization_id=$5 AND (provider_id IS NULL OR provider_id=$2 OR status IN ('canceled','incomplete_expired'))").bind::<Text, _>(status.as_ref()).bind::<Text, _>(provider_id.as_ref()).bind::<Text, _>(customer_id.as_ref()).bind::<Nullable<Timestamptz>, _>(period_end).bind::<SqlUuid, _>(organization_id).execute(self.connection)?)
    }
    pub(crate) fn billing_initialize(&mut self, organization_id: Uuid) -> Result<usize> {
        Ok(
            sql_query("INSERT INTO subscriptions(organization_id) VALUES($1)")
                .bind::<SqlUuid, _>(organization_id)
                .execute(self.connection)?,
        )
    }
}
