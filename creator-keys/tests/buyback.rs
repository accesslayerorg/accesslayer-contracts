//! Integration tests for creator-authorized buybacks.

mod contract_test_env;

use contract_test_env::{
    compute_expected_bonding_curve_price, compute_expected_protocol_fee, register_creator_keys,
    register_test_creator, set_curve_slope, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{events, ContractError};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

fn setup<'a>(env: &'a Env) -> (creator_keys::CreatorKeysContractClient<'a>, Address) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

fn self_buy_keys(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    count: u32,
) {
    for _ in 0..count {
        let quote = client.get_buy_quote(creator);
        client.buy_key(creator, creator, &quote.total_amount, &None);
    }
}

#[test]
fn test_get_buyback_quote_returns_price_plus_protocol_fee_only() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 2);

    let quote = client.get_buyback_quote(&creator, &2);
    let expected_base_price = KEY_PRICE * 2;
    let expected_protocol_fee = compute_expected_protocol_fee(expected_base_price, PROTOCOL_BPS);

    assert_eq!(
        quote,
        expected_base_price + expected_protocol_fee,
        "buyback quote should waive creator fee and charge only protocol fee"
    );
}

#[test]
fn test_buyback_reduces_supply_and_creator_balance() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 2);

    let total_cost = client.get_buyback_quote(&creator, &1);
    let new_supply = client.buyback(&creator, &creator, &1, &total_cost, &None);

    assert_eq!(new_supply, 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &creator), 1);
    assert_eq!(client.get_creator_holder_count(&creator), 1);
}

#[test]
fn test_buyback_full_supply_clears_creator_position() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 2);

    let total_cost = client.get_buyback_quote(&creator, &2);
    let new_supply = client.buyback(&creator, &creator, &2, &total_cost, &None);

    assert_eq!(new_supply, 0);
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &creator), 0);
    assert_eq!(client.get_creator_holder_count(&creator), 0);
}

#[test]
fn test_buyback_does_not_credit_creator_fee_balance_and_does_credit_protocol_balance() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 1);

    let creator_fee_before = client.get_creator_fee_balance(&creator);
    let protocol_balance_before = client.get_protocol_recipient_balance();
    let total_cost = client.get_buyback_quote(&creator, &1);
    let expected_protocol_fee = compute_expected_protocol_fee(KEY_PRICE, PROTOCOL_BPS);

    client.buyback(&creator, &creator, &1, &total_cost, &None);

    let creator_fee_after = client.get_creator_fee_balance(&creator);
    let protocol_balance_after = client.get_protocol_recipient_balance();

    assert_eq!(
        creator_fee_after, creator_fee_before,
        "buyback should waive creator fee accrual"
    );
    assert_eq!(
        protocol_balance_after,
        protocol_balance_before + expected_protocol_fee,
        "buyback should accrue only the protocol fee"
    );
}

#[test]
fn test_buyback_rejects_non_creator_caller() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 1);
    let outsider = Address::generate(&env);
    let total_cost = client.get_buyback_quote(&creator, &1);

    let result = client.try_buyback(&creator, &outsider, &1, &total_cost, &None);

    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_buyback_zero_amount_reverts() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 1);

    let quote_result = client.try_get_buyback_quote(&creator, &0);
    let buyback_result = client.try_buyback(&creator, &creator, &0, &1, &None);

    assert_eq!(quote_result, Err(Ok(ContractError::NotPositiveAmount)));
    assert_eq!(buyback_result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn test_buyback_exceeding_supply_reverts_with_insufficient_supply() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 1);

    let quote_result = client.try_get_buyback_quote(&creator, &2);
    let buyback_result = client.try_buyback(&creator, &creator, &2, &2_200, &None);

    assert_eq!(quote_result, Err(Ok(ContractError::InsufficientSupply)));
    assert_eq!(buyback_result, Err(Ok(ContractError::InsufficientSupply)));
}

