use crate::{Result, RuleError};
use validator::ValidateEmail;
/// # Errors
/// Rejects malformed addresses and addresses longer than 254 bytes.
pub fn email(value: &str) -> Result<String> {
    let value = value.trim().to_lowercase();
    if value.len() > 254 || !value.validate_email() {
        return Err(RuleError::Invalid("Enter a valid email address."));
    }
    Ok(value)
}
/// # Errors
/// Rejects names that are empty after trimming or longer than 80 characters.
pub fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 {
        return Err(RuleError::Invalid(
            "Use between 1 and 80 characters for names.",
        ));
    }
    Ok(value.into())
}
/// # Errors
/// Rejects passwords shorter than 12 characters or longer than 128 bytes.
pub fn password(value: &str) -> Result<()> {
    if value.chars().count() < 12 || value.len() > 128 {
        return Err(RuleError::Invalid(
            "Use a password with at least 12 characters and at most 128 bytes.",
        ));
    }
    Ok(())
}

/// # Errors
/// Returns `RuleError::EmailUnverified` when the email has not been verified.
#[expect(
    clippy::fn_params_excessive_bools,
    reason = "Verification is a persisted boolean fact, not a behavior switch."
)]
pub fn verified(is_verified: bool) -> Result<()> {
    if !is_verified {
        return Err(RuleError::EmailUnverified);
    }
    Ok(())
}
/// # Errors
/// Returns `RuleError::RateLimited` when hits exceed the limit.
pub fn rate_limit(hits: i64, limit: i32) -> Result<()> {
    if hits > i64::from(limit) {
        return Err(RuleError::RateLimited);
    }
    Ok(())
}
/// # Errors
/// Rejects tokens whose length differs from the 64-byte token format.
pub fn action_token(token: &str) -> Result<()> {
    if token.len() != 64 {
        return Err(RuleError::Invalid("This link is invalid or has expired."));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_and_limits() {
        assert_eq!(
            email("  PERSON@Example.com ").unwrap(),
            "person@example.com"
        );
        assert!(email("oops").is_err());
        assert!(name("   ").is_err());
        assert!(password("short").is_err());
        assert!(password(&"é".repeat(64)).is_ok());
        assert!(password(&"é".repeat(65)).is_err());
        assert!(verified(false).is_err());
        assert!(rate_limit(5, 5).is_ok());
        assert!(rate_limit(6, 5).is_err());
    }
}
