//! Shared time comparison rules for contracts.
//!
//! These helpers operate on values in one consistent unit chosen by the
//! caller (for example, ledger timestamps in seconds or ledger sequences).

/// Returns true when a deadline has been passed.
///
/// Expiry is exclusive: a value is still valid at the exact deadline and
/// becomes expired only on the next unit.
pub fn is_expired(now: u64, deadline: u64) -> bool {
    now > deadline
}

/// Returns true when a scheduled action is available.
///
/// Unlocking is inclusive: an action may run at the exact unlock value.
pub fn is_unlocked(now: u64, unlock_at: u64) -> bool {
    now >= unlock_at
}

/// Returns elapsed units without underflow when clocks are out of order.
pub fn elapsed_since(now: u64, started_at: u64) -> u64 {
    now.saturating_sub(started_at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_is_false_at_deadline_and_true_after() {
        assert!(!is_expired(100, 100));
        assert!(is_expired(101, 100));
    }

    #[test]
    fn unlocking_is_true_at_deadline() {
        assert!(!is_unlocked(99, 100));
        assert!(is_unlocked(100, 100));
    }

    #[test]
    fn elapsed_time_saturates_when_clock_moves_backwards() {
        assert_eq!(elapsed_since(100, 40), 60);
        assert_eq!(elapsed_since(40, 100), 0);
    }
}
