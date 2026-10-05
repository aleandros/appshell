#![allow(clippy::unwrap_used)] // Disposable local integration fixtures fail fast.
use super::*;
use crate::{AppState, config::Config, router};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn request(
    state: &AppState,
    path: &str,
    body: Value,
    cookie: Option<&str>,
) -> (StatusCode, Value, String) {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header("origin", "http://localhost:5173")
        .header("content-type", "application/json");
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    let response = router(state.clone(), None)
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned())
        .unwrap_or_default();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap(), cookie)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dynamodb_transactions_lifecycle_and_isolation() {
    let mut pool = crate::db::from_env()
        .await
        .expect("Use npm run test:dynamodb with DynamoDB Local");
    pool.table = format!("appshell-test-{}", crate::infrastructure::crypto::token());
    crate::db::migrate(pool.clone()).await.unwrap();
    let mut config = Config::from_env();
    config.production = false;
    config.app_url = "http://localhost:5173".into();
    let state = AppState::new(pool.clone(), config);
    let (status,user,cookie)=request(&state,"/api/auth/signup",json!({"email":"owner@example.test","password":"a sufficiently long password","name":"Owner","organization":"First"}),None).await;
    assert_eq!(status, StatusCode::OK, "{user}");
    let uid: Uuid = decode(user["user"]["id"].clone()).unwrap();
    let org: Uuid = decode(user["organizations"][0]["id"].clone()).unwrap();
    let (status,_,_)=request(&state,"/api/auth/signup",json!({"email":"owner@example.test","password":"a sufficiently long password","name":"Duplicate","organization":"Duplicate"}),None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    // Rollback includes uniqueness, the user, mail, jobs, and their histories.
    let failed: Result<()> = run(pool.clone(), |c| {
        c.transaction(|c| {
            c.identity_create_user("rollback@example.test", "Rollback", "secret")?;
            c.mail_enqueue(
                "rollback@example.test",
                "subject",
                "secret token",
                "secret token",
            )?;
            c.jobs_enqueue(json!({"secret":"never committed"}), 0)?;
            Err(ApiError::bad("abort"))
        })
    })
    .await;
    assert!(failed.is_err());
    assert!(
        run(pool.clone(), |c| c
            .identity_credentials("rollback@example.test"))
        .await
        .unwrap()
        .is_none()
    );
    // A single action has one winner even when both requests read it before commit.
    run(pool.clone(), move |c| {
        c.transaction(|c| {
            c.identity_issue_action("one-use", uid, "verify", None)?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let consume = |barrier: std::sync::Arc<std::sync::Barrier>| {
        let pool = pool.clone();
        async move {
            run(pool, move |c| {
                c.transaction(|c| {
                    let a = c.identity_consume_action("one-use", "verify")?;
                    barrier.wait();
                    if a.is_some() {
                        c.identity_verify_email(uid)?;
                    }
                    Ok(a.is_some())
                })
            })
            .await
        }
    };
    let (a, b) = tokio::join!(consume(barrier.clone()), consume(barrier));
    assert_eq!(
        [a, b].into_iter().filter(|r| matches!(r, Ok(true))).count(),
        1
    );
    // Other tenants cannot invite into this organization.
    let(status,other,other_cookie)=request(&state,"/api/auth/signup",json!({"email":"other@example.test","password":"a sufficiently long password","name":"Other","organization":"Second"}),None).await;
    assert_eq!(status, StatusCode::OK, "{other}");
    let other_id: Uuid = decode(other["user"]["id"].clone()).unwrap();
    run(pool.clone(), move |c| {
        c.transaction(|c| {
            c.identity_verify_email(other_id)?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let (status, _, _) = request(
        &state,
        &format!("/api/organizations/{org}/invitations"),
        json!({"email":"guest@example.test","role":"member"}),
        Some(&other_cookie),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, v, _) = request(
        &state,
        &format!("/api/organizations/{org}/invitations"),
        json!({"email":"guest@example.test","role":"member"}),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    // Two invitations competing for the final free seat cannot both commit.
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let invite = |barrier: std::sync::Arc<std::sync::Barrier>, email: &'static str| {
        let pool = pool.clone();
        async move {
            run(pool, move |c| {
                c.transaction(|c| {
                    c.organizations_lock(org)?;
                    let reserved = c.organizations_reserved_seats(org, true)?.count;
                    barrier.wait();
                    assert_eq!(reserved, 2);
                    c.organizations_insert_invitation(org, email, "member", email)?;
                    Ok(())
                })
            })
            .await
        }
    };
    let (a, b) = tokio::join!(
        invite(barrier.clone(), "seat-a@example.test"),
        invite(barrier, "seat-b@example.test")
    );
    assert_eq!([a, b].into_iter().filter(Result::is_ok).count(), 1);
    assert_eq!(
        run(pool.clone(), move |c| c
            .organizations_reserved_seats(org, true))
        .await
        .unwrap()
        .count,
        3
    );
    // Exceeding the transaction limit never partially persists users or email reservations.
    let oversized = run(pool.clone(), |c| {
        c.transaction(|c| {
            for index in 0..40 {
                c.identity_create_user(
                    format!("oversized-{index}@example.test"),
                    "Too many",
                    "redacted",
                )?;
            }
            Ok(())
        })
    })
    .await
    .unwrap_err();
    assert_eq!(oversized.1, "transaction_limit");
    assert!(
        run(pool.clone(), |c| c
            .identity_credentials("oversized-0@example.test"))
        .await
        .unwrap()
        .is_none()
    );
    // Versions revoke arbitrary numbers of sessions without a fan-out transaction.
    run(pool.clone(), move |c| {
        c.transaction(|c| {
            c.identity_create_session("session-a", uid)?;
            c.identity_create_session("session-b", uid)?;
            c.identity_issue_action("old-action", uid, "reset", None)?;
            Ok(())
        })
    })
    .await
    .unwrap();
    run(pool.clone(), move |c| {
        c.transaction(|c| {
            c.actor("user", uid)?;
            c.identity_update_password("new-secret-hash", uid)?;
            c.identity_revoke_sessions(uid)?;
            c.identity_revoke_actions(uid)?;
            Ok(())
        })
    })
    .await
    .unwrap();
    run(pool.clone(), move |c| {
        c.transaction(|c| {
            assert!(c.identity_session_user("session-a")?.is_none());
            assert!(c.identity_session_user("session-b")?.is_none());
            assert!(c.identity_consume_action("old-action", "reset")?.is_none());
            Ok(())
        })
    })
    .await
    .unwrap();
    let history = run(pool.clone(), move |c| c.identity_user_history(uid, 0))
        .await
        .unwrap();
    let history = serde_json::to_string(&history).unwrap();
    assert!(!history.contains("new-secret-hash"));
    assert!(history.contains("[redacted]"));
    assert!(history.contains(&uid.to_string()));
    // Expiry is enforced on reads without depending on physical TTL deletion.
    run(pool.clone(), move |c| {
        c.transaction(|c| {
            c.identity_create_session("expired-session", uid)?;
            let k = key("session", "expired-session");
            let mut session = c.get(&k)?;
            session["expires_at"] = json!(now() - chrono::Duration::seconds(1));
            c.save(k, session)?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(
        run(pool.clone(), |c| c.identity_session_user("expired-session"))
            .await
            .unwrap()
            .is_none()
    );
    // Provider event IDs remain reserved across retries.
    assert_eq!(
        run(pool.clone(), |c| c
            .transaction(|c| c.billing_record_event("evt_test")))
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        run(pool.clone(), |c| c
            .transaction(|c| c.billing_record_event("evt_test")))
        .await
        .unwrap(),
        0
    );
    // A deleted/restored account cannot reuse a revoked session.
    run(pool.clone(), move |c| {
        c.transaction(|c| {
            c.identity_manage_user(uid, "owner@example.test", "Owner", "deleted")?;
            c.identity_revoke_sessions(uid)?;
            Ok(())
        })
    })
    .await
    .unwrap();
    run(pool.clone(), move |c| {
        c.transaction(|c| {
            c.identity_manage_user(uid, "owner@example.test", "Owner", "active")?;
            assert!(c.identity_session_user("session-a")?.is_none());
            Ok(())
        })
    })
    .await
    .unwrap();
    // Durable job claims are exclusive, fenced, and idempotent after completion.
    let job = run(pool.clone(), |c| {
        c.transaction(|c| c.jobs_enqueue(json!({"type":"test"}), 0))
    })
    .await
    .unwrap();
    let (a, b) = tokio::join!(
        run(pool.clone(), move |c| c.jobs_claim(Some(job))),
        run(pool.clone(), move |c| c.jobs_claim(Some(job)))
    );
    let rows = [a.unwrap(), b.unwrap()];
    assert_eq!(rows.iter().filter(|r| r.is_some()).count(), 1);
    let lease = rows.into_iter().flatten().next().unwrap().lease_version;
    assert!(
        !run(pool.clone(), move |c| c.jobs_complete(job, lease - 1))
            .await
            .unwrap()
    );
    assert!(
        run(pool.clone(), move |c| c.jobs_complete(job, lease))
            .await
            .unwrap()
    );
    assert!(run(pool.clone(), move |c| c.jobs_done(job)).await.unwrap());
    // Management bootstrap is permanent, and admin writes have their own actor realm.
    let input = || crate::models::CreateAdmin {
        email: "admin@example.test".into(),
        name: "Admin".into(),
        password: "a sufficiently long password".into(),
    };
    let admin = crate::bootstrap_admin(pool.clone(), input()).await.unwrap();
    assert!(crate::bootstrap_admin(pool.clone(), input()).await.is_err());
    run(pool.clone(), move |c| {
        c.transaction(|c| {
            c.admin_require_actor(admin.id)?;
            c.identity_manage_user(uid, "owner@example.test", "Renamed", "active")
        })
    })
    .await
    .unwrap();
    let history = run(pool.clone(), move |c| c.identity_user_history(uid, 0))
        .await
        .unwrap();
    assert!(
        history
            .iter()
            .any(|h| h.actor_kind == "admin" && h.actor_id == Some(admin.id))
    );
    pool.client
        .delete_table()
        .table_name(&pool.table)
        .send()
        .await
        .unwrap();
}
