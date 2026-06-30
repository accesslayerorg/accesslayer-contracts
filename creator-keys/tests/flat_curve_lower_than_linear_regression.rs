//! Regression test: flat curve (slope=0) buy price must be strictly lower than
//! linear curve (slope>0) at high supply.
//!
//! Flat price = KEY_PRICE (constant regardless of supply)
//! Linear price = KEY_PRICE + slope * supply (grows with supply)
//!
//! At any positive supply with slope > 0, the linear price exceeds the flat price.
//! This test locks in that invariant at supply levels 100, 1000, and 10000 so
//! any change to the curve dispatch logic cannot silently break the ordering.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_curve_slope, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::constants;
use soroban_sdk::Address;

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;
const LINEAR_SLOPE: i128 = 1;
fn set_registered_supply(
    env: &soroban_sdk::Env,
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

#[test]
fn test_flat_buy_price_lower_than_linear_at_supply_100() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    // Flat curve: slope = 0 → price stays at KEY_PRICE regardless of supply
    set_curve_slope(&env, &client, 0);
    let creator_flat = register_test_creator(&env, &client, "flat");
    set_registered_supply(&env, &contract_id, &client, &creator_flat, 100);
    let flat_quote = client.get_buy_quote(&creator_flat);

    // Linear curve: slope > 0 → price grows with supply
    set_curve_slope(&env, &client, LINEAR_SLOPE);
    let creator_linear = register_test_creator(&env, &client, "linear");
    set_registered_supply(&env, &contract_id, &client, &creator_linear, 100);
    let linear_quote = client.get_buy_quote(&creator_linear);

    assert!(
        flat_quote.price < linear_quote.price,
        "flat buy price ({}) must be strictly less than linear ({}) at supply 100",
        flat_quote.price,
        linear_quote.price
    );
}

#[test]
fn test_flat_buy_price_lower_than_linear_at_supply_1000() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    set_curve_slope(&env, &client, 0);
    let creator_flat = register_test_creator(&env, &client, "flat");
    set_registered_supply(&env, &contract_id, &client, &creator_flat, 1000);
    let flat_quote = client.get_buy_quote(&creator_flat);

    set_curve_slope(&env, &client, LINEAR_SLOPE);
    let creator_linear = register_test_creator(&env, &client, "linear");
    set_registered_supply(&env, &contract_id, &client, &creator_linear, 1000);
    let linear_quote = client.get_buy_quote(&creator_linear);

    assert!(
        flat_quote.price < linear_quote.price,
        "flat buy price ({}) must be strictly less than linear ({}) at supply 1000",
        flat_quote.price,
        linear_quote.price
    );
}

#[test]
fn test_flat_buy_price_lower_than_linear_at_supply_10000() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    set_curve_slope(&env, &client, 0);
    let creator_flat = register_test_creator(&env, &client, "flat");
    set_registered_supply(&env, &contract_id, &client, &creator_flat, 10_000);
    let flat_quote = client.get_buy_quote(&creator_flat);

    set_curve_slope(&env, &client, LINEAR_SLOPE);
    let creator_linear = register_test_creator(&env, &client, "linear");
    set_registered_supply(&env, &contract_id, &client, &creator_linear, 10_000);
    let linear_quote = client.get_buy_quote(&creator_linear);

    assert!(
        flat_quote.price < linear_quote.price,
        "flat buy price ({}) must be strictly less than linear ({}) at supply 10000",
        flat_quote.price,
        linear_quote.price
    );
}
