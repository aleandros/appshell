use crate::{Result, RuleError};
pub fn seat_limit(plan: &str, status: &str) -> i64 {
    if plan == "pro" && ["active", "trialing"].contains(&status) {
        50
    } else {
        3
    }
}
pub fn allow_checkout(plan: &str, status: &str) -> Result<()> {
    if plan == "pro" && !["canceled", "incomplete_expired"].contains(&status) {
        return Err(RuleError::Invalid(
            "Manage your existing subscription in the billing portal.",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entitlements_and_checkout() {
        for status in ["active", "trialing"] {
            assert_eq!(seat_limit("pro", status), 50);
            assert!(allow_checkout("pro", status).is_err());
        }
        for status in [
            "past_due",
            "unpaid",
            "canceled",
            "incomplete",
            "incomplete_expired",
        ] {
            assert_eq!(seat_limit("pro", status), 3);
        }
        assert_eq!(seat_limit("free", "active"), 3);
        assert!(allow_checkout("pro", "canceled").is_ok());
        assert!(allow_checkout("pro", "past_due").is_err());
    }
}
