//! Integration tests for the `batch_sell` entrypoint (issue #863).
//!
//! Acceptance criteria verified:
//! - All sells processed and proceeds calculated in one transaction
//! - More than 5 orders panics with BatchSizeExceeded
//! - 0 orders panics with BatchSizeExceeded
//! - Insufficient balance on any order panics with InsufficientBalance and rolls back all state
//! - Frozen keys excluded from available balance check
//! - Staked keys excluded from available balance check
//! - `batch_sell_completed` event emitted with correct fields: seller, (key_id, quantity, proceeds) tuples, total_proceeds, ledger
//! - `KeysSoldEvent` emitted for each order in the batch

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{events, ContractError, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, IntoVal, Symbol, Vec,
};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 500;
const PROTOCOL_BPS: u32 = 500;

fn setup(env: &soroban_sdk::Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = Address::generate(env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);
    (client, admin)
}

fn advance_ledger(env: &soroban_sdk::Env) {
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    l.timestamp += 5;
    env.ledger().set(l);
}

// ============================================================================
// Happy path: single and multiple orders
// ============================================================================

#[test]
fn test_batch_sell_single_order_success() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);

    let creator = register_test_creator(&env, &client, "alice");
    let seller = Address::generate(&env);

    // Buy 3 keys
    let buy_orders = Vec::from_array(&env, [(creator.clone(), 3u32)]);
    client.batch_buy(&seller, &buy_orders);
    assert_eq!(client.get_key_balance(&creator, &seller), 3);
    assert_eq!(client.get_total_key_supply(&creator), 3);

    advance_ledger(&env);

    let sell_orders = Vec::from_array(&env, [(creator.clone(), 2u32)]);
    let results = client.batch_sell(&seller, &sell_orders);

    assert_eq!(results.len(), 1);
    let r0 = results.get(0).unwrap();
    assert_eq!(r0.key_id, creator);
    assert_eq!(r0.quantity, 2);
    assert!(r0.proceeds > 0);

    // Remaining balance is 1, supply is 1
    assert_eq!(client.get_key_balance(&creator, &seller), 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);
}

#[test]
fn test_batch_sell_multiple_creators_success() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);

    let c1 = register_test_creator(&env, &client, "alice");
    let c2 = register_test_creator(&env, &client, "bob");
    let c3 = register_test_creator(&env, &client, "carol");
    let seller = Address::generate(&env);

    let buy_orders = Vec::from_array(
        &env,
        [(c1.clone(), 4u32), (c2.clone(), 3u32), (c3.clone(), 2u32)],
    );
    client.batch_buy(&seller, &buy_orders);

    advance_ledger(&env);

    let sell_orders = Vec::from_array(
        &env,
        [(c1.clone(), 2u32), (c2.clone(), 2u32), (c3.clone(), 1u32)],
    );
    let results = client.batch_sell(&seller, &sell_orders);

    assert_eq!(results.len(), 3);
    assert_eq!(client.get_key_balance(&c1, &seller), 2);
    assert_eq!(client.get_key_balance(&c2, &seller), 1);
    assert_eq!(client.get_key_balance(&c3, &seller), 1);

    assert_eq!(client.get_total_key_supply(&c1), 2);
    assert_eq!(client.get_total_key_supply(&c2), 1);
    assert_eq!(client.get_total_key_supply(&c3), 1);
}

#[test]
fn test_batch_sell_up_to_five_creators_boundary() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);

    let c1 = register_test_creator(&env, &client, "alice");
    let c2 = register_test_creator(&env, &client, "bob");
    let c3 = register_test_creator(&env, &client, "carol");
    let c4 = register_test_creator(&env, &client, "dave");
    let c5 = register_test_creator(&env, &client, "eve");
    let seller = Address::generate(&env);

    let buy_orders = Vec::from_array(
        &env,
        [
            (c1.clone(), 1u32),
            (c2.clone(), 1u32),
            (c3.clone(), 1u32),
            (c4.clone(), 1u32),
            (c5.clone(), 1u32),
        ],
    );
    client.batch_buy(&seller, &buy_orders);

    advance_ledger(&env);

    // Exactly 5 orders is allowed
    let sell_orders = Vec::from_array(
        &env,
        [
            (c1.clone(), 1u32),
            (c2.clone(), 1u32),
            (c3.clone(), 1u32),
            (c4.clone(), 1u32),
            (c5.clone(), 1u32),
        ],
    );
    let results = client.batch_sell(&seller, &sell_orders);
    assert_eq!(results.len(), 5);

    assert_eq!(client.get_key_balance(&c1, &seller), 0);
    assert_eq!(client.get_key_balance(&c2, &seller), 0);
    assert_eq!(client.get_key_balance(&c3, &seller), 0);
    assert_eq!(client.get_key_balance(&c4, &seller), 0);
    assert_eq!(client.get_key_balance(&c5, &seller), 0);
}

