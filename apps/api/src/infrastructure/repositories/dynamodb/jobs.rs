use super::*;
#[cfg(feature = "lambda")]
use crate::infrastructure::rows::Id;
use crate::infrastructure::rows::JobRow;
use appshell_domain::jobs::{LEASE_SECONDS, MAX_ATTEMPTS};
fn pending(v: &Value) -> bool {
    active(v) && v["completed_at"].is_null() && v["failed_at"].is_null()
}
impl UnitOfWork<'_> {
    pub(crate) fn jobs_enqueue(&mut self, payload: Value, delay: i32) -> Result<Uuid> {
        if !self.in_transaction {
            return Err(ApiError::internal(
                "Job enqueue requires a unit-of-work transaction",
            ));
        }
        let v = fresh(
            json!({"payload":payload,"attempts":0,"lease_version":0,"available_at":now()+chrono::Duration::seconds(i64::from(delay)),"_listing":"pending_jobs"}),
        );
        let uid = id(&v)?;
        self.save(key("job", uid), v)?;
        Ok(uid)
    }
    pub(crate) fn jobs_claim(&mut self, uid: Option<Uuid>) -> Result<Option<JobRow>> {
        let keys = if let Some(uid) = uid {
            vec![key("job", uid)]
        } else {
            self.query("pending_jobs", "", true)?
                .into_iter()
                .map(|(k, _)| k)
                .collect()
        };
        for k in keys {
            self.clear();
            let mut v = self.get(&k)?;
            if !pending(&v) || expires(&v, "locked_until") {
                continue;
            }
            if number(&v, "attempts") >= i64::from(MAX_ATTEMPTS) {
                v["failed_at"] = json!(now());
                v["error_code"] = json!("attempts_exhausted");
                v["_listing"] = Value::Null;
                self.save(k, v)?;
                continue;
            }
            if expires(&v, "available_at") {
                continue;
            }
            v["attempts"] = json!(number(&v, "attempts") + 1);
            v["lease_version"] = json!(number(&v, "lease_version") + 1);
            v["locked_until"] = json!(now() + chrono::Duration::seconds(i64::from(LEASE_SECONDS)));
            match self.save(k, v.clone()) {
                Ok(()) => return Ok(Some(decode(v)?)),
                Err(e) if e.1 == "write_conflict" => self.clear(),
                Err(e) => return Err(e),
            }
        }
        Ok(None)
    }
    pub(crate) fn jobs_done(&mut self, uid: Uuid) -> Result<bool> {
        let v = self.get(&key("job", uid))?;
        Ok(!v.is_null() && (!active(&v) || !v["completed_at"].is_null()))
    }
    pub(crate) fn jobs_complete(&mut self, uid: Uuid, lease: i64) -> Result<bool> {
        let k = key("job", uid);
        let mut v = self.get(&k)?;
        if !pending(&v) || !expires(&v, "locked_until") || number(&v, "lease_version") != lease {
            return Ok(false);
        }
        v["completed_at"] = json!(now());
        v["locked_until"] = Value::Null;
        v["error_code"] = Value::Null;
        v["payload"] = json!({});
        v["_listing"] = Value::Null;
        self.save(k, v)?;
        Ok(true)
    }
    pub(crate) fn jobs_fail(
        &mut self,
        uid: Uuid,
        lease: i64,
        delay: i32,
        code: &str,
    ) -> Result<()> {
        let k = key("job", uid);
        let mut v = self.get(&k)?;
        if !pending(&v) || !expires(&v, "locked_until") || number(&v, "lease_version") != lease {
            return Ok(());
        }
        v["locked_until"] = Value::Null;
        v["available_at"] = json!(now() + chrono::Duration::seconds(i64::from(delay)));
        v["error_code"] = json!(code);
        if number(&v, "attempts") >= i64::from(MAX_ATTEMPTS) {
            v["failed_at"] = json!(now());
            v["_listing"] = Value::Null;
        }
        self.save(k, v)
    }
    #[cfg(feature = "lambda")]
    pub(crate) fn jobs_dispatch(&mut self) -> Result<Vec<Id>> {
        let mut ids = Vec::new();
        for (k, _) in self.query("pending_jobs", "", true)? {
            if ids.len() == 10 {
                break;
            }
            self.clear();
            let mut v = self.get(&k)?;
            if !pending(&v)
                || expires(&v, "locked_until")
                || expires(&v, "available_at")
                || expires(&v, "dispatched_until")
                || number(&v, "attempts") >= i64::from(MAX_ATTEMPTS)
            {
                continue;
            }
            v["dispatched_until"] = json!(now() + chrono::Duration::minutes(15));
            let uid = id(&v)?;
            match self.save(k, v) {
                Ok(()) => ids.push(Id { id: uid }),
                Err(e) if e.1 == "write_conflict" => self.clear(),
                Err(e) => return Err(e),
            }
        }
        Ok(ids)
    }
}
