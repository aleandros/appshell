//! Shared job dispatch, used by both the Postgres process and SQS Lambda adapter.
//! Add typed handlers here that call the owning context's service, not its repository.
use crate::{
    AppState,
    error::{ApiError, Result},
    infrastructure::{job_queue::Job, mail, repositories, rows::JobRow},
};
use appshell_domain::jobs::{HANDLER_TIMEOUT_SECONDS, retry_delay_seconds};
use uuid::Uuid;

async fn execute(state: &AppState, row: JobRow) -> Result<()> {
    let result = match serde_json::from_value::<Job>(row.payload) {
        Ok(job) => {
            let handler = async {
                match job {
                    Job::DeliverMail { mail_id } => mail::deliver_queued(state, mail_id).await,
                }
            };
            match tokio::time::timeout(
                std::time::Duration::from_secs(HANDLER_TIMEOUT_SECONDS),
                handler,
            )
            .await
            {
                Ok(result) => result.map_err(|_| "handler_failed"),
                Err(_) => Err("handler_timeout"),
            }
        }
        Err(_) => Err("invalid_payload"),
    };
    let id = row.id;
    let lease = row.lease_version;
    if let Err(code) = result {
        tracing::warn!(job_id=%id, attempt=row.attempts, code, "background job failed");
        repositories::run(state.pool.clone(), move |c| {
            c.jobs_fail(id, lease, retry_delay_seconds(row.attempts), code)
        })
        .await?;
        return Err(ApiError::bad("Background job failed; inspect job status."));
    }
    if !repositories::run(state.pool.clone(), move |c| c.jobs_complete(id, lease)).await? {
        return Err(ApiError::bad("Background job lease expired."));
    }
    Ok(())
}

/// One leased job, or false if there is no runnable job. Errors leave retry state.
pub async fn tick(state: &AppState) -> Result<bool> {
    let row = repositories::run(state.pool.clone(), |c| c.jobs_claim(None)).await?;
    if let Some(row) = row {
        execute(state, row).await?;
        return Ok(true);
    }
    Ok(false)
}

/// SQS notifications carry only this durable record ID. Busy/not-due/failed jobs
/// fail their message for later redelivery; completed and deleted jobs are acknowledged.
pub async fn process(state: &AppState, id: Uuid) -> Result<()> {
    let row = repositories::run(state.pool.clone(), move |c| c.jobs_claim(Some(id))).await?;
    if let Some(row) = row {
        return execute(state, row).await;
    }
    if repositories::run(state.pool.clone(), move |c| c.jobs_done(id)).await? {
        return Ok(());
    }
    Err(ApiError::bad("Background job is unavailable or failed."))
}

/// A separate process can run this loop using the same library as the HTTP API.
/// Shutdown stops new claims and waits for the active job/batch to finish.
pub async fn worker(state: AppState, mut stop: tokio::sync::watch::Receiver<bool>) {
    loop {
        if *stop.borrow() {
            break;
        }
        // Drain already-enqueued work even after the feature is disabled.
        match tick(&state).await {
            Ok(true) => continue,
            Err(error) => tracing::warn!(?error, "job attempt failed"),
            Ok(false) => {}
        }
        // Drains pre-upgrade mail and mail created while the optional queue was off.
        if let Err(error) = mail::tick(&state).await {
            tracing::error!(?error, "mail worker failed");
        }
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {},
            _ = stop.changed() => {},
        }
    }
}
