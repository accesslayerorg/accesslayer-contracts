//! Regression test: holder count unchanged after a failed buy due to supply cap exceeded.
//!
//! When a buy reverts because it would exceed the creator's supply cap, the holder
//! count must remain unchanged. A new wallet that did not previously hold keys must
//! not be added to the holder count just because a buy attempt was made and reverted.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_key_price_for_tests, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_holder_count_unchanged_after_failed_buy_supply_cap_exceeded() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100_i128);

    // Register creator with a supply cap of 10.
    let creator = Address::generate(&env);
    client.register_creator(
        &creator,
        &String::from_str(&env, "alice"),
        &None,
        &Some(10u32),
        &None,
        &None,
    );

    // First wallet buys 10 keys to fill the cap.
    let first_buyer = Address::generate(&env);
    for _ in 0..10 {
        client.buy_key(&creator, &first_buyer, &100_i128, &None);
    }

    assert_eq!(
        client.get_creator_holder_count(&creator),
        1,
        "setup: holder count should be 1 after first wallet fills the cap"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        10,
        "setup: supply should be 10 after filling the cap"
    );

    // Second wallet attempts to buy — supply cap is already reached.
    let second_buyer = Address::generate(&env);
    let result = client.try_buy_key(&creator, &second_buyer, &100_i128, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::SupplyCapExceeded)),
        "buy must fail with SupplyCapExceeded when supply cap is reached"
    );

    // Holder count must remain unchanged after the failed buy.
    assert_eq!(
        client.get_creator_holder_count(&creator),
        1,
        "holder count must remain 1 after failed buy due to supply cap"
    );

    // Second wallet must not be added to holders.
    assert_eq!(
        client.get_key_balance(&creator, &second_buyer),
        0,
        "second wallet must have zero balance after failed buy"
    );

    // Total supply must remain unchanged.
    assert_eq!(
        client.get_total_key_supply(&creator),
        10,
        "total supply must remain 10 after failed buy"
    );
}
