use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema, Clone)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub email_verified_at: Option<DateTime<Utc>>,
}
#[derive(Serialize, ToSchema)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub role: String,
}
#[derive(Serialize, ToSchema)]
pub struct Session {
    pub user: User,
    pub organizations: Vec<Organization>,
}
#[derive(Deserialize, ToSchema)]
pub struct Signup {
    pub name: String,
    pub email: String,
    pub password: String,
    pub organization: String,
}
#[derive(Deserialize, ToSchema)]
pub struct Login {
    pub email: String,
    pub password: String,
}
#[derive(Deserialize, ToSchema)]
pub struct EmailInput {
    pub email: String,
}
#[derive(Deserialize, ToSchema)]
pub struct TokenInput {
    pub token: String,
}
#[derive(Deserialize, ToSchema)]
pub struct ResetPassword {
    pub token: String,
    pub password: String,
}
#[derive(Deserialize, ToSchema)]
pub struct ChangePassword {
    pub current_password: String,
    pub password: String,
}
#[derive(Deserialize, ToSchema)]
pub struct ChangeEmail {
    pub email: String,
    pub password: String,
}
#[derive(Deserialize, ToSchema)]
pub struct NameInput {
    pub name: String,
}
#[derive(Deserialize, ToSchema)]
pub struct InviteInput {
    pub email: String,
    pub role: String,
}
#[derive(Serialize, ToSchema)]
pub struct Message {
    pub message: String,
}
#[derive(Serialize, ToSchema)]
pub struct Member {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub role: String,
}
#[derive(Serialize, ToSchema)]
pub struct Invitation {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub expires_at: DateTime<Utc>,
}
#[derive(Serialize, ToSchema)]
pub struct Team {
    pub members: Vec<Member>,
    pub invitations: Vec<Invitation>,
}
#[derive(Serialize, ToSchema)]
pub struct Subscription {
    pub plan: String,
    pub status: String,
    pub current_period_end: Option<DateTime<Utc>>,
}
#[derive(Serialize, ToSchema)]
pub struct Billing {
    pub subscription: Subscription,
    pub seat_limit: i64,
    pub seats_used: i64,
    pub billing_enabled: bool,
}
#[derive(Serialize, ToSchema)]
pub struct RedirectUrl {
    pub url: String,
}
pub(crate) fn message(text: &str) -> Message {
    Message {
        message: text.into(),
    }
}

#[derive(Serialize, ToSchema, Clone)]
pub struct AdminAccount {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub status: String,
    pub deleted_at: Option<DateTime<Utc>>,
}
#[derive(Deserialize, ToSchema)]
pub struct CreateAdmin {
    pub email: String,
    pub name: String,
    pub password: String,
}
#[derive(Deserialize, ToSchema)]
pub struct UpdateAdmin {
    pub email: String,
    pub name: String,
    pub status: String,
    pub password: Option<String>,
}
#[derive(Serialize, ToSchema)]
pub struct ManagedUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub status: String,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}
#[derive(Deserialize, ToSchema)]
pub struct UpdateUser {
    pub email: String,
    pub name: String,
    /// active, suspended, or deleted. Setting active explicitly restores a deleted account.
    pub status: String,
}
#[derive(Serialize, ToSchema)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub operation: String,
    pub changes: serde_json::Value,
    pub actor_id: Option<Uuid>,
    pub actor_kind: String,
    pub changed_at: DateTime<Utc>,
}
#[derive(Deserialize, ToSchema)]
pub struct AdminSearch {
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub offset: i64,
}
