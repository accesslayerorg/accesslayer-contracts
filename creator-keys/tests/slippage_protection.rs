//! Slippage protection for `buy_key` (`max_price`) and `sell_key` (`min_proceeds`).

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, Env};

const KEY_PRICE: i128 = 1_000;

fn setup_buy(
    env: &Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    Address,
    Address,
    Address,
) {
    let (client, contract_id) = register_creator_keys(env);
    let _admin = set_pricing_and_fees(env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(env, &client, "alice");
    let buyer = Address::generate(env);
    (client, contract_id, creator, buyer)
}

fn setup_sell(
    env: &Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    Address,
    Address,
    Address,
) {
    let (client, contract_id, creator, holder) = setup_buy(env);
    let buy_quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &holder, &buy_quote.total_amount, &None);
    (client, contract_id, creator, holder)
}

#[test]
fn test_slippage_exceeded_discriminant_is_16() {
    assert_eq!(ContractError::SlippageExceeded as u32, 16);
}

#[test]
fn test_buy_slippage_reverts_when_price_exceeds_max_price() {
    let env = test_env_with_auths();
    let (client, _, creator, buyer) = setup_buy(&env);

    let before = capture_snapshot(&client, &creator, &buyer);
    let result = client.try_buy_key(&creator, &buyer, &KEY_PRICE, &Some(KEY_PRICE - 1));
    let after = capture_snapshot(&client, &creator, &buyer);

    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));
    before.assert_unchanged(&after);
}

#[test]
fn test_buy_slippage_succeeds_when_price_at_or_below_max_price() {
    let env = test_env_with_auths();
    let (client, _, creator, buyer) = setup_buy(&env);
    let buy_quote = client.get_buy_quote(&creator);

    let supply_at_limit =
        client.buy_key(&creator, &buyer, &buy_quote.total_amount, &Some(KEY_PRICE));
    assert_eq!(supply_at_limit, 1);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);

    let buyer_two = Address::generate(&env);
    let supply_below_limit = client.buy_key(
        &creator,
        &buyer_two,
        &buy_quote.total_amount,
        &Some(KEY_PRICE + 1),
    );
    assert_eq!(supply_below_limit, 2);
}

#[test]
fn test_sell_slippage_reverts_when_proceeds_below_min_proceeds() {
    let env = test_env_with_auths();
    let (client, _, creator, holder) = setup_sell(&env);
    let sell_quote = client.get_sell_quote(&creator, &holder);

    let before = capture_snapshot(&client, &creator, &holder);
    let result = client.try_sell_key(&creator, &holder, &Some(sell_quote.total_amount + 1));
    let after = capture_snapshot(&client, &creator, &holder);

    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));
    before.assert_unchanged(&after);
}

#[test]
fn test_sell_slippage_succeeds_when_proceeds_meet_or_exceed_min_proceeds() {
    let env = test_env_with_auths();
    let (client, _, creator, holder) = setup_sell(&env);
    let sell_quote = client.get_sell_quote(&creator, &holder);

    let supply_at_limit = client.sell_key(&creator, &holder, &Some(sell_quote.total_amount));
    assert_eq!(supply_at_limit, 0);

    let holder_two = Address::generate(&env);
    let buy_quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &holder_two, &buy_quote.total_amount, &None);
    let sell_quote_two = client.get_sell_quote(&creator, &holder_two);

    let supply_below_limit = client.sell_key(
        &creator,
        &holder_two,
        &Some(sell_quote_two.total_amount - 1),
    );
    assert_eq!(supply_below_limit, 0);
}

#[test]
fn test_slippage_none_passthrough_preserves_existing_behavior() {
    let env = test_env_with_auths();
    let (client, _, creator, buyer) = setup_buy(&env);
    let buy_quote = client.get_buy_quote(&creator);

    let supply = client.buy_key(&creator, &buyer, &buy_quote.total_amount, &None);
    assert_eq!(supply, 1);

    let sell_quote = client.get_sell_quote(&creator, &buyer);
    let supply_after_sell = client.sell_key(&creator, &buyer, &None);
    assert_eq!(supply_after_sell, 0);
    assert_eq!(
        sell_quote.total_amount,
        buy_quote.price - buy_quote.creator_fee - buy_quote.protocol_fee
    );
}

// ---------------------------------------------------------------------------
// Issue #676 Unit Tests: Slippage Protection Guard for buy_key max_price
// ---------------------------------------------------------------------------

#[test]
fn test_buy_slippage_max_price_u128_max_always_succeeds() {
    let env = test_env_with_auths();
    let (client, _, creator, buyer) = setup_buy(&env);
    let buy_quote = client.get_buy_quote(&creator);

    // Setting max_price to max value (i128::MAX) never triggers the slippage guard
    let max_price = Some(i128::MAX);
    let supply = client.buy_key(&creator, &buyer, &buy_quote.total_amount, &max_price);
    assert_eq!(supply, 1);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
}

#[test]
fn test_buy_slippage_max_price_zero_panics_unless_key_is_free() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "bob");
    let buyer = Address::generate(&env);

    // Case 1: Key is NOT free (price > 0). max_price = Some(0) panics with SlippageExceeded
    let before = capture_snapshot(&client, &creator, &buyer);
    let result = client.try_buy_key(&creator, &buyer, &KEY_PRICE, &Some(0));
    let after = capture_snapshot(&client, &creator, &buyer);

    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));
    before.assert_unchanged(&after);

    // Case 2: Key IS free (price == 0). max_price = Some(0) succeeds without triggering guard
    let free_creator = register_test_creator(&env, &client, "free_creator");
    contract_test_env::set_stored_key_price(&env, &contract_id, 0);
    let free_buyer = Address::generate(&env);

    let supply = client.buy_key(&free_creator, &free_buyer, &100, &Some(0));
    assert_eq!(supply, 1);
    assert_eq!(client.get_key_balance(&free_creator, &free_buyer), 1);
}

#[test]
fn test_buy_slippage_boundary_exact_cost_and_exceeded_by_one_stroop() {
    let env = test_env_with_auths();
    let (client, _, creator, _buyer) = setup_buy(&env);
    let buy_quote = client.get_buy_quote(&creator);
    let actual_price = buy_quote.price;

    // 1. Actual cost equal to max_price succeeds
    let buyer1 = Address::generate(&env);
    let supply = client.buy_key(
        &creator,
        &buyer1,
        &buy_quote.total_amount,
        &Some(actual_price),
    );
    assert_eq!(supply, 1);

    // 2. Actual cost exceeding max_price by 1 stroop (max_price = actual_price - 1)
    // panics with SlippageExceeded
    let buyer2 = Address::generate(&env);
    let before = capture_snapshot(&client, &creator, &buyer2);
    let result = client.try_buy_key(
        &creator,
        &buyer2,
        &buy_quote.total_amount,
        &Some(actual_price - 1),
    );
    let after = capture_snapshot(&client, &creator, &buyer2);

    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));
    // 3. Assert no state is mutated when the guard panics
    before.assert_unchanged(&after);
}
