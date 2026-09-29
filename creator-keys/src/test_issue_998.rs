//! Unit and integration tests for issue #998:
//! Whitelist-gated buy function for early access keys.

use crate::{
    events::{WhitelistUpdatedEvent, WHITELIST_UPDATED_EVENT_NAME},
    ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
    MAX_WHITELIST_BATCH_SIZE,
};
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    vec, Address, Env, String, TryIntoVal, Vec,
};

fn setup_test() -> (Env, CreatorKeysContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_treasury_address(&admin, &treasury);
    client.set_key_price(&admin, &100i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_protocol_fee_recipient(&admin, &treasury);

    (env, client, admin, treasury)
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient, creator: &Address) {
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

#[test]
fn test_set_whitelist_creator_only_and_not_registered() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let attacker = Address::generate(&env);
    let wallet = Address::generate(&env);
    let wallets = vec![&env, wallet.clone()];

    // Unregistered attacker cannot call set_whitelist
    assert_eq!(
        client.try_set_whitelist(&attacker, &wallets),
        Err(Ok(ContractError::NotRegistered))
    );

    // Unregistered attacker cannot call remove_from_whitelist
    assert_eq!(
        client.try_remove_from_whitelist(&attacker, &wallet),
        Err(Ok(ContractError::NotRegistered))
    );

    // Unregistered attacker cannot call disable_whitelist
    assert_eq!(
        client.try_disable_whitelist(&attacker),
        Err(Ok(ContractError::NotRegistered))
    );
}

#[test]
fn test_set_whitelist_batch_limit_100() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    // Test exactly 100 wallets succeeds
    let mut wallets_100 = Vec::new(&env);
    for _ in 0..100 {
        wallets_100.push_back(Address::generate(&env));
    }
    assert_eq!(wallets_100.len(), MAX_WHITELIST_BATCH_SIZE);
    assert_eq!(client.try_set_whitelist(&creator, &wallets_100), Ok(Ok(())));

    // Verify view check is_whitelisted on one of the wallets
    let first_wallet = wallets_100.get(0).unwrap();
    assert!(client.is_whitelisted(&creator, &first_wallet));

    // Test 101 wallets reverts with WhitelistTooLarge
    let mut wallets_101 = Vec::new(&env);
    for _ in 0..101 {
        wallets_101.push_back(Address::generate(&env));
    }
    assert_eq!(
        client.try_set_whitelist(&creator, &wallets_101),
        Err(Ok(ContractError::WhitelistTooLarge))
    );
}

#[test]
fn test_whitelist_gated_buy_and_view_check() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let allowed_wallet = Address::generate(&env);
    let stranger_wallet = Address::generate(&env);

    // Initially neither is whitelisted
    assert!(!client.is_whitelisted(&creator, &allowed_wallet));
    assert!(!client.is_whitelisted(&creator, &stranger_wallet));

    // Creator configures whitelist with allowed_wallet
    let wallets = vec![&env, allowed_wallet.clone()];
    client.set_whitelist(&creator, &wallets);

    // Gate is active, allowed_wallet is whitelisted
    assert!(client.is_whitelisted(&creator, &allowed_wallet));
    assert!(!client.is_whitelisted(&creator, &stranger_wallet));

    // Non-whitelisted wallet is blocked from buying
    assert_eq!(
        client.try_buy(&creator, &stranger_wallet, &1000i128, &None),
        Err(Ok(ContractError::NotWhitelisted))
    );
    assert_eq!(
        client.try_buy_key(&creator, &stranger_wallet, &1000i128, &None),
        Err(Ok(ContractError::NotWhitelisted))
    );

    // Whitelisted wallet can buy using buy or buy_key
    let supply_1 = client.buy(&creator, &allowed_wallet, &1000i128, &None);
    assert_eq!(supply_1, 1);
    assert_eq!(client.get_key_balance(&creator, &allowed_wallet), 1);
}

#[test]
fn test_remove_from_whitelist_blocks_subsequent_buys() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let wallet = Address::generate(&env);
    client.set_whitelist(&creator, &vec![&env, wallet.clone()]);
    assert!(client.is_whitelisted(&creator, &wallet));

    // Buy succeeds while whitelisted
    assert_eq!(client.buy(&creator, &wallet, &1000i128, &None), 1);

    // Creator removes wallet from whitelist
    client.remove_from_whitelist(&creator, &wallet);
    assert!(!client.is_whitelisted(&creator, &wallet));

    // Buy is now blocked
    assert_eq!(
        client.try_buy(&creator, &wallet, &1000i128, &None),
        Err(Ok(ContractError::NotWhitelisted))
    );
}

