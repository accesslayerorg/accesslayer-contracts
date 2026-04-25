//! Invariant tests for fee rounding behavior at boundary values.
//!
//! These tests verify that fee split logic maintains key invariants:
//! - Balance conservation: creator_amount + protocol_amount == total
//! - Rounding direction: remainder from integer division goes to creator
//! - Boundary handling: zero, negative, and minimum amounts
//! - Maximum input values near i128 limits

use creator_keys::fee;

#[test]
fn fee_split_preserves_balance_for_boundary_value_one() {
    let (creator, protocol) = fee::compute_fee_split(1, 9000, 1000);
    assert_eq!(creator + protocol, 1, "balance conservation at amount=1");
}

#[test]
fn fee_split_preserves_balance_for_boundary_value_two() {
    let (creator, protocol) = fee::compute_fee_split(2, 9000, 1000);
    assert_eq!(creator + protocol, 2, "balance conservation at amount=2");
}

#[test]
fn fee_split_preserves_balance_for_boundary_value_ten() {
    let (creator, protocol) = fee::compute_fee_split(10, 5000, 5000);
    assert_eq!(
        creator + protocol,
        10,
        "balance conservation at 50/50 split"
    );
}

#[test]
fn fee_split_remainder_favors_creator_at_boundary_999() {
    let (creator, protocol) = fee::compute_fee_split(999, 9000, 1000);
    // 999 * 1000 / 10000 = 99.9 -> 99 protocol; creator gets remainder
    assert_eq!(protocol, 99);
    assert_eq!(creator, 900);
    assert_eq!(creator + protocol, 999);
}

#[test]
fn fee_split_remainder_favors_creator_at_boundary_1001() {
    let (creator, protocol) = fee::compute_fee_split(1001, 9000, 1000);
    // 1001 * 1000 / 10000 = 100.1 -> 100 protocol; creator gets remainder
    assert_eq!(protocol, 100);
    assert_eq!(creator, 901);
    assert_eq!(creator + protocol, 1001);
}

#[test]
fn fee_split_dust_price_creates_zero_protocol_fee() {
    let (creator, protocol) = fee::compute_fee_split(1, 1000, 9000);
    // 1 * 9000 / 10000 = 0.9 -> 0 protocol; creator gets full amount
    assert_eq!(protocol, 0);
    assert_eq!(creator, 1);
    assert_eq!(creator + protocol, 1);
}

#[test]
fn fee_split_equal_split_50_50_even_amount() {
    let (creator, protocol) = fee::compute_fee_split(100, 5000, 5000);
    assert_eq!(creator, 50);
    assert_eq!(protocol, 50);
    assert_eq!(creator + protocol, 100);
}

#[test]
fn fee_split_equal_split_50_50_odd_amount_remainder_to_creator() {
    let (creator, protocol) = fee::compute_fee_split(101, 5000, 5000);
    // 101 * 5000 / 10000 = 50.5 -> 50 protocol; creator gets 51
    assert_eq!(protocol, 50);
    assert_eq!(creator, 51);
    assert_eq!(creator + protocol, 101);
}

#[test]
fn fee_split_preserves_balance_for_large_amount() {
    let large_amount = 1_000_000_000_000i128;
    let (creator, protocol) = fee::compute_fee_split(large_amount, 9000, 1000);
    assert_eq!(creator + protocol, large_amount);
}

#[test]
fn fee_split_preserves_balance_for_max_safe_amount() {
    // Near i128::MAX but safe for multiplication by protocol_bps
    let safe_amount = 9_223_372_036_854_775i128; // (i128::MAX / 10000) approx
    let (creator, protocol) = fee::compute_fee_split(safe_amount, 9000, 1000);
    assert_eq!(creator + protocol, safe_amount);
}

#[test]
fn fee_split_zero_amount_returns_zero_split() {
    let (creator, protocol) = fee::compute_fee_split(0, 9000, 1000);
    assert_eq!(creator, 0);
    assert_eq!(protocol, 0);
}

#[test]
fn fee_split_negative_amount_returns_zero_split() {
    let (creator, protocol) = fee::compute_fee_split(-100, 9000, 1000);
    assert_eq!(creator, 0);
    assert_eq!(protocol, 0);
}

#[test]
fn fee_split_minimum_protocol_bps_zero() {
    let (creator, protocol) = fee::compute_fee_split(1000, 10000, 0);
    assert_eq!(creator, 1000);
    assert_eq!(protocol, 0);
    assert_eq!(creator + protocol, 1000);
}

#[test]
fn fee_split_maximum_protocol_bps_5000() {
    let (creator, protocol) = fee::compute_fee_split(10000, 5000, 5000);
    assert_eq!(creator, 5000);
    assert_eq!(protocol, 5000);
    assert_eq!(creator + protocol, 10000);
}

#[test]
fn fee_split_invariant_holds_across_typical_amounts() {
    for total in [1i128, 10, 99, 100, 999, 1000, 10000, 100000, 1_000_000] {
        let (creator, protocol) = fee::compute_fee_split(total, 9000, 1000);
        assert_eq!(
            creator + protocol,
            total,
            "balance conservation fails at amount={}",
            total
        );
    }
}

#[test]
fn fee_split_invariant_holds_for_all_valid_fee_configs() {
    let total = 1000i128;
    for protocol_bps in [0u32, 1, 100, 1000, 5000, 9999, 10000] {
        let creator_bps = 10000u32.saturating_sub(protocol_bps);
        let (creator, protocol) = fee::compute_fee_split(total, creator_bps, protocol_bps);
        assert_eq!(
            creator + protocol,
            total,
            "balance conservation fails at protocol_bps={}",
            protocol_bps
        );
    }
}

#[test]
fn checked_fee_split_boundary_prevents_overflow() {
    // Test near-maximum safe amounts
    let safe_amount = 9_223_372_036_854_775i128;
    let result = fee::checked_compute_fee_split(safe_amount, 9000, 1000);
    assert!(
        result.is_some(),
        "checked split should succeed for safe amount"
    );
    let (creator, protocol) = result.unwrap();
    assert_eq!(creator + protocol, safe_amount);
}

#[test]
fn checked_fee_split_rejects_overflow() {
    // Amounts that would overflow during multiplication
    let unsafe_amount = i128::MAX;
    let result = fee::checked_compute_fee_split(unsafe_amount, 9000, 1000);
    assert!(
        result.is_none(),
        "checked split should fail for unsafe amount"
    );
}

#[test]
fn checked_fee_split_zero_amount_succeeds() {
    let result = fee::checked_compute_fee_split(0, 9000, 1000);
    assert_eq!(result, Some((0, 0)));
}

#[test]
fn checked_fee_split_negative_amount_succeeds() {
    let result = fee::checked_compute_fee_split(-100, 9000, 1000);
    assert_eq!(result, Some((0, 0)));
}
