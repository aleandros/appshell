//! Typed transactional enqueue API; transports only carry the resulting job ID.
use crate::{
    config::{Config, JobBackend},
    error::{ApiError, Result},
    infrastructure::repositories::UnitOfWork,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub(crate) enum Job {
    #[serde(rename = "mail.v1")]
    DeliverMail { mail_id: Uuid },
}

/// Called inside an authorized service transaction. Delays are bounded to one week.
pub(crate) fn enqueue(
    c: &mut UnitOfWork<'_>,
    config: &Config,
    job: Job,
    delay_seconds: u32,
) -> Result<Uuid> {
    if config.job_backend == JobBackend::Disabled {
        return Err(ApiError::bad("Background jobs are disabled."));
    }
    if delay_seconds > 604_800 {
        return Err(ApiError::bad("Job delay exceeds one week."));
    }
    c.jobs_enqueue(
        serde_json::to_value(job).map_err(ApiError::internal)?,
        delay_seconds as i32,
    )
}
