use crate::{
    AppState, db,
    error::{ApiError, ApiJson, Result},
    mail,
    models::*,
    security::{self, Auth},
};
use axum::{
    Json,
    extract::{Path, State},
};
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Text, Uuid as SqlUuid},
};
use uuid::Uuid;

pub fn seat_limit(c: &mut PgConnection, org: Uuid) -> Result<i64> {
    let sub = sql_query(
        "SELECT plan,status,current_period_end FROM subscriptions WHERE organization_id=$1",
    )
    .bind::<SqlUuid, _>(org)
    .get_result::<Subscription>(c)?;
    Ok(
        if sub.plan == "pro" && ["active", "trialing"].contains(&sub.status.as_str()) {
            50
        } else {
            3
        },
    )
}
pub fn lock_org(c: &mut PgConnection, org: Uuid) -> Result<()> {
    sql_query("SELECT id FROM organizations WHERE id=$1 FOR UPDATE")
        .bind::<SqlUuid, _>(org)
        .execute(c)?;
    Ok(())
}
fn check_seats(c: &mut PgConnection, org: Uuid, include_pending: bool) -> Result<()> {
    let used=sql_query("SELECT (SELECT count(*) FROM memberships WHERE organization_id=$1) + CASE WHEN $2 THEN (SELECT count(*) FROM invitations WHERE organization_id=$1 AND expires_at>now()) ELSE 0 END AS count")
        .bind::<SqlUuid,_>(org).bind::<diesel::sql_types::Bool,_>(include_pending).get_result::<Count>(c)?.count;
    if used >= seat_limit(c, org)? {
        return Err(ApiError::bad("Your workspace has reached its seat limit."));
    }
    Ok(())
}
#[utoipa::path(post,path="/api/organizations",request_body=NameInput,responses((status=200,body=Organization)))]
pub async fn create(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(input): ApiJson<NameInput>,
) -> Result<Json<Organization>> {
    security::verified(&user)?;
    let name = security::name(&input.name)?;
    Ok(Json(
        db::run(state.pool, move |c| {
            security::rate_limit(c, &format!("create-org:{}", user.id), 10)?;
            c.transaction::<_, ApiError, _>(|c| {
                let id = Uuid::new_v4();
                sql_query("INSERT INTO organizations(id,name) VALUES($1,$2)")
                    .bind::<SqlUuid, _>(id)
                    .bind::<Text, _>(&name)
                    .execute(c)?;
                sql_query(
                    "INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'owner')",
                )
                .bind::<SqlUuid, _>(id)
                .bind::<SqlUuid, _>(user.id)
                .execute(c)?;
                sql_query("INSERT INTO subscriptions(organization_id) VALUES($1)")
                    .bind::<SqlUuid, _>(id)
                    .execute(c)?;
                Ok(Organization {
                    id,
                    name,
                    role: "owner".into(),
                })
            })
        })
        .await?,
    ))
}
#[utoipa::path(get,path="/api/organizations/{org}/team",params(("org"=Uuid,Path)),responses((status=200,body=Team)))]
pub async fn team(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path(org): Path<Uuid>,
) -> Result<Json<Team>> {
    security::verified(&user)?;
    Ok(Json(db::run(state.pool,move |c| {
        let role=security::role(c,user.id,org)?;
        let members=sql_query("SELECT u.id,u.name,u.email,m.role FROM users u JOIN memberships m ON m.user_id=u.id WHERE m.organization_id=$1 ORDER BY u.name")
            .bind::<SqlUuid,_>(org).load::<Member>(c)?;
        let invitations=if role!="member" { sql_query("SELECT id,email,role,expires_at FROM invitations WHERE organization_id=$1 AND expires_at>now() ORDER BY email").bind::<SqlUuid,_>(org).load::<Invitation>(c)? } else { vec![] };
        Ok(Team { members,invitations })
    }).await?))
}
#[utoipa::path(post,path="/api/organizations/{org}/invitations",params(("org"=Uuid,Path)),request_body=InviteInput,responses((status=200,body=Message)))]
pub async fn invite(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path(org): Path<Uuid>,
    ApiJson(input): ApiJson<InviteInput>,
) -> Result<Json<Message>> {
    security::verified(&user)?;
    let email = security::email(&input.email)?;
    if !["admin", "member"].contains(&input.role.as_str()) {
        return Err(ApiError::bad("Choose admin or member."));
    }
    db::run(state.pool,move |c| {
        security::rate_limit(c,&format!("invite:{}",user.id),30)?;
        c.transaction::<_,ApiError,_>(|c| {
            lock_org(c,org)?; security::admin(c,user.id,org)?;
            if input.role=="admin" { security::owner(c,user.id,org)?; }
            let exists=sql_query("SELECT count(*) AS count FROM memberships m JOIN users u ON u.id=m.user_id WHERE m.organization_id=$1 AND u.email=$2").bind::<SqlUuid,_>(org).bind::<Text,_>(&email).get_result::<Count>(c)?.count;
            if exists>0 { return Err(ApiError::bad("This person is already a member.")); }
            // Reissuing replaces the old invite atomically and does not consume an extra seat.
            sql_query("DELETE FROM invitations WHERE organization_id=$1 AND email=$2").bind::<SqlUuid,_>(org).bind::<Text,_>(&email).execute(c)?;
            check_seats(c,org,true)?;
            let token=security::token();
            sql_query("INSERT INTO invitations(id,organization_id,email,role,token_hash,expires_at) VALUES($1,$2,$3,$4,$5,now()+interval '7 days')")
                .bind::<SqlUuid,_>(Uuid::new_v4()).bind::<SqlUuid,_>(org).bind::<Text,_>(&email).bind::<Text,_>(input.role).bind::<Text,_>(security::digest(&token)).execute(c)?;
            mail::enqueue(c,&email,"You're invited to a workspace",&format!("{} invited you to their workspace.\n\nSign in or create an account with {email}, then accept:\n{}/accept-invite#token={token}\n\nThis invitation expires in 7 days.",user.name,state.config.app_url))
        })
    }).await?;
    Ok(crate::auth::message(
        "Invitation sent. They'll receive an email shortly.",
    ))
}
#[derive(QueryableByName)]
struct InviteRecord {
    #[diesel(sql_type=SqlUuid)]
    organization_id: Uuid,
    #[diesel(sql_type=Text)]
    email: String,
    #[diesel(sql_type=Text)]
    role: String,
}
#[utoipa::path(post,path="/api/invitations/accept",request_body=TokenInput,responses((status=200,body=Message)))]
pub async fn accept(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(input): ApiJson<TokenInput>,
) -> Result<Json<Message>> {
    security::verified(&user)?;
    db::run(state.pool,move |c| c.transaction::<_,ApiError,_>(|c| {
        let hash=security::digest(&input.token);
        let invitation=sql_query("SELECT organization_id,email,role FROM invitations WHERE token_hash=$1 AND expires_at>now()")
            .bind::<Text,_>(&hash).get_result::<InviteRecord>(c).optional()?.ok_or_else(|| ApiError::bad("This invitation is invalid or has expired."))?;
        if invitation.email!=user.email { return Err(ApiError::bad("Sign in with the email address that received this invitation.")); }
        lock_org(c,invitation.organization_id)?;
        let deleted=sql_query("DELETE FROM invitations WHERE token_hash=$1 AND expires_at>now()").bind::<Text,_>(hash).execute(c)?;
        if deleted!=1 { return Err(ApiError::bad("This invitation has already been used or revoked.")); }
        if security::role(c,user.id,invitation.organization_id).is_err() {
            check_seats(c,invitation.organization_id,false)?;
            sql_query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,$3)").bind::<SqlUuid,_>(invitation.organization_id).bind::<SqlUuid,_>(user.id).bind::<Text,_>(invitation.role).execute(c)?;
        } Ok(())
    })).await?;
    Ok(crate::auth::message(
        "Invitation accepted. Switch to your new workspace.",
    ))
}
#[utoipa::path(delete,path="/api/organizations/{org}/invitations/{id}",params(("org"=Uuid,Path),("id"=Uuid,Path)),responses((status=200,body=Message)))]
pub async fn revoke_invite(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path((org, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Message>> {
    security::verified(&user)?;
    db::run(state.pool, move |c| {
        c.transaction::<_, ApiError, _>(|c| {
            lock_org(c, org)?;
            security::admin(c, user.id, org)?;
            sql_query("DELETE FROM invitations WHERE organization_id=$1 AND id=$2")
                .bind::<SqlUuid, _>(org)
                .bind::<SqlUuid, _>(id)
                .execute(c)?;
            Ok(())
        })
    })
    .await?;
    Ok(crate::auth::message("Invitation revoked."))
}
#[utoipa::path(delete,path="/api/organizations/{org}/members/{id}",params(("org"=Uuid,Path),("id"=Uuid,Path)),responses((status=200,body=Message)))]
pub async fn remove_member(
    State(state): State<AppState>,
    Auth(user): Auth,
    Path((org, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Message>> {
    security::verified(&user)?;
    db::run(state.pool, move |c| {
        c.transaction::<_, ApiError, _>(|c| {
            lock_org(c, org)?;
            security::admin(c, user.id, org)?;
            let target = security::role(c, id, org)?;
            if target == "owner" {
                return Err(ApiError::bad("The workspace owner cannot be removed."));
            }
            if target == "admin" {
                security::owner(c, user.id, org)?;
            }
            sql_query("DELETE FROM memberships WHERE organization_id=$1 AND user_id=$2")
                .bind::<SqlUuid, _>(org)
                .bind::<SqlUuid, _>(id)
                .execute(c)?;
            Ok(())
        })
    })
    .await?;
    Ok(crate::auth::message("Member removed."))
}
