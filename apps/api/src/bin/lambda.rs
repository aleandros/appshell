//! Deployment adapter. API requests, scheduled mail, and migrations have separate
//! Lambda functions/roles; no background work survives an invocation boundary.
use appshell_api::{
    AppState,
    config::{Config, JobBackend},
    db, jobs, mail, router, sqs,
};
use lambda_http::{
    Error,
    lambda_runtime::{self, LambdaEvent, service_fn},
};
use serde_json::{Value, json};

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .without_time()
        .init();
    let mode = std::env::var("RUN_MODE").unwrap_or_else(|_| "api".into());
    assert!(
        ["api", "worker", "migrate", "sqs", "dispatcher"].contains(&mode.as_str()),
        "Lambda RUN_MODE must be api, worker, migrate, sqs, or dispatcher"
    );
    let pool = tokio::task::spawn_blocking(|| {
        db::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL is required"))
            .expect("database pool")
    })
    .await?;
    if mode == "migrate" {
        return lambda_runtime::run(service_fn(|_: LambdaEvent<Value>| {
            let pool = pool.clone();
            async move {
                db::migrate(pool)
                    .await
                    .map_err(|_| Error::from("database migration failed; inspect logs"))?;
                Ok::<_, Error>(json!({"migrated": true}))
            }
        }))
        .await;
    }
    let state =
        tokio::task::spawn_blocking(move || AppState::new(pool, Config::from_env())).await?;
    assert!(
        state.config.job_backend != JobBackend::Postgres,
        "Lambda jobs use JOB_BACKEND=sqs"
    );
    if mode == "sqs" {
        assert!(
            state.config.job_backend == JobBackend::Sqs,
            "SQS worker requires JOB_BACKEND=sqs"
        );
        return lambda_runtime::run(service_fn(|event: LambdaEvent<sqs::Event>| {
            let state = state.clone();
            async move { Ok::<_, Error>(sqs::handle(&state, event.payload).await) }
        }))
        .await;
    }
    let publisher = if state.config.job_backend == JobBackend::Sqs {
        Some(sqs::Publisher::from_env().await)
    } else {
        None
    };
    if mode == "dispatcher" {
        let publisher = publisher.expect("Dispatcher requires JOB_BACKEND=sqs");
        return lambda_runtime::run(service_fn(|_: LambdaEvent<Value>| {
            let state = state.clone();
            let publisher = publisher.clone();
            async move {
                publisher
                    .publish(&state)
                    .await
                    .map_err(|_| Error::from("job publication failed"))?;
                // Existing mail from before jobs were enabled still needs delivery.
                mail::tick(&state)
                    .await
                    .map_err(|_| Error::from("legacy mail batch failed"))?;
                Ok::<_, Error>(json!({"published": true}))
            }
        }))
        .await;
    }
    if mode == "worker" {
        return lambda_runtime::run(service_fn(|_: LambdaEvent<Value>| {
            let state = state.clone();
            async move {
                // Also drain durable jobs left behind when SQS was disabled.
                if !jobs::tick(&state)
                    .await
                    .map_err(|_| Error::from("job attempt failed"))?
                {
                    mail::tick(&state)
                        .await
                        .map_err(|_| Error::from("mail batch failed; inspect logs"))?;
                }
                Ok::<_, Error>(json!({"processed": true}))
            }
        }))
        .await;
    }
    // Migrations run explicitly before publishing the frontend. Reuse all of the
    // HTTP authorization, cookies, origin checks, and webhook verification.
    let app = router(state.clone(), None).layer(axum::middleware::from_fn(
        move |request: axum::extract::Request, next: axum::middleware::Next| {
            let state = state.clone();
            let publisher = publisher.clone();
            async move {
                let write = !matches!(
                    *request.method(),
                    axum::http::Method::GET
                        | axum::http::Method::HEAD
                        | axum::http::Method::OPTIONS
                );
                let response = next.run(request).await;
                if write && let Some(publisher) = publisher {
                    // Await the fast path before returning to Lambda. Publication failure
                    // never turns an already-committed request into a retryable API error.
                    if !matches!(
                        tokio::time::timeout(
                            std::time::Duration::from_secs(3),
                            publisher.publish(&state)
                        )
                        .await,
                        Ok(Ok(()))
                    ) {
                        tracing::warn!("job publication deferred to recovery dispatcher");
                    }
                }
                response
            }
        },
    ));
    lambda_http::run(app).await
}
