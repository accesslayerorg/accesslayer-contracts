//! Integration tests for issue #834 — key deprecation and holder buybacks.
//!
//! Acceptance criteria verified:
//!
//! 1. Buy on a deprecated key returns `KeyDeprecated`.
//! 2. Holder redeems and receives correct XLM payout.
//! 3. Creator must escrow sufficient XLM or `deprecate_key` returns `InsufficientEscrow`.
//! 4. Non-creator `deprecate_key` returns `Unauthorized`.
//! 5. `key_deprecated` and `keys_redeemed` events are emitted with correct fields.

use creator_keys::{
    events, ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String,
};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn setup() -> (Env, CreatorKeysContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    // Flat price of 100 per key, no slope so every key costs exactly 100.
    client.set_key_price(&admin, &100_i128);
    client.set_curve_slope(&admin, &0_i128);
    client.set_fee_config(&admin, &9_000_u32, &1_000_u32);
    // Disable circuit breaker so sequential buys never trip it.
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

fn buy_one(client: &CreatorKeysContractClient, creator: &Address, buyer: &Address) -> u32 {
    client.buy_key(creator, buyer, &10_000_i128, &None)
}

// ---------------------------------------------------------------------------
// 1. Buy on a deprecated key returns KeyDeprecated
// ---------------------------------------------------------------------------

#[test]
fn test_buy_on_deprecated_key_returns_key_deprecated() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    buy_one(&client, &creator, &buyer);

    // escrow = 1 key × 100 = 100
    client.deprecate_key(&creator, &creator, &100_i128, &100_i128);

    let result = client.try_buy_key(&creator, &buyer, &10_000_i128, &None);
    assert_eq!(result, Err(Ok(ContractError::KeyDeprecated)));
}

// ---------------------------------------------------------------------------
// 2. Holder redeems and receives correct payout
// ---------------------------------------------------------------------------

#[test]
fn test_holder_redeem_receives_correct_payout() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "bob");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder);
    buy_one(&client, &creator, &holder);
    buy_one(&client, &creator, &holder);

    assert_eq!(client.get_creator_supply(&creator), 3);

    let buyback_price: i128 = 500;
    let required_escrow: i128 = 3 * buyback_price; // 1500
    client.deprecate_key(&creator, &creator, &buyback_price, &required_escrow);

    let payout = client.redeem(&creator, &holder);
    assert_eq!(payout, required_escrow);

    assert_eq!(client.get_key_balance(&creator, &holder), 0);
    assert_eq!(client.get_creator_supply(&creator), 0);
}

// ---------------------------------------------------------------------------
// 3. Insufficient escrow returns InsufficientEscrow
// ---------------------------------------------------------------------------

#[test]
fn test_deprecate_key_insufficient_escrow_returns_error() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "charlie");
    let buyer = Address::generate(&env);

    // Supply = 2, required escrow = 200, provide 199.
    buy_one(&client, &creator, &buyer);
    buy_one(&client, &creator, &buyer);

    let result = client.try_deprecate_key(&creator, &creator, &100_i128, &199_i128);
    assert_eq!(result, Err(Ok(ContractError::InsufficientEscrow)));
}

#[test]
fn test_deprecate_key_exact_escrow_succeeds() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "diana");
    let buyer = Address::generate(&env);

    // Supply = 2, required = 200, provide exactly 200.
    buy_one(&client, &creator, &buyer);
    buy_one(&client, &creator, &buyer);

    let result = client.try_deprecate_key(&creator, &creator, &100_i128, &200_i128);
    assert_eq!(result, Ok(Ok(())));
}

#[test]
fn test_deprecate_key_zero_supply_requires_zero_escrow() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "edgar");

    // No keys minted; required escrow = 0 so even escrow_payment = 0 is fine.
    let result = client.try_deprecate_key(&creator, &creator, &100_i128, &0_i128);
    assert_eq!(result, Ok(Ok(())));
}

// ---------------------------------------------------------------------------
// 4. Non-creator deprecate_key returns Unauthorized
// ---------------------------------------------------------------------------

