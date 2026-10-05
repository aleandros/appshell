use chrono::{DateTime, Utc};
#[cfg(not(feature = "dynamodb"))]
use diesel::{
    QueryableByName,
    sql_types::{BigInt, Nullable, Text, Timestamptz, Uuid as SqlUuid},
};
use uuid::Uuid;
#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct Mail {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = SqlUuid))]
    pub id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub recipient: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub subject: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub body: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=Nullable<Text>))]
    pub html_body: Option<String>,
}
#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct CheckoutAttempt {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub request_key: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = diesel::sql_types::Jsonb))]
    pub parameters: serde_json::Value,
}
#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct InviteRecord {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=SqlUuid))]
    pub organization_id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=Text))]
    pub email: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=Text))]
    pub role: String,
}
#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct Credentials {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=SqlUuid))]
    pub id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=Nullable<Text>))]
    pub password_hash: Option<String>,
}
#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct Action {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=SqlUuid))]
    pub user_id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=Nullable<Text>))]
    pub payload: Option<String>,
}
#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct User {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = diesel::sql_types::Uuid))]
    pub id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub email: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub name: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Nullable<Timestamptz>))]
    pub email_verified_at: Option<DateTime<Utc>>,
}
impl From<User> for crate::models::User {
    fn from(row: User) -> Self {
        Self {
            id: row.id,
            email: row.email,
            name: row.name,
            email_verified_at: row.email_verified_at,
        }
    }
}

#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct Organization {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = diesel::sql_types::Uuid))]
    pub id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub name: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub role: String,
}
impl From<Organization> for crate::models::Organization {
    fn from(row: Organization) -> Self {
        Self {
            id: row.id,
            name: row.name,
            role: row.role,
        }
    }
}

#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct Member {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = diesel::sql_types::Uuid))]
    pub id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub name: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub email: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub role: String,
}
impl From<Member> for crate::models::Member {
    fn from(row: Member) -> Self {
        Self {
            id: row.id,
            name: row.name,
            email: row.email,
            role: row.role,
        }
    }
}

#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct Invitation {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = diesel::sql_types::Uuid))]
    pub id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub email: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub role: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Timestamptz))]
    pub expires_at: DateTime<Utc>,
}
impl From<Invitation> for crate::models::Invitation {
    fn from(row: Invitation) -> Self {
        Self {
            id: row.id,
            email: row.email,
            role: row.role,
            expires_at: row.expires_at,
        }
    }
}

#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct Subscription {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub plan: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub status: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Nullable<Timestamptz>))]
    pub current_period_end: Option<DateTime<Utc>>,
}
impl From<Subscription> for crate::models::Subscription {
    fn from(row: Subscription) -> Self {
        Self {
            plan: row.plan,
            status: row.status,
            current_period_end: row.current_period_end,
        }
    }
}

#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct Count {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = BigInt))]
    pub count: i64,
}
#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct TextValue {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub value: String,
}

#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
#[cfg(any(not(feature = "dynamodb"), feature = "lambda"))]
pub struct Id {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = SqlUuid))]
    pub id: Uuid,
}

#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct AdminAccount {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = SqlUuid))]
    pub id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub email: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub name: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub status: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Nullable<Timestamptz>))]
    pub deleted_at: Option<DateTime<Utc>>,
}
impl From<AdminAccount> for crate::models::AdminAccount {
    fn from(r: AdminAccount) -> Self {
        Self {
            id: r.id,
            email: r.email,
            name: r.name,
            status: r.status,
            deleted_at: r.deleted_at,
        }
    }
}
#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct ManagedUser {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = SqlUuid))]
    pub id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub email: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub name: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub status: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Nullable<Timestamptz>))]
    pub email_verified_at: Option<DateTime<Utc>>,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Nullable<Timestamptz>))]
    pub deleted_at: Option<DateTime<Utc>>,
}
impl From<ManagedUser> for crate::models::ManagedUser {
    fn from(r: ManagedUser) -> Self {
        Self {
            id: r.id,
            email: r.email,
            name: r.name,
            status: r.status,
            email_verified_at: r.email_verified_at,
            deleted_at: r.deleted_at,
        }
    }
}
#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub struct HistoryEntry {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = SqlUuid))]
    pub id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub operation: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = diesel::sql_types::Jsonb))]
    pub changes: serde_json::Value,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Nullable<SqlUuid>))]
    pub actor_id: Option<Uuid>,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Text))]
    pub actor_kind: String,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type = Timestamptz))]
    pub changed_at: DateTime<Utc>,
}
impl From<HistoryEntry> for crate::models::HistoryEntry {
    fn from(r: HistoryEntry) -> Self {
        Self {
            id: r.id,
            operation: r.operation,
            changes: r.changes,
            actor_id: r.actor_id,
            actor_kind: r.actor_kind,
            changed_at: r.changed_at,
        }
    }
}

#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
pub(crate) struct JobRow {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=SqlUuid))]
    pub id: Uuid,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=diesel::sql_types::Jsonb))]
    pub payload: serde_json::Value,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=diesel::sql_types::Integer))]
    pub attempts: i32,
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=BigInt))]
    pub lease_version: i64,
}
#[derive(serde::Deserialize)]
#[cfg_attr(not(feature = "dynamodb"), derive(QueryableByName))]
#[cfg(not(feature = "dynamodb"))]
pub(crate) struct JobDone {
    #[cfg_attr(not(feature = "dynamodb"), diesel(sql_type=diesel::sql_types::Bool))]
    pub done: bool,
}
