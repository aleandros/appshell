use crate::{
    AppState,
    contexts::identity,
    error::{ApiError, Result},
    infrastructure::{crypto, repositories},
    models::*,
};
use uuid::Uuid;

pub(crate) async fn bootstrap(
    pool: repositories::DbPool,
    input: CreateAdmin,
) -> Result<AdminAccount> {
    let email = appshell_domain::identity::email(&input.email)?;
    let name = appshell_domain::identity::name(&input.name)?;
    repositories::run(pool, move |c| {
        let hash = crypto::hash_password(&input.password)?;
        c.transaction(|c| {
            c.admin_management_lock()?;
            if c.admin_count()? != 0 {
                return Err(ApiError::bad(
                    "Bootstrap is disabled after the first administrator. Use the admin console.",
                ));
            }
            c.admin_create(&email, &name, &hash)
        })
    })
    .await
}
pub(crate) async fn authenticate(state: &AppState, token: String) -> Result<AdminAccount> {
    repositories::run(state.pool.clone(), move |c| {
        c.admin_session(&crypto::digest(&token))?
            .ok_or_else(ApiError::unauthorized)
    })
    .await
}
pub(crate) async fn login(state: AppState, input: Login) -> Result<(String, AdminAccount)> {
    let email = appshell_domain::identity::email(&input.email)?;
    if input.password.len() > 128 {
        return Err(ApiError::unauthorized());
    }
    let token = crypto::token();
    let hash = crypto::digest(&token);
    let account = repositories::run(state.pool, move |c| {
        identity::rate_limit(c, &format!("admin-login:{email}"), 10)?;
        c.transaction(|c| {
            c.admin_management_lock()?;
            let credentials = c.admin_credentials(&email)?;
            let valid = crypto::verify_password(
                &input.password,
                credentials
                    .as_ref()
                    .and_then(|a| a.password_hash.as_deref())
                    .unwrap_or(&state.dummy_password_hash),
            );
            let id = credentials
                .filter(|_| valid)
                .ok_or_else(ApiError::unauthorized)?
                .id;
            c.admin_require_actor(id)?;
            c.admin_create_session(id, &hash)?;
            c.admin_session(&hash)?.ok_or_else(ApiError::unauthorized)
        })
    })
    .await?;
    Ok((token, account))
}
pub(crate) async fn logout(state: AppState, token: Option<String>) -> Result<Message> {
    if let Some(token) = token {
        repositories::run(state.pool, move |c| {
            c.transaction(|c| {
                let hash = crypto::digest(&token);
                if let Some(admin) = c.admin_session(&hash)? {
                    c.actor("admin", admin.id)?;
                }
                c.admin_logout(&hash)
            })
        })
        .await?;
    }
    Ok(message("Signed out of administration."))
}
fn search(input: AdminSearch) -> Result<AdminSearch> {
    if input.search.len() > 254 || input.offset < 0 {
        return Err(ApiError::bad("Invalid search or offset."));
    }
    Ok(input)
}
pub(crate) async fn users(
    state: AppState,
    actor: AdminAccount,
    input: AdminSearch,
) -> Result<Vec<ManagedUser>> {
    let input = search(input)?;
    repositories::run(state.pool, move |c| {
        c.transaction(|c| {
            c.admin_require_actor(actor.id)?;
            identity::managed_users(c, &input.search, input.offset)
        })
    })
    .await
}
pub(crate) async fn accounts(
    state: AppState,
    actor: AdminAccount,
    input: AdminSearch,
) -> Result<Vec<AdminAccount>> {
    let input = search(input)?;
    repositories::run(state.pool, move |c| {
        c.transaction(|c| {
            c.admin_require_actor(actor.id)?;
            c.admin_list(&input.search, input.offset)
        })
    })
    .await
}
pub(crate) async fn create(
    state: AppState,
    actor: AdminAccount,
    input: CreateAdmin,
) -> Result<AdminAccount> {
    let email = appshell_domain::identity::email(&input.email)?;
    let name = appshell_domain::identity::name(&input.name)?;
    repositories::run(state.pool, move |c| {
        let hash = crypto::hash_password(&input.password)?;
        c.transaction(|c| {
            c.admin_management_lock()?;
            c.admin_require_actor(actor.id)?;
            c.admin_create(&email, &name, &hash)
        })
    })
    .await
}
pub(crate) async fn update(
    state: AppState,
    actor: AdminAccount,
    id: Uuid,
    input: UpdateAdmin,
) -> Result<Message> {
    let email = appshell_domain::identity::email(&input.email)?;
    let name = appshell_domain::identity::name(&input.name)?;
    appshell_domain::admin::allow_admin_change(id == actor.id, &input.status)?;
    repositories::run(state.pool, move |c| {
        let hash = input
            .password
            .as_deref()
            .map(crypto::hash_password)
            .transpose()?;
        c.transaction(|c| {
            c.admin_management_lock()?;
            c.admin_require_actor(actor.id)?;
            c.admin_update(id, &email, &name, &input.status, hash.as_deref())
        })
    })
    .await?;
    Ok(message(
        "Administrator updated. Their sessions have been revoked.",
    ))
}
pub(crate) async fn update_user(
    state: AppState,
    actor: AdminAccount,
    id: Uuid,
    input: UpdateUser,
) -> Result<Message> {
    repositories::run(state.pool, move |c| {
        c.transaction(|c| {
            c.admin_require_actor(actor.id)?;
            identity::manage_user(c, id, input, &state.config)
        })
    })
    .await?;
    Ok(message("User updated. Their sessions have been revoked."))
}
pub(crate) async fn reset_user(state: AppState, actor: AdminAccount, id: Uuid) -> Result<Message> {
    repositories::run(state.pool, move |c| {
        identity::rate_limit(c, &format!("admin-reset:{id}"), 5)?;
        c.transaction(|c| {
            c.admin_require_actor(actor.id)?;
            identity::managed_reset(c, id, &state.config)
        })
    })
    .await?;
    Ok(message(
        "Sessions revoked and a password reset email queued.",
    ))
}
pub(crate) async fn history(
    state: AppState,
    actor: AdminAccount,
    id: Uuid,
    input: AdminSearch,
) -> Result<Vec<HistoryEntry>> {
    let input = search(input)?;
    repositories::run(state.pool, move |c| {
        c.transaction(|c| {
            c.admin_require_actor(actor.id)?;
            identity::managed_history(c, id, input.offset)
        })
    })
    .await
}