#[test]
fn test_non_creator_deprecate_key_returns_unauthorized() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "frank");
    let attacker = Address::generate(&env);

    let result = client.try_deprecate_key(&creator, &attacker, &100_i128, &0_i128);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ---------------------------------------------------------------------------
// 5a. key_deprecated event emitted with correct fields
// ---------------------------------------------------------------------------

#[test]
fn test_deprecate_key_emits_key_deprecated_event() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "grace");
    let buyer = Address::generate(&env);

    buy_one(&client, &creator, &buyer);

    let buyback_price: i128 = 200;
    let escrow: i128 = 200; // 1 × 200
    client.deprecate_key(&creator, &creator, &buyback_price, &escrow);

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
    assert_eq!(data.buyback_price_per_key, buyback_price);
    assert_eq!(data.circulating_supply, 1);
    assert_eq!(data.total_escrow, escrow);
}

// ---------------------------------------------------------------------------
// 5b. keys_redeemed event emitted with correct fields
// ---------------------------------------------------------------------------

#[test]
fn test_redeem_emits_keys_redeemed_event() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "henry");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder);
    buy_one(&client, &creator, &holder);

    let buyback_price: i128 = 300;
    client.deprecate_key(&creator, &creator, &buyback_price, &(2 * buyback_price));

    client.redeem(&creator, &holder);

    let all_events = env.events().all();
    let rdm_event = all_events
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .map(|v| {
                    let sym: soroban_sdk::Symbol = v.into_val(&env);
                    sym == events::KEYS_REDEEMED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("keys_redeemed event not found");

    let data: events::KeysRedeemedEvent = rdm_event.2.into_val(&env);
    assert_eq!(data.creator, creator);
    assert_eq!(data.holder, holder);
    assert_eq!(data.quantity, 2);
    assert_eq!(data.payout, 2 * buyback_price);
    assert_eq!(data.new_supply, 0);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_redeem_zero_balance_returns_insufficient_balance() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "iris");
    let other = Address::generate(&env);
    let no_keys = Address::generate(&env);

    buy_one(&client, &creator, &other);
    client.deprecate_key(&creator, &creator, &100_i128, &100_i128);

    let result = client.try_redeem(&creator, &no_keys);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

#[test]
fn test_double_deprecation_returns_key_deprecated() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "jake");

    client.deprecate_key(&creator, &creator, &100_i128, &0_i128);

    let result = client.try_deprecate_key(&creator, &creator, &200_i128, &0_i128);
    assert_eq!(result, Err(Ok(ContractError::KeyDeprecated)));
}

#[test]
fn test_multiple_holders_redeem_independently() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "kate");
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    // holder_a buys 2, holder_b buys 1. Supply = 3.
    buy_one(&client, &creator, &holder_a);
    buy_one(&client, &creator, &holder_a);
    buy_one(&client, &creator, &holder_b);

    let buyback_price: i128 = 150;
    client.deprecate_key(&creator, &creator, &buyback_price, &(3 * buyback_price));

    let payout_a = client.redeem(&creator, &holder_a);
    assert_eq!(payout_a, 2 * buyback_price);
    assert_eq!(client.get_creator_supply(&creator), 1);

    let payout_b = client.redeem(&creator, &holder_b);
    assert_eq!(payout_b, buyback_price);
    assert_eq!(client.get_creator_supply(&creator), 0);
}

#[test]
fn test_deprecate_key_negative_price_returns_not_positive_amount() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "leo");

    let result = client.try_deprecate_key(&creator, &creator, &-1_i128, &0_i128);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn test_deprecate_key_unregistered_creator_returns_not_registered() {
    let (env, client, _admin) = setup();
    let ghost = Address::generate(&env);

    let result = client.try_deprecate_key(&ghost, &ghost, &100_i128, &0_i128);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_buy_key_with_referrer_also_blocked_on_deprecated_key() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "mia");
    let buyer = Address::generate(&env);
    let referrer = Address::generate(&env);

    buy_one(&client, &creator, &buyer);
    client.deprecate_key(&creator, &creator, &100_i128, &100_i128);

    let result =
        client.try_buy_key_with_referrer(&creator, &buyer, &10_000_i128, &None, &Some(referrer));
    assert_eq!(result, Err(Ok(ContractError::KeyDeprecated)));
}