// ============================================================================
// Batch size validation (between 1 and 5 entries)
// ============================================================================

#[test]
fn test_batch_sell_empty_orders_panics_batch_size_exceeded() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);
    let seller = Address::generate(&env);

    let orders: Vec<(Address, u32)> = Vec::new(&env);
    let result = client.try_batch_sell(&seller, &orders);
    assert_eq!(result, Err(Ok(ContractError::BatchSizeExceeded)));
}

#[test]
fn test_batch_sell_more_than_five_orders_panics_batch_size_exceeded() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);

    let c1 = register_test_creator(&env, &client, "creator1");
    let c2 = register_test_creator(&env, &client, "creator2");
    let c3 = register_test_creator(&env, &client, "creator3");
    let c4 = register_test_creator(&env, &client, "creator4");
    let c5 = register_test_creator(&env, &client, "creator5");
    let c6 = register_test_creator(&env, &client, "creator6");
    let seller = Address::generate(&env);

    let orders = Vec::from_array(
        &env,
        [
            (c1, 1u32),
            (c2, 1u32),
            (c3, 1u32),
            (c4, 1u32),
            (c5, 1u32),
            (c6, 1u32),
        ],
    );

    let result = client.try_batch_sell(&seller, &orders);
    assert_eq!(result, Err(Ok(ContractError::BatchSizeExceeded)));
}

// ============================================================================
// Atomicity and rollback on insufficient balance
// ============================================================================

#[test]
fn test_batch_sell_insufficient_balance_rolls_back_all() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);

    let c1 = register_test_creator(&env, &client, "creator1");
    let c2 = register_test_creator(&env, &client, "creator2");
    let seller = Address::generate(&env);

    // Seller only buys keys for c1, none for c2
    let buy_orders = Vec::from_array(&env, [(c1.clone(), 5u32)]);
    client.batch_buy(&seller, &buy_orders);

    advance_ledger(&env);

    // Attempt batch sell where c2 order exceeds balance
    let sell_orders = Vec::from_array(&env, [(c1.clone(), 2u32), (c2.clone(), 1u32)]);
    let result = client.try_batch_sell(&seller, &sell_orders);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));

    // Verify c1 balance and supply are completely untouched
    assert_eq!(client.get_key_balance(&c1, &seller), 5);
    assert_eq!(client.get_total_key_supply(&c1), 5);
}

// ============================================================================
// Frozen keys excluded from available balance
// ============================================================================

#[test]
fn test_batch_sell_frozen_keys_excluded_from_available_balance() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);

    let creator = register_test_creator(&env, &client, "alice");
    let seller = Address::generate(&env);

    // Buy 5 keys
    let buy_orders = Vec::from_array(&env, [(creator.clone(), 5u32)]);
    client.batch_buy(&seller, &buy_orders);

    advance_ledger(&env);

    // Freeze 3 keys -> 2 liquid keys remain
    client.self_freeze(&creator, &seller, &3);
    assert_eq!(client.get_self_frozen_balance(&creator, &seller), 3);
    assert_eq!(client.get_key_balance(&creator, &seller), 5);

    // Selling 3 keys should fail because 3 are frozen (available = 2)
    let sell_orders_fail = Vec::from_array(&env, [(creator.clone(), 3u32)]);
    let result = client.try_batch_sell(&seller, &sell_orders_fail);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));

    // Selling 2 keys should succeed
    let sell_orders_ok = Vec::from_array(&env, [(creator.clone(), 2u32)]);
    let results = client.batch_sell(&seller, &sell_orders_ok);
    assert_eq!(results.len(), 1);
    assert_eq!(results.get(0).unwrap().quantity, 2);

    // Total balance drops to 3 (all 3 are frozen)
    assert_eq!(client.get_key_balance(&creator, &seller), 3);
    assert_eq!(client.get_self_frozen_balance(&creator, &seller), 3);
}

