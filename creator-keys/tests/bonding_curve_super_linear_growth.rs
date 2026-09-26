//! Unit tests verifying the bonding curve formula output grows faster than
//! linearly as supply increases (Issue #650).
//!
//! The Quadratic curve preset is defined as
//!
//! ```text
//!     P(s) = base_price + slope * s^2
//! ```
//!
//! so consecutive per-step deltas must themselves grow as supply grows:
//!
//! ```text
//!     delta(s -> s + 1) = slope * ((s + 1)^2 - s^2) = slope * (2 * s + 1)
//! ```
//!
//! At `s = 0, 4, 9` the deltas become `slope`, `9 * slope`, `19 * slope` —
//! strictly increasing. This file pins that invariant by querying the public
//! `get_buy_quote` at supply levels 0, 1, 4, 5, 9, and 10 for a single creator
//! registered with the Quadratic preset.
//!
//! A Linear (`P(s) = base_price + slope * s`) curve would yield three equal
//! deltas (`slope, slope, slope`) and fail the strict-ordering assertions; a
//! flat curve would yield `0, 0, 0` and fail the equality assertions on the
//! deltas themselves. Either failure mode is the intended regression signal:
//! if the curve silently regresses to a linear or sub-linear shape, this test
//! fails.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, set_curve_slope, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::constants;
use creator_keys::{CurvePreset, RegisterCreatorParams};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

const BASE_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;
const QUADRATIC_SLOPE: i128 = 10;

/// Overrides the registered creator's `supply` directly so `get_buy_quote`
/// can be read for any supply value without performing actual buys.
///
/// Mirrors the same direct-storage-setter pattern used by
/// `flat_curve_lower_than_linear_regression.rs`; the public buy path is not
/// under test here, only the price math inside `compute_bonding_curve_price`.
fn set_registered_supply(
    env: &Env,
    contract_id: &Address,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    target: u32,
) {
    let mut profile = client.get_creator(creator);
    profile.supply = target;
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::creator(creator), &profile);
    });
}

/// Registers a Quadratic-curve creator and returns its address.
fn register_quadratic_creator(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    handle: &str,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &None,
        &None,
        &Some(CurvePreset::Quadratic),
        &None,
        &None,
    );
    creator
}

/// Reads `quote.price` for one supply level without mutating state.
fn price_at_supply(
    env: &Env,
    contract_id: &Address,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    supply: u32,
) -> i128 {
    set_registered_supply(env, contract_id, client, creator, supply);
    let quote = client.get_buy_quote(creator);
    assert!(
        quote.price > 0,
        "buy quote at supply {} must be positive; got price={}, total={}",
        supply,
        quote.price,
        quote.total_amount,
    );
    quote.price
}

/// Reads the six prices required by the super-linear invariant checks.
fn read_window_prices(
    env: &Env,
    contract_id: &Address,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
) -> (i128, i128, i128, i128, i128, i128) {
    let p0 = price_at_supply(env, contract_id, client, creator, 0);
    let p1 = price_at_supply(env, contract_id, client, creator, 1);
    let p4 = price_at_supply(env, contract_id, client, creator, 4);
    let p5 = price_at_supply(env, contract_id, client, creator, 5);
    let p9 = price_at_supply(env, contract_id, client, creator, 9);
    let p10 = price_at_supply(env, contract_id, client, creator, 10);
    (p0, p1, p4, p5, p9, p10)
}

/// Closed-form Quadratic price: `base_price + slope * s^2`. Mirrors the
/// contract's `compute_bonding_curve_price` for the Quadratic branch and is
/// kept inline (rather than reusing `compute_expected_bonding_curve_price`
/// from `contract_test_env`) because that helper targets the Linear formula.
fn quadratic_price(supply: u32) -> i128 {
    BASE_PRICE + QUADRATIC_SLOPE * i128::from(supply) * i128::from(supply)
}

