//! Integration test for full sell removing holder balance entry from persistent storage (#613).

mod contract_test_env;

use contract_test_env::{
    assert_storage_absent, register_creator_keys, register_test_creator, set_key_price_for_tests,
};
use creator_keys::constants;
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 100;

fn setup(env: &soroban_sdk::Env) -> (creator_keys::CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

#[test]
fn test_full_sell_removes_holder_balance_storage_key() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    // 1. Set up a holder with exactly 3 keys via buy transactions
    for _ in 0..3 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }
    assert_eq!(client.get_key_balance(&creator, &holder), 3);
    assert_eq!(client.get_total_key_supply(&creator), 3);

    // Verify key exists in persistent storage before full sell
    let balance_key = constants::storage::holder_balance_key(&creator, &holder);
    let contract_id = client.address.clone();
    env.as_contract(&contract_id, || {
        assert!(env.storage().persistent().has(&balance_key));
    });

    // 2. Sell all 3 keys in a single session / transactions
    for _ in 0..3 {
        client.sell_key(&creator, &holder, &None);
    }

    // 3. Assert holder balance storage key is absent from persistent storage (not present with value 0)
    env.as_contract(&contract_id, || {
        assert_storage_absent(&env, &balance_key);
    });

    // 4. Assert creator supply is decremented by the full sold quantity (3 -> 0)
    assert_eq!(client.get_total_key_supply(&creator), 0);

    // 5. Assert a subsequent read of holder balance returns 0 (default) without error
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
}

#[test]
fn test_partial_sell_does_not_remove_holder_balance_storage_key() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    // Set up a holder with 3 keys
    for _ in 0..3 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }

    // Perform a partial sell of 2 keys
    client.sell_key(&creator, &holder, &None);
    client.sell_key(&creator, &holder, &None);

    // Assert key is still present in storage
    let balance_key = constants::storage::holder_balance_key(&creator, &holder);
    let contract_id = client.address.clone();
    env.as_contract(&contract_id, || {
        assert!(env.storage().persistent().has(&balance_key));
    });

    assert_eq!(client.get_key_balance(&creator, &holder), 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);
}
