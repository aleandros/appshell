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
        job_backend: appshell_api::config::JobBackend::Disabled,
        app_url: ORIGIN.into(),
        production: false,
        mail_mode: "console".into(),
        resend_key: None,
        mail_from: "test@example.com".into(),
        mail_brand: "AppShell".into(),
        smtp_host: "localhost".into(),
        smtp_port: 1025,
        stripe_key: None,
        stripe_price: None,
        stripe_webhook_secret: Some("test-webhook-secret".into()),
    };
    let state = AppState::new(pool.clone(), config);
    let app = router(state.clone(), None);
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
    #[cfg(feature = "lambda")]
    {
        // API Gateway v2 puts cookies outside headers. Exercise the real Lambda
        // request adapter so session and origin semantics survive the deployment change.
        for (method, path, origin, expected) in [
            ("GET", "/api/auth/session", ORIGIN, 200),
            ("POST", "/api/auth/logout", "https://evil.example", 403),
        ] {
            let event = json!({
                "version": "2.0", "routeKey": "ANY /{proxy+}",
                "rawPath": path, "rawQueryString": "", "cookies": [owner],
                "headers": {"origin": origin, "content-type": "application/json"},
                "requestContext": {
                    "routeKey": "ANY /{proxy+}", "stage": "$default",
                    "requestId": "test", "timeEpoch": 0,
                    "http": {"method": method, "path": path, "protocol": "HTTP/1.1", "sourceIp": "127.0.0.1", "userAgent": "test"}
                },
                "body": "{}", "isBase64Encoded": false
            });
            let request = lambda_http::request::from_str(&event.to_string()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), expected, "Lambda {method} {path}");
        }
    }

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
    // A removed membership remains stored and an invitation creates a new active one.
    let outsider_id = read(&app, "/api/auth/session", &outsider).await.1["user"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/api/organizations/{org}/members/{outsider_id}"),
            json!({}),
            Some(&owner),
            ORIGIN
        )
        .await
        .0,
        200
    );
    assert_eq!(
        read(&app, &format!("/api/organizations/{org}/team"), &outsider)
            .await
            .0,
        403
    );
    let target = outsider_id.clone();
    let organization = org.clone();
    db::run(pool.clone(),move |c| {
        let row=sql_query("SELECT deleted_by_kind AS value FROM memberships WHERE user_id=$1::uuid AND organization_id=$2::uuid AND deleted_at IS NOT NULL")
            .bind::<Text,_>(&target).bind::<Text,_>(&organization).get_result::<TextValue>(c)?;
        assert_eq!(row.value,"user"); Ok(())
    }).await.unwrap();
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
    let rejoin = mail_token(
        pool.clone(),
        "outsider@example.com",
        "You're invited to a workspace",
    )
    .await;
    assert_eq!(
        call(
            &app,
            "/api/invitations/accept",
            json!({"token":rejoin}),
            Some(&outsider)
        )
        .await
        .0,
        200
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

    // Delivery failure preserves content and schedules a retry. Success scrubs both
    // formats; a subsequent worker tick must not claim a delivered message again.
    let job_id = db::run(pool.clone(), move |c| {
        sql_query("UPDATE mail_outbox SET available_at=now()+interval '1 day'").execute(c)?;
        let row = sql_query("INSERT INTO mail_outbox(recipient,subject,body,html_body) VALUES('delivery@example.test','Test delivery','plain secret','<p>html secret</p>') RETURNING id::text AS value").get_result::<TextValue>(c)?;
        Ok(Uuid::parse_str(&row.value).unwrap())
    }).await.unwrap();
    let mut failed = state.clone();
    failed.config.mail_mode = "smtp".into();
    failed.config.smtp_host = "127.0.0.1".into();
    failed.config.smtp_port = 1;
    appshell_api::mail::tick(&failed).await.unwrap();
    db::run(pool.clone(), move |c| {
        let row=sql_query("SELECT body,html_body,attempts,sent_at IS NOT NULL AS sent,available_at>now() AS deferred FROM mail_outbox WHERE id=$1")
            .bind::<diesel::sql_types::Uuid,_>(job_id).get_result::<OutboxState>(c)?;
        assert_eq!(row.attempts,1); assert!(row.deferred); assert!(!row.sent);
        assert_eq!(row.body,"plain secret"); assert_eq!(row.html_body.as_deref(),Some("<p>html secret</p>"));
        sql_query("UPDATE mail_outbox SET available_at=now() WHERE id=$1").bind::<diesel::sql_types::Uuid,_>(job_id).execute(c)?;
        Ok(())
    }).await.unwrap();
    appshell_api::mail::tick(&state).await.unwrap();
    appshell_api::mail::tick(&state).await.unwrap();
    db::run(pool.clone(), move |c| {
        let row=sql_query("SELECT body,html_body,attempts,sent_at IS NOT NULL AS sent,available_at>now() AS deferred FROM mail_outbox WHERE id=$1")
            .bind::<diesel::sql_types::Uuid,_>(job_id).get_result::<OutboxState>(c)?;
        assert_eq!(row.attempts,2); assert!(row.sent);
        assert_eq!(row.body,"[delivered]"); assert!(row.html_body.is_none());
        Ok(())
    }).await.unwrap();
    admin_and_data_patterns(&app, pool.clone()).await;
    drop(failed);
    drop(state);
    drop(app);
    drop(pool);
    db::run(admin, move |c| {
        sql_query(format!("DROP SCHEMA {schema} CASCADE")).execute(c)?;
        Ok(())
    })
    .await
    .unwrap();
}

