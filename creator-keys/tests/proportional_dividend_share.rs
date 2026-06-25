//! Unit tests for the `proportional_dividend_share` helper (#406).
//!
//! These tests exercise the standalone share calculation in isolation so the
//! math can be verified without deploying the full contract.

mod contract_test_env;

use contract_test_env::proportional_dividend_share;

#[test]
fn test_proportional_share_equal_split_two_holders() {
    // Two holders with 1 key each; each receives half.
    assert_eq!(proportional_dividend_share(1_000, 1, 2), 500);
}

#[test]
fn test_proportional_share_unequal_balances() {
    // Whale holds 3 of 4 keys; small holds 1 of 4.
    let net = 10_000i128;
    assert_eq!(proportional_dividend_share(net, 3, 4), 7_500);
    assert_eq!(proportional_dividend_share(net, 1, 4), 2_500);
}

#[test]
fn test_proportional_share_single_holder_full_supply() {
    // Single holder with all keys receives the entire net amount.
    assert_eq!(proportional_dividend_share(9_000, 1, 1), 9_000);
}

#[test]
fn test_proportional_share_zero_total_supply_returns_zero() {
    // Guard against divide-by-zero; returns 0.
    assert_eq!(proportional_dividend_share(10_000, 1, 0), 0);
}

#[test]
fn test_proportional_share_zero_holder_balance_returns_zero() {
    assert_eq!(proportional_dividend_share(10_000, 0, 5), 0);
}

#[test]
fn test_proportional_share_rounding_floors_toward_zero() {
    // net=10, total_supply=3 → per_key=3 (floor), holder with 1 key gets 3 not 3.33.
    assert_eq!(proportional_dividend_share(10, 1, 3), 3);
    // Two holders with 1 key each: 3 + 3 = 6 ≤ 10 (no over-distribution).
    let share_a = proportional_dividend_share(10, 1, 3);
    let share_b = proportional_dividend_share(10, 1, 3);
    assert!(share_a + share_b <= 10);
}

#[test]
fn test_proportional_share_total_does_not_exceed_net() {
    // Sum of all holders' shares must never exceed net_amount.
    let net = 9_999i128;
    let total_supply = 7u32;
    let sum: i128 = (0..total_supply)
        .map(|_| proportional_dividend_share(net, 1, total_supply))
        .sum();
    assert!(sum <= net);
}
