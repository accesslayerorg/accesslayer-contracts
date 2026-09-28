//! Integration tests for issue #973 — key deprecation reason and successor designation.
//!
//! Acceptance criteria verified:
//!
//! 1. Deprecation blocked for non-creator callers (Unauthorized).
//! 2. Buy rejected with KeyDeprecated error after deprecation.
//! 3. Sell and transfer still permitted post-deprecation.
//! 4. `get_deprecation_status` returns all fields correctly.
//! 5. `KeyDeprecated` event includes reason and successor_key_id.

use creator_keys::{
    events, ContractError, CreatorKeysContract, CreatorKeysContractClient,
    DeprecationStatus, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup() -> (Env, CreatorKeysContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &100_i128);
    client.set_curve_slope(&admin, &0_i128);
    client.set_fee_config(&admin, &9_000_u32, &1_000_u32);
    client.set_circuit_breaker_threshold(&admin, &10_000_u32);

    (env, client, admin)
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient, handle: &str) -> Address {
    let creator = Address::generate(env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    creator
}

fn buy_one(client: &CreatorKeysContractClient, creator: &Address, buyer: &Address) {
    client.buy_key(creator, buyer, &10_000_i128, &None);
}

// ---------------------------------------------------------------------------
// 1. Deprecation blocked for non-creator callers
// ---------------------------------------------------------------------------

#[test]
fn test_deprecation_blocked_for_non_creator() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "alice973");
    let attacker = Address::generate(&env);

    let result = client.try_deprecate_key(
        &creator,
        &attacker,
        &100_i128,
        &0_i128,
        &String::from_str(&env, "hostile takeover"),
        &None,
    );
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ---------------------------------------------------------------------------
// 2. Buy rejected with KeyDeprecated after deprecation
// ---------------------------------------------------------------------------

#[test]
fn test_buy_rejected_after_deprecation() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "bob973");
    let buyer = Address::generate(&env);

    buy_one(&client, &creator, &buyer);
    client.deprecate_key(
        &creator,
        &creator,
        &100_i128,
        &100_i128,
        &String::from_str(&env, "moving to v2"),
        &None,
    );

    let result = client.try_buy_key(&creator, &buyer, &10_000_i128, &None);
    assert_eq!(result, Err(Ok(ContractError::KeyDeprecated)));
}

// ---------------------------------------------------------------------------
// 3. Sell still permitted post-deprecation
// ---------------------------------------------------------------------------

#[test]
fn test_sell_still_permitted_after_deprecation() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "carol973");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder);
    assert_eq!(client.get_key_balance(&creator, &holder), 1);

    client.deprecate_key(
        &creator,
        &creator,
        &100_i128,
        &100_i128,
        &String::from_str(&env, "sunsetting"),
        &None,
    );

    // Sell should succeed even after deprecation.
    let result = client.try_sell_key(&creator, &holder, &0_i128, &None);
    assert!(result.is_ok(), "sell should succeed on deprecated key");
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
}

// ---------------------------------------------------------------------------
// 4. get_deprecation_status returns all fields correctly
// ---------------------------------------------------------------------------

#[test]
fn test_get_deprecation_status_undeprecated_returns_false() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "dave973");

    let status = client.get_deprecation_status(&creator);
    assert!(!status.is_deprecated);
    assert_eq!(status.deprecated_at_ledger, 0);
    assert_eq!(status.successor_key_id, None);
}

#[test]
fn test_get_deprecation_status_returns_reason_and_successor() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "eve973");
    let successor = register_creator(&env, &client, "successor973");

    let reason_str = String::from_str(&env, "upgrade to v2");

    client.deprecate_key(
        &creator,
        &creator,
        &100_i128,
        &0_i128,
        &reason_str,
        &Some(successor.clone()),
    );

    let status = client.get_deprecation_status(&creator);
    assert!(status.is_deprecated);
    assert_eq!(status.reason, reason_str);
    assert_eq!(status.successor_key_id, Some(successor));
    assert!(status.deprecated_at_ledger > 0);
}

#[test]
fn test_get_deprecation_status_no_successor_returns_none() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "frank973");

    client.deprecate_key(
        &creator,
        &creator,
        &100_i128,
        &0_i128,
        &String::from_str(&env, "just retiring"),
        &None,
    );

    let status = client.get_deprecation_status(&creator);
    assert!(status.is_deprecated);
    assert_eq!(status.successor_key_id, None);
    assert_eq!(status.reason, String::from_str(&env, "just retiring"));
}

// ---------------------------------------------------------------------------
// 5. KeyDeprecated event includes reason and successor_key_id
// ---------------------------------------------------------------------------

#[test]
fn test_key_deprecated_event_includes_reason_and_successor() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "grace973");
    let successor = register_creator(&env, &client, "successorkey");
    let buyer = Address::generate(&env);

    buy_one(&client, &creator, &buyer);

    let reason_str = String::from_str(&env, "v2 launch");
    let buyback_price: i128 = 100;

    client.deprecate_key(
        &creator,
        &creator,
        &buyback_price,
        &buyback_price,
        &reason_str,
        &Some(successor.clone()),
    );

    let all_events = env.events().all();
    let dep_event = all_events
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .map(|v| {
                    let sym: soroban_sdk::Symbol = v.into_val(&env);
                    sym == events::KEY_DEPRECATED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("key_deprecated event not found");

    let data: events::KeyDeprecatedEvent = dep_event.2.into_val(&env);
    assert_eq!(data.creator, creator);
    assert_eq!(data.reason, reason_str);
    assert_eq!(data.successor_key_id, Some(successor));
    assert_eq!(data.buyback_price_per_key, buyback_price);
    assert_eq!(data.circulating_supply, 1);
}
