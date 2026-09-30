use super::UnitOfWork;
use crate::{
    error::{ApiError, Result},
    infrastructure::rows::{Id, JobDone, JobRow},
};
use appshell_domain::jobs::{LEASE_SECONDS, MAX_ATTEMPTS};
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{BigInt, Integer, Jsonb, Nullable, Text, Uuid as SqlUuid},
};
use uuid::Uuid;

impl UnitOfWork<'_> {
    pub(crate) fn jobs_enqueue(
        &mut self,
        payload: serde_json::Value,
        delay_seconds: i32,
    ) -> Result<Uuid> {
        if !self.in_transaction {
            return Err(ApiError::internal(
                "Job enqueue requires a unit-of-work transaction",
            ));
        }
        Ok(sql_query("INSERT INTO background_jobs(payload,available_at) VALUES($1,now()+$2*interval '1 second') RETURNING id")
            .bind::<Jsonb,_>(payload).bind::<Integer,_>(delay_seconds).get_result::<Id>(self.connection)?.id)
    }
    pub(crate) fn jobs_claim(&mut self, id: Option<Uuid>) -> Result<Option<JobRow>> {
        // A process killed on its last attempt must eventually become visibly failed.
        sql_query("UPDATE background_jobs SET failed_at=now(),locked_until=NULL,error_code='attempts_exhausted' WHERE deleted_at IS NULL AND completed_at IS NULL AND failed_at IS NULL AND attempts >= $1 AND locked_until <= now()")
            .bind::<Integer,_>(MAX_ATTEMPTS).execute(self.connection)?;
        Ok(sql_query("UPDATE background_jobs SET attempts=attempts+1,lease_version=lease_version+1,locked_until=now()+$3*interval '1 second' WHERE id=(SELECT id FROM background_jobs WHERE deleted_at IS NULL AND completed_at IS NULL AND failed_at IS NULL AND attempts<$2 AND available_at<=now() AND (locked_until IS NULL OR locked_until<=now()) AND ($1::uuid IS NULL OR id=$1) ORDER BY available_at,created_at LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING id,payload,attempts,lease_version")
            .bind::<Nullable<SqlUuid>,_>(id).bind::<Integer,_>(MAX_ATTEMPTS).bind::<Integer,_>(LEASE_SECONDS)
            .get_result::<JobRow>(self.connection).optional()?)
    }
    pub(crate) fn jobs_done(&mut self, id: Uuid) -> Result<bool> {
        Ok(sql_query("SELECT completed_at IS NOT NULL OR deleted_at IS NOT NULL AS done FROM background_jobs WHERE id=$1")
            .bind::<SqlUuid,_>(id).get_result::<JobDone>(self.connection).optional()?.is_some_and(|row| row.done))
    }
    pub(crate) fn jobs_complete(&mut self, id: Uuid, lease: i64) -> Result<bool> {
        Ok(sql_query("UPDATE background_jobs SET completed_at=now(),locked_until=NULL,error_code=NULL,payload='{}'::jsonb WHERE id=$1 AND lease_version=$2 AND locked_until>now() AND deleted_at IS NULL AND completed_at IS NULL AND failed_at IS NULL")
            .bind::<SqlUuid,_>(id).bind::<BigInt,_>(lease).execute(self.connection)? == 1)
    }
    pub(crate) fn jobs_fail(&mut self, id: Uuid, lease: i64, delay: i32, code: &str) -> Result<()> {
        sql_query("UPDATE background_jobs SET locked_until=NULL,available_at=now()+$3*interval '1 second',failed_at=CASE WHEN attempts >= $4 THEN now() ELSE NULL END,error_code=$5 WHERE id=$1 AND lease_version=$2 AND locked_until>now() AND deleted_at IS NULL AND completed_at IS NULL AND failed_at IS NULL")
            .bind::<SqlUuid,_>(id).bind::<BigInt,_>(lease).bind::<Integer,_>(delay).bind::<Integer,_>(MAX_ATTEMPTS).bind::<Text,_>(code).execute(self.connection)?;
        Ok(())
    }
    #[cfg(feature = "lambda")]
    pub(crate) fn jobs_dispatch(&mut self) -> Result<Vec<Id>> {
        // Lease publication before the network call. Failed or interrupted sends are
        // republished by recovery; duplicate SQS messages use the execution lease.
        Ok(sql_query("UPDATE background_jobs SET dispatched_until=now()+interval '15 minutes' WHERE id IN (SELECT id FROM background_jobs WHERE deleted_at IS NULL AND completed_at IS NULL AND failed_at IS NULL AND attempts<$1 AND available_at<=now() AND (locked_until IS NULL OR locked_until<=now()) AND (dispatched_until IS NULL OR dispatched_until<=now()) ORDER BY available_at,created_at LIMIT 10 FOR UPDATE SKIP LOCKED) RETURNING id")
            .bind::<Integer,_>(MAX_ATTEMPTS).load(self.connection)?)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // Disposable integration fixtures fail fast.
    use super::*;
    use crate::{
        AppState,
        config::{Config, JobBackend},
        db,
        infrastructure::{
            crypto,
            job_queue::{self, Job},
            mail, repositories,
        },
        jobs,
    };
    use serde_json::{Value, json};

    #[derive(QueryableByName)]
    struct Snapshot {
        #[diesel(sql_type = Jsonb)]
        value: Value,
    }
    async fn snapshot(pool: db::DbPool, query: &'static str) -> Value {
        db::run(pool, move |c| {
            Ok(sql_query(query).get_result::<Snapshot>(c)?.value)
        })
        .await
        .unwrap()
    }
    async fn sql(pool: db::DbPool, query: &'static str) {
        db::run(pool, move |c| {
            sql_query(query).execute(c)?;
            Ok(())
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn durable_jobs_are_atomic_leased_retryable_and_idempotent() {
        let url =
            std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL for queue tests");
        let admin = db::connect(&url).unwrap();
        let schema = format!("jobs_test_{}", &crypto::digest(&crypto::token())[..24]);
        let create = schema.clone();
        db::run(admin.clone(), move |c| {
            sql_query(format!("CREATE SCHEMA {create}")).execute(c)?;
            Ok(())
        })
        .await
        .unwrap();
        let sep = if url.contains('?') { '&' } else { '?' };
        let pool = db::connect(&format!("{url}{sep}options=-csearch_path%3D{schema}")).unwrap();
        db::migrate(pool.clone()).await.unwrap();
        let mut config = Config::from_env();
        config.job_backend = JobBackend::Postgres;
        config.production = false;
        config.mail_mode = "console".into();
        let state = AppState::new(pool.clone(), config);
        assert!(
            repositories::run(pool.clone(), |c| c.jobs_enqueue(json!({}), 0))
                .await
                .is_err()
        );
        let cfg = state.config.clone();
        let rolled_back: Result<()> = repositories::run(pool.clone(), move |c| {
            c.transaction(|c| {
                mail::email_change_notice(c, "rollback@example.com", &cfg)?;
                Err(ApiError::bad("Rollback"))
            })
        })
        .await;
        assert!(rolled_back.is_err());
        assert_eq!(snapshot(pool.clone(), "SELECT jsonb_build_array((SELECT count(*) FROM background_jobs),(SELECT count(*) FROM mail_outbox)) AS value").await, json!([0,0]));

        let cfg = state.config.clone();
        let actor = repositories::run(pool.clone(), move |c| {
            c.transaction(|c| {
                let actor = c.generated_id()?;
                c.actor("user", actor)?;
                mail::email_change_notice(c, "queued@example.com", &cfg)?;
                Ok(actor)
            })
        })
        .await
        .unwrap();
        assert_eq!(snapshot(pool.clone(), "SELECT jsonb_build_array(actor_id,changes->'payload'->'to') AS value FROM background_jobs_history WHERE operation='INSERT'").await, json!([actor,"[redacted]"]));
        // Legacy mail polling must not race the shared job handler.
        mail::tick(&state).await.unwrap();
        assert_eq!(
            snapshot(
                pool.clone(),
                "SELECT to_jsonb(sent_at IS NULL) AS value FROM mail_outbox"
            )
            .await,
            json!(true)
        );
        let (first, second) = tokio::join!(
            repositories::run(pool.clone(), |c| c.jobs_claim(None)),
            repositories::run(pool.clone(), |c| c.jobs_claim(None))
        );
        let first = first.unwrap();
        let second = second.unwrap();
        assert_ne!(first.is_some(), second.is_some());
        let row = first.or(second).unwrap();
        let id = row.id;
        assert!(
            jobs::process(&state, id).await.is_err(),
            "active lease cannot be stolen"
        );
        sql(
            pool.clone(),
            "UPDATE background_jobs SET locked_until=now()-interval '1 second'",
        )
        .await;
        let newer = repositories::run(pool.clone(), |c| c.jobs_claim(None))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(newer.lease_version, row.lease_version + 1);
        assert!(
            !repositories::run(pool.clone(), move |c| c
                .jobs_complete(id, row.lease_version))
            .await
            .unwrap()
        );
        sql(
            pool.clone(),
            "UPDATE background_jobs SET locked_until=now()-interval '1 second'",
        )
        .await;
        jobs::process(&state, id).await.unwrap();
        jobs::process(&state, id).await.unwrap();
        assert_eq!(snapshot(pool.clone(), "SELECT jsonb_build_array(sent_at IS NOT NULL,body,html_body) AS value FROM mail_outbox").await, json!([true,"[delivered]",null]));
        assert_eq!(snapshot(pool.clone(), "SELECT jsonb_build_array(completed_at IS NOT NULL,payload,attempts) AS value FROM background_jobs").await, json!([true,{},3]));

        let poison = repositories::run(pool.clone(), |c| {
            c.transaction(|c| c.jobs_enqueue(json!({"type":"unknown","secret":"private"}), 0))
        })
        .await
        .unwrap();
        assert!(jobs::process(&state, poison).await.is_err());
        assert!(
            !jobs::tick(&state).await.unwrap(),
            "backoff prevents immediate retry"
        );
        sql(
            pool.clone(),
            "UPDATE background_jobs SET attempts=9,available_at=now() WHERE completed_at IS NULL",
        )
        .await;
        assert!(jobs::process(&state, poison).await.is_err());
        assert!(!jobs::tick(&state).await.unwrap());
        assert_eq!(snapshot(pool.clone(), "SELECT jsonb_build_array(failed_at IS NOT NULL,attempts,error_code) AS value FROM background_jobs WHERE completed_at IS NULL").await, json!([true,10,"invalid_payload"]));
        let cfg = state.config.clone();
        let delayed = repositories::run(pool.clone(), move |c| {
            c.transaction(|c| job_queue::enqueue(c, &cfg, Job::DeliverMail { mail_id: id }, 600))
        })
        .await
        .unwrap();
        assert!(jobs::process(&state, delayed).await.is_err());
        sql(pool.clone(), "UPDATE background_jobs SET attempts=10,locked_until=now()-interval '1 second' WHERE failed_at IS NULL AND completed_at IS NULL").await;
        assert!(
            !jobs::tick(&state).await.unwrap(),
            "last-attempt crashes become terminal"
        );
        assert_eq!(snapshot(pool.clone(), "SELECT to_jsonb(count(*)) AS value FROM background_jobs WHERE failed_at IS NOT NULL").await, json!(2));

        #[cfg(feature = "lambda")]
        {
            use crate::sqs::{self, Event, Message};
            let response = sqs::handle(
                &state,
                Event {
                    records: vec![
                        Message {
                            message_id: "done".into(),
                            body: json!({"version":1,"job_id":id}).to_string(),
                        },
                        Message {
                            message_id: "failed".into(),
                            body: json!({"version":1,"job_id":poison}).to_string(),
                        },
                        Message {
                            message_id: "invalid".into(),
                            body: "not json".into(),
                        },
                    ],
                },
            )
            .await;
            assert_eq!(
                serde_json::to_value(response).unwrap(),
                json!({"batchItemFailures":[{"itemIdentifier":"failed"},{"itemIdentifier":"invalid"}]})
            );
            publication_recovers_after_partial_failure(&state).await;
        }
        sql(
            pool.clone(),
            "UPDATE background_jobs SET deleted_at=now() WHERE completed_at IS NULL",
        )
        .await;
        jobs::process(&state, poison).await.unwrap();
        assert!(!jobs::tick(&state).await.unwrap());
        drop(state);
        drop(pool);
        db::run(admin, move |c| {
            sql_query(format!("DROP SCHEMA {schema} CASCADE")).execute(c)?;
            Ok(())
        })
        .await
        .unwrap();
    }

    #[cfg(feature = "lambda")]
    async fn publication_recovers_after_partial_failure(state: &AppState) {
        use aws_sdk_sqs::{
            Client,
            config::{BehaviorVersion, Credentials, Region},
        };
        use axum::{Json, Router, body::Bytes};
        let (send, mut receive) = tokio::sync::mpsc::unbounded_channel::<Value>();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new().fallback(move |body:Bytes| {
            let send=send.clone();
            async move {
                let body:Value=serde_json::from_slice(&body).unwrap();
                send.send(body.clone()).unwrap();
                Json(json!({"Successful":[],"Failed":[{"Id":body["Entries"][0]["Id"],"SenderFault":false,"Code":"Unavailable"}]}))
            }
        });
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = Client::from_conf(
            aws_sdk_sqs::Config::builder()
                .behavior_version(BehaviorVersion::latest())
                .region(Region::new("us-east-1"))
                .credentials_provider(Credentials::new("test", "test", None, None, "local-test"))
                .endpoint_url(&endpoint)
                .build(),
        );
        let publisher = crate::sqs::Publisher::for_test(client, format!("{endpoint}/queue"));
        let job = repositories::run(state.pool.clone(), |c| {
            c.transaction(|c| c.jobs_enqueue(json!({"secret":"never sent to SQS"}), 0))
        })
        .await
        .unwrap();
        assert!(publisher.publish(state).await.is_err());
        let first = tokio::time::timeout(std::time::Duration::from_secs(5), receive.recv())
            .await
            .unwrap()
            .unwrap();
        let notification: Value =
            serde_json::from_str(first["Entries"][0]["MessageBody"].as_str().unwrap()).unwrap();
        assert_eq!(notification, json!({"version":1,"job_id":job}));
        publisher.publish(state).await.unwrap();
        assert!(
            receive.try_recv().is_err(),
            "publication lease suppresses immediate resend"
        );
        sql(
            state.pool.clone(),
            "UPDATE background_jobs SET dispatched_until=now()-interval '1 second'",
        )
        .await;
        assert!(publisher.publish(state).await.is_err());
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(5), receive.recv())
                .await
                .unwrap()
                .unwrap(),
            first,
            "recovery republishes the same durable ID"
        );
        server.abort();
    }
}
