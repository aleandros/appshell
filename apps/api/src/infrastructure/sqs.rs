//! SQS is a notification transport. Job payloads and completion state stay in the selected database.
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
    #[cfg(all(test, not(feature = "dynamodb")))]
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
        self.publish_ids(pending.into_iter().map(|row| row.id).collect())
            .await
    }
    /// Stream inserts carry only keys into this adapter; payloads stay in storage.
    pub async fn publish_stream(&self, event: &serde_json::Value) -> Result<()> {
        let ids = stream_job_ids(event)?;
        for batch in ids.chunks(10) {
            self.publish_ids(batch.to_vec()).await?;
        }
        Ok(())
    }
    async fn publish_ids(&self, pending: Vec<Uuid>) -> Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        let entries = pending
            .into_iter()
            .map(|id| {
                SendMessageBatchRequestEntry::builder()
                    .id(id.to_string())
                    .message_body(
                        serde_json::to_string(&Notification {
                            version: 1,
                            job_id: id,
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

fn stream_job_ids(event: &serde_json::Value) -> Result<Vec<Uuid>> {
    let mut ids = Vec::new();
    for record in event["Records"]
        .as_array()
        .ok_or_else(|| ApiError::bad("Invalid stream event"))?
    {
        if record["eventName"] != "INSERT" {
            continue;
        }
        let keys = &record["dynamodb"]["Keys"];
        if keys["sk"]["S"] != "record" {
            continue;
        }
        if let Some(value) = keys["pk"]["S"]
            .as_str()
            .and_then(|s| s.strip_prefix("job#"))
        {
            ids.push(Uuid::parse_str(value).map_err(|_| ApiError::bad("Invalid job key"))?);
        }
    }
    Ok(ids)
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
    fn stream_dispatch_ignores_history_and_updates() {
        use serde_json::json;
        let record = |event: &str, pk: &str| json!({"eventName":event,"dynamodb":{"Keys":{"pk":{"S":pk},"sk":{"S":"record"}},"NewImage":{"data":{"S":"secret payload"}}}});
        let event = json!({"Records":[
            record("INSERT", "job#00000000-0000-0000-0000-000000000001"),
            record("MODIFY", "job#00000000-0000-0000-0000-000000000002"),
            record("INSERT", "history#00000000-0000-0000-0000-000000000001")
        ]});
        let ids = super::stream_job_ids(&event).expect("valid stream");
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].to_string(), "00000000-0000-0000-0000-000000000001");
        assert!(
            super::stream_job_ids(&json!({"Records":[record("INSERT", "job#invalid")]})).is_err()
        );
    }
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
