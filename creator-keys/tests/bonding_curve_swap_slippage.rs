//! Slippage-protected bonding-curve `swap` (price-impact guard) and the
//! `get_price_impact` view.

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_curve_slope,
    set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{events, ContractError, SwapDirection};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1_000;
const SLOPE: i128 = 100;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

/// price(supply) = KEY_PRICE + SLOPE * supply
fn curve_price(supply: u32) -> i128 {
    KEY_PRICE + SLOPE * supply as i128
}

fn setup(
    env: &Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    Address,
    Address,
) {
    let (client, _) = register_creator_keys(env);
    let _admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    set_curve_slope(env, &client, SLOPE);
    let creator = register_test_creator(env, &client, "alice");
    let buyer = Address::generate(env);
    (client, creator, buyer)
}

/// Bumps the ledger sequence so a sell is not blocked by the same-ledger
/// flash-loan guard after a prior buy.
fn next_ledger(env: &Env) {
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
}

// ---------------------------------------------------------------------------
// Acceptance: swap reverts with SlippageExceeded when impact exceeds tolerance
// ---------------------------------------------------------------------------

#[test]
fn test_swap_reverts_with_slippage_exceeded_when_impact_exceeds_tolerance() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    // supply 0 -> 5: spot moves 1000 -> 1500, impact = 5000 bps.
    let before = capture_snapshot(&client, &creator, &buyer);
    let result = client.try_swap(&creator, &buyer, &5u32, &SwapDirection::Buy, &4_000u32);
    let after = capture_snapshot(&client, &creator, &buyer);

    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));
    before.assert_unchanged(&after);
}

#[test]
fn test_swap_sell_reverts_with_slippage_exceeded_when_impact_exceeds_tolerance() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);

    let cost: i128 = (0..10u32).map(curve_price).sum();
    client.buy_keys(&creator, &holder, &10u32, &cost, &None);
    next_ledger(&env);

    // supply 10 -> 5: spot moves 2000 -> 1500, impact = 2500 bps.
    let before = capture_snapshot(&client, &creator, &holder);
    let result = client.try_swap(&creator, &holder, &5u32, &SwapDirection::Sell, &2_499u32);
    let after = capture_snapshot(&client, &creator, &holder);

    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));
    before.assert_unchanged(&after);
}

// ---------------------------------------------------------------------------
// Acceptance: swap succeeds when impact is within tolerance
// ---------------------------------------------------------------------------

#[test]
fn test_swap_succeeds_when_impact_equals_tolerance() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    // Impact is exactly 5000 bps; equality with the bound must pass.
    let expected_cost: i128 = (0..5u32).map(curve_price).sum();
    let notional = client.swap(&creator, &buyer, &5u32, &SwapDirection::Buy, &5_000u32);

    assert_eq!(notional, expected_cost);
    assert_eq!(client.get_total_key_supply(&creator), 5);
    assert_eq!(client.get_key_balance(&creator, &buyer), 5);
}

#[test]
fn test_swap_succeeds_when_impact_below_tolerance() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    // One key from supply 0: 1000 -> 1100, impact = 1000 bps.
    let notional = client.swap(&creator, &buyer, &1u32, &SwapDirection::Buy, &1_000u32);

    assert_eq!(notional, curve_price(0));
    assert_eq!(client.get_total_key_supply(&creator), 1);
}

#[test]
fn test_swap_sell_succeeds_within_tolerance_and_reports_net_proceeds() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);

    let cost: i128 = (0..10u32).map(curve_price).sum();
    client.buy_keys(&creator, &holder, &10u32, &cost, &None);
    next_ledger(&env);

    // supply 10 -> 8: spot moves 2000 -> 1800, impact = 1000 bps.
    let sell_quote = client.get_sell_quote(&creator, &holder);
    assert_eq!(sell_quote.price, curve_price(9));
    let notional = client.swap(&creator, &holder, &2u32, &SwapDirection::Sell, &1_000u32);

    // In the standard 9000/1000 split the fee share consumes the whole key
    // price, so net seller proceeds are zero — matching get_sell_quote.
    assert_eq!(sell_quote.total_amount, 0);
    assert_eq!(notional, 0);

    assert_eq!(client.get_total_key_supply(&creator), 8);
    assert_eq!(client.get_key_balance(&creator, &holder), 8);
}

// ---------------------------------------------------------------------------
// Acceptance: get_price_impact returns accurate bps for any amount
// ---------------------------------------------------------------------------

