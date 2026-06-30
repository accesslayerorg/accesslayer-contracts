mod contract_test_env;

use contract_test_env::{
    compute_expected_buy_price, register_creator_keys, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::{ContractError, WhitelistConfig, MAX_WHITELIST_SIZE};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    vec, Address, String, Vec,
};

fn register_whitelisted_creator(
    env: &soroban_sdk::Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    whitelist: Vec<Address>,
    window_ledgers: u32,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(
        &creator,
        &String::from_str(env, "alice"),
        &None,
        &None,
        &None,
        &Some(WhitelistConfig {
            addresses: whitelist,
            window_ledgers,
        }),
    );
    creator
}

fn advance_ledgers(env: &soroban_sdk::Env, ledgers: u32) {
    let mut ledger = env.ledger().get();
    ledger.sequence_number += ledgers;
    env.ledger().set(ledger);
}

#[test]
fn test_non_whitelisted_wallet_cannot_buy_during_window() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);
    let approved = Address::generate(&env);
    let creator = register_whitelisted_creator(&env, &client, vec![&env, approved], 10);
    let buyer = Address::generate(&env);

    let result = client.try_buy_key(&creator, &buyer, &100, &None);

    assert_eq!(result, Err(Ok(ContractError::WhitelistOnly)));
    assert_eq!(client.get_total_key_supply(&creator), 0);
}

#[test]
fn test_whitelisted_wallet_can_buy_during_window() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);
    let buyer = Address::generate(&env);
    let creator = register_whitelisted_creator(&env, &client, vec![&env, buyer.clone()], 10);

    let supply = client.buy_key(&creator, &buyer, &100, &None);

    assert_eq!(supply, 1);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
}

#[test]
fn test_anyone_can_buy_after_window_expires() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);
    let approved = Address::generate(&env);
    let creator = register_whitelisted_creator(&env, &client, vec![&env, approved], 5);
    advance_ledgers(&env, 5);
    let public_buyer = Address::generate(&env);

    let supply = client.buy_key(&creator, &public_buyer, &100, &None);

    assert_eq!(supply, 1);
}

#[test]
fn test_get_whitelist_status_tracks_active_and_expired_state() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let buyer = Address::generate(&env);
    let registered_at = env.ledger().sequence();
    let creator = register_whitelisted_creator(&env, &client, vec![&env, buyer], 7);

    let active = client.get_whitelist_status(&creator);
    assert!(active.active);
    assert_eq!(active.expires_at_ledger, registered_at + 7);
    assert_eq!(active.remaining_ledgers, 7);

    advance_ledgers(&env, 7);
    let expired = client.get_whitelist_status(&creator);
    assert!(!expired.active);
    assert_eq!(expired.expires_at_ledger, registered_at + 7);
    assert_eq!(expired.remaining_ledgers, 0);
}

#[test]
fn test_whitelist_over_500_addresses_reverts_at_registration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = Address::generate(&env);
    let mut addresses = Vec::new(&env);
    for _ in 0..=MAX_WHITELIST_SIZE {
        addresses.push_back(Address::generate(&env));
    }

    let result = client.try_register_creator(
        &creator,
        &String::from_str(&env, "alice"),
        &None,
        &None,
        &None,
        &Some(WhitelistConfig {
            addresses,
            window_ledgers: 10,
        }),
    );

    assert_eq!(result, Err(Ok(ContractError::WhitelistTooLarge)));
    assert!(!client.is_creator_registered(&creator));
}

#[test]
fn test_none_whitelist_allows_public_buy_immediately() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100);
    let creator = Address::generate(&env);
    client.register_creator(
        &creator,
        &String::from_str(&env, "alice"),
        &None,
        &None,
        &None,
        &None,
    );
    let buyer = Address::generate(&env);

    let quote = compute_expected_buy_price(0, 100);
    let supply = client.buy_key(&creator, &buyer, &quote, &None);
    let status = client.get_whitelist_status(&creator);

    assert_eq!(supply, 1);
    assert!(!status.active);
    assert_eq!(status.remaining_ledgers, 0);
}
