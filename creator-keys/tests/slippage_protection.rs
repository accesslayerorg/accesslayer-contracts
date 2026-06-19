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
    let (client, contract_id, creator, holder) = {
        let (client, contract_id, creator, buyer) = setup_buy(env);
        (client, contract_id, creator, buyer)
    };
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
