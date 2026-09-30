//! Transport-independent retry policy. Handlers must tolerate at-least-once delivery.
pub const MAX_ATTEMPTS: i32 = 10;
pub const HANDLER_TIMEOUT_SECONDS: u64 = 60;
pub const LEASE_SECONDS: i32 = 120;

pub fn retry_delay_seconds(attempt: i32) -> i32 {
    5 * 2_i32.pow(attempt.clamp(1, 7) as u32 - 1).min(60)
}

#[cfg(test)]
mod tests {
    #[test]
    fn retries_are_bounded() {
        assert_eq!(super::retry_delay_seconds(1), 5);
        assert_eq!(super::retry_delay_seconds(2), 10);
        assert_eq!(super::retry_delay_seconds(10), 300);
        assert_eq!(super::retry_delay_seconds(i32::MAX), 300);
    }
}
