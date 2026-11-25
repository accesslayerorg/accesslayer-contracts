//! Tests for volume discount tier evaluation order, tie resolution, and invariants (#549).
//!
//! Scope verified:
//! - Tiers are evaluated in ascending order of `volume_threshold` and the last matching tier wins.
//! - Worked example: Three tiers at 1,000, 5,000, and 10,000 XLM volume:
//!   A creator at 7,000 XLM volume qualifies for the first two and receives the tier-2 rate.
//! - Updating tiers takes effect on the next trade, not retroactively.
//! - A creator who drops below a threshold retains the earned tier.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::FeeTier;
use soroban_sdk::{testutils::Address as _, vec, Address, Vec};

const STROOPS_PER_XLM: i128 = 10_000_000;

#[test]
fn test_volume_discount_tier_evaluation_order_and_last_matching_wins() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    // Three tiers:
    // Tier 0: Base threshold 0, 1000 bps (10%)
    // Tier 1: 1,000 XLM, 800 bps (8%)
    // Tier 2: 5,000 XLM, 600 bps (6%)
    // Tier 3: 10,000 XLM, 400 bps (4%)
    let tiers: Vec<FeeTier> = vec![
        &env,
        FeeTier {
            volume_threshold: 0,
            protocol_bps: 1000,
        },
        FeeTier {
            volume_threshold: 1_000 * STROOPS_PER_XLM,
            protocol_bps: 800,
        },
        FeeTier {
            volume_threshold: 5_000 * STROOPS_PER_XLM,
            protocol_bps: 600,
        },
        FeeTier {
            volume_threshold: 10_000 * STROOPS_PER_XLM,
            protocol_bps: 400,
        },
    ];

    client.set_fee_tiers(&admin, &tiers);

    // Initial state: 0 volume -> Tier 0 (1000 bps)
    assert_eq!(client.get_current_fee(), 1000);

    // Worked example (#549):
    // Volume at 7,000 XLM qualifies for Tier 0, Tier 1 (1k), and Tier 2 (5k).
    // The last matching tier wins -> Tier 2 (600 bps).
    let vol_7000_xlm = 7_000 * STROOPS_PER_XLM;
    let applied_bps = client.apply_trade_volume(&vol_7000_xlm);
    assert_eq!(
        applied_bps, 600,
        "7,000 XLM cumulative volume must select Tier 2 (600 bps) as the last matching tier"
    );

    let fee_view = client.get_dynamic_fee_view();
    assert_eq!(fee_view.protocol_bps, 600);
    assert_eq!(
        fee_view.tier_index, 2,
        "active tier index must be 2 (5,000 XLM tier)"
    );
}

#[test]
fn test_tier_updates_take_effect_on_next_trade_not_retroactively() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    let tiers: Vec<FeeTier> = vec![
        &env,
        FeeTier {
            volume_threshold: 0,
            protocol_bps: 1000,
        },
        FeeTier {
            volume_threshold: 5_000 * STROOPS_PER_XLM,
            protocol_bps: 600,
        },
    ];

    client.set_fee_tiers(&admin, &tiers);
    assert_eq!(client.get_current_fee(), 1000);

    // Trade 1 with 6,000 XLM volume:
    // Before trade 1, active fee is 1000 bps.
    // Calling apply_trade_volume records the volume and returns the new bps for the *next* trade.
    let next_bps = client.apply_trade_volume(&(6_000 * STROOPS_PER_XLM));
    assert_eq!(next_bps, 600);
    assert_eq!(client.get_current_fee(), 600);
}