#[test]
fn test_bonding_curve_grows_super_linearly() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, BASE_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    set_curve_slope(&env, &client, QUADRATIC_SLOPE);

    let creator = register_quadratic_creator(&env, &client, "superlinear");

    let (price_at_0, price_at_1, price_at_4, price_at_5, price_at_9, price_at_10) =
        read_window_prices(&env, &contract_id, &client, &creator);

    // Per-step deltas. For a Quadratic curve with slope=10:
    //   delta(0 -> 1)  = 10 * (1^2 - 0^2)  = 10
    //   delta(4 -> 5)  = 10 * (5^2 - 4^2)  = 90
    //   delta(9 -> 10) = 10 * (10^2 - 9^2) = 190
    // A Linear curve would yield three equal deltas (slope, slope, slope).
    let delta_0_1 = price_at_1 - price_at_0;
    let delta_4_5 = price_at_5 - price_at_4;
    let delta_9_10 = price_at_10 - price_at_9;

    // Acceptance criterion: delta(0 -> 1) < delta(4 -> 5) < delta(9 -> 10).
    assert!(
        delta_0_1 < delta_4_5,
        "delta(0->1)={} must be strictly less than delta(4->5)={} (super-linear growth)",
        delta_0_1,
        delta_4_5,
    );
    assert!(
        delta_4_5 < delta_9_10,
        "delta(4->5)={} must be strictly less than delta(9->10)={} (super-linear growth)",
        delta_4_5,
        delta_9_10,
    );

    // Acceptance criterion: all three deltas are strictly different (catches a
    // Linear/Flat regression where one or more deltas would collapse to slope
    // or zero).
    assert_ne!(
        delta_0_1, delta_4_5,
        "delta(0->1) and delta(4->5) must differ; got equal {} — implies a non-super-linear curve",
        delta_0_1,
    );
    assert_ne!(
        delta_4_5, delta_9_10,
        "delta(4->5) and delta(9->10) must differ; got equal {} — implies a non-super-linear curve",
        delta_4_5,
    );
    assert_ne!(
        delta_0_1, delta_9_10,
        "delta(0->1) and delta(9->10) must differ; got equal {} — implies a non-super-linear curve",
        delta_0_1,
    );
}

#[test]
fn test_bonding_curve_super_linear_deltas_match_quadratic_formula() {
    // Locks in the exact delta magnitudes against the closed-form formula for
    // the Quadratic preset. Any future change to `compute_bonding_curve_price`
    // that drifts the Quadratic delta shape must update this test intentionally
    // rather than silently.
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, BASE_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    set_curve_slope(&env, &client, QUADRATIC_SLOPE);

    let creator = register_quadratic_creator(&env, &client, "superlinear_pinned");

    let (price_at_0, price_at_1, price_at_4, price_at_5, price_at_9, price_at_10) =
        read_window_prices(&env, &contract_id, &client, &creator);

    assert_eq!(price_at_0, quadratic_price(0));
    assert_eq!(price_at_10, quadratic_price(10));

    assert_eq!(
        price_at_1 - price_at_0,
        QUADRATIC_SLOPE,
        "delta(0->1) must equal slope * (1^2 - 0^2) = slope",
    );
    assert_eq!(
        price_at_5 - price_at_4,
        QUADRATIC_SLOPE * 9,
        "delta(4->5) must equal slope * (5^2 - 4^2) = 9 * slope",
    );
    assert_eq!(
        price_at_10 - price_at_9,
        QUADRATIC_SLOPE * 19,
        "delta(9->10) must equal slope * (10^2 - 9^2) = 19 * slope",
    );

    // Reaffirm the strict-ordering invariant alongside the closed-form check.
    let delta_0_1 = price_at_1 - price_at_0;
    let delta_4_5 = price_at_5 - price_at_4;
    let delta_9_10 = price_at_10 - price_at_9;
    assert!(
        delta_0_1 < delta_4_5 && delta_4_5 < delta_9_10,
        "deltas must be strictly increasing: {} < {} < {}",
        delta_0_1,
        delta_4_5,
        delta_9_10,
    );
}
