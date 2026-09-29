#![allow(clippy::unwrap_used)] // Test fixtures fail fast, including helper functions.
#[derive(diesel::QueryableByName)]
struct TextValue {
    #[diesel(sql_type=diesel::sql_types::Text)]
    value: String,
}
use appshell_api::{AppState, config::Config, db, router};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use diesel::{prelude::*, sql_query, sql_types::Text};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    body: Value,
    cookie: Option<&str>,
    origin: &str,
) -> (StatusCode, Value, Option<String>) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("origin", origin);
    if let Some(cookie) = cookie {
        req = req.header("cookie", cookie);
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"text":String::from_utf8_lossy(&bytes)}));
    (status, json, cookie)
}
const ORIGIN: &str = "http://localhost:5173";
async fn call(
    app: &Router,
    path: &str,
    body: Value,
    cookie: Option<&str>,
) -> (StatusCode, Value, Option<String>) {
    request(app, "POST", path, body, cookie, ORIGIN).await
}
async fn read(app: &Router, path: &str, cookie: &str) -> (StatusCode, Value, Option<String>) {
    request(app, "GET", path, json!(null), Some(cookie), ORIGIN).await
}
async fn mail_token(pool: db::DbPool, email: &str, subject: &str) -> String {
    let email = email.to_owned();
    let subject = subject.to_owned();
    db::run(pool,move |c| {
        let row=sql_query("SELECT body AS value FROM mail_outbox WHERE recipient=$1 AND subject=$2 ORDER BY created_at DESC LIMIT 1")
            .bind::<Text,_>(email).bind::<Text,_>(subject).get_result::<TextValue>(c)?;
        Ok(row.value.split("#token=").nth(1).unwrap().split_whitespace().next().unwrap().to_owned())
    }).await.unwrap()
}
async fn signup(app: &Router, email: &str) -> (String, String) {
    let (status,value,cookie)=call(app,"/api/auth/signup",json!({"email":email,"password":"correct horse battery staple","name":"Test Person","organization":"Test Studio"}),None).await;
    assert_eq!(status, 200, "{value}");
    (
        cookie.unwrap(),
        value["organizations"][0]["id"].as_str().unwrap().into(),
    )
}
async fn verify(app: &Router, pool: db::DbPool, email: &str) -> String {
    let token = mail_token(pool, email, "Verify your email").await;
    assert_eq!(
        call(app, "/api/auth/verify-email", json!({"token":token}), None)
            .await
            .0,
        200
    );
    token
}