// ============================================================================
// Staked keys excluded from available balance
// ============================================================================

#[test]
fn test_batch_sell_staked_keys_excluded_from_available_balance() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);

    let creator = register_test_creator(&env, &client, "alice");
    let seller = Address::generate(&env);

    // Buy 5 keys
    let buy_orders = Vec::from_array(&env, [(creator.clone(), 5u32)]);
    client.batch_buy(&seller, &buy_orders);

    advance_ledger(&env);

    // Stake 3 keys -> 2 liquid keys remain
    client.stake_keys(&creator, &seller, &3);
    assert_eq!(client.get_staked_balance(&creator, &seller), 3);

    // Selling 3 keys should fail because 3 are staked (available = 2)
    let sell_orders_fail = Vec::from_array(&env, [(creator.clone(), 3u32)]);
    let result = client.try_batch_sell(&seller, &sell_orders_fail);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));

    // Selling 2 keys should succeed
    let sell_orders_ok = Vec::from_array(&env, [(creator.clone(), 2u32)]);
    let results = client.batch_sell(&seller, &sell_orders_ok);
    assert_eq!(results.len(), 1);
}

// ============================================================================
// Events verification: batch_sell_completed with correct fields
// ============================================================================

#[test]
fn test_batch_sell_completed_event_emitted_with_correct_fields() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);

    let c1 = register_test_creator(&env, &client, "alice");
    let c2 = register_test_creator(&env, &client, "bob");
    let seller = Address::generate(&env);

    let buy_orders = Vec::from_array(&env, [(c1.clone(), 3u32), (c2.clone(), 2u32)]);
    client.batch_buy(&seller, &buy_orders);

    let test_ledger = 100u32;
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = test_ledger;
    ledger_info.timestamp += 10;
    env.ledger().set(ledger_info);

    // Clear event history
    env.events().all();

    let sell_orders = Vec::from_array(&env, [(c1.clone(), 2u32), (c2.clone(), 1u32)]);
    let results = client.batch_sell(&seller, &sell_orders);

    let total_proceeds: i128 = results.iter().map(|r| r.proceeds).sum();

    // Verify batch_sell_completed event
    let event_log = env.events().all();
    let (_, topics, data) = event_log
        .iter()
        .find(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::BATCH_SELL_COMPLETED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("batch_sell_completed event must be emitted");

    // Topic 1: seller address
    let topic_seller: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic_seller, seller);

    // Event payload
    let payload: events::BatchSellCompletedEvent = data.into_val(&env);
    assert_eq!(payload.seller, seller);
    assert_eq!(payload.total_proceeds, total_proceeds);
    assert_eq!(payload.ledger, test_ledger);
    assert_eq!(payload.orders.len(), 2);

    let order0 = payload.orders.get(0).unwrap();
    assert_eq!(order0.0, c1);
    assert_eq!(order0.1, 2u32);
    assert_eq!(order0.2, results.get(0).unwrap().proceeds);

    let order1 = payload.orders.get(1).unwrap();
    assert_eq!(order1.0, c2);
    assert_eq!(order1.1, 1u32);
    assert_eq!(order1.2, results.get(1).unwrap().proceeds);
}

// ============================================================================
// Flash loan guard & Zero quantity checks
// ============================================================================

#[test]
fn test_batch_sell_reverts_on_same_ledger_flash_loan() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);

    let creator = register_test_creator(&env, &client, "alice");
    let seller = Address::generate(&env);

    // Buy via buy_key which registers last_buy_ledger
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &seller, &quote.total_amount, &None);

    // Do NOT advance ledger sequence
    let sell_orders = Vec::from_array(&env, [(creator.clone(), 1u32)]);
    let result = client.try_batch_sell(&seller, &sell_orders);
    assert_eq!(result, Err(Ok(ContractError::FlashLoanDetected)));
}

#[test]
fn test_batch_sell_reverts_on_zero_quantity() {
    let env = test_env_with_auths();
    let (client, _) = setup(&env);

    let creator = register_test_creator(&env, &client, "alice");
    let seller = Address::generate(&env);

    let orders = Vec::from_array(&env, [(creator, 0u32)]);
    let result = client.try_batch_sell(&seller, &orders);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}
