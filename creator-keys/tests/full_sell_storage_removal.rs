//! Integration tests for a full sell removing the holder balance entry from
//! persistent storage (Issue #613).
//!
//! When a holder sells all of their keys for a creator, the corresponding
//! `KeyBalance(creator, holder)` entry must be removed entirely from
//! persistent storage rather than left behind with a value of zero, to avoid
//! accumulating empty records and unnecessary storage fees.

mod contract_test_env;

use contract_test_env::{
    assert_storage_absent, register_creator_keys, register_test_creator, set_key_price_for_tests,
};
use creator_keys::constants;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

const KEY_PRICE: i128 = 250;
const HOLDER_KEYS: u32 = 3;

fn setup_holder_with_full_balance(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    handle: &str,
) -> (Address, Address) {
    let creator = register_test_creator(env, client, handle);
    let holder = Address::generate(env);

    for _ in 0..HOLDER_KEYS {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }
    assert_eq!(client.get_key_balance(&creator, &holder), HOLDER_KEYS);

    (creator, holder)
}

#[test]
fn test_full_sell_removes_holder_balance_storage_key() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let (creator, holder) = setup_holder_with_full_balance(&env, &client, "alice");

    for _ in 0..HOLDER_KEYS {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &holder, &None);
    }

    env.as_contract(&contract_id, || {
        assert_storage_absent(
            &env,
            &constants::storage::holder_balance_key(&creator, &holder),
        );
    });
}

#[test]
fn test_full_sell_decrements_creator_supply_by_full_sold_quantity() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let (creator, holder) = setup_holder_with_full_balance(&env, &client, "bob");

    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(supply_before, HOLDER_KEYS);

    for _ in 0..HOLDER_KEYS {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &holder, &None);
    }

    let supply_after = client.get_total_key_supply(&creator);
    assert_eq!(
        supply_after,
        supply_before - HOLDER_KEYS,
        "creator supply must decrease by the full sold quantity"
    );
    assert_eq!(supply_after, 0);
}

#[test]
fn test_balance_read_after_full_sell_returns_zero_without_error() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let (creator, holder) = setup_holder_with_full_balance(&env, &client, "carol");

    for _ in 0..HOLDER_KEYS {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &holder, &None);
    }

    assert_eq!(
        client.get_key_balance(&creator, &holder),
        0,
        "balance read after full sell must return 0 by default, even though \
         the underlying storage key is absent"
    );
}

#[test]
fn test_partial_sell_does_not_remove_holder_balance_storage_key() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let (creator, holder) = setup_holder_with_full_balance(&env, &client, "dave");

    // Sell fewer than the full balance.
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &holder, &None);
    assert_eq!(client.get_key_balance(&creator, &holder), HOLDER_KEYS - 1);

    env.as_contract(&contract_id, || {
        assert!(
            env.storage()
                .persistent()
                .has(&constants::storage::holder_balance_key(&creator, &holder)),
            "partial sell must not remove the holder balance storage key"
        );
    });
}
