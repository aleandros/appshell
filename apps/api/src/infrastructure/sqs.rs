//! SQS is a notification transport. Job payloads and completion state stay in Postgres.
use crate::{
    AppState,
    error::{ApiError, Result},
    infrastructure::repositories,
    jobs,
};
use aws_sdk_sqs::{Client, types::SendMessageBatchRequestEntry};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone)]
pub struct Publisher {
    client: Client,
    queue_url: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Notification {
    version: u8,
    job_id: Uuid,
}
impl Publisher {
    #[cfg(test)]
    pub(crate) fn for_test(client: Client, queue_url: String) -> Self {
        Self { client, queue_url }
    }
    pub async fn from_env() -> Self {
        let queue_url =
            std::env::var("JOBS_QUEUE_URL").expect("JOBS_QUEUE_URL is required for SQS jobs");
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .retry_config(aws_config::retry::RetryConfig::standard().with_max_attempts(2))
            .timeout_config(
                aws_config::timeout::TimeoutConfig::builder()
                    .operation_timeout(std::time::Duration::from_secs(10))
                    .build(),
            )
            .load()
            .await;
        Self {
            client: Client::new(&config),
            queue_url,
        }
    }
    pub async fn publish(&self, state: &AppState) -> Result<()> {
        let pending = repositories::run(state.pool.clone(), |c| c.jobs_dispatch()).await?;
        if pending.is_empty() {
            return Ok(());
        }
        let entries = pending
            .into_iter()
            .map(|row| {
                SendMessageBatchRequestEntry::builder()
                    .id(row.id.to_string())
                    .message_body(
                        serde_json::to_string(&Notification {
                            version: 1,
                            job_id: row.id,
                        })
                        .map_err(ApiError::internal)?,
                    )
                    .build()
                    .map_err(ApiError::internal)
            })
            .collect::<Result<Vec<_>>>()?;
        let result = self
            .client
            .send_message_batch()
            .queue_url(&self.queue_url)
            .set_entries(Some(entries))
            .send()
            .await
            .map_err(|_| ApiError::bad("SQS publication failed; jobs remain in the outbox."))?;
        if !result.failed().is_empty() {
            return Err(ApiError::bad(
                "Some SQS publications failed; jobs remain in the outbox.",
            ));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct Event {
    #[serde(rename = "Records")]
    pub records: Vec<Message>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub message_id: String,
    pub body: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub batch_item_failures: Vec<Failure>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub item_identifier: String,
}

fn notification_id(body: &str) -> Result<Uuid> {
    let notification: Notification =
        serde_json::from_str(body).map_err(|_| ApiError::bad("Invalid job notification."))?;
    if notification.version != 1 {
        return Err(ApiError::bad("Unsupported job notification version."));
    }
    Ok(notification.job_id)
}

pub async fn handle(state: &AppState, event: Event) -> Response {
    let mut failures = Vec::new();
    for message in event.records {
        let result = match notification_id(&message.body) {
            Ok(id) => jobs::process(state, id).await,
            Err(error) => Err(error),
        };
        if result.is_err() {
            // Do not log payloads: malformed input may contain secrets.
            failures.push(Failure {
                item_identifier: message.message_id,
            });
        }
    }
    Response {
        batch_item_failures: failures,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn notifications_are_versioned_ids_only() {
        assert!(
            super::notification_id(
                r#"{"version":1,"job_id":"00000000-0000-0000-0000-000000000001"}"#
            )
            .is_ok()
        );
        for body in [
            r#"{"version":2,"job_id":"00000000-0000-0000-0000-000000000001"}"#,
            r#"{"version":1,"job_id":"invalid"}"#,
            r#"{"version":1,"job_id":"00000000-0000-0000-0000-000000000001","body":"secret"}"#,
        ] {
            assert!(super::notification_id(body).is_err());
        }
    }
}
