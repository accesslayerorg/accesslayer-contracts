//! Regression test for holder count across multiple distinct buyers.
//!
//! Existing tests cover the count after a single buyer and after the last holder
//! exits. This covers the in-between state: several distinct addresses each
//! holding keys simultaneously, confirming the count tracks unique holders rather
//! than total key amount, and decrements correctly as each buyer fully exits.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_key_price_for_tests, test_env_with_auths};
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn holder_count_tracks_distinct_buyers_and_decrements_on_exit() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);
    let _admin = set_key_price_for_tests(&env, &client, 100);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator,
        &String::from_str(&env, "creator"),
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);
    let buyer_c = Address::generate(&env);

    client.buy_key(&creator, &buyer_a, &100i128, &None);
    client.buy_key(&creator, &buyer_b, &100i128, &None);
    client.buy_key(&creator, &buyer_c, &100i128, &None);

    // Three distinct holders, each with one key.
    assert_eq!(client.get_creator_holder_count(&creator), 3);
    assert_eq!(client.get_creator_supply(&creator), 3);

    // Count decrements by one as each buyer fully exits.
    client.sell_key(&creator, &buyer_a, &None);
    assert_eq!(client.get_creator_holder_count(&creator), 2);

    client.sell_key(&creator, &buyer_b, &None);
    assert_eq!(client.get_creator_holder_count(&creator), 1);

    client.sell_key(&creator, &buyer_c, &None);
    assert_eq!(client.get_creator_holder_count(&creator), 0);
    assert_eq!(client.get_creator_supply(&creator), 0);
}
