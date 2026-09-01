//! Integration tests for persistent storage TTL maintenance (issue #749).
//!
//! Every read and write on persistent entries keeps the entry live for at
//! least [`TTL_MIN_EXTENSION_LEDGERS`] (~30 days), and the admin-only
//! `refresh_ttl` entrypoint re-extends all known global entries plus the
//! scoped entries of the supplied creators in one call.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::constants::storage;
use creator_keys::{ContractError, TTL_MIN_EXTENSION_LEDGERS};
use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _, Ledger},
    Address, Env, Vec,
};

const KEY_PRICE: i128 = 100;

/// Extends the contract instance and code so far-future ledger advances do
/// not archive them mid-test.
fn extend_contract_lifetime(env: &Env, contract_id: &Address) {
    let horizon = creator_keys::CREATOR_TTL_LEDGERS;
    env.deployer()
        .extend_ttl(contract_id.clone(), horizon, horizon);
}

fn key_ttl(env: &Env, contract_id: &Address, key: &creator_keys::DataKey) -> u32 {
    env.as_contract(contract_id, || env.storage().persistent().get_ttl(key))
}

fn advance_ledger(env: &Env, ledgers: u32) {
    let mut ledger = env.ledger().get();
    ledger.sequence_number += ledgers;
    env.ledger().set(ledger);
}

#[test]
fn test_buy_bumps_price_entry_ttl_by_at_least_thirty_days() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    extend_contract_lifetime(&env, &contract_id);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    // Drain every entry close to expiry so only fresh writes keep state live.
    advance_ledger(&env, creator_keys::CREATOR_TTL_LEDGERS - 100);
    let price_ttl_before = key_ttl(&env, &contract_id, &storage::KEY_PRICE);
    assert!(
        price_ttl_before < TTL_MIN_EXTENSION_LEDGERS,
        "precondition: price entry is close to expiry"
    );

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    let price_ttl_after = key_ttl(&env, &contract_id, &storage::KEY_PRICE);
    assert!(
        price_ttl_after >= TTL_MIN_EXTENSION_LEDGERS,
        "the buy's read of KEY_PRICE must bump its TTL to at least 30 days: \
         before={price_ttl_before} after={price_ttl_after}"
    );
}

#[test]
fn test_sell_bumps_price_and_balance_entry_ttl() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    extend_contract_lifetime(&env, &contract_id);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    // Drain the balance and price entries below the 30-day floor.
    advance_ledger(&env, creator_keys::CREATOR_TTL_LEDGERS - 100);

    client.sell_key(&creator, &buyer, &None);

    assert!(
        key_ttl(&env, &contract_id, &storage::KEY_PRICE) >= TTL_MIN_EXTENSION_LEDGERS,
        "the sell must keep the price entry live for at least 30 days"
    );
    let balance_key = storage::key_balance(&creator, &buyer);
    assert!(
        key_ttl(&env, &contract_id, &balance_key) >= TTL_MIN_EXTENSION_LEDGERS,
        "the partial-sell balance write must be extended past 30 days"
    );
}

#[test]
fn test_admin_config_update_bumps_fee_config_ttl() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    extend_contract_lifetime(&env, &contract_id);
    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);

    let version_key = storage::PROTOCOL_STATE_VERSION;
    env.as_contract(&contract_id, || {
        env.storage().persistent().extend_ttl(
            &version_key,
            creator_keys::CREATOR_TTL_LEDGERS,
            creator_keys::CREATOR_TTL_LEDGERS,
        );
    });

    // Drain the fee config entry close to expiry.
    advance_ledger(&env, creator_keys::CREATOR_TTL_LEDGERS - 100);
    let fee_ttl_before = key_ttl(&env, &contract_id, &storage::FEE_CONFIG);
    assert!(fee_ttl_before < TTL_MIN_EXTENSION_LEDGERS);

    client.set_fee_config(&admin, &8000, &2000);

    let fee_ttl_after = key_ttl(&env, &contract_id, &storage::FEE_CONFIG);
    assert!(
        fee_ttl_after >= TTL_MIN_EXTENSION_LEDGERS,
        "the admin config write must extend the fee config TTL past 30 days: \
         before={fee_ttl_before} after={fee_ttl_after}"
    );
}

#[test]
fn test_refresh_ttl_extends_all_known_global_entries() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    extend_contract_lifetime(&env, &contract_id);
    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    let treasury = Address::generate(&env);
    client.set_protocol_fee(&admin, &None, &treasury);

    // A trade so treasury/creator entries exist before the drain.
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    let version_key = storage::PROTOCOL_STATE_VERSION;
    env.as_contract(&contract_id, || {
        if env.storage().persistent().has(&version_key) {
            env.storage().persistent().extend_ttl(
                &version_key,
                creator_keys::CREATOR_TTL_LEDGERS,
                creator_keys::CREATOR_TTL_LEDGERS,
            );
        }
    });

    advance_ledger(&env, creator_keys::CREATOR_TTL_LEDGERS - 100);

    let creators = Vec::from_array(&env, [creator.clone()]);
    client.refresh_ttl(&admin, &creators);

    for key in [
        storage::FEE_CONFIG,
        storage::KEY_PRICE,
        storage::TREASURY_ADDRESS,
        storage::TREASURY_BALANCE,
        storage::PROTOCOL_FEE_BPS,
        storage::creator(&creator),
    ] {
        let ttl = key_ttl(&env, &contract_id, &key);
        assert!(
            ttl >= TTL_MIN_EXTENSION_LEDGERS,
            "refresh_ttl must restore {ttl:?} to at least 30 days"
        );
    }
}

#[test]
fn test_refresh_ttl_rejects_non_admin_callers() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let impostor = Address::generate(&env);
    let creators = Vec::new(&env);

    let result = client.try_refresh_ttl(&impostor, &creators);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}