#[test]
fn test_buyback_rejects_when_creator_balance_is_below_amount_even_if_supply_is_higher() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 1);

    let outside_holder = Address::generate(&env);
    let buy_quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &outside_holder, &buy_quote.total_amount, &None);

    let total_cost = client.get_buyback_quote(&creator, &2);
    let result = client.try_buyback(&creator, &creator, &2, &total_cost, &None);

    assert_eq!(client.get_total_key_supply(&creator), 2);
    assert_eq!(client.get_key_balance(&creator, &creator), 1);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

#[test]
fn test_buyback_event_emits_expected_payload() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 2);

    let total_cost = client.get_buyback_quote(&creator, &1);
    let expected_ledger = env.ledger().sequence();
    client.buyback(&creator, &creator, &1, &total_cost, &None);

    let event_log = env.events().all();
    let last = event_log.last().unwrap();

    let event_name: Symbol = last
        .1
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .unwrap()
        .into_val(&env);
    let event_creator: Address = last
        .1
        .get(events::TOPIC_CREATOR_INDEX)
        .unwrap()
        .into_val(&env);
    let payload: events::KeysBoughtBackEvent = last.2.into_val(&env);

    assert_eq!(event_name, events::BUYBACK_EVENT_NAME);
    assert_eq!(event_creator, creator);
    assert_eq!(
        payload,
        events::KeysBoughtBackEvent {
            creator,
            amount: 1,
            price_paid: total_cost,
            new_supply: 1,
            ledger: expected_ledger,
        }
    );
}

#[test]
fn test_buyback_event_payload_field_order_is_documented() {
    assert_eq!(
        events::BUYBACK_EVENT_DATA_FIELDS,
        ["creator", "amount", "price_paid", "new_supply", "ledger"]
    );
}

#[test]
fn test_buyback_does_not_change_follow_on_buy_price_under_fixed_price_model() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    self_buy_keys(&client, &creator, 2);

    let before = client.get_buy_quote(&creator);
    let total_cost = client.get_buyback_quote(&creator, &1);
    client.buyback(&creator, &creator, &1, &total_cost, &None);
    let after = client.get_buy_quote(&creator);

    assert_eq!(
        before.price, after.price,
        "current contract pricing remains flat after buyback"
    );
    assert_eq!(
        before.total_amount, after.total_amount,
        "current buy total remains unchanged under the fixed-price model"
    );
}

const CURVE_SLOPE: i128 = 10;

#[test]
fn test_buy_quote_decreases_after_buyback_under_bonding_curve() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    // Buy keys to reach a non-zero supply.
    self_buy_keys(&client, &creator, 5);
    let supply_before: u32 = client.get_total_key_supply(&creator);
    assert_eq!(supply_before, 5);

    // Record buy quote at supply = 5.
    let quote_before = client.get_buy_quote(&creator);
    let expected_price_before =
        compute_expected_bonding_curve_price(CURVE_SLOPE, KEY_PRICE, supply_before);
    assert_eq!(quote_before.price, expected_price_before);

    // Creator buyback reduces supply by 2.
    let total_cost = client.get_buyback_quote(&creator, &2);
    client.buyback(&creator, &creator, &2, &total_cost, &None);
    let supply_after: u32 = client.get_total_key_supply(&creator);
    assert_eq!(supply_after, 3);

    // Record buy quote at supply = 3.
    let quote_after = client.get_buy_quote(&creator);
    let expected_price_after =
        compute_expected_bonding_curve_price(CURVE_SLOPE, KEY_PRICE, supply_after);
    assert_eq!(quote_after.price, expected_price_after);

    // After buyback, the bonding curve price must be lower (supply decreased).
    assert!(
        quote_after.price < quote_before.price,
        "buy quote after buyback ({}) should be lower than before ({}) under bonding curve",
        quote_after.price,
        quote_before.price,
    );

    // The difference must match the curve formula: slope * supply_reduced.
    let expected_difference = CURVE_SLOPE * (supply_before - supply_after) as i128;
    assert_eq!(
        quote_before.price - quote_after.price,
        expected_difference,
        "price difference should match the bonding curve formula for the reduced supply"
    );
}
