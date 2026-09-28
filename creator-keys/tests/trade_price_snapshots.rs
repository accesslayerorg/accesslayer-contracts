//! Tests for price snapshots recorded with each trade and age-based
//! retention pruning of those snapshots (TWAP input data).

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_ledger_sequence,
    test_env_with_auths,
};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

const KEY_PRICE: i128 = 100;

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = set_key_price_for_tests(env, &client, KEY_PRICE);
    client.set_protocol_admin(&admin, &admin);
    let creator = register_test_creator(env, &client, "alice");
    (client, admin, creator)
}

#[test]
fn test_each_trade_records_a_snapshot() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);
    assert_eq!(client.get_price_snapshot_count(&creator), 0);

    set_ledger_sequence(&env, 100);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(client.get_price_snapshot_count(&creator), 1);

    set_ledger_sequence(&env, 110);
    client.buy_key(&creator, &buyer, &(KEY_PRICE * 10), &None);
    assert_eq!(client.get_price_snapshot_count(&creator), 2);

    set_ledger_sequence(&env, 120);
    client.sell_key(&creator, &buyer, &None);
    assert_eq!(client.get_price_snapshot_count(&creator), 3);
}

#[test]
fn test_retention_prunes_snapshots_older_than_configured_age() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let buyer = Address::generate(&env);
    assert_eq!(client.get_price_retention(), 0);
    client.set_price_retention(&admin, &50);
    assert_eq!(client.get_price_retention(), 50);

    set_ledger_sequence(&env, 100);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    set_ledger_sequence(&env, 120);
    client.buy_key(&creator, &buyer, &(KEY_PRICE * 10), &None);
    assert_eq!(client.get_price_snapshot_count(&creator), 2);

    // Ledger 100 and 120 are both older than 50 ledgers at ledger 200, so only
    // the fresh snapshot recorded by this trade remains.
    set_ledger_sequence(&env, 200);
    client.buy_key(&creator, &buyer, &(KEY_PRICE * 10), &None);
    assert_eq!(client.get_price_snapshot_count(&creator), 1);
}

#[test]
fn test_without_retention_snapshots_are_kept() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    set_ledger_sequence(&env, 100);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    set_ledger_sequence(&env, 1_000);
    client.buy_key(&creator, &buyer, &(KEY_PRICE * 10), &None);
    assert_eq!(client.get_price_snapshot_count(&creator), 2);
}

#[test]
fn test_set_price_retention_requires_admin() {
    let env = test_env_with_auths();
    let (client, _admin, _creator) = setup(&env);
    let outsider = Address::generate(&env);

    assert_eq!(
        client.try_set_price_retention(&outsider, &50),
        Err(Ok(ContractError::Unauthorized))
    );
}
