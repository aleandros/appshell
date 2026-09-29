use super::UnitOfWork;
use crate::error::Result;
use crate::infrastructure::rows::*;
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Bool, Text, Uuid as SqlUuid},
};
use uuid::Uuid;
impl UnitOfWork<'_> {
    pub(crate) fn organizations_role(
        &mut self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> Result<Option<super::super::rows::TextValue>> {
        Ok(sql_query(
            "SELECT role AS value FROM memberships WHERE user_id=$1 AND organization_id=$2",
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<SqlUuid, _>(organization_id)
        .get_result::<TextValue>(self.connection)
        .optional()?)
    }
    pub(crate) fn organizations_member_count(
        &mut self,
        organization_id: Uuid,
    ) -> Result<super::super::rows::Count> {
        Ok(
            sql_query("SELECT count(*) AS count FROM memberships WHERE organization_id=$1")
                .bind::<SqlUuid, _>(organization_id)
                .get_result::<Count>(self.connection)?,
        )
    }
    pub(crate) fn organizations_for_user(
        &mut self,
        user_id: Uuid,
    ) -> Result<Vec<crate::models::Organization>> {
        Ok(sql_query("SELECT o.id,o.name,m.role FROM organizations o JOIN memberships m ON m.organization_id=o.id WHERE m.user_id=$1 ORDER BY o.created_at").bind::<SqlUuid, _>(user_id).load::<Organization>(self.connection)?.into_iter().map(Into::into).collect())
    }
    pub(crate) fn organizations_insert(
        &mut self,
        id: Uuid,
        name: impl AsRef<str>,
    ) -> Result<usize> {
        Ok(
            sql_query("INSERT INTO organizations(id,name) VALUES($1,$2)")
                .bind::<SqlUuid, _>(id)
                .bind::<Text, _>(name.as_ref())
                .execute(self.connection)?,
        )
    }
    pub(crate) fn organizations_add_owner(
        &mut self,
        organization_id: Uuid,
        user_id: Uuid,
    ) -> Result<usize> {
        Ok(
            sql_query(
                "INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,'owner')",
            )
            .bind::<SqlUuid, _>(organization_id)
            .bind::<SqlUuid, _>(user_id)
            .execute(self.connection)?,
        )
    }
    pub(crate) fn organizations_lock(&mut self, organization_id: Uuid) -> Result<usize> {
        Ok(
            sql_query("SELECT id FROM organizations WHERE id=$1 FOR UPDATE")
                .bind::<SqlUuid, _>(organization_id)
                .execute(self.connection)?,
        )
    }
    pub(crate) fn organizations_reserved_seats(
        &mut self,
        organization_id: Uuid,
        include_pending: bool,
    ) -> Result<super::super::rows::Count> {
        Ok(sql_query("SELECT (SELECT count(*) FROM memberships WHERE organization_id=$1) + CASE WHEN $2 THEN (SELECT count(*) FROM invitations WHERE organization_id=$1 AND expires_at>now()) ELSE 0 END AS count").bind::<SqlUuid, _>(organization_id).bind::<Bool, _>(include_pending).get_result::<Count>(self.connection)?)
    }
    pub(crate) fn organizations_members(
        &mut self,
        organization_id: Uuid,
    ) -> Result<Vec<crate::models::Member>> {
        Ok(sql_query("SELECT u.id,u.name,u.email,m.role FROM users u JOIN memberships m ON m.user_id=u.id WHERE m.organization_id=$1 ORDER BY u.name").bind::<SqlUuid, _>(organization_id).load::<Member>(self.connection)?.into_iter().map(Into::into).collect())
    }
    pub(crate) fn organizations_invitations(
        &mut self,
        organization_id: Uuid,
    ) -> Result<Vec<crate::models::Invitation>> {
        Ok(sql_query("SELECT id,email,role,expires_at FROM invitations WHERE organization_id=$1 AND expires_at>now() ORDER BY email").bind::<SqlUuid, _>(organization_id).load::<Invitation>(self.connection)?.into_iter().map(Into::into).collect())
    }
    pub(crate) fn organizations_members_with_email(
        &mut self,
        organization_id: Uuid,
        email: impl AsRef<str>,
    ) -> Result<super::super::rows::Count> {
        Ok(sql_query("SELECT count(*) AS count FROM memberships m JOIN users u ON u.id=m.user_id WHERE m.organization_id=$1 AND u.email=$2").bind::<SqlUuid, _>(organization_id).bind::<Text, _>(email.as_ref()).get_result::<Count>(self.connection)?)
    }
    pub(crate) fn organizations_delete_invitation_for_email(
        &mut self,
        organization_id: Uuid,
        email: impl AsRef<str>,
    ) -> Result<usize> {
        Ok(
            sql_query("DELETE FROM invitations WHERE organization_id=$1 AND email=$2")
                .bind::<SqlUuid, _>(organization_id)
                .bind::<Text, _>(email.as_ref())
                .execute(self.connection)?,
        )
    }
    pub(crate) fn organizations_insert_invitation(
        &mut self,
        id: Uuid,
        organization_id: Uuid,
        email: impl AsRef<str>,
        role: impl AsRef<str>,
        token_hash: impl AsRef<str>,
    ) -> Result<usize> {
        Ok(sql_query("INSERT INTO invitations(id,organization_id,email,role,token_hash,expires_at) VALUES($1,$2,$3,$4,$5,now()+interval '7 days')").bind::<SqlUuid, _>(id).bind::<SqlUuid, _>(organization_id).bind::<Text, _>(email.as_ref()).bind::<Text, _>(role.as_ref()).bind::<Text, _>(token_hash.as_ref()).execute(self.connection)?)
    }
    pub(crate) fn organizations_invitation_by_token(
        &mut self,
        token_hash: impl AsRef<str>,
    ) -> Result<Option<super::super::rows::InviteRecord>> {
        Ok(sql_query("SELECT organization_id,email,role FROM invitations WHERE token_hash=$1 AND expires_at>now()").bind::<Text, _>(token_hash.as_ref()).get_result::<InviteRecord>(self.connection).optional()?)
    }
    pub(crate) fn organizations_consume_invitation(
        &mut self,
        token_hash: impl AsRef<str>,
    ) -> Result<usize> {
        Ok(
            sql_query("DELETE FROM invitations WHERE token_hash=$1 AND expires_at>now()")
                .bind::<Text, _>(token_hash.as_ref())
                .execute(self.connection)?,
        )
    }
    pub(crate) fn organizations_add_member(
        &mut self,
        organization_id: Uuid,
        user_id: Uuid,
        role: impl AsRef<str>,
    ) -> Result<usize> {
        Ok(
            sql_query("INSERT INTO memberships(organization_id,user_id,role) VALUES($1,$2,$3)")
                .bind::<SqlUuid, _>(organization_id)
                .bind::<SqlUuid, _>(user_id)
                .bind::<Text, _>(role.as_ref())
                .execute(self.connection)?,
        )
    }
    pub(crate) fn organizations_revoke_invitation(
        &mut self,
        organization_id: Uuid,
        invitation_id: Uuid,
    ) -> Result<usize> {
        Ok(
            sql_query("DELETE FROM invitations WHERE organization_id=$1 AND id=$2")
                .bind::<SqlUuid, _>(organization_id)
                .bind::<SqlUuid, _>(invitation_id)
                .execute(self.connection)?,
        )
    }
    pub(crate) fn organizations_remove_member(
        &mut self,
        organization_id: Uuid,
        user_id: Uuid,
    ) -> Result<usize> {
        Ok(
            sql_query("DELETE FROM memberships WHERE organization_id=$1 AND user_id=$2")
                .bind::<SqlUuid, _>(organization_id)
                .bind::<SqlUuid, _>(user_id)
                .execute(self.connection)?,
        )
    }
}
