use super::*;
use crate::infrastructure::rows::Mail;
impl UnitOfWork<'_> {
    pub(crate) fn mail_enqueue(
        &mut self,
        recipient: impl AsRef<str>,
        subject: impl AsRef<str>,
        body: impl AsRef<str>,
        html: impl AsRef<str>,
    ) -> Result<Uuid> {
        let v = fresh(
            json!({"recipient":recipient.as_ref(),"subject":subject.as_ref(),"body":body.as_ref(),"html_body":html.as_ref(),"attempts":0,"available_at":now(),"_listing":"pending_mail"}),
        );
        let uid = id(&v)?;
        self.save(key("mail", uid), v)?;
        Ok(uid)
    }
    pub(crate) fn mail_set_job(&mut self, uid: Uuid, job: Uuid) -> Result<()> {
        let k = key("mail", uid);
        let mut v = self.require(&k)?;
        v["job_id"] = json!(job);
        v["_listing"] = Value::Null;
        self.save(k, v)
    }
    pub(crate) fn mail_pending(&mut self, uid: Uuid) -> Result<Option<Mail>> {
        let v = self.get(&key("mail", uid))?;
        if active(&v) && v["sent_at"].is_null() {
            Ok(Some(decode(v)?))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn mail_claim(&mut self) -> Result<Vec<Mail>> {
        let mut mails = Vec::new();
        for (k, _) in self.query("pending_mail", "", true)? {
            if mails.len() == 10 {
                break;
            }
            self.clear();
            let mut v = self.get(&k)?;
            if !active(&v)
                || !v["job_id"].is_null()
                || !v["sent_at"].is_null()
                || number(&v, "attempts") >= 10
                || expires(&v, "available_at")
            {
                continue;
            }
            v["attempts"] = json!(number(&v, "attempts") + 1);
            v["available_at"] = json!(now() + chrono::Duration::minutes(5));
            match self.save(k, v.clone()) {
                Ok(()) => mails.push(decode(v)?),
                Err(e) if e.1 == "write_conflict" => self.clear(),
                Err(e) => return Err(e),
            }
        }
        Ok(mails)
    }
    pub(crate) fn mail_mark_delivered(&mut self, uid: Uuid) -> Result<usize> {
        let k = key("mail", uid);
        let mut v = self.require(&k)?;
        v["sent_at"] = json!(now());
        v["body"] = json!("[delivered]");
        v["html_body"] = Value::Null;
        v["_listing"] = Value::Null;
        self.save(k, v)?;
        Ok(1)
    }
}
