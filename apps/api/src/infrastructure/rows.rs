use chrono::{DateTime, Utc};
use diesel::{
    QueryableByName,
    sql_types::{BigInt, Nullable, Text, Timestamptz, Uuid as SqlUuid},
};
use uuid::Uuid;
#[derive(QueryableByName)]
pub struct Mail {
    #[diesel(sql_type = SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type = Text)]
    pub recipient: String,
    #[diesel(sql_type = Text)]
    pub subject: String,
    #[diesel(sql_type = Text)]
    pub body: String,
    #[diesel(sql_type=Nullable<Text>)]
    pub html_body: Option<String>,
}
#[derive(QueryableByName)]
pub struct CheckoutAttempt {
    #[diesel(sql_type = Text)]
    pub request_key: String,
    #[diesel(sql_type = diesel::sql_types::Jsonb)]
    pub parameters: serde_json::Value,
}
#[derive(QueryableByName)]
pub struct InviteRecord {
    #[diesel(sql_type=SqlUuid)]
    pub organization_id: Uuid,
    #[diesel(sql_type=Text)]
    pub email: String,
    #[diesel(sql_type=Text)]
    pub role: String,
}
#[derive(QueryableByName)]
pub struct Credentials {
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type=Nullable<Text>)]
    pub password_hash: Option<String>,
}
#[derive(QueryableByName)]
pub struct Action {
    #[diesel(sql_type=SqlUuid)]
    pub user_id: Uuid,
    #[diesel(sql_type=Nullable<Text>)]
    pub payload: Option<String>,
}
#[derive(QueryableByName)]
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

#[derive(QueryableByName)]
pub struct Organization {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub id: Uuid,
    #[diesel(sql_type = Text)]
    pub name: String,
    #[diesel(sql_type = Text)]
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

#[derive(QueryableByName)]
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

#[derive(QueryableByName)]
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

#[derive(QueryableByName)]
pub struct Subscription {
    #[diesel(sql_type = Text)]
    pub plan: String,
    #[diesel(sql_type = Text)]
    pub status: String,
    #[diesel(sql_type = Nullable<Timestamptz>)]
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
