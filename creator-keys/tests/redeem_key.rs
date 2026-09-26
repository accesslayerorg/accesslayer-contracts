//! Integration tests for issue #855 — holders redeem deprecated keys for the
//! fixed buyback price escrowed by `deprecate_key` (#834).
//!
//! Acceptance criteria verified:
//!
//! 1. Caller receives `holder_balance * buyback_price_per_key` from escrow.
//! 2. Caller's balance is zeroed after redemption.
//! 3. Circulating supply is decremented by exactly the redeemed quantity.
//! 4. Redeem on a non-deprecated key fails with `KeyNotDeprecated`.
//! 5. `keys_redeemed` event carries holder (wallet), creator (key id),
//!    quantity, and payout.

use creator_keys::{
    events, ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol,
};

const BUYBACK_PRICE: i128 = 250;

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

fn buy(client: &CreatorKeysContractClient, creator: &Address, buyer: &Address, n: u32) {
    for _ in 0..n {
        client.buy_key(creator, buyer, &10_000_i128, &None);
    }
}

fn deprecate(client: &CreatorKeysContractClient, creator: &Address) {
    let supply = client.get_creator_supply(creator) as i128;
    client.deprecate_key(creator, creator, &BUYBACK_PRICE, &(supply * BUYBACK_PRICE));
}

// ---------------------------------------------------------------------------
// 1–3. Payout, zeroed balance, supply decrement
// ---------------------------------------------------------------------------

#[test]
fn redeem_pays_balance_times_buyback_price_and_burns_only_callers_keys() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "redeemer");
    let holder = Address::generate(&env);
    let other = Address::generate(&env);

    buy(&client, &creator, &holder, 3);
    buy(&client, &creator, &other, 2);
    assert_eq!(client.get_creator_supply(&creator), 5);
    assert_eq!(client.get_creator_holder_count(&creator), 2);

    deprecate(&client, &creator);

    let payout = client.redeem(&creator, &holder);

    assert_eq!(payout, 3 * BUYBACK_PRICE);
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
    assert_eq!(client.get_key_balance(&creator, &other), 2);
    assert_eq!(client.get_creator_supply(&creator), 2);
    assert_eq!(client.get_creator_holder_count(&creator), 1);
}

#[test]
fn second_redeem_by_same_holder_fails_and_cannot_drain_escrow() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "twice");
    let holder = Address::generate(&env);
    let other = Address::generate(&env);

    buy(&client, &creator, &holder, 1);
    buy(&client, &creator, &other, 1);
    deprecate(&client, &creator);

    client.redeem(&creator, &holder);
    let again = client.try_redeem(&creator, &holder);
    assert_eq!(again, Err(Ok(ContractError::InsufficientBalance)));

    // The remaining holder is still fully covered by escrow.
    assert_eq!(client.redeem(&creator, &other), BUYBACK_PRICE);
    assert_eq!(client.get_creator_supply(&creator), 0);
}

// ---------------------------------------------------------------------------
// 4. KeyNotDeprecated
// ---------------------------------------------------------------------------

#[test]
fn redeem_on_active_key_returns_key_not_deprecated_and_leaves_state_untouched() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "active");
    let holder = Address::generate(&env);

    buy(&client, &creator, &holder, 2);

    let result = client.try_redeem(&creator, &holder);
    assert_eq!(result, Err(Ok(ContractError::KeyNotDeprecated)));

    assert_eq!(client.get_key_balance(&creator, &holder), 2);
    assert_eq!(client.get_creator_supply(&creator), 2);
    assert_eq!(client.get_creator_holder_count(&creator), 1);
}

#[test]
fn deprecation_check_runs_before_balance_check() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "order");
    let no_keys = Address::generate(&env);

    let result = client.try_redeem(&creator, &no_keys);
    assert_eq!(result, Err(Ok(ContractError::KeyNotDeprecated)));
}

#[test]
fn redeem_on_unregistered_creator_returns_not_registered() {
    let (env, client, _admin) = setup();
    let ghost = Address::generate(&env);
    let holder = Address::generate(&env);

    let result = client.try_redeem(&ghost, &holder);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn key_not_deprecated_discriminant_is_stable() {
    assert_eq!(ContractError::KeyNotDeprecated as u32, 82);
}

// ---------------------------------------------------------------------------
// 5. keys_redeemed event
// ---------------------------------------------------------------------------

#[test]
fn redeem_emits_keys_redeemed_event_with_wallet_key_quantity_and_payout() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "evented");
    let holder = Address::generate(&env);

    buy(&client, &creator, &holder, 4);
    deprecate(&client, &creator);
    client.redeem(&creator, &holder);

    let (_, topics, data) = env
        .events()
        .all()
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .map(|v| {
                    let sym: Symbol = v.into_val(&env);
                    sym == events::KEYS_REDEEMED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("keys_redeemed event not found");

    let topic_creator: Address = topics.get(1).unwrap().into_val(&env);
    let topic_holder: Address = topics.get(2).unwrap().into_val(&env);
    assert_eq!(topic_creator, creator);
    assert_eq!(topic_holder, holder);

    let payload: events::KeysRedeemedEvent = data.into_val(&env);
    assert_eq!(payload.holder, holder);
    assert_eq!(payload.creator, creator);
    assert_eq!(payload.quantity, 4);
    assert_eq!(payload.payout, 4 * BUYBACK_PRICE);
    assert_eq!(payload.new_supply, 0);
}

#[test]
fn failed_redeem_emits_no_keys_redeemed_event() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "silent");
    let holder = Address::generate(&env);

    buy(&client, &creator, &holder, 1);
    let _ = client.try_redeem(&creator, &holder);

    let emitted = env.events().all().iter().any(|(_, topics, _)| {
        topics
            .get(0)
            .map(|v| {
                let sym: Symbol = v.into_val(&env);
                sym == events::KEYS_REDEEMED_EVENT_NAME
            })
            .unwrap_or(false)
    });
    assert!(!emitted);
}
