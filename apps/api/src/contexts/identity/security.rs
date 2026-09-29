use crate::{
    AppState,
    error::{ApiError, Result},
    infrastructure::{
        crypto,
        repositories::{self, UnitOfWork},
    },
    models::User,
};
pub(crate) fn verified(user: &User) -> Result<()> {
    Ok(appshell_domain::identity::verified(
        user.email_verified_at.is_some(),
    )?)
}
pub(crate) fn rate_limit(c: &mut UnitOfWork<'_>, key: &str, limit: i32) -> Result<()> {
    let count = c.identity_increment_rate_limit(crypto::digest(key))?.count;
    Ok(appshell_domain::identity::rate_limit(count, limit)?)
}
pub(crate) async fn authenticate(state: &AppState, token: String) -> Result<User> {
    let hash = crypto::digest(&token);
    repositories::run(state.pool.clone(), move |c| {
        c.identity_session_user(hash)?
            .ok_or_else(ApiError::unauthorized)
    })
    .await
}
pub(crate) async fn throttle_requests(state: &AppState) -> Result<()> {
    repositories::run(state.pool.clone(), |c| {
        rate_limit(c, "global:mutations", 2000)
    })
    .await
}

/// Lock against administrative changes before applying an authenticated mutation.
pub(crate) fn authorize(c: &mut UnitOfWork<'_>, user: &User) -> Result<()> {
    if c.identity_lock_user(user.id)? != 1 {
        return Err(ApiError::unauthorized());
    }
    let current = c.identity_user(user.id)?;
    if current.email != user.email {
        return Err(ApiError::unauthorized());
    }
    c.actor("user", user.id)
}
