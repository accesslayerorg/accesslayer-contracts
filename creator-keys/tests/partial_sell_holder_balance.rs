//! Integration test for holder balance correctly decremented after a partial sell (#600).
//!
//! Confirms that when a holder sells fewer keys than their total balance,
//! the remaining balance reflects the exact difference and the creator supply
//! decreases by the same amount.

mod contract_test_env;

use contract_test_env::{
    assert_storage_absent, register_creator_keys, register_test_creator, set_key_price_for_tests,
};
use creator_keys::constants;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address};

const KEY_PRICE: i128 = 100;

fn setup(env: &soroban_sdk::Env) -> (creator_keys::CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

#[test]
fn test_partial_sell_decrements_holder_balance_by_sold_quantity() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    // Buy 5 keys
    for _ in 0..5 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }
    assert_eq!(client.get_key_balance(&creator, &holder), 5);
    assert_eq!(client.get_total_key_supply(&creator), 5);

    // Sell 2 keys
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &holder, &None);
    client.sell_key(&creator, &holder, &None);

    assert_eq!(
        client.get_key_balance(&creator, &holder),
        3,
        "holder balance should be 3 after selling 2 of 5 keys"
    );
}

#[test]
fn test_partial_sell_decrements_creator_supply_by_sold_quantity() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    // Buy 5 keys
    for _ in 0..5 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }
    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(supply_before, 5);

    // Sell 2 keys
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &holder, &None);
    client.sell_key(&creator, &holder, &None);

    let supply_after = client.get_total_key_supply(&creator);
    assert_eq!(
        supply_after,
        supply_before - 2,
        "creator supply should decrease by 2 after selling 2 keys"
    );
    assert_eq!(supply_after, 3);
}

#[test]
fn test_two_sequential_partial_sells_each_produce_correct_balance() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    // Buy 5 keys
    for _ in 0..5 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }
    assert_eq!(client.get_key_balance(&creator, &holder), 5);
    assert_eq!(client.get_total_key_supply(&creator), 5);

    // First partial sell: sell 2 keys
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &holder, &None);
    client.sell_key(&creator, &holder, &None);

    assert_eq!(
        client.get_key_balance(&creator, &holder),
        3,
        "after first partial sell (2 keys), balance should be 3"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        3,
        "after first partial sell (2 keys), supply should be 3"
    );

    // Second partial sell: sell 1 key
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &holder, &None);

    assert_eq!(
        client.get_key_balance(&creator, &holder),
        2,
        "after second partial sell (1 key), balance should be 2"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        2,
        "after second partial sell (1 key), supply should be 2"
    );
}

#[test]
fn test_holder_entry_not_removed_after_partial_sell() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    // Buy 5 keys
    for _ in 0..5 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }
    let holder_count_before = client.get_creator_holder_count(&creator);
    assert_eq!(holder_count_before, 1);

    // Partial sell: sell 3 keys (leaving 2)
    for _ in 0..3 {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &holder, &None);
    }

    // Holder entry should still exist
    assert_eq!(
        client.get_key_balance(&creator, &holder),
        2,
        "holder should still have 2 keys after partial sell"
    );
    assert_eq!(
        client.get_creator_holder_count(&creator),
        1,
        "holder count should still be 1 after partial sell (entry not removed)"
    );
}

#[test]
fn test_full_sell_removes_holder_entry() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    // Buy 5 keys
    for _ in 0..5 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }

    // Full sell: sell all 5 keys
    for _ in 0..5 {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &holder, &None);
    }

    assert_eq!(
        client.get_key_balance(&creator, &holder),
        0,
        "holder should have 0 keys after full sell"
    );
    assert_eq!(
        client.get_creator_holder_count(&creator),
        0,
        "holder count should be 0 after full sell (entry removed)"
    );

    // Verify the underlying storage key has been removed.
    env.as_contract(&contract_id, || {
        assert_storage_absent(
            &env,
            &constants::storage::holder_balance_key(&creator, &holder),
        );
    });
}

#[test]
fn test_supply_matches_balance_after_partial_sell() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    // Buy 5 keys
    for _ in 0..5 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }

    // Sell 2 keys
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &holder, &None);
    client.sell_key(&creator, &holder, &None);

    let balance = client.get_key_balance(&creator, &holder);
    let supply = client.get_total_key_supply(&creator);
    assert_eq!(
        balance, supply,
        "supply ({supply}) should equal holder balance ({balance}) when there is only one holder"
    );
    assert_eq!(balance, 3);
}
