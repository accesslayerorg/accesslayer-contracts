//! #554 — a sell for more keys than the holder owns must be rejected, and the
//! rejection must leave every piece of state exactly as it was.
//!
//! The scenario is the one in the issue: a wallet holding **exactly 2** keys
//! asks to sell **3**. The call must
//!
//! - return `ContractError::InsufficientBalance`,
//! - leave the holder's balance at 2,
//! - leave the creator's supply unchanged, and
//! - emit no sale event and credit no proceeds.
//!
//! Covered for both batch entrypoints, `batch_sell` (pre-flight validated) and
//! `batch_sell_v2` (validated only while selling, one key at a time), plus the
//! case where the oversell is spread across two orders for the same creator.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{events, ContractError, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Symbol, TryIntoVal, Vec,
};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 500;
const PROTOCOL_BPS: u32 = 500;

fn setup(env: &soroban_sdk::Env) -> CreatorKeysContractClient<'_> {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    client.set_protocol_fee_recipient(&admin, &Address::generate(env));
    client
}

fn advance_ledger(env: &soroban_sdk::Env) {
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    l.timestamp += 5;
    env.ledger().set(l);
}

/// A wallet that holds exactly two keys, bought in two separate transactions.
fn holder_of_two(
    env: &soroban_sdk::Env,
    client: &CreatorKeysContractClient<'_>,
) -> (Address, Address) {
    let creator = register_test_creator(env, client, "alice");
    let seller = Address::generate(env);

    for _ in 0..2 {
        client.buy_key(&creator, &seller, &KEY_PRICE, &None);
        advance_ledger(env);
    }

    assert_eq!(client.get_key_balance(&creator, &seller), 2);
    assert_eq!(client.get_total_key_supply(&creator), 2);
    (creator, seller)
}

fn assert_state_untouched(
    env: &soroban_sdk::Env,
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    seller: &Address,
) {
    assert_eq!(
        client.get_key_balance(creator, seller),
        2,
        "a rejected oversell must not deduct any key"
    );
    assert_eq!(
        client.get_total_key_supply(creator),
        2,
        "a rejected oversell must not reduce creator supply"
    );
    assert_eq!(
        client.get_holder_count(creator),
        1,
        "the holder must still be counted"
    );

    // Nothing was sold, so no sale event may survive the rejected call.
    let sell_topic = events::SELL_EVENT_NAME;
    for (_, topics, _) in env.events().all().iter() {
        let first: Symbol = topics.get(0).unwrap().try_into_val(env).unwrap();
        assert_ne!(
            first, sell_topic,
            "a rejected oversell must not emit a sale event"
        );
    }
}

#[test]
fn test_batch_sell_rejects_more_than_balance_and_preserves_state() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let (creator, seller) = holder_of_two(&env, &client);

    let orders = Vec::from_array(&env, [(creator.clone(), 3u32)]);
    let result = client.try_batch_sell(&seller, &orders);

    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
    assert_eq!(ContractError::InsufficientBalance as u32, 9);
    assert_state_untouched(&env, &client, &creator, &seller);
}

#[test]
fn test_batch_sell_v2_rejects_more_than_balance_and_preserves_state() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let (creator, seller) = holder_of_two(&env, &client);

    let orders = Vec::from_array(&env, [(creator.clone(), 3u32, None::<i128>)]);
    let result = client.try_batch_sell_v2(&seller, &orders);

    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
    assert_state_untouched(&env, &client, &creator, &seller);
}

/// Two orders that are individually plausible but together exceed the balance.
#[test]
fn test_batch_sell_v2_rejects_split_orders_exceeding_balance() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let (creator, seller) = holder_of_two(&env, &client);

    let orders = Vec::from_array(
        &env,
        [
            (creator.clone(), 2u32, None::<i128>),
            (creator.clone(), 1u32, None::<i128>),
        ],
    );
    let result = client.try_batch_sell_v2(&seller, &orders);

    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
    assert_state_untouched(&env, &client, &creator, &seller);
}

/// Control: the same holder can sell exactly what they own, in one order.
#[test]
fn test_batch_sell_v2_accepts_selling_exactly_the_balance() {
    let env = test_env_with_auths();
    let client = setup(&env);
    let (creator, seller) = holder_of_two(&env, &client);

    let orders = Vec::from_array(&env, [(creator.clone(), 2u32, None::<i128>)]);
    let result = client.try_batch_sell_v2(&seller, &orders);

    assert!(result.is_ok(), "selling exactly the balance must succeed");
    assert_eq!(client.get_key_balance(&creator, &seller), 0);
    assert_eq!(client.get_total_key_supply(&creator), 0);
}