// A disposable schema keeps tests isolated from developer data, even when sharing a database.
#[tokio::test]
async fn full_account_and_tenant_lifecycle() {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("Set TEST_DATABASE_URL to run Postgres integration tests");
    let admin = db::connect(&url).unwrap();
    let schema = format!("test_{}", Uuid::new_v4().simple());
    let schema_create = schema.clone();
    db::run(admin.clone(), move |c| {
        sql_query(format!("CREATE SCHEMA {schema_create}")).execute(c)?;
        Ok(())
    })
    .await
    .unwrap();
    let sep = if url.contains('?') { '&' } else { '?' };
    let pool = db::connect(&format!("{url}{sep}options=-csearch_path%3D{schema}")).unwrap();
    db::migrate(pool.clone()).await.unwrap();
    let config = Config {
        app_url: ORIGIN.into(),
        production: false,
        mail_mode: "console".into(),
        resend_key: None,
        mail_from: "test@example.com".into(),
        stripe_key: None,
        stripe_price: None,
        stripe_webhook_secret: Some("test-webhook-secret".into()),
    };
    let app = router(AppState::new(pool.clone(), config), None);
    // Browser-origin middleware must exempt the nested, signature-protected provider route.
    {
        use hmac::{Hmac, Mac};
        let body = r#"{"type":"ignored.event","id":"evt_test"}"#;
        let timestamp = chrono::Utc::now().timestamp();
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(b"test-webhook-secret").unwrap();
        mac.update(format!("{timestamp}.{body}").as_bytes());
        let signature = format!(
            "t={timestamp},v1={}",
            hex::encode(mac.finalize().into_bytes())
        );
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/webhooks/stripe")
                    .header("stripe-signature", signature)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            200,
            "signed webhooks don't have a browser Origin"
        );
    }
    let (owner, org) = signup(&app, "owner@example.com").await;
    assert_eq!(
        read(&app, &format!("/api/organizations/{org}/team"), &owner)
            .await
            .0,
        403,
        "unverified accounts cannot access team data"
    );
    let token = verify(&app, pool.clone(), "owner@example.com").await;
    assert_eq!(
        call(&app, "/api/auth/verify-email", json!({"token":token}), None)
            .await
            .0,
        400,
        "tokens are single use"
    );
    assert_eq!(
        read(&app, &format!("/api/organizations/{org}/team"), &owner)
            .await
            .0,
        200
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/auth/logout",
            json!({}),
            Some(&owner),
            "https://evil.example"
        )
        .await
        .0,
        403,
        "CSRF origin check"
    );
    assert_eq!(
        call(&app, "/api/auth/signup", json!({"email":"bad"}), None)
            .await
            .0,
        400,
        "invalid JSON shape has a structured error"
    );
    let (outsider, _) = signup(&app, "outsider@example.com").await;
    verify(&app, pool.clone(), "outsider@example.com").await;
    assert_eq!(
        read(&app, &format!("/api/organizations/{org}/team"), &outsider)
            .await
            .0,
        403,
        "tenant isolation"
    );
    let invite_path = format!("/api/organizations/{org}/invitations");
    assert_eq!(
        call(
            &app,
            &invite_path,
            json!({"email":"outsider@example.com","role":"member"}),
            Some(&outsider)
        )
        .await
        .0,
        403
    );
    assert_eq!(
        call(
            &app,
            &invite_path,
            json!({"email":"outsider@example.com","role":"member"}),
            Some(&owner)
        )
        .await
        .0,
        200
    );
    let invite = mail_token(
        pool.clone(),
        "outsider@example.com",
        "You're invited to a workspace",
    )
    .await;
    assert_eq!(
        call(
            &app,
            "/api/invitations/accept",
            json!({"token":invite}),
            Some(&owner)
        )
        .await
        .0,
        400,
        "invitations are bound to email"
    );
    let (one, two) = tokio::join!(
        call(
            &app,
            "/api/invitations/accept",
            json!({"token":invite}),
            Some(&outsider)
        ),
        call(
            &app,
            "/api/invitations/accept",
            json!({"token":invite}),
            Some(&outsider)
        )
    );
    assert!(
        (one.0 == 200 && two.0 == 400) || (one.0 == 400 && two.0 == 200),
        "only one concurrent acceptance succeeds: {} {}",
        one.0,
        two.0
    );
    assert_eq!(
        call(
            &app,
            &invite_path,
            json!({"email":"third@example.com","role":"member"}),
            Some(&outsider)
        )
        .await
        .0,
        403,
        "members cannot invite"
    );
    assert_eq!(
        call(
            &app,
            &invite_path,
            json!({"email":"third@example.com","role":"member"}),
            Some(&owner)
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            &invite_path,
            json!({"email":"fourth@example.com","role":"member"}),
            Some(&owner)
        )
        .await
        .0,
        400,
        "pending invites reserve seats"
    );
    let billing = read(&app, &format!("/api/organizations/{org}/billing"), &owner)
        .await
        .1;
    assert_eq!(billing["seat_limit"], 3);
    assert_eq!(billing["seats_used"], 2);
    assert_eq!(billing["billing_enabled"], false);
    assert_eq!(
        call(
            &app,
            &format!("/api/organizations/{org}/checkout"),
            json!({}),
            Some(&owner)
        )
        .await
        .0,
        400
    );
    assert_eq!(
        call(
            &app,
            "/api/auth/change-email",
            json!({"email":"new@example.com","password":"wrong"}),
            Some(&owner)
        )
        .await
        .0,
        400
    );
    assert_eq!(
        call(
            &app,
            "/api/auth/change-email",
            json!({"email":"new@example.com","password":"correct horse battery staple"}),
            Some(&owner)
        )
        .await
        .0,
        200
    );
    assert_eq!(
        read(&app, "/api/auth/session", &owner).await.1["user"]["email"],
        "owner@example.com",
        "email changes only after confirmation"
    );
    let change = mail_token(pool.clone(), "new@example.com", "Confirm your new email").await;
    assert_eq!(
        call(
            &app,
            "/api/auth/confirm-email",
            json!({"token":change}),
            None
        )
        .await
        .0,
        200
    );
    assert_eq!(
        read(&app, "/api/auth/session", &owner).await.0,
        401,
        "email change revokes sessions"
    );
    let (_, _, new_cookie) = call(
        &app,
        "/api/auth/login",
        json!({"email":"new@example.com","password":"correct horse battery staple"}),
        None,
    )
    .await;
    let new_cookie = new_cookie.unwrap();
    let known = call(
        &app,
        "/api/auth/forgot-password",
        json!({"email":"new@example.com"}),
        None,
    )
    .await;
    let unknown = call(
        &app,
        "/api/auth/forgot-password",
        json!({"email":"missing@example.com"}),
        None,
    )
    .await;
    assert_eq!(
        known.1, unknown.1,
        "password reset does not enumerate accounts"
    );
    let reset = mail_token(pool.clone(), "new@example.com", "Reset your password").await;
    assert_eq!(
        call(
            &app,
            "/api/auth/reset-password",
            json!({"token":reset,"password":"a brand new secure password"}),
            None
        )
        .await
        .0,
        200
    );
    assert_eq!(
        read(&app, "/api/auth/session", &new_cookie).await.0,
        401,
        "reset revokes all sessions"
    );
    assert_eq!(
        call(
            &app,
            "/api/auth/reset-password",
            json!({"token":reset,"password":"a different secure password"}),
            None
        )
        .await
        .0,
        400
    );
    assert_eq!(
        call(
            &app,
            "/api/auth/login",
            json!({"email":"new@example.com","password":"correct horse battery staple"}),
            None
        )
        .await
        .0,
        401
    );
    let (status, _, cookie) = call(
        &app,
        "/api/auth/login",
        json!({"email":"new@example.com","password":"a brand new secure password"}),
        None,
    )
    .await;
    assert_eq!(status, 200);
    let cookie = cookie.unwrap();
    assert_eq!(call(&app,"/api/auth/change-password",json!({"current_password":"a brand new secure password","password":"yet another secure password"}),Some(&cookie)).await.0,200);
    assert_eq!(read(&app, "/api/auth/session", &cookie).await.0, 401);
    let (expired_cookie, _) = signup(&app, "expired@example.com").await;
    let expired = mail_token(pool.clone(), "expired@example.com", "Verify your email").await;
    db::run(pool.clone(), |c| {
        sql_query("UPDATE action_tokens SET expires_at=now()-interval '1 hour'").execute(c)?;
        sql_query("UPDATE sessions SET expires_at=now()-interval '1 hour'").execute(c)?;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(
        call(
            &app,
            "/api/auth/verify-email",
            json!({"token":expired}),
            None
        )
        .await
        .0,
        400
    );
    assert_eq!(
        read(&app, "/api/auth/session", &expired_cookie).await.0,
        401
    );
    drop(app);
    drop(pool);
    db::run(admin, move |c| {
        sql_query(format!("DROP SCHEMA {schema} CASCADE")).execute(c)?;
        Ok(())
    })
    .await
    .unwrap();
}