#[test]
fn test_get_price_impact_matches_curve_math_for_various_amounts() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);

    // Zero amount never moves the spot price.
    assert_eq!(
        client.get_price_impact(&creator, &0u32, &SwapDirection::Buy),
        0
    );

    // supply 0 -> 1: 1000 -> 1100 = 1000 bps.
    assert_eq!(
        client.get_price_impact(&creator, &1u32, &SwapDirection::Buy),
        1_000
    );

    // supply 0 -> 5: 1000 -> 1500 = 5000 bps.
    assert_eq!(
        client.get_price_impact(&creator, &5u32, &SwapDirection::Buy),
        5_000
    );

    // After a buy of 10, supply is 10. Sell 5: 2000 -> 1500 = 2500 bps.
    let cost: i128 = (0..10u32).map(curve_price).sum();
    client.buy_keys(&creator, &holder, &10u32, &cost, &None);
    assert_eq!(
        client.get_price_impact(&creator, &5u32, &SwapDirection::Sell),
        2_500
    );

    // Sell the full supply: 2000 -> 1000 = 5000 bps.
    assert_eq!(
        client.get_price_impact(&creator, &10u32, &SwapDirection::Sell),
        5_000
    );
}

#[test]
fn test_get_price_impact_flat_curve_is_always_zero() {
    let env = test_env_with_auths();
    let (client, creator, _) = setup(&env);
    let admin = Address::generate(&env);
    client.set_curve_slope(&admin, &0i128);

    assert_eq!(
        client.get_price_impact(&creator, &1u32, &SwapDirection::Buy),
        0
    );
    assert_eq!(
        client.get_price_impact(&creator, &100u32, &SwapDirection::Buy),
        0
    );
}

#[test]
fn test_get_price_impact_sell_exceeding_supply_reverts() {
    let env = test_env_with_auths();
    let (client, creator, holder) = setup(&env);

    let buy_quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &holder, &buy_quote.total_amount, &None);

    let result = client.try_get_price_impact(&creator, &2u32, &SwapDirection::Sell);
    assert_eq!(result, Err(Ok(ContractError::InsufficientSupply)));
}

// ---------------------------------------------------------------------------
// Acceptance: zero slippage param bypasses check without error
// ---------------------------------------------------------------------------

#[test]
fn test_swap_zero_slippage_param_bypasses_check() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    // Impact would be 5000 bps, far above any sane bound, but a zero
    // tolerance skips the check entirely.
    let expected_cost: i128 = (0..5u32).map(curve_price).sum();
    let notional = client.swap(&creator, &buyer, &5u32, &SwapDirection::Buy, &0u32);

    assert_eq!(notional, expected_cost);
    assert_eq!(client.get_total_key_supply(&creator), 5);
}

// ---------------------------------------------------------------------------
// Acceptance: SlippageExceeded includes actual and max bps values
// ---------------------------------------------------------------------------

#[test]
fn test_slippage_exceeded_event_reports_actual_and_max_bps() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    set_curve_slope(&env, &client, SLOPE);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let result = client.try_swap(&creator, &buyer, &5u32, &SwapDirection::Buy, &4_000u32);
    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));

    let mut found = false;
    for (contract, topics, data) in env.events().all().iter() {
        if contract != contract_id {
            continue;
        }
        let name: Symbol = topics.get(0).unwrap().into_val(&env);
        if name == events::SWAP_SLIPPAGE_EXCEEDED_EVENT_NAME {
            let payload: events::SwapSlippageExceededEvent = data.clone().into_val(&env);
            assert_eq!(payload.creator_id, creator);
            assert_eq!(payload.actual_bps, 5_000);
            assert_eq!(payload.max_bps, 4_000);
            found = true;
        }
    }
    assert!(
        found,
        "expected a SwapSlippageExceeded event carrying actual and max bps"
    );
}

// ---------------------------------------------------------------------------
// Swap input validation
// ---------------------------------------------------------------------------

#[test]
fn test_swap_zero_amount_reverts() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    let result = client.try_swap(&creator, &buyer, &0u32, &SwapDirection::Buy, &0u32);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn test_swap_buy_charges_exact_curve_cost_for_multi_key_order() {
    let env = test_env_with_auths();
    let (client, creator, buyer) = setup(&env);

    let expected_cost: i128 = (0..3u32).map(curve_price).sum();
    let quote = client.get_buy_quote(&creator);
    assert_eq!(quote.price, curve_price(0));

    let notional = client.swap(&creator, &buyer, &3u32, &SwapDirection::Buy, &0u32);
    assert_eq!(notional, expected_cost);

    // The next spot price reflects the moved supply.
    let quote_after = client.get_buy_quote(&creator);
    assert_eq!(quote_after.price, curve_price(3));
}
