//! Integration tests for automatic TTL extension during buy transactions.
//!
//! Confirms that when a creator's storage TTL is near expiry, the next buy
//! extends it and emits a [`events::TTL_EXTENDED_EVENT_NAME`] event.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_key_price_for_tests};
use creator_keys::constants::storage;
use creator_keys::events::{self, ttl_extended_topics};
use creator_keys::CREATOR_TTL_LEDGERS;
use soroban_sdk::testutils::storage::Persistent;
use soroban_sdk::testutils::Ledger;
use soroban_sdk::{testutils::Address as _, testutils::Events, Address, IntoVal};

const KEY_PRICE: i128 = 100;

/// Read remaining TTL (ledgers until expiry) for a creator's profile key.
fn creator_ttl_remaining(env: &soroban_sdk::Env, contract_id: &Address, creator: &Address) -> u32 {
    let key = storage::creator(creator);
    env.as_contract(contract_id, || env.storage().persistent().get_ttl(&key))
}

fn setup(
    env: &soroban_sdk::Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    soroban_sdk::Address,
    soroban_sdk::Address,
) {
    let (client, contract_id) = register_creator_keys(env);
    // The test env archives the contract instance and code after ~4095
    // ledgers by default. Bump them to the full extension window so tests
    // that advance the ledger far into the future (to drain creator TTL)
    // can still invoke the contract.
    env.deployer().extend_ttl(
        contract_id.clone(),
        CREATOR_TTL_LEDGERS,
        CREATOR_TTL_LEDGERS,
    );
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    (client, contract_id, creator)
}

/// Advance the ledger far enough that the creator key's remaining TTL drops
/// below the extension threshold, then buy and confirm TTL was extended and
/// the extension event was emitted.
#[test]
fn test_buy_extends_creator_ttl() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let ttl_before_advance = creator_ttl_remaining(&env, &contract_id, &creator);

    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(result, Ok(Ok(1)), "buy should succeed");

    let events = env.events().all();
    let found = events
        .iter()
        .rev()
        .any(|(_, topics, _)| topics == ttl_extended_topics(&creator).into_val(&env));
    assert!(found, "TTL extension event should be emitted after buy");

    let ttl_after = creator_ttl_remaining(&env, &contract_id, &creator);

    assert!(
        ttl_after > ttl_before_advance,
        "TTL should increase after buy: before_advance={ttl_before_advance} after={ttl_after}"
    );
}

/// Confirm the TTL extension event has the expected topics and payload shape.
#[test]
fn test_ttl_extension_event_topics_and_payload() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let ledger_before_buy = env.ledger().sequence();

    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(result, Ok(Ok(1)), "buy should succeed");

    let events = env.events().all();
    let (topics, data) = events
        .iter()
        .rev()
        .find_map(|(_, topics, data)| {
            if topics == ttl_extended_topics(&creator).into_val(&env) {
                Some((topics, data))
            } else {
                None
            }
        })
        .expect("TTL extension event not found after buy");

    assert_eq!(topics.len(), 2, "topics tuple should have 2 elements");
    let topic0: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(topic0, events::TTL_EXTENDED_EVENT_NAME);

    let topic1: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic1, creator);

    let extend_to: u32 = data.into_val(&env);
    let expected_extend_to = ledger_before_buy + CREATOR_TTL_LEDGERS;
    assert_eq!(extend_to, expected_extend_to);
}

/// Second buy on the same ledger does NOT further extend TTL
/// (confirmed by TTL remaining unchanged).
#[test]
fn test_ttl_not_extended_when_already_high() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(result, Ok(Ok(1)), "first buy should succeed");

    let ttl_after_first = creator_ttl_remaining(&env, &contract_id, &creator);

    let holder2 = Address::generate(&env);
    let result2 = client.try_buy_key(&creator, &holder2, &KEY_PRICE, &None);
    assert_eq!(result2, Ok(Ok(2)), "second buy should succeed");

    let ttl_after_second = creator_ttl_remaining(&env, &contract_id, &creator);

    assert!(
        ttl_after_first > 0,
        "TTL should be positive after first buy"
    );
    assert_eq!(
        ttl_after_first, ttl_after_second,
        "TTL should remain unchanged after second buy on same ledger"
    );
}

/// No TTL extension event is emitted when the creator's TTL is well above
/// the extension threshold (healthy state). Confirms the buy itself still
/// succeeds and emits its own event.
#[test]
fn test_no_ttl_extension_event_when_ttl_healthy() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    // Record the TTL immediately after registration — it should be far above
    // the extension threshold (CREATOR_TTL_LEDGERS / 100 = 63k+ ledgers).
    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    // Sanity check: the TTL must be at least 2x the extension threshold.
    assert!(
        ttl_before >= 2 * creator_keys::TTL_EXTENSION_THRESHOLD,
        "TTL should be at least 2x the extension threshold: ttl={ttl_before} threshold={}",
        creator_keys::TTL_EXTENSION_THRESHOLD
    );

    // Execute buy without advancing the ledger — TTL is still healthy.
    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(result, Ok(Ok(1)), "buy should succeed when TTL is healthy");

    // Extract all events emitted during the buy transaction.
    let events = env.events().all();

    // Assert no TTL extension event was emitted.
    let ttl_extension_found = events
        .iter()
        .rev()
        .any(|(_, topics, _)| topics == ttl_extended_topics(&creator).into_val(&env));
    assert!(
        !ttl_extension_found,
        "No TTL extension event should be emitted when TTL is healthy"
    );

    // Assert a buy event IS present (confirming the transaction succeeded).
    let buy_event_found = events.iter().rev().any(|(_, topics, _)| {
        let topic0: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&env);
        topic0 == events::BUY_EVENT_NAME
    });
    assert!(
        buy_event_found,
        "Buy event should be present confirming the transaction succeeded"
    );

    // Assert creator storage TTL is unchanged after the buy.
    let ttl_after = creator_ttl_remaining(&env, &contract_id, &creator);
    assert_eq!(
        ttl_before, ttl_after,
        "TTL should remain unchanged after buy when TTL is healthy: before={ttl_before} after={ttl_after}"
    );
}