#[derive(diesel::QueryableByName)]
struct OutboxState {
    #[diesel(sql_type=diesel::sql_types::Text)]
    body: String,
    #[diesel(sql_type=diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    html_body: Option<String>,
    #[diesel(sql_type=diesel::sql_types::Integer)]
    attempts: i32,
    #[diesel(sql_type=diesel::sql_types::Bool)]
    sent: bool,
    #[diesel(sql_type=diesel::sql_types::Bool)]
    deferred: bool,
}

async fn admin_and_data_patterns(app: &Router, pool: db::DbPool) {
    use appshell_api::models::CreateAdmin;
    let input = || CreateAdmin {
        email: "root@example.test".into(),
        name: "Root Operator".into(),
        password: "first secure admin password".into(),
    };
    let root = appshell_api::bootstrap_admin(pool.clone(), input())
        .await
        .unwrap();
    assert!(
        appshell_api::bootstrap_admin(pool.clone(), input())
            .await
            .is_err(),
        "bootstrap cannot create subsequent admins"
    );
    let (user_cookie, _) = signup(app, "support@example.test").await;
    verify(app, pool.clone(), "support@example.test").await;
    let user = read(app, "/api/auth/session", &user_cookie).await.1["user"].clone();
    let user_id = user["id"].as_str().unwrap();
    let (manager, managed_org) = signup(app, "manager@example.test").await;
    verify(app, pool.clone(), "manager@example.test").await;
    assert_eq!(
        call(
            app,
            &format!("/api/organizations/{managed_org}/invitations"),
            json!({"email":"support@example.test","role":"member"}),
            Some(&manager)
        )
        .await
        .0,
        200
    );
    let invite = mail_token(
        pool.clone(),
        "support@example.test",
        "You're invited to a workspace",
    )
    .await;
    assert_eq!(
        call(
            app,
            "/api/invitations/accept",
            json!({"token":invite}),
            Some(&user_cookie)
        )
        .await
        .0,
        200
    );
    assert_eq!(
        read(app, "/api/admin/users", &user_cookie).await.0,
        401,
        "ordinary users have no admin access"
    );
    let (_, _, cookie) = call(
        app,
        "/api/admin/login",
        json!({"email":"root@example.test","password":"first secure admin password"}),
        None,
    )
    .await;
    let cookie = cookie.unwrap();
    assert!(cookie.starts_with("appshell_admin_session="));
    assert_eq!(
        read(app, "/api/auth/session", &cookie).await.0,
        401,
        "admin sessions cannot enter the user realm"
    );
    assert_eq!(
        call(
            app,
            "/api/auth/login",
            json!({"email":"root@example.test","password":"first secure admin password"}),
            None
        )
        .await
        .0,
        401
    );
    assert_eq!(
        request(
            app,
            "POST",
            "/api/admin/accounts",
            json!({}),
            Some(&cookie),
            "https://evil.example"
        )
        .await
        .0,
        403
    );
    let (status,second,_)=call(app,"/api/admin/accounts",json!({"email":"second@example.test","name":"Second Operator","password":"second secure admin password"}),Some(&cookie)).await;
    assert_eq!(status, 200, "{second}");
    assert_eq!(
        call(
            app,
            &format!("/api/admin/accounts/{}", root.id),
            json!({"email":root.email,"name":root.name,"status":"suspended"}),
            Some(&cookie)
        )
        .await
        .0,
        400,
        "cannot disable own administrator"
    );
    let (_, _, second_cookie) = call(
        app,
        "/api/admin/login",
        json!({"email":"second@example.test","password":"second secure admin password"}),
        None,
    )
    .await;
    let second_cookie = second_cookie.unwrap();
    assert_eq!(
        call(
            app,
            &format!("/api/admin/accounts/{}", second["id"].as_str().unwrap()),
            json!({"email":"second@example.test","name":"Second Operator","status":"deleted"}),
            Some(&cookie)
        )
        .await
        .0,
        200
    );
    assert_eq!(read(app, "/api/admin/session", &second_cookie).await.0, 401);
    assert_eq!(
        call(
            app,
            "/api/admin/login",
            json!({"email":"second@example.test","password":"second secure admin password"}),
            None
        )
        .await
        .0,
        401
    );
    let path = format!("/api/admin/users/{user_id}");
    let update = |email: &str, status: &str| json!({"email":email,"name":"Supported Person","status":status});
    let (status, body, _) = call(
        app,
        &path,
        update("corrected@example.test", "active"),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        read(app, "/api/auth/session", &user_cookie).await.0,
        401,
        "email correction revokes user sessions"
    );
    let managed = read(app, "/api/admin/users?search=corrected", &cookie)
        .await
        .1;
    assert!(
        managed[0]["email_verified_at"].is_null(),
        "corrected emails require verification"
    );
    let verification =
        mail_token(pool.clone(), "corrected@example.test", "Verify your email").await;
    assert_eq!(
        call(
            app,
            "/api/auth/verify-email",
            json!({"token":verification}),
            None
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            app,
            &format!("{path}/reset-password"),
            json!({}),
            Some(&cookie)
        )
        .await
        .0,
        200
    );
    let reset = mail_token(
        pool.clone(),
        "corrected@example.test",
        "Reset your password",
    )
    .await;
    assert_eq!(
        call(
            app,
            "/api/auth/reset-password",
            json!({"token":reset,"password":"replacement secure user password"}),
            None
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            app,
            &path,
            update("corrected@example.test", "suspended"),
            Some(&cookie)
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            app,
            "DELETE",
            &format!("/api/organizations/{managed_org}/members/{user_id}"),
            json!({}),
            Some(&manager),
            ORIGIN
        )
        .await
        .0,
        200,
        "owners can remove suspended members and release their seat"
    );
    assert_eq!(
        call(
            app,
            "/api/auth/login",
            json!({"email":"corrected@example.test","password":"replacement secure user password"}),
            None
        )
        .await
        .0,
        401
    );
    assert_eq!(
        call(
            app,
            &format!("{path}/reset-password"),
            json!({}),
            Some(&cookie)
        )
        .await
        .0,
        400
    );
    assert_eq!(
        call(
            app,
            &path,
            update("corrected@example.test", "deleted"),
            Some(&cookie)
        )
        .await
        .0,
        200
    );
    assert!(
        !read(app, "/api/admin/users?search=corrected", &cookie)
            .await
            .1[0]["deleted_at"]
            .is_null()
    );
    let actor = root.id.to_string();
    let target = user_id.to_owned();
    db::run(pool.clone(),move |c| {
        let value=sql_query("SELECT deleted_by::text AS value FROM users WHERE id=$1::uuid").bind::<Text,_>(&target).get_result::<TextValue>(c)?;
        assert_eq!(value.value,actor);
        let value=sql_query("SELECT changes::text AS value FROM users_history WHERE record_id=$1::uuid AND changes ? 'deleted_at' AND actor_kind='admin' ORDER BY changed_at DESC LIMIT 1").bind::<Text,_>(&target).get_result::<TextValue>(c)?;
        assert!(value.value.contains("deleted_by"));
        Ok(())
    }).await.unwrap();
    assert_eq!(
        call(
            app,
            &path,
            update("corrected@example.test", "active"),
            Some(&cookie)
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            app,
            "/api/auth/login",
            json!({"email":"corrected@example.test","password":"replacement secure user password"}),
            None
        )
        .await
        .0,
        200
    );
    let history = read(app, &format!("{path}/history"), &cookie).await.1;
    assert!(
        history
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["actor_kind"] == "admin"
                && entry["actor_id"] == root.id.to_string()
                && entry["changes"]["email"]["from"] == "support@example.test"
                && entry["changes"]["email"]["to"] == "corrected@example.test")
    );
    assert!(history.to_string().contains("[redacted]"));
    assert!(!history.to_string().contains("$argon2"));
    assert_eq!(
        call(app, "/api/admin/logout", json!({}), Some(&cookie))
            .await
            .0,
        200
    );
    assert_eq!(read(app, "/api/admin/users", &cookie).await.0, 401);

    db::run(pool.clone(),move |c| {
        // Protection applies to direct SQL too; bypass requires privileged trigger DDL.
        for statement in ["DELETE FROM users", "TRUNCATE mail_outbox", "UPDATE users_history SET operation='UPDATE'", "DELETE FROM sessions_history", "TRUNCATE users_history"] {
            assert!(sql_query(statement).execute(c).is_err(),"accepted {statement}");
        }
        sql_query("SELECT validate_model_conventions()").execute(c)?;
        // UUID generation overrides even explicitly supplied IDs, and IDs are immutable.
        let supplied=Uuid::new_v4().to_string();
        let generated=sql_query("INSERT INTO organizations(id,name) VALUES($1::uuid,'UUID test') RETURNING id::text AS value").bind::<Text,_>(&supplied).get_result::<TextValue>(c)?.value;
        assert_ne!(generated,supplied);
        assert!(sql_query("UPDATE organizations SET id=gen_random_uuid() WHERE id=$1::uuid").bind::<Text,_>(&generated).execute(c).is_err());
        // Changes and history commit or roll back together.
        let result=c.transaction::<(),appshell_api::error::ApiError,_>(|c| {
            sql_query("UPDATE organizations SET name='rolled back' WHERE id=$1::uuid").bind::<Text,_>(&generated).execute(c)?;
            Err(appshell_api::error::ApiError::bad("rollback"))
        });
        assert!(result.is_err());
        let retained=sql_query("SELECT count(*)::text AS value FROM organizations_history WHERE record_id=$1::uuid").bind::<Text,_>(&generated).get_result::<TextValue>(c)?;
        assert_eq!(retained.value,"1");
        // Attribution cannot leak between pooled requests and the system worker.
        sql_query("UPDATE organizations SET deleted_at=now() WHERE id=$1::uuid").bind::<Text,_>(&generated).execute(c)?;
        let system=sql_query("SELECT actor_kind AS value FROM organizations_history WHERE record_id=$1::uuid ORDER BY changed_at DESC LIMIT 1").bind::<Text,_>(&generated).get_result::<TextValue>(c)?;
        assert_eq!(system.value,"system");
        let redaction=sql_query("SELECT changes::text AS value FROM mail_outbox_history WHERE changes ? 'body' LIMIT 1").get_result::<TextValue>(c)?;
        assert!(redaction.value.contains("[redacted]")); assert!(!redaction.value.contains("#token="));
        // New tables must register, even if they have an apparently valid UUID ID.
        sql_query("CREATE TABLE missing_protection(id uuid PRIMARY KEY DEFAULT gen_random_uuid())").execute(c)?;
        assert!(sql_query("SELECT validate_model_conventions()").execute(c).is_err());
        sql_query("SELECT protect_model('missing_protection')").execute(c)?;
        sql_query("SELECT validate_model_conventions()").execute(c)?;
        sql_query("ALTER TABLE missing_protection DISABLE TRIGGER protect_removal").execute(c)?;
        assert!(sql_query("SELECT validate_model_conventions()").execute(c).is_err());
        sql_query("ALTER TABLE missing_protection ENABLE TRIGGER protect_removal").execute(c)?;
        Ok(())
    }).await.unwrap();
}
