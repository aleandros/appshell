use appshell_api::{
    AppState,
    config::{Config, JobBackend},
    db, jobs, router,
};
#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "appshell_api=info,tower_http=info".into()),
        )
        .init();
    let config = Config::from_env();
    assert!(
        config.job_backend != JobBackend::Sqs,
        "SQS jobs use appshell-lambda; Docker uses JOB_BACKEND=postgres"
    );
    let pool = db::from_env().await.expect("database connection");
    db::migrate(pool.clone())
        .await
        .expect("database migrations");
    let state = AppState::new(pool, config);
    let mode = std::env::var("RUN_MODE").unwrap_or_else(|_| "combined".into());
    assert!(
        ["combined", "api", "worker"].contains(&mode.as_str()),
        "RUN_MODE must be combined, api, or worker"
    );
    let (stop, stop_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        shutdown().await;
        let _ = stop.send(true);
    });
    if mode == "worker" {
        jobs::worker(state, stop_rx).await;
        return;
    }
    let worker = if mode == "combined" {
        Some(tokio::spawn(jobs::worker(state.clone(), stop_rx.clone())))
    } else {
        None
    };
    let static_dir = std::env::var("STATIC_DIR").ok();
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("bind port");
    tracing::info!(%port,%mode,"AppShell listening");
    axum::serve(listener, router(state, static_dir.as_deref()))
        .with_graceful_shutdown(async move {
            let mut stop_rx = stop_rx;
            while !*stop_rx.borrow() {
                if stop_rx.changed().await.is_err() {
                    break;
                }
            }
        })
        .await
        .expect("server");
    if let Some(worker) = worker {
        let _ = worker.await;
    }
}

async fn shutdown() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.ok();
    }
}
