//! Regression test for peer-to-peer key transfers preserving supply and quotes.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_transfer_keys_keeps_supply_and_same_level_quotes_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let buy_quote = client.get_buy_quote(&creator);

    client.buy_key(&creator, &sender, &buy_quote.total_amount, &None);
    client.buy_key(&creator, &sender, &buy_quote.total_amount, &None);

    let supply_before = client.get_total_key_supply(&creator);
    let buy_quote_before = client.get_buy_quote(&creator);
    let sell_quote_before = client.get_sell_quote(&creator, &sender);

    client.transfer_keys(&creator, &sender, &recipient, &1);

    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "peer-to-peer transfer must not change total supply"
    );
    assert_eq!(
        client.get_buy_quote(&creator),
        buy_quote_before,
        "buy quote at the same supply level must be unchanged"
    );
    assert_eq!(
        client.get_sell_quote(&creator, &sender),
        sell_quote_before,
        "sell quote at the same supply level must be unchanged"
    );
    assert_eq!(client.get_key_balance(&creator, &sender), 1);
    assert_eq!(client.get_key_balance(&creator, &recipient), 1);
}
