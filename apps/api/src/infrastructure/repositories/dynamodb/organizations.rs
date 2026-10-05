use super::*;
use crate::{
    infrastructure::rows::{self, Count, InviteRecord, TextValue},
    models,
};
fn member(org: Uuid, user: Uuid) -> Key {
    (format!("org#{org}"), format!("member#{user}"))
}
impl UnitOfWork<'_> {
    pub(crate) fn organizations_role(
        &mut self,
        user: Uuid,
        org: Uuid,
    ) -> Result<Option<TextValue>> {
        let m = self.get(&member(org, user))?;
        let o = self.get(&key("organization", org))?;
        let u = self.get(&key("user", user))?;
        Ok(if active(&m) && active(&o) && active(&u) {
            m["role"].as_str().map(|s| TextValue { value: s.into() })
        } else {
            None
        })
    }
    pub(crate) fn organizations_member_count(&mut self, org: Uuid) -> Result<Count> {
        let mut count = 0;
        for (_, m) in self.query(&format!("org#{org}"), "member#", false)? {
            if active(&m) && active(&self.get(&key("user", decode::<Uuid>(m["user_id"].clone())?))?)
            {
                count += 1;
            }
        }
        Ok(Count { count })
    }
    pub(crate) fn organizations_for_user(
        &mut self,
        user: Uuid,
    ) -> Result<Vec<models::Organization>> {
        let mut values = Vec::new();
        for (_, m) in self.query(&format!("user#{user}"), "org#", false)? {
            if active(&m) {
                let o = self.projection(&key(
                    "organization",
                    decode::<Uuid>(m["organization_id"].clone())?,
                ))?;
                if active(&o) {
                    values.push((
                        o["created_at"].as_str().unwrap_or_default().to_owned(),
                        models::Organization {
                            id: id(&o)?,
                            name: decode(o["name"].clone())?,
                            role: decode(m["role"].clone())?,
                        },
                    ));
                }
            }
        }
        values.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(values.into_iter().map(|(_, o)| o).collect())
    }
    pub(crate) fn organizations_insert(&mut self, name: impl AsRef<str>) -> Result<Uuid> {
        let o = fresh(json!({"name":name.as_ref()}));
        let uid = id(&o)?;
        self.save(key("organization", uid), o)?;
        Ok(uid)
    }
    pub(crate) fn organizations_add_owner(&mut self, org: Uuid, user: Uuid) -> Result<usize> {
        self.organizations_add_member(org, user, "owner")
    }
    pub(crate) fn organizations_lock(&mut self, org: Uuid) -> Result<usize> {
        let k = key("organization", org);
        if !active(&self.get(&k)?) {
            return Ok(0);
        }
        self.touch(k)?;
        Ok(1)
    }
    pub(crate) fn organizations_reserved_seats(
        &mut self,
        org: Uuid,
        include_pending: bool,
    ) -> Result<Count> {
        let mut count = self.organizations_member_count(org)?.count;
        if include_pending {
            count += i64::try_from(self.organizations_invitations(org)?.len())
                .map_err(ApiError::internal)?;
        }
        Ok(Count { count })
    }
    pub(crate) fn organizations_members(&mut self, org: Uuid) -> Result<Vec<models::Member>> {
        let mut members = Vec::new();
        for (_, m) in self.query(&format!("org#{org}"), "member#", false)? {
            if active(&m) {
                let u = self.get(&key("user", decode::<Uuid>(m["user_id"].clone())?))?;
                if active(&u) {
                    members.push(models::Member {
                        id: id(&u)?,
                        name: decode(u["name"].clone())?,
                        email: decode(u["email"].clone())?,
                        role: decode(m["role"].clone())?,
                    });
                }
            }
        }
        members.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(members)
    }
    pub(crate) fn organizations_invitations(
        &mut self,
        org: Uuid,
    ) -> Result<Vec<models::Invitation>> {
        let mut values: Vec<models::Invitation> = self
            .query(&format!("org#{org}"), "invite#", false)?
            .into_iter()
            .filter(|(_, v)| active(v) && expires(v, "expires_at"))
            .map(|(_, v)| decode::<rows::Invitation>(v).map(Into::into))
            .collect::<Result<_>>()?;
        values.sort_by(|a, b| a.email.cmp(&b.email));
        Ok(values)
    }
    pub(crate) fn organizations_members_with_email(
        &mut self,
        org: Uuid,
        email: impl AsRef<str>,
    ) -> Result<Count> {
        Ok(Count {
            count: i64::try_from(
                self.organizations_members(org)?
                    .iter()
                    .filter(|m| m.email == email.as_ref())
                    .count(),
            )
            .map_err(ApiError::internal)?,
        })
    }
    pub(crate) fn organizations_delete_invitation_for_email(
        &mut self,
        org: Uuid,
        email: impl AsRef<str>,
    ) -> Result<usize> {
        let mut count = 0;
        for (k, v) in self.query(&format!("org#{org}"), "invite#", false)? {
            if active(&v) && v["email"] == email.as_ref() {
                count += self.remove(k)?;
            }
        }
        Ok(count)
    }
    pub(crate) fn organizations_insert_invitation(
        &mut self,
        org: Uuid,
        email: impl AsRef<str>,
        role: impl AsRef<str>,
        hash: impl AsRef<str>,
    ) -> Result<usize> {
        let v = fresh(
            json!({"organization_id":org,"email":email.as_ref(),"role":role.as_ref(),"expires_at":now()+chrono::Duration::days(7)}),
        );
        let uid = id(&v)?;
        self.save((format!("org#{org}"), format!("invite#{uid}")), v)?;
        self.save(
            key("invite_token", hash.as_ref()),
            fresh(json!({"organization_id":org,"invitation_id":uid})),
        )?;
        Ok(1)
    }
    fn invitation_key(&mut self, hash: &str) -> Result<Option<Key>> {
        let p = self.get(&key("invite_token", hash))?;
        if p.is_null() {
            return Ok(None);
        }
        Ok(Some((
            format!("org#{}", decode::<Uuid>(p["organization_id"].clone())?),
            format!("invite#{}", decode::<Uuid>(p["invitation_id"].clone())?),
        )))
    }
    pub(crate) fn organizations_invitation_by_token(
        &mut self,
        hash: impl AsRef<str>,
    ) -> Result<Option<InviteRecord>> {
        let Some(k) = self.invitation_key(hash.as_ref())? else {
            return Ok(None);
        };
        let v = self.get(&k)?;
        if active(&v) && expires(&v, "expires_at") {
            Ok(Some(decode(v)?))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn organizations_consume_invitation(
        &mut self,
        hash: impl AsRef<str>,
    ) -> Result<usize> {
        let Some(k) = self.invitation_key(hash.as_ref())? else {
            return Ok(0);
        };
        let v = self.get(&k)?;
        if !expires(&v, "expires_at") {
            return Ok(0);
        }
        self.remove(k)
    }
    pub(crate) fn organizations_add_member(
        &mut self,
        org: Uuid,
        user: Uuid,
        role: impl AsRef<str>,
    ) -> Result<usize> {
        let k = member(org, user);
        let old = self.get(&k)?;
        if active(&old) {
            return Err(ApiError(409, "conflict", "Membership already exists."));
        }
        // Membership slots are stable and each removal/restoration is recorded in history.
        let mut m = if old.is_null() { fresh(json!({})) } else { old };
        m["organization_id"] = json!(org);
        m["user_id"] = json!(user);
        m["role"] = json!(role.as_ref());
        m["deleted_at"] = Value::Null;
        self.save(k, m.clone())?;
        self.save((format!("user#{user}"), format!("org#{org}")), m)?;
        Ok(1)
    }
    pub(crate) fn organizations_revoke_invitation(
        &mut self,
        org: Uuid,
        uid: Uuid,
    ) -> Result<usize> {
        self.remove((format!("org#{org}"), format!("invite#{uid}")))
    }
    pub(crate) fn organizations_remove_member(&mut self, org: Uuid, user: Uuid) -> Result<usize> {
        let count = self.remove(member(org, user))?;
        self.remove((format!("user#{user}"), format!("org#{org}")))?;
        Ok(count)
    }
}
