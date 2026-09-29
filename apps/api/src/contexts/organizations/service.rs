use crate::infrastructure::repositories;
use crate::infrastructure::repositories::UnitOfWork;
use crate::{
    AppState,
    contexts::identity as security,
    error::{ApiError, Result},
    infrastructure::crypto,
    mail,
    models::*,
};
use uuid::Uuid;

pub fn seat_limit(c: &mut UnitOfWork<'_>, org: Uuid) -> Result<i64> {
    crate::contexts::billing::seat_limit(c, org)
}
pub fn lock_org(c: &mut UnitOfWork<'_>, org: Uuid) -> Result<()> {
    c.organizations_lock(org)?;
    Ok(())
}
fn check_seats(c: &mut UnitOfWork<'_>, org: Uuid, include_pending: bool) -> Result<()> {
    let used = c.organizations_reserved_seats(org, include_pending)?.count;
    Ok(appshell_domain::organizations::seats_available(
        used,
        seat_limit(c, org)?,
    )?)
}
pub async fn create(state: AppState, user: User, input: NameInput) -> Result<Organization> {
    security::verified(&user)?;
    let name = appshell_domain::identity::name(&input.name)?;
    repositories::run(state.pool, move |c| {
        security::rate_limit(c, &format!("create-org:{}", user.id), 10)?;
        c.transaction(|c| {
            security::authorize(c, &user)?;
            create_for_owner(c, user.id, name)
        })
    })
    .await
}
pub async fn team(state: AppState, user: User, org: Uuid) -> Result<Team> {
    security::verified(&user)?;
    repositories::run(state.pool, move |c| {
        let role = crate::contexts::organizations::role(c, user.id, org)?;
        let members = c.organizations_members(org)?;
        let invitations = if appshell_domain::organizations::can_view_invitations(&role) {
            c.organizations_invitations(org)?
        } else {
            vec![]
        };
        Ok(Team {
            members,
            invitations,
        })
    })
    .await
}
pub async fn invite(state: AppState, user: User, org: Uuid, input: InviteInput) -> Result<Message> {
    security::verified(&user)?;
    let email = appshell_domain::identity::email(&input.email)?;
    appshell_domain::organizations::invitation_role(&input.role)?;
    repositories::run(state.pool, move |c| {
        security::rate_limit(c, &format!("invite:{}", user.id), 30)?;
        c.transaction(|c| {
            security::authorize(c, &user)?;
            lock_org(c, org)?;
            let actor = role(c, user.id, org)?;
            let exists = c.organizations_members_with_email(org, &email)?.count;
            appshell_domain::organizations::allow_invitation(&actor, &input.role, exists)?;
            // Reissuing replaces the old invite atomically and does not consume an extra seat.
            c.organizations_delete_invitation_for_email(org, &email)?;
            check_seats(c, org, true)?;
            let token = crypto::token();
            c.organizations_insert_invitation(org, &email, input.role, crypto::digest(&token))?;
            mail::invitation(c, &email, &user.name, &token, &state.config)
        })
    })
    .await?;
    Ok(crate::models::message(
        "Invitation sent. They'll receive an email shortly.",
    ))
}

pub async fn accept(state: AppState, user: User, input: TokenInput) -> Result<Message> {
    security::verified(&user)?;
    repositories::run(state.pool, move |c| {
        c.transaction(|c| {
            security::authorize(c, &user)?;
            let hash = crypto::digest(&input.token);
            let invitation = c
                .organizations_invitation_by_token(&hash)?
                .ok_or_else(|| ApiError::bad("This invitation is invalid or has expired."))?;
            appshell_domain::organizations::invitation_recipient(&invitation.email, &user.email)?;
            lock_org(c, invitation.organization_id)?;
            let deleted = c.organizations_consume_invitation(hash)?;
            if deleted != 1 {
                return Err(ApiError::bad(
                    "This invitation has already been used or revoked.",
                ));
            }
            if c.organizations_role(user.id, invitation.organization_id)?
                .is_none()
            {
                check_seats(c, invitation.organization_id, false)?;
                c.organizations_add_member(invitation.organization_id, user.id, invitation.role)?;
            }
            Ok(())
        })
    })
    .await?;
    Ok(crate::models::message(
        "Invitation accepted. Switch to your new workspace.",
    ))
}
pub async fn revoke_invite(state: AppState, user: User, org: Uuid, id: Uuid) -> Result<Message> {
    security::verified(&user)?;
    repositories::run(state.pool, move |c| {
        c.transaction(|c| {
            security::authorize(c, &user)?;
            lock_org(c, org)?;
            crate::contexts::organizations::admin(c, user.id, org)?;
            c.organizations_revoke_invitation(org, id)?;
            Ok(())
        })
    })
    .await?;
    Ok(crate::models::message("Invitation revoked."))
}
pub async fn remove_member(state: AppState, user: User, org: Uuid, id: Uuid) -> Result<Message> {
    security::verified(&user)?;
    repositories::run(state.pool, move |c| {
        c.transaction(|c| {
            security::authorize(c, &user)?;
            lock_org(c, org)?;
            let actor = role(c, user.id, org)?;
            let target = role(c, id, org)?;
            appshell_domain::organizations::allow_removal(&actor, &target)?;
            c.organizations_remove_member(org, id)?;
            Ok(())
        })
    })
    .await?;
    Ok(crate::models::message("Member removed."))
}

pub(crate) fn role(c: &mut UnitOfWork<'_>, user: Uuid, org: Uuid) -> Result<String> {
    c.organizations_role(user, org)?
        .map(|r| r.value)
        .ok_or_else(ApiError::forbidden)
}
pub(crate) fn admin(c: &mut UnitOfWork<'_>, user: Uuid, org: Uuid) -> Result<()> {
    Ok(appshell_domain::organizations::require_admin(&role(
        c, user, org,
    )?)?)
}
pub(crate) fn owner(c: &mut UnitOfWork<'_>, user: Uuid, org: Uuid) -> Result<()> {
    Ok(appshell_domain::organizations::require_owner(&role(
        c, user, org,
    )?)?)
}

pub(crate) fn create_for_owner(
    c: &mut UnitOfWork<'_>,
    user: Uuid,
    name: String,
) -> Result<Organization> {
    let id = c.organizations_insert(&name)?;
    c.organizations_add_owner(id, user)?;
    crate::contexts::billing::initialize(c, id)?;
    Ok(Organization {
        id,
        name,
        role: "owner".into(),
    })
}
pub(crate) fn for_user(c: &mut UnitOfWork<'_>, user: Uuid) -> Result<Vec<Organization>> {
    c.organizations_for_user(user)
}
pub(crate) fn member_count(c: &mut UnitOfWork<'_>, org: Uuid) -> Result<i64> {
    Ok(c.organizations_member_count(org)?.count)
}
