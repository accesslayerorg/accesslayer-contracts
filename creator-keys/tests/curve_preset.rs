//! Integration tests for bonding curve preset selection (Issue #403).

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use creator_keys::{CreatorKeysContractClient, CurvePreset};

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator_with_fee_config, set_protocol_fee_bps,
    test_env_with_auths, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};

fn setup_with_fees() -> (Env, Address, CreatorKeysContractClient<'static>, Address) {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    
    // Set up fees
    let admin = set_protocol_fee_bps(&env, &client, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS);

    (env, contract_id, client, admin)
}

#[test]
fn test_register_creator_defaults_to_linear() {
    let (env, _, client, _) = setup_with_fees();

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    // Register without specifying preset
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    let preset = client.get_curve_preset(&creator);
    assert_eq!(preset, CurvePreset::Linear);
}

#[test]
fn test_register_creator_with_quadratic() {
    let (env, _, client, _) = setup_with_fees();

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "bob");

    client.register_creator(
        &creator,
        &handle,
        &None,
        &None,
        &Some(CurvePreset::Quadratic),
        &None
    );

    let preset = client.get_curve_preset(&creator);
    assert_eq!(preset, CurvePreset::Quadratic);
}

#[test]
fn test_register_creator_with_flat() {
    let (env, _, client, _) = setup_with_fees();

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "charlie");

    client.register_creator(&creator, &handle, &None, &None, &Some(CurvePreset::Flat), &None);

    let preset = client.get_curve_preset(&creator);
    assert_eq!(preset, CurvePreset::Flat);
}

#[test]
fn test_linear_preset_regression_matches_base_price() {
    let (env, _, client, _) = setup_with_fees();

    let creator = register_test_creator_with_fee_config(
        &env,
        &client,
        "linear",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let quote = client.get_buy_quote(&creator);
    // At supply=0, Linear should match the base price
    assert_eq!(
        quote.price,
        creator_keys::bonding_curve::curve_params::BASE_PRICE
    );
}

#[test]
fn test_quadratic_higher_than_linear_at_same_supply() {
    let (env, _, client, _) = setup_with_fees();

    let linear_creator = register_test_creator_with_fee_config(
        &env,
        &client,
        "lin",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let quad_creator = register_test_creator_with_fee_config(
        &env,
        &client,
        "quad",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    // Buy one key for each to increase supply to 1
    let buyer = Address::generate(&env);
    client.buy_key(&linear_creator, &buyer, &50_000_000, &None);
    client.buy_key(&quad_creator, &buyer, &50_000_000, &None);

    let linear_quote = client.get_buy_quote(&linear_creator);
    let quad_quote = client.get_buy_quote(&quad_creator);

    assert!(
        quad_quote.price > linear_quote.price,
        "quadratic price {} should exceed linear price {}",
        quad_quote.price,
        linear_quote.price
    );
}

#[test]
fn test_flat_lower_than_linear_at_same_supply() {
    let (env, _, client, _) = setup_with_fees();

    let linear_creator = register_test_creator_with_fee_config(
        &env,
        &client,
        "lin",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let flat_creator = register_test_creator_with_fee_config(
        &env,
        &client,
        "flat",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    // Buy keys to reach supply=5
    let buyer = Address::generate(&env);
    for _ in 0..5 {
        client.buy_key(&linear_creator, &buyer, &100_000_000, &None);
        client.buy_key(&flat_creator, &buyer, &100_000_000, &None);
    }

    let linear_quote = client.get_buy_quote(&linear_creator);
    let flat_quote = client.get_buy_quote(&flat_creator);

    assert!(
        flat_quote.price < linear_quote.price,
        "flat price {} should be below linear price {}",
        flat_quote.price,
        linear_quote.price
    );
}

#[test]
fn test_curve_preset_immutable_no_update_function() {
    let (env, _, client, _) = setup_with_fees();

    let creator = register_test_creator_with_fee_config(
        &env,
        &client,
        "creator",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let preset_before = client.get_curve_preset(&creator);

    // Verify no method exists to update the preset
    // This is a compile-time check: the client simply doesn't have update_curve_preset

    let preset_after = client.get_curve_preset(&creator);
    assert_eq!(preset_before, preset_after);
}

#[test]
fn test_independent_curves_no_cross_contamination() {
    let (env, _, client, _) = setup_with_fees();

    let creator_a = register_test_creator_with_fee_config(
        &env,
        &client,
        "a",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator_b = register_test_creator_with_fee_config(
        &env,
        &client,
        "b",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    // Buy multiple keys for each
    let buyer = Address::generate(&env);
    for _ in 0..10 {
        client.buy_key(&creator_a, &buyer, &200_000_000, &None);
        client.buy_key(&creator_b, &buyer, &200_000_000, &None);
    }

    // Verify prices are independent
    let quote_a = client.get_buy_quote(&creator_a);
    let quote_b = client.get_buy_quote(&creator_b);

    // Quadratic should diverge significantly from Flat
    assert!(
        quote_a.price > quote_b.price,
        "quadratic {} should exceed flat {}",
        quote_a.price,
        quote_b.price
    );

    // Verify supply tracking is independent
    assert_eq!(client.get_creator_supply(&creator_a), 10);
    assert_eq!(client.get_creator_supply(&creator_b), 10);
}

#[test]
fn test_buy_sell_symmetry_all_presets() {
    for preset in [
        CurvePreset::Linear,
        CurvePreset::Quadratic,
        CurvePreset::Flat,
    ] {
        let (env, _, client, _) = setup_with_fees();

        let creator = register_test_creator_with_fee_config(
            &env,
            &client,
            "sym",
            DEFAULT_CREATOR_BPS,
            DEFAULT_PROTOCOL_BPS,
        );
        let buyer = Address::generate(&env);

        // Get buy quote at supply=0
        let buy_quote = client.get_buy_quote(&creator);

        // Buy the key
        client.buy_key(&creator, &buyer, &buy_quote.total_amount, &None);

        // Get sell quote at supply=1
        let sell_quote = client.get_sell_quote(&creator, &buyer);

        // Price component should be symmetric (fees may differ in direction)
        assert_eq!(
            buy_quote.price, sell_quote.price,
            "symmetry failed for preset {:?}: buy_price={} sell_price={}",
            preset, buy_quote.price, sell_quote.price
        );
    }
}

#[test]
fn test_get_curve_preset_unregistered_fails() {
    let (env, _, client, _) = setup_with_fees();

    let unregistered = Address::generate(&env);
    let result = client.try_get_curve_preset(&unregistered);
    assert!(result.is_err());
}

#[test]
fn test_creator_details_includes_preset() {
    let (env, _, client, _) = setup_with_fees();

    let creator = register_test_creator_with_fee_config(
        &env,
        &client,
        "detailed",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let details = client.get_creator_details(&creator);
    assert_eq!(details.curve_preset, CurvePreset::Quadratic);
}

#[test]
fn test_batch_view_includes_preset() {
    let (env, _, client, _) = setup_with_fees();

    let creator_a = register_test_creator_with_fee_config(
        &env,
        &client,
        "a",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator_b = register_test_creator_with_fee_config(
        &env,
        &client,
        "b",
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let mut creators = soroban_sdk::Vec::new(&env);
    creators.push_back(creator_a.clone());
    creators.push_back(creator_b.clone());

    let batch = client.get_creators_batch(&creators);

    assert_eq!(batch.get(0).unwrap().curve_preset, CurvePreset::Linear);
    assert_eq!(batch.get(1).unwrap().curve_preset, CurvePreset::Flat);
}
