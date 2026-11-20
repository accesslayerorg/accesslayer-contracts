//! Pure building blocks for #1217 (limit orders), #1218 (decaying fee
//! schedule), #1219 (fractional wrapper) and #1221 (Dutch auction).
//!
//! These helpers hold no storage and expose no entrypoints. The entrypoints
//! for each issue will build on them in follow-up PRs.

/// Linear interpolation from `start` to `end` over `duration`, clamped at `end`.
fn lerp(start: i128, end: i128, duration: u64, elapsed: u64) -> i128 {
    if duration == 0 || elapsed >= duration {
        return end;
    }
    start + (end - start) * elapsed as i128 / duration as i128
}

/// #1221: Dutch auction price, `start_price` at start and `floor_price` at or after `duration`.
pub fn auction_price(start_price: i128, floor_price: i128, duration: u64, elapsed: u64) -> i128 {
    lerp(start_price, floor_price, duration, elapsed)
}

/// #1218: creator fee in bps decaying linearly from `start_bps` to `floor_bps`.
pub fn current_fee_bps(start_bps: u32, floor_bps: u32, decay_ledgers: u64, elapsed: u64) -> u32 {
    lerp(start_bps as i128, floor_bps as i128, decay_ledgers, elapsed) as u32
}

/// #1218: a schedule is valid when `floor <= start <= protocol_max_bps`.
pub fn is_valid_fee_schedule(start_bps: u32, floor_bps: u32, protocol_max_bps: u32) -> bool {
    floor_bps <= start_bps && start_bps <= protocol_max_bps
}

/// #1217: buy orders fill at or below target, sell orders at or above it.
pub fn order_fillable(is_buy: bool, curve_price: i128, target_price: i128) -> bool {
    if is_buy {
        curve_price <= target_price
    } else {
        curve_price >= target_price
    }
}

/// #1217: keeper fee, rounded down so it never exceeds `keeper_fee_bps` of proceeds.
pub fn keeper_fee(proceeds: i128, keeper_fee_bps: u32) -> Option<i128> {
    proceeds
        .checked_mul(keeper_fee_bps as i128)
        .map(|v| v / 10_000)
}

/// #1219: shares minted when wrapping `quantity` keys.
pub fn shares_for_keys(quantity: u32, shares_per_key: u32) -> Option<i128> {
    (quantity as i128).checked_mul(shares_per_key as i128)
}

/// #1219: unwrap requires a non-zero whole-key multiple of shares.
pub fn is_whole_key_multiple(shares: i128, shares_per_key: u32) -> bool {
    shares_per_key > 0 && shares > 0 && shares % shares_per_key as i128 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auction_price_decays_to_floor() {
        assert_eq!(auction_price(1_000, 200, 100, 0), 1_000);
        assert_eq!(auction_price(1_000, 200, 100, 50), 600);
        assert_eq!(auction_price(1_000, 200, 100, 100), 200);
        assert_eq!(auction_price(1_000, 200, 100, 500), 200);
    }

    #[test]
    fn fee_schedule_decays_and_validates() {
        assert_eq!(current_fee_bps(500, 100, 200, 0), 500);
        assert_eq!(current_fee_bps(500, 100, 200, 100), 300);
        assert_eq!(current_fee_bps(500, 100, 200, 200), 100);
        assert!(is_valid_fee_schedule(500, 100, 1_000));
        assert!(!is_valid_fee_schedule(100, 500, 1_000));
        assert!(!is_valid_fee_schedule(1_001, 100, 1_000));
    }

    #[test]
    fn limit_order_conditions_and_keeper_fee() {
        assert!(order_fillable(true, 90, 100) && !order_fillable(true, 101, 100));
        assert!(order_fillable(false, 110, 100) && !order_fillable(false, 99, 100));
        assert_eq!(keeper_fee(1_999, 50), Some(9));
        assert_eq!(keeper_fee(i128::MAX, 50), None);
    }

    #[test]
    fn wrapper_share_math() {
        assert_eq!(shares_for_keys(3, 1_000), Some(3_000));
        assert!(is_whole_key_multiple(2_000, 1_000));
        assert!(!is_whole_key_multiple(1_500, 1_000));
        assert!(!is_whole_key_multiple(0, 1_000));
    }
}