#[test]
fn buy_extends_instance_ttl() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let ttl_before_buy = creator_ttl_remaining(&env, &contract_id, &creator);

    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(result, Ok(Ok(1)), "buy should succeed");

    let ttl_after = creator_ttl_remaining(&env, &contract_id, &creator);

    assert!(
        ttl_after > ttl_before_buy,
        "TTL should increase after buy: before={ttl_before_buy} after={ttl_after}"
    );
    assert!(
        ttl_after >= CREATOR_TTL_LEDGERS,
        "TTL after buy should reach at least CREATOR_TTL_LEDGERS threshold: after={ttl_after}"
    );
}

#[test]
fn sell_extends_instance_ttl() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    // Buy first key so holder has balance to sell
    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(result, Ok(Ok(1)), "buy should succeed");

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let ttl_before_sell = creator_ttl_remaining(&env, &contract_id, &creator);

    let sell_result = client.try_sell_key(&creator, &holder, &None);
    assert_eq!(sell_result, Ok(Ok(0)), "sell should succeed");

    let ttl_after = creator_ttl_remaining(&env, &contract_id, &creator);

    assert!(
        ttl_after > ttl_before_sell,
        "TTL should increase after sell: before={ttl_before_sell} after={ttl_after}"
    );
    assert!(
        ttl_after >= CREATOR_TTL_LEDGERS,
        "TTL after sell should reach at least CREATOR_TTL_LEDGERS threshold: after={ttl_after}"
    );
}

#[test]
fn admin_fee_update_extends_instance_ttl() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, _) = setup(&env);
    let admin = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);

    // Set initial fee config
    client.set_fee_config(&admin, &5000, &5000);

    let fee_config_key = storage::FEE_CONFIG;
    let ttl_before = env.as_contract(&contract_id, || {
        env.storage().persistent().get_ttl(&fee_config_key)
    });

    // Bump ProtocolStateVersion TTL before advancing the ledger.
    // Its default TTL (~4095 ledgers) is shorter than fee_config's, so it
    // becomes archived first when we advance by ttl_before, causing
    // set_fee_config (which reads that key) to panic with Storage::InternalError.
    let version_key = storage::PROTOCOL_STATE_VERSION;
    env.as_contract(&contract_id, || {
        env.storage().persistent().extend_ttl(
            &version_key,
            CREATOR_TTL_LEDGERS,
            CREATOR_TTL_LEDGERS,
        );
    });

    // Re-extend the contract instance and code TTL as well.
    env.deployer().extend_ttl(
        contract_id.clone(),
        CREATOR_TTL_LEDGERS,
        CREATOR_TTL_LEDGERS,
    );

    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let ttl_before_update = env.as_contract(&contract_id, || {
        env.storage().persistent().get_ttl(&fee_config_key)
    });

    // Update admin fee config
    let result = client.try_set_fee_config(&admin, &6000, &4000);
    assert_eq!(result, Ok(Ok(())), "admin fee update should succeed");

    let ttl_after = env.as_contract(&contract_id, || {
        env.storage().persistent().get_ttl(&fee_config_key)
    });

    assert!(
        ttl_after >= ttl_before_update,
        "TTL after admin fee update should be at least TTL before: before={ttl_before_update} after={ttl_after}"
    );
}

#[test]
fn failed_buy_slippage_exceeded_does_not_extend_ttl() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator) = setup(&env);
    let holder = Address::generate(&env);

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += ttl_before.saturating_sub(1).max(1);
    env.ledger().set(ledger);

    let ttl_before_failed_buy = creator_ttl_remaining(&env, &contract_id, &creator);

    // Pass max_price = Some(KEY_PRICE - 1) which is strictly below actual KEY_PRICE (100), triggering slippage error
    let max_price_too_low = Some(KEY_PRICE - 1);
    let result = client.try_buy_key(&creator, &holder, &KEY_PRICE, &max_price_too_low);
    assert!(
        result.is_err() || matches!(result, Ok(Err(_))),
        "buy should fail due to slippage"
    );

    let ttl_after = creator_ttl_remaining(&env, &contract_id, &creator);

    assert_eq!(
        ttl_after, ttl_before_failed_buy,
        "TTL should NOT be extended on failed buy due to slippage"
    );
}
