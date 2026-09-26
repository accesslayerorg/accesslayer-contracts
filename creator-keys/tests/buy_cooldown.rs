//! Integration tests for the per-wallet buy cooldown feature.
//!
//! Once a creator calls `set_buy_cooldown(cooldown_ledgers)`, `buy_key` rejects
//! a second purchase by the same wallet within that window with
//! `CooldownError::CooldownActive` and emits a `cooldown_blocked` event.
//! Buys after the cooldown has elapsed succeed normally.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_ledger_sequence,
    test_env_with_auths,
};
use creator_keys::events::{self, COOLDOWN_BLOCKED_EVENT_NAME};
use creator_keys::{ContractError, CooldownError, MAX_BUY_COOLDOWN_LEDGERS};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 100;
const COOLDOWN: u32 = 10;
const BASE_LEDGER: u32 = 100;

// ── helpers ──────────────────────────────────────────────────────────────────

struct Setup<'a> {
    client: creator_keys::CreatorKeysContractClient<'a>,
    creator: Address,
}

/// Build a contract + registered creator, set key price, set a cooldown, and
/// advance to a deterministic starting ledger.
fn setup_with_cooldown(env: &Env, cooldown: u32) -> Setup<'_> {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    set_ledger_sequence(env, BASE_LEDGER);
    client.set_buy_cooldown(&creator, &cooldown);
    Setup { client, creator }
}

/// Collect all `CooldownBlockedEvent` payloads emitted during the most recent
/// contract invocation.
fn cooldown_blocked_events(env: &Env) -> soroban_sdk::Vec<events::CooldownBlockedEvent> {
    let mut found = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == COOLDOWN_BLOCKED_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

// ── acceptance criteria ───────────────────────────────────────────────────────

/// AC: Second buy within cooldown period panics with CooldownActive.
#[test]
fn test_second_buy_within_cooldown_is_rejected() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN);

    let buyer = Address::generate(&env);
    // First buy succeeds (no prior ledger recorded).
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    // Advance by fewer than COOLDOWN ledgers – still inside the window.
    set_ledger_sequence(&env, BASE_LEDGER + COOLDOWN - 1);

    let result = s.client.try_buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::CooldownActive)),
        "buy within cooldown must return CooldownActive"
    );
    // Supply must be unchanged after the rejection.
    assert_eq!(s.client.get_total_key_supply(&s.creator), 1);
    assert_eq!(s.client.get_key_balance(&s.creator, &buyer), 1);
}

/// AC: Buy after cooldown has elapsed succeeds normally.
#[test]
fn test_buy_after_cooldown_elapsed_succeeds() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN);

    let buyer = Address::generate(&env);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    // Advance exactly COOLDOWN ledgers – boundary is inclusive.
    set_ledger_sequence(&env, BASE_LEDGER + COOLDOWN);
    let supply = s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(supply, 2, "buy at cooldown boundary must succeed");
    assert_eq!(s.client.get_key_balance(&s.creator, &buyer), 2);
}

/// AC: cooldown_blocked event emitted with correct ledgers_remaining.
#[test]
fn test_cooldown_blocked_event_has_correct_ledgers_remaining() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN);

    let buyer = Address::generate(&env);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    // Advance by 3 ledgers → 7 ledgers remaining in the cooldown.
    set_ledger_sequence(&env, BASE_LEDGER + 3);

    let _ = s.client.try_buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    let found = cooldown_blocked_events(&env);
    assert_eq!(
        found.len(),
        1,
        "exactly one cooldown_blocked event per rejection"
    );

    let ev = found.get(0).unwrap();
    assert_eq!(ev.wallet, buyer, "event.wallet must be the blocked buyer");
    assert_eq!(
        ev.creator_id, s.creator,
        "event.creator_id must match the creator"
    );
    assert_eq!(
        ev.ledgers_remaining,
        COOLDOWN - 3,
        "ledgers_remaining must equal cooldown_ledgers minus elapsed"
    );
}

/// AC: cooldown_ledgers above 720 panics with CooldownTooLong.
#[test]
fn test_set_buy_cooldown_above_max_is_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "bob");

    // Exactly at the limit is allowed.
    client.set_buy_cooldown(&creator, &MAX_BUY_COOLDOWN_LEDGERS);
    assert_eq!(client.get_buy_cooldown(&creator), MAX_BUY_COOLDOWN_LEDGERS);

    // One above the limit must be rejected.
    let result = client.try_set_buy_cooldown(&creator, &(MAX_BUY_COOLDOWN_LEDGERS + 1));
    assert_eq!(
        result,
        Err(Ok(CooldownError::CooldownTooLong)),
        "cooldown above 720 ledgers must return CooldownTooLong"
    );
    // The stored value must still be the previous valid setting.
    assert_eq!(client.get_buy_cooldown(&creator), MAX_BUY_COOLDOWN_LEDGERS);
}

