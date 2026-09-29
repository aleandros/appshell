use crate::{Result, RuleError};
pub fn require_admin(role: &str) -> Result<()> {
    if !["owner", "admin"].contains(&role) {
        return Err(RuleError::Forbidden);
    }
    Ok(())
}
pub fn require_owner(role: &str) -> Result<()> {
    if role != "owner" {
        return Err(RuleError::Forbidden);
    }
    Ok(())
}
pub fn invitation_role(role: &str) -> Result<()> {
    if !["admin", "member"].contains(&role) {
        return Err(RuleError::Invalid("Choose admin or member."));
    }
    Ok(())
}
pub fn allow_invitation(actor: &str, target: &str, existing_members: i64) -> Result<()> {
    require_admin(actor)?;
    invitation_role(target)?;
    if target == "admin" {
        require_owner(actor)?;
    }
    if existing_members > 0 {
        return Err(RuleError::Invalid("This person is already a member."));
    }
    Ok(())
}
pub fn seats_available(used: i64, limit: i64) -> Result<()> {
    if used >= limit {
        return Err(RuleError::Invalid(
            "Your workspace has reached its seat limit.",
        ));
    }
    Ok(())
}
pub fn invitation_recipient(invited: &str, signed_in: &str) -> Result<()> {
    if invited != signed_in {
        return Err(RuleError::Invalid(
            "Sign in with the email address that received this invitation.",
        ));
    }
    Ok(())
}
pub fn allow_removal(actor: &str, target: &str) -> Result<()> {
    require_admin(actor)?;
    if target == "owner" {
        return Err(RuleError::Invalid("The workspace owner cannot be removed."));
    }
    if target == "admin" {
        require_owner(actor)?;
    }
    Ok(())
}
pub fn can_view_invitations(role: &str) -> bool {
    ["owner", "admin"].contains(&role)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn role_matrix() {
        for actor in ["owner", "admin", "member", "unknown"] {
            assert!(allow_removal(actor, "owner").is_err());
            assert_eq!(allow_removal(actor, "admin").is_ok(), actor == "owner");
            assert_eq!(
                allow_removal(actor, "member").is_ok(),
                ["owner", "admin"].contains(&actor)
            );
            assert_eq!(
                allow_invitation(actor, "admin", 0).is_ok(),
                actor == "owner"
            );
            assert!(allow_invitation(actor, "owner", 0).is_err());
        }
        assert!(allow_invitation("owner", "member", 1).is_err());
    }
    #[test]
    fn seats_and_email_binding() {
        assert!(seats_available(2, 3).is_ok());
        assert!(seats_available(3, 3).is_err());
        assert!(seats_available(51, 3).is_err());
        assert!(invitation_recipient("a@example.com", "b@example.com").is_err());
    }
}
