use super::*;
use crate::{
    infrastructure::rows::{self, Credentials},
    models,
};
impl UnitOfWork<'_> {
    pub(crate) fn admin_management_lock(&mut self) -> Result<()> {
        self.touch(key("guard", "admins"))
    }
    pub(crate) fn admin_count(&mut self) -> Result<i64> {
        let g = self.get(&key("guard", "admins"))?;
        Ok(number(&g, "count"))
    }
    pub(crate) fn admin_require_actor(&mut self, uid: Uuid) -> Result<()> {
        let u = self.get(&key("admin", uid))?;
        if !enabled(&u) {
            return Err(ApiError::unauthorized());
        }
        self.actor("admin", uid)
    }
    pub(crate) fn admin_create(
        &mut self,
        email: &str,
        name: &str,
        hash: &str,
    ) -> Result<models::AdminAccount> {
        let u = fresh(
            json!({"email":email,"name":name,"password_hash":hash,"status":"active","_listing":"admins"}),
        );
        let uid = id(&u)?;
        self.reserve(key("admin_email", email), uid)?;
        self.save(key("admin", uid), u.clone())?;
        let k = key("guard", "admins");
        let mut g = self.require(&k)?;
        g["count"] = json!(number(&g, "count") + 1);
        self.save(k, g)?;
        Ok(decode::<rows::AdminAccount>(u)?.into())
    }
    pub(crate) fn admin_credentials(&mut self, email: &str) -> Result<Option<Credentials>> {
        let p = self.get(&key("admin_email", email))?;
        if p.is_null() {
            return Ok(None);
        }
        let u = self.get(&key("admin", decode::<Uuid>(p["owner"].clone())?))?;
        if enabled(&u) && u["email"] == email {
            Ok(Some(decode(u)?))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn admin_session(&mut self, hash: &str) -> Result<Option<models::AdminAccount>> {
        let s = self.get(&key("admin_session", hash))?;
        if !active(&s) || !expires(&s, "expires_at") {
            return Ok(None);
        }
        let u = self.get(&key("admin", decode::<Uuid>(s["admin_id"].clone())?))?;
        if enabled(&u) && number(&s, "epoch") == number(&u, "session_epoch") {
            Ok(Some(decode::<rows::AdminAccount>(u)?.into()))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn admin_create_session(&mut self, uid: Uuid, hash: &str) -> Result<()> {
        let u = self.require(&key("admin", uid))?;
        self.save(key("admin_session",hash),fresh(json!({"admin_id":uid,"epoch":number(&u,"session_epoch"),"expires_at":now()+chrono::Duration::hours(8)})))
    }
    pub(crate) fn admin_logout(&mut self, hash: &str) -> Result<()> {
        self.remove(key("admin_session", hash))?;
        Ok(())
    }
    pub(crate) fn admin_list(
        &mut self,
        search: &str,
        offset: i64,
    ) -> Result<Vec<models::AdminAccount>> {
        self.search_accounts("admins", search, offset)?
            .into_iter()
            .map(|v| decode::<rows::AdminAccount>(v).map(Into::into))
            .collect()
    }
    pub(crate) fn admin_update(
        &mut self,
        uid: Uuid,
        email: &str,
        name: &str,
        status: &str,
        hash: Option<&str>,
    ) -> Result<()> {
        let k = key("admin", uid);
        let mut u = self.get(&k)?;
        if u.is_null() {
            return Err(missing());
        }
        self.change_email_reservation("admin_email", &u["email"], email, uid)?;
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
        if let Some(hash) = hash {
            u["password_hash"] = json!(hash);
        }
        u["session_epoch"] = json!(number(&u, "session_epoch") + 1);
        self.save(k, u)
    }
}