/// AC: Non-creator set_buy_cooldown panics with Unauthorized.
#[test]
fn test_non_creator_cannot_set_cooldown() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let _creator = register_test_creator(&env, &client, "carol");
    let impostor = Address::generate(&env);

    // Attempt by impostor is rejected at the host auth level.
    let result = client.try_set_buy_cooldown(&impostor, &5u32);
    assert!(
        result.is_err(),
        "non-creator must not be able to set the buy cooldown"
    );
}

/// AC: First buy by any wallet always succeeds even when a cooldown is configured.
#[test]
fn test_first_buy_always_succeeds_with_cooldown_configured() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN);

    for i in 0..3 {
        let buyer = Address::generate(&env);
        let supply = s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
        assert_eq!(supply, i + 1, "first buy for wallet {i} must succeed");
    }
}

/// AC: Cooldown is per-wallet; one wallet being blocked does not affect others.
#[test]
fn test_cooldown_is_independent_per_wallet() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN);

    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);

    s.client.buy_key(&s.creator, &buyer_a, &KEY_PRICE, &None);
    s.client.buy_key(&s.creator, &buyer_b, &KEY_PRICE, &None);

    set_ledger_sequence(&env, BASE_LEDGER + 3);

    // buyer_a is blocked.
    let result_a = s
        .client
        .try_buy_key(&s.creator, &buyer_a, &KEY_PRICE, &None);
    assert_eq!(result_a, Err(Ok(ContractError::CooldownActive)));

    // buyer_b is also blocked independently.
    let result_b = s
        .client
        .try_buy_key(&s.creator, &buyer_b, &KEY_PRICE, &None);
    assert_eq!(result_b, Err(Ok(ContractError::CooldownActive)));
}

/// AC: Setting cooldown to 0 disables it; consecutive buys succeed freely.
#[test]
fn test_zero_cooldown_disables_restriction() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, 0);

    let buyer = Address::generate(&env);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    let supply = s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(supply, 2, "zero cooldown must allow back-to-back buys");
}

/// AC: Cooldown tracks each buy so a successful second buy refreshes the window.
#[test]
fn test_last_buy_ledger_refreshes_on_each_buy() {
    let env = test_env_with_auths();
    let s = setup_with_cooldown(&env, COOLDOWN);

    let buyer = Address::generate(&env);

    // First buy at BASE_LEDGER.
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    // Second buy at BASE_LEDGER + COOLDOWN (allowed).
    set_ledger_sequence(&env, BASE_LEDGER + COOLDOWN);
    s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);

    // Third attempt before next window closes must be blocked.
    set_ledger_sequence(&env, BASE_LEDGER + COOLDOWN + COOLDOWN - 1);
    let result = s.client.try_buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::CooldownActive)),
        "cooldown window must reset after each successful buy"
    );

    // Exactly at the new boundary succeeds.
    set_ledger_sequence(&env, BASE_LEDGER + COOLDOWN + COOLDOWN);
    let supply = s.client.buy_key(&s.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(supply, 3);
}

/// AC: Cooldown is per-creator; two creators can have independent cooldowns.
#[test]
fn test_cooldown_is_independent_per_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    set_ledger_sequence(&env, BASE_LEDGER);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");

    client.set_buy_cooldown(&creator_a, &COOLDOWN);
    client.set_buy_cooldown(&creator_b, &0);

    let buyer = Address::generate(&env);

    client.buy_key(&creator_a, &buyer, &KEY_PRICE, &None);
    client.buy_key(&creator_b, &buyer, &KEY_PRICE, &None);

    set_ledger_sequence(&env, BASE_LEDGER + 3);

    // Blocked for creator_a.
    assert_eq!(
        client.try_buy_key(&creator_a, &buyer, &KEY_PRICE, &None),
        Err(Ok(ContractError::CooldownActive))
    );
    // Unrestricted for creator_b.
    let supply_b = client.buy_key(&creator_b, &buyer, &KEY_PRICE, &None);
    assert_eq!(supply_b, 2);
}
