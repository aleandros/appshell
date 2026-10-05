use super::*;
use crate::{
    infrastructure::rows::{self, Action, Count, Credentials, TextValue},
    models,
};
impl UnitOfWork<'_> {
    pub(crate) fn identity_session_user(
        &mut self,
        hash: impl AsRef<str>,
    ) -> Result<Option<models::User>> {
        let s = self.get(&key("session", hash.as_ref()))?;
        if !active(&s) || !expires(&s, "expires_at") {
            return Ok(None);
        }
        let u = self.get(&key("user", decode::<Uuid>(s["user_id"].clone())?))?;
        if !enabled(&u) || number(&s, "epoch") != number(&u, "session_epoch") {
            return Ok(None);
        }
        Ok(Some(decode::<rows::User>(u)?.into()))
    }
    pub(crate) fn identity_increment_rate_limit(&mut self, hash: impl AsRef<str>) -> Result<Count> {
        let k = key("rate", hash.as_ref());
        let mut v = self.get(&k)?;
        if v.is_null() {
            v = fresh(json!({}));
        }
        if !active(&v) || !expires(&v, "expires_at") {
            v["hits"] = json!(0);
            v["expires_at"] = json!(now() + chrono::Duration::minutes(15));
            v["deleted_at"] = Value::Null;
        }
        let count = number(&v, "hits") + 1;
        v["hits"] = json!(count);
        self.save(k, v)?;
        Ok(Count { count })
    }
    pub(crate) fn identity_issue_action(
        &mut self,
        hash: impl AsRef<str>,
        user_id: Uuid,
        purpose: impl AsRef<str>,
        payload: Option<&str>,
    ) -> Result<usize> {
        let k = key("user", user_id);
        let mut u = self.require(&k)?;
        let field = format!("action_{}", purpose.as_ref());
        let generation = number(&u, &field) + 1;
        u[&field] = json!(generation);
        self.save(k, u.clone())?;
        self.save(key("action",hash.as_ref()),fresh(json!({"user_id":user_id,"purpose":purpose.as_ref(),"payload":payload,"epoch":number(&u,"action_epoch"),"generation":generation,"expires_at":now()+chrono::Duration::hours(1)})))?;
        Ok(1)
    }
    pub(crate) fn identity_create_session(
        &mut self,
        hash: impl AsRef<str>,
        user_id: Uuid,
    ) -> Result<usize> {
        let u = self.require(&key("user", user_id))?;
        self.save(key("session",hash.as_ref()),fresh(json!({"user_id":user_id,"epoch":number(&u,"session_epoch"),"expires_at":now()+chrono::Duration::days(14)})))?;
        Ok(1)
    }
    pub(crate) fn identity_create_user(
        &mut self,
        email: impl AsRef<str>,
        name: impl AsRef<str>,
        hash: impl AsRef<str>,
    ) -> Result<models::User> {
        let u = fresh(
            json!({"email":email.as_ref(),"name":name.as_ref(),"password_hash":hash.as_ref(),"status":"active","email_verified_at":null,"_listing":"users"}),
        );
        let uid = id(&u)?;
        self.reserve(key("user_email", email.as_ref()), uid)?;
        self.save(key("user", uid), u.clone())?;
        Ok(decode::<rows::User>(u)?.into())
    }
    pub(crate) fn identity_password_hash(&mut self, user_id: Uuid) -> Result<Option<TextValue>> {
        let u = self.get(&key("user", user_id))?;
        Ok(if enabled(&u) {
            u["password_hash"]
                .as_str()
                .map(|s| TextValue { value: s.into() })
        } else {
            None
        })
    }
    fn identity_by_email(&mut self, email: &str) -> Result<Value> {
        let p = self.get(&key("user_email", email))?;
        if p.is_null() {
            return Ok(Value::Null);
        }
        let u = self.get(&key("user", decode::<Uuid>(p["owner"].clone())?))?;
        Ok(if u["email"] == email { u } else { Value::Null })
    }
    pub(crate) fn identity_credentials(
        &mut self,
        email: impl AsRef<str>,
    ) -> Result<Option<Credentials>> {
        let u = self.identity_by_email(email.as_ref())?;
        if enabled(&u) {
            Ok(Some(decode(u)?))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn identity_user(&mut self, user_id: Uuid) -> Result<models::User> {
        let u = self.get(&key("user", user_id))?;
        if !enabled(&u) {
            return Err(missing());
        }
        Ok(decode::<rows::User>(u)?.into())
    }
    pub(crate) fn identity_delete_session(&mut self, hash: impl AsRef<str>) -> Result<usize> {
        self.remove(key("session", hash.as_ref()))
    }
    pub(crate) fn identity_password_user(
        &mut self,
        email: impl AsRef<str>,
    ) -> Result<Option<models::User>> {
        let u = self.identity_by_email(email.as_ref())?;
        if enabled(&u) && u["password_hash"].is_string() {
            Ok(Some(decode::<rows::User>(u)?.into()))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn identity_consume_action(
        &mut self,
        hash: impl AsRef<str>,
        purpose: impl AsRef<str>,
    ) -> Result<Option<Action>> {
        let k = key("action", hash.as_ref());
        let a = self.get(&k)?;
        if !active(&a) || !expires(&a, "expires_at") || a["purpose"] != purpose.as_ref() {
            return Ok(None);
        }
        let user_id = decode::<Uuid>(a["user_id"].clone())?;
        let u = self.get(&key("user", user_id))?;
        if !enabled(&u)
            || number(&a, "epoch") != number(&u, "action_epoch")
            || number(&a, "generation") != number(&u, &format!("action_{}", purpose.as_ref()))
        {
            return Ok(None);
        }
        self.actor("user", user_id)?;
        self.remove(k)?;
        Ok(Some(decode(a)?))
    }
    pub(crate) fn identity_update_password(
        &mut self,
        hash: impl AsRef<str>,
        user_id: Uuid,
    ) -> Result<usize> {
        let k = key("user", user_id);
        let mut u = self.require(&k)?;
        u["password_hash"] = json!(hash.as_ref());
        self.save(k, u)?;
        Ok(1)
    }
    pub(crate) fn identity_verify_email(&mut self, user_id: Uuid) -> Result<usize> {
        let k = key("user", user_id);
        let mut u = self.require(&k)?;
        u["email_verified_at"] = json!(now());
        self.save(k, u)?;
        Ok(1)
    }
    pub(crate) fn identity_revoke_sessions(&mut self, user_id: Uuid) -> Result<usize> {
        self.identity_epoch(user_id, "session_epoch")
    }
    pub(crate) fn identity_revoke_actions(&mut self, user_id: Uuid) -> Result<usize> {
        self.identity_epoch(user_id, "action_epoch")
    }
    fn identity_epoch(&mut self, user_id: Uuid, field: &str) -> Result<usize> {
        let k = key("user", user_id);
        let mut u = self.get(&k)?;
        if u.is_null() {
            return Ok(0);
        }
        u[field] = json!(number(&u, field) + 1);
        self.save(k, u)?;
        Ok(1)
    }
    pub(crate) fn identity_lock_user(&mut self, user_id: Uuid) -> Result<usize> {
        let u = self.get(&key("user", user_id))?;
        Ok(usize::from(enabled(&u)))
    }
    pub(crate) fn identity_update_email(
        &mut self,
        email: impl AsRef<str>,
        user_id: Uuid,
    ) -> Result<usize> {
        let k = key("user", user_id);
        let mut u = self.require(&k)?;
        self.change_email_reservation("user_email", &u["email"], email.as_ref(), user_id)?;
        u["email"] = json!(email.as_ref());
        u["email_verified_at"] = json!(now());
        self.save(k, u)?;
        Ok(1)
    }
    pub(crate) fn identity_managed_users(
        &mut self,
        search: &str,
        offset: i64,
    ) -> Result<Vec<models::ManagedUser>> {
        self.search_accounts("users", search, offset)?
            .into_iter()
            .map(|v| decode::<rows::ManagedUser>(v).map(Into::into))
            .collect()
    }
    pub(super) fn search_accounts(
        &mut self,
        listing: &str,
        search: &str,
        offset: i64,
    ) -> Result<Vec<Value>> {
        let mut rows: Vec<Value> = self
            .query(listing, "", true)?
            .into_iter()
            .map(|(_, v)| v)
            .filter(|v| {
                format!(
                    "{} {}",
                    v["email"].as_str().unwrap_or_default(),
                    v["name"].as_str().unwrap_or_default()
                )
                .to_lowercase()
                .contains(&search.to_lowercase())
            })
            .collect();
        rows.sort_by_key(|v| {
            (
                v["created_at"].as_str().unwrap_or_default().to_owned(),
                v["id"].as_str().unwrap_or_default().to_owned(),
            )
        });
        Ok(rows
            .into_iter()
            .skip(usize::try_from(offset).unwrap_or(0))
            .take(50)
            .collect())
    }
    pub(crate) fn identity_managed_user(&mut self, uid: Uuid) -> Result<models::ManagedUser> {
        let u = self.get(&key("user", uid))?;
        if u.is_null() {
            return Err(missing());
        }
        Ok(decode::<rows::ManagedUser>(u)?.into())
    }
    pub(crate) fn identity_manage_user(
        &mut self,
        uid: Uuid,
        email: &str,
        name: &str,
        status: &str,
    ) -> Result<()> {
        let k = key("user", uid);
        let mut u = self.get(&k)?;
        if u.is_null() {
            return Err(missing());
        }
        self.change_email_reservation("user_email", &u["email"], email, uid)?;
        if u["email"] != email {
            u["email_verified_at"] = Value::Null;
        }
        u["email"] = json!(email);
        u["name"] = json!(name);
        u["status"] = json!(if status == "deleted" {
            "suspended"
        } else {
            status
        });
        u["deleted_at"] = if status == "deleted" {
            if u["deleted_at"].is_null() {
                json!(now())
            } else {
                u["deleted_at"].clone()
            }
        } else {
            Value::Null
        };
        self.save(k, u)
    }
    pub(crate) fn identity_user_history(
        &mut self,
        uid: Uuid,
        offset: i64,
    ) -> Result<Vec<models::HistoryEntry>> {
        let mut rows = self.query(&format!("history#{uid}"), "", false)?;
        rows.sort_by(|a, b| b.0.cmp(&a.0));
        rows.into_iter()
            .skip(usize::try_from(offset).unwrap_or(0))
            .take(50)
            .map(|(_, v)| decode::<rows::HistoryEntry>(v).map(Into::into))
            .collect()
    }
}
