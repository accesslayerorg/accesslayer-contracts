//! Unit tests for the creator fee split distribution (issue #710).
//!
//! The fee split function [`creator_keys::fee::compute_fee_split`] divides a
//! total fee amount between the protocol treasury and the creator's fee
//! recipient according to configured basis points. These tests confirm:
//!
//! - an even 70/30 split assigns exactly 700/300 per 1000 stroops
//! - odd totals are handled by floor division without losing stroops
//! - 100/0 and 0/100 configurations assign the full fee to one side
//! - both amounts always sum to the total for complete splits
//!
//! # Rounding semantics (documented contract invariant)
//!
//! The protocol share is always floored: `protocol = total * protocol_bps / BPS_MAX`.
//! When `creator_bps + protocol_bps == BPS_MAX` (a complete split), the remainder
//! from that floor division is assigned to the **creator**, so the two outputs
//! conserve the total exactly. When the configured bps sum to less than
//! `BPS_MAX`, each side is floored independently and any dust remains with the
//! trader by design.

use creator_keys::fee;

const CREATOR_BPS_70_30_SPLIT: u32 = 3_000;
const PROTOCOL_BPS_70_30_SPLIT: u32 = 7_000;

// ---------------------------------------------------------------------------
// Even amounts: exact 70/30 split
// ---------------------------------------------------------------------------

#[test]
fn test_split_70_30_even_amount_is_exact() {
    // 1000 stroops split 70% protocol / 30% creator.
    let (creator, protocol) =
        fee::compute_fee_split(1000, CREATOR_BPS_70_30_SPLIT, PROTOCOL_BPS_70_30_SPLIT);
    assert_eq!(protocol, 700, "protocol must receive exactly 700 stroops");
    assert_eq!(creator, 300, "creator must receive exactly 300 stroops");
    assert_eq!(creator + protocol, 1000);
}

#[test]
fn test_split_70_30_even_amount_scales_across_totals() {
    for total in [10i128, 1_000, 7_300, 100_000] {
        let (creator, protocol) =
            fee::compute_fee_split(total, CREATOR_BPS_70_30_SPLIT, PROTOCOL_BPS_70_30_SPLIT);
        assert_eq!(
            protocol,
            total * 7 / 10,
            "protocol floor share mismatch for total={total}"
        );
        assert_eq!(creator, total - protocol);
        assert_eq!(
            creator + protocol,
            total,
            "split must conserve total={total}"
        );
    }
}

// ---------------------------------------------------------------------------
// Odd amounts: floor division remainder handling
// ---------------------------------------------------------------------------

#[test]
fn test_split_70_30_odd_amount_conserves_total() {
    // 1001 * 7000 / 10000 = 700.7 -> floor gives protocol 700; the remainder
    // stroop is assigned to the creator so no stroop is lost.
    let (creator, protocol) =
        fee::compute_fee_split(1001, CREATOR_BPS_70_30_SPLIT, PROTOCOL_BPS_70_30_SPLIT);
    assert_eq!(
        protocol, 700,
        "protocol share must be floored to 700 stroops"
    );
    assert_eq!(
        creator, 301,
        "remainder stroop must stay out of rounding loss"
    );
    assert_eq!(
        creator + protocol,
        1001,
        "no stroop may be lost to rounding"
    );
}

#[test]
fn test_split_odd_amounts_never_lose_stroops() {
    for total in [999i128, 1001, 1234, 4321, 9_876_543_210] {
        let (creator, protocol) =
            fee::compute_fee_split(total, CREATOR_BPS_70_30_SPLIT, PROTOCOL_BPS_70_30_SPLIT);
        assert_eq!(
            creator + protocol,
            total,
            "remainder must be assigned, not dropped, for total={total}"
        );
        assert!(creator >= 0 && protocol >= 0);
    }
}

// ---------------------------------------------------------------------------
// Extreme splits: everything to one side
// ---------------------------------------------------------------------------

