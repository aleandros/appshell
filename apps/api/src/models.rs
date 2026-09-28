use chrono::{DateTime, Utc};
use diesel::QueryableByName;
use diesel::sql_types::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema, QueryableByName, Clone)]
pub struct User {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub id: Uuid,
    #[diesel(sql_type = Text)]
    pub email: String,
    #[diesel(sql_type = Text)]
    pub name: String,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub email_verified_at: Option<DateTime<Utc>>,
}
#[derive(Serialize, ToSchema, QueryableByName)]
pub struct Organization {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub id: Uuid,
    #[diesel(sql_type = Text)]
    pub name: String,
    #[diesel(sql_type = Text)]
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
#[derive(Serialize, ToSchema, QueryableByName)]
pub struct Member {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub id: Uuid,
    #[diesel(sql_type = Text)]
    pub name: String,
    #[diesel(sql_type = Text)]
    pub email: String,
    #[diesel(sql_type = Text)]
    pub role: String,
}
#[derive(Serialize, ToSchema, QueryableByName)]
pub struct Invitation {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub id: Uuid,
    #[diesel(sql_type = Text)]
    pub email: String,
    #[diesel(sql_type = Text)]
    pub role: String,
    #[diesel(sql_type = Timestamptz)]
    pub expires_at: DateTime<Utc>,
}
#[derive(Serialize, ToSchema)]
pub struct Team {
    pub members: Vec<Member>,
    pub invitations: Vec<Invitation>,
}
#[derive(Serialize, ToSchema, QueryableByName)]
pub struct Subscription {
    #[diesel(sql_type = Text)]
    pub plan: String,
    #[diesel(sql_type = Text)]
    pub status: String,
    #[diesel(sql_type = Nullable<Timestamptz>)]
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
#[derive(QueryableByName)]
pub struct Count {
    #[diesel(sql_type = BigInt)]
    pub count: i64,
}
#[derive(QueryableByName)]
pub struct TextValue {
    #[diesel(sql_type = Text)]
    pub value: String,
}
