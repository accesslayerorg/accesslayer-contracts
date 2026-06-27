//! Regression tests for slippage protection boundary conditions.
//! Buy must succeed when `max_price` equals the actual price exactly and
//! revert when `max_price` is one stroop below. Symmetric tests for sell
//! with `min_proceeds`.

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 1_000;

// ---------------------------------------------------------------------------
// Buy: max_price boundary
// ---------------------------------------------------------------------------

#[test]
fn test_buy_succeeds_when_max_price_equals_actual_price() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let quote = client.get_buy_quote(&creator);

    let supply = client.buy_key(&creator, &buyer, &quote.total_amount, &Some(quote.price));
    assert_eq!(supply, 1, "buy must succeed when max_price == actual price");
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
}

#[test]
fn test_buy_reverts_when_max_price_is_one_stroop_below() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    let quote = client.get_buy_quote(&creator);

    let before = capture_snapshot(&client, &creator, &buyer);
    let result = client.try_buy_key(
        &creator,
        &buyer,
        &quote.total_amount,
        &Some(quote.price - 1),
    );
    let after = capture_snapshot(&client, &creator, &buyer);

    assert_eq!(
        result,
        Err(Ok(ContractError::SlippageExceeded)),
        "buy must revert when max_price is one stroop below actual price"
    );
    before.assert_unchanged(&after);
}

// ---------------------------------------------------------------------------
// Sell: min_proceeds boundary
// ---------------------------------------------------------------------------

#[test]
fn test_sell_succeeds_when_min_proceeds_equals_actual_proceeds() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    let buy_quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &holder, &buy_quote.total_amount, &None);

    let sell_quote = client.get_sell_quote(&creator, &holder);

    let supply = client.sell_key(&creator, &holder, &Some(sell_quote.total_amount));
    assert_eq!(
        supply, 0,
        "sell must succeed when min_proceeds == actual proceeds"
    );
}

#[test]
fn test_sell_reverts_when_min_proceeds_is_one_stroop_above() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    let buy_quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &holder, &buy_quote.total_amount, &None);

    let sell_quote = client.get_sell_quote(&creator, &holder);

    let before = capture_snapshot(&client, &creator, &holder);
    let result = client.try_sell_key(&creator, &holder, &Some(sell_quote.total_amount + 1));
    let after = capture_snapshot(&client, &creator, &holder);

    assert_eq!(
        result,
        Err(Ok(ContractError::SlippageExceeded)),
        "sell must revert when min_proceeds is one stroop above actual proceeds"
    );
    before.assert_unchanged(&after);
}