#[test]
fn test_disable_whitelist_permanently_opens_buys_and_cannot_be_re_enabled() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let whitelisted = Address::generate(&env);
    let non_whitelisted = Address::generate(&env);

    client.set_whitelist(&creator, &vec![&env, whitelisted.clone()]);

    // Non-whitelisted blocked while gate is active
    assert_eq!(
        client.try_buy(&creator, &non_whitelisted, &1000i128, &None),
        Err(Ok(ContractError::NotWhitelisted))
    );

    // Creator disables whitelist
    client.disable_whitelist(&creator);

    // Non-whitelisted wallet can now buy
    assert_eq!(client.buy(&creator, &non_whitelisted, &1000i128, &None), 1);
    // Whitelisted wallet can also buy
    assert_eq!(client.buy(&creator, &whitelisted, &1000i128, &None), 2);

    // Whitelist cannot be re-enabled: enable_whitelist fails
    assert_eq!(
        client.try_enable_whitelist(&creator),
        Err(Ok(ContractError::WhitelistPermanentlyDisabled))
    );

    // set_whitelist fails
    assert_eq!(
        client.try_set_whitelist(&creator, &vec![&env, non_whitelisted.clone()]),
        Err(Ok(ContractError::WhitelistPermanentlyDisabled))
    );

    // add_to_whitelist fails
    assert_eq!(
        client.try_add_to_whitelist(&creator, &non_whitelisted),
        Err(Ok(ContractError::WhitelistPermanentlyDisabled))
    );

    // remove_from_whitelist fails
    assert_eq!(
        client.try_remove_from_whitelist(&creator, &whitelisted),
        Err(Ok(ContractError::WhitelistPermanentlyDisabled))
    );

    // set_early_access_mode fails
    assert_eq!(
        client.try_set_early_access_mode(&creator, &creator, &true),
        Err(Ok(ContractError::WhitelistPermanentlyDisabled))
    );

    // update_whitelist fails
    assert_eq!(
        client.try_update_whitelist(&admin, &creator, &non_whitelisted, &true),
        Err(Ok(ContractError::WhitelistPermanentlyDisabled))
    );

    // Calling disable_whitelist again is idempotent and buys remain open
    assert_eq!(client.try_disable_whitelist(&creator), Ok(Ok(())));
    let another_buyer = Address::generate(&env);
    assert_eq!(client.buy(&creator, &another_buyer, &1000i128, &None), 3);
}

#[test]
fn test_whitelist_updated_events_emitted_on_add_and_remove() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let wallet1 = Address::generate(&env);
    let wallet2 = Address::generate(&env);

    // Batch add via set_whitelist
    client.set_whitelist(&creator, &vec![&env, wallet1.clone(), wallet2.clone()]);

    let contract_id = client.address.clone();
    let all_events = env.events().all();

    // Verify WhitelistUpdatedEvent for wallet1 (allowed: true)
    let has_wallet1_added = all_events.iter().any(|e| {
        if e.0 != contract_id || e.1.len() < 2 {
            return false;
        }
        let topic: Result<soroban_sdk::Symbol, _> = e.1.get(0).unwrap().try_into_val(&env);
        let event_creator: Result<Address, _> = e.1.get(1).unwrap().try_into_val(&env);
        let data: Result<WhitelistUpdatedEvent, _> = e.2.try_into_val(&env);
        match (topic, event_creator, data) {
            (Ok(t), Ok(c), Ok(event)) => {
                t == WHITELIST_UPDATED_EVENT_NAME
                    && c == creator
                    && event.creator == creator
                    && event.wallet == wallet1
                    && event.allowed
            }
            _ => false,
        }
    });
    assert!(
        has_wallet1_added,
        "Expected WhitelistUpdatedEvent for wallet1 add"
    );

    // Verify WhitelistUpdatedEvent for wallet2 (allowed: true)
    let has_wallet2_added = all_events.iter().any(|e| {
        if e.0 != contract_id || e.1.len() < 2 {
            return false;
        }
        let topic: Result<soroban_sdk::Symbol, _> = e.1.get(0).unwrap().try_into_val(&env);
        let event_creator: Result<Address, _> = e.1.get(1).unwrap().try_into_val(&env);
        let data: Result<WhitelistUpdatedEvent, _> = e.2.try_into_val(&env);
        match (topic, event_creator, data) {
            (Ok(t), Ok(c), Ok(event)) => {
                t == WHITELIST_UPDATED_EVENT_NAME
                    && c == creator
                    && event.creator == creator
                    && event.wallet == wallet2
                    && event.allowed
            }
            _ => false,
        }
    });
    assert!(
        has_wallet2_added,
        "Expected WhitelistUpdatedEvent for wallet2 add"
    );

    // Remove wallet1
    client.remove_from_whitelist(&creator, &wallet1);

    let updated_events = env.events().all();
    let has_wallet1_removed = updated_events.iter().any(|e| {
        if e.0 != contract_id || e.1.len() < 2 {
            return false;
        }
        let topic: Result<soroban_sdk::Symbol, _> = e.1.get(0).unwrap().try_into_val(&env);
        let event_creator: Result<Address, _> = e.1.get(1).unwrap().try_into_val(&env);
        let data: Result<WhitelistUpdatedEvent, _> = e.2.try_into_val(&env);
        match (topic, event_creator, data) {
            (Ok(t), Ok(c), Ok(event)) => {
                t == WHITELIST_UPDATED_EVENT_NAME
                    && c == creator
                    && event.creator == creator
                    && event.wallet == wallet1
                    && !event.allowed
            }
            _ => false,
        }
    });
    assert!(
        has_wallet1_removed,
        "Expected WhitelistUpdatedEvent for wallet1 remove"
    );
}

#[test]
fn test_creator_isolation_for_whitelist() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator_a = Address::generate(&env);
    register_creator(&env, &client, &creator_a);

    let creator_b = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator_b.clone(),
            handle: String::from_str(&env, "bob"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let wallet = Address::generate(&env);

    // Whitelist on creator_a only
    client.set_whitelist(&creator_a, &vec![&env, wallet.clone()]);

    assert!(client.is_whitelisted(&creator_a, &wallet));
    assert!(!client.is_whitelisted(&creator_b, &wallet));

    // Disable whitelist on creator_a
    client.disable_whitelist(&creator_a);

    // creator_b can still set whitelist and is not disabled
    client.set_whitelist(&creator_b, &vec![&env, wallet.clone()]);
    assert!(client.is_whitelisted(&creator_b, &wallet));
}
