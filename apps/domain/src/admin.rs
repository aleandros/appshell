use crate::RuleError;

pub fn status(value: &str) -> Result<(), RuleError> {
    if matches!(value, "active" | "suspended" | "deleted") {
        Ok(())
    } else {
        Err(RuleError::Invalid("Choose active, suspended, or deleted."))
    }
}
pub fn allow_admin_change(is_self: bool, status: &str) -> Result<(), RuleError> {
    self::status(status)?;
    if is_self && status != "active" {
        return Err(RuleError::Invalid(
            "You cannot suspend or delete your own administrator account.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_and_self_lockout_policy() {
        assert!(allow_admin_change(true, "active").is_ok());
        for value in ["suspended", "deleted"] {
            assert!(allow_admin_change(true, value).is_err());
            assert!(allow_admin_change(false, value).is_ok());
        }
        assert!(status("owner").is_err());
    }
}
