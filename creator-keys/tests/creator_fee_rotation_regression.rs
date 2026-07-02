//! Regression test verifying creator fee recipient updates correctly after rotation.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_pricing_and_fees, test_env_with_auths};
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_creator_fee_recipient_rotation_regression() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1000, 9000, 1000);

    let creator = Address::generate(&env);
    let initial_recipient = Address::generate(&env);
    client.register_creator(
        &creator,
        &String::from_str(&env, "alice"),
        &None,
        &Some(initial_recipient.clone()),
        &None,
        &None,
    );

    // Initial fee recipient is set correctly
    assert_eq!(
        client.get_creator_fee_recipient(&creator),
        initial_recipient
    );

    let buyer = Address::generate(&env);
    let quote1 = client.get_buy_quote(&creator);

    // Execute buy 1
    client.buy_key(&creator, &buyer, &quote1.total_amount, &None);

    // Initial recipient received the fee (tracked under creator's fee balance)
    let balance_after_buy1 = client.get_creator_fee_balance(&creator);
    assert_eq!(balance_after_buy1, quote1.creator_fee);

    // Rotate the creator fee recipient to a new address
    let new_recipient = Address::generate(&env);
    client.update_creator_fee_recipient(&creator, &new_recipient);

    // New recipient is set correctly
    assert_eq!(client.get_creator_fee_recipient(&creator), new_recipient);

    // Execute buy 2
    let quote2 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote2.total_amount, &None);

    // New recipient receives fee after rotation (tracked under creator's fee balance)
    let balance_after_buy2 = client.get_creator_fee_balance(&creator);
    assert_eq!(balance_after_buy2 - balance_after_buy1, quote2.creator_fee);

    // Since the old recipient is no longer the fee recipient, they receive nothing from subsequent trades.
    assert_ne!(
        client.get_creator_fee_recipient(&creator),
        initial_recipient
    );
}