#[test]
fn test_split_100_protocol_assigns_full_fee_to_protocol() {
    for total in [1i128, 999, 1000, 100_001] {
        let (creator, protocol) = fee::compute_fee_split(total, 0, 10_000);
        assert_eq!(
            protocol, total,
            "full fee must go to protocol for total={total}"
        );
        assert_eq!(creator, 0, "creator must receive nothing for total={total}");
    }
}

#[test]
fn test_split_100_creator_assigns_full_fee_to_creator() {
    for total in [1i128, 999, 1000, 100_001] {
        let (creator, protocol) = fee::compute_fee_split(total, 10_000, 0);
        assert_eq!(
            creator, total,
            "full fee must go to creator for total={total}"
        );
        assert_eq!(
            protocol, 0,
            "protocol must receive nothing for total={total}"
        );
    }
}

// ---------------------------------------------------------------------------
// Conservation invariant across configurations
// ---------------------------------------------------------------------------

#[test]
fn test_complete_splits_always_sum_to_total() {
    // Complete splits (bps sum == BPS_MAX): conservation must hold for every total.
    let complete_configs = [
        (CREATOR_BPS_70_30_SPLIT, PROTOCOL_BPS_70_30_SPLIT),
        (9_000, 1_000),
        (5_000, 5_000),
        (10_000, 0),
        (0, 10_000),
        (1, 9_999),
    ];
    for (creator_bps, protocol_bps) in complete_configs {
        for total in [
            0i128,
            1,
            2,
            3,
            17,
            500,
            999,
            1000,
            1001,
            65_535,
            i128::MAX / 10_000,
        ] {
            let (creator, protocol) = fee::compute_fee_split(total, creator_bps, protocol_bps);
            assert_eq!(
                creator + protocol,
                total,
                "creator={creator} protocol={protocol} config=({creator_bps},{protocol_bps}) total={total}"
            );
        }
    }
}

#[test]
fn test_partial_split_floors_each_side_independently() {
    // Configurations where bps sum to less than BPS_MAX leave the dust with the
    // trader by design: each side is floored independently.
    let (creator, protocol) = fee::compute_fee_split(1001, 4_000, 4_000);
    assert_eq!(protocol, 400, "4000 bps of 1001 floors to 400");
    assert_eq!(creator, 400, "4000 bps of 1001 floors to 400");
    assert!(
        creator + protocol <= 1001,
        "partial split can never over-assign the total"
    );
}

// ---------------------------------------------------------------------------
// Checked variant parity and live entrypoint cross-check
// ---------------------------------------------------------------------------

#[test]
fn test_checked_compute_fee_split_matches_unchecked_results() {
    for total in [0i128, 1, 999, 1000, 1001, 1_000_007] {
        let unchecked =
            fee::compute_fee_split(total, CREATOR_BPS_70_30_SPLIT, PROTOCOL_BPS_70_30_SPLIT);
        let checked = fee::checked_compute_fee_split(
            total,
            CREATOR_BPS_70_30_SPLIT,
            PROTOCOL_BPS_70_30_SPLIT,
        );
        assert_eq!(
            checked,
            Some(unchecked),
            "checked and unchecked splits must agree for total={total}"
        );
    }
}

#[test]
fn test_contract_entrypoint_distributes_via_same_split_math() {
    use soroban_sdk::testutils::Address as _;

    let env = soroban_sdk::Env::default();
    env.mock_all_auths();

    let contract_id = env.register(creator_keys::CreatorKeysContract, ());
    let client = creator_keys::CreatorKeysContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_fee_config(&admin, &CREATOR_BPS_70_30_SPLIT, &PROTOCOL_BPS_70_30_SPLIT);

    // Even total: exact 700/300 distribution through the deployed entrypoint.
    let (creator, protocol) = client.compute_fees_for_payment(&1000i128);
    assert_eq!(creator, 300);
    assert_eq!(protocol, 700);
    assert_eq!(creator + protocol, 1000);

    // Odd total: remainder stroop preserved, not lost to rounding.
    let (creator, protocol) = client.compute_fees_for_payment(&1001i128);
    assert_eq!(creator, 301);
    assert_eq!(protocol, 700);
    assert_eq!(creator + protocol, 1001);
}
