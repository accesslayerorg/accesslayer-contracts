//! Integration tests for buy operations correctly incrementing/updating the holder count.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address,
};

#[test]
fn test_buy_holder_count_behavior() {
    let env = test_env_with_auths();
    let (client, _contract_id) = register_creator_keys(&env);
    let base_price = 100i128;
    set_key_price_for_tests(&env, &client, base_price);

    let creator = register_test_creator(&env, &client, "alice");

    // 1. First buy increments holder count by 1 (and matches expected contract state)
    let buyer1 = Address::generate(&env);
    // Buy 1 key (with sufficient payment)
    client.buy_key(&creator, &buyer1, &base_price, &None);

    assert_eq!(client.get_creator_holder_count(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &buyer1), 1);
    assert_eq!(client.get_creator(&creator).holder_count, 1);

    // 2. Repeat buy by same wallet does not increment count
    client.buy_key(&creator, &buyer1, &base_price, &None);

    assert_eq!(client.get_creator_holder_count(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &buyer1), 2);
    assert_eq!(client.get_creator(&creator).holder_count, 1);

    // 3. Two new wallets buying increments count by 2 (total +2)
    let buyer2 = Address::generate(&env);
    let buyer3 = Address::generate(&env);

    client.buy_key(&creator, &buyer2, &base_price, &None);
    assert_eq!(client.get_creator_holder_count(&creator), 2);
    assert_eq!(client.get_creator(&creator).holder_count, 2);

    client.buy_key(&creator, &buyer3, &base_price, &None);
    assert_eq!(client.get_creator_holder_count(&creator), 3);
    assert_eq!(client.get_creator(&creator).holder_count, 3);

    // 4. Selling all keys and rebuying increments count again (wallet re-enters as a holder)
    // Sell first key of buyer3
    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.sell_key(&creator, &buyer3, &None);
    // buyer3 has 0 keys left, holder count decrements
    assert_eq!(client.get_key_balance(&creator, &buyer3), 0);
    assert_eq!(client.get_creator_holder_count(&creator), 2);
    assert_eq!(client.get_creator(&creator).holder_count, 2);

    // Rebuy by buyer3 increments holder count back to 3
    client.buy_key(&creator, &buyer3, &base_price, &None);
    assert_eq!(client.get_creator_holder_count(&creator), 3);
    assert_eq!(client.get_creator(&creator).holder_count, 3);
}
