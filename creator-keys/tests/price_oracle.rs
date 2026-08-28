//! Integration tests for the price oracle interface (#803).
//!
//! The oracle exposes `get_price` and `get_twap_price` as cross-contract
//! callable view functions gated by an admin-maintained allowlist of approved
//! callers. Unapproved callers are rejected with `CallerNotApproved`. Every
//! successful read emits a `price_queried` event carrying the caller address
//! and the returned price.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_ledger_sequence, test_env_with_auths};
use creator_keys::{events, ContractError, CreatorKeysContractClient, CurvePreset};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol,
};

const KEY_PRICE: i128 = 100;
const SLOPE: i128 = 10;

/// Approves `caller` using a fresh admin address and registers `creator`.
/// Returns the admin address so tests can later remove callers.
fn setup_approved(env: &Env, client: &CreatorKeysContractClient<'_>, caller: &Address) -> Address {
    let admin = Address::generate(env);
    // Set pricing + linear bonding curve so price varies with supply, plus a
    // fee config (required by buy quotes) and the protocol admin.
    client.set_key_price(&admin, &KEY_PRICE);
    client.set_curve_slope(&admin, &SLOPE);
    client.set_fee_config(&admin, &9000, &1000);
    client.set_protocol_admin(&admin, &admin);
    client.add_approved_caller(&admin, caller);
    admin
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient<'_>) -> Address {
    let creator = Address::generate(env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "oracle_creator"),
        },
        &None,
        &None,
        &None,
        &Some(CurvePreset::Linear),
        &None,
        &None,
    );
    creator
}

// ── get_price ────────────────────────────────────────────────────────────────

#[test]
fn test_get_price_returns_bonding_curve_price_with_zero_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    setup_approved(&env, &client, &caller);
    let creator = register_creator(&env, &client);

    let price = client.get_price(&creator, &caller);
    assert_eq!(
        price, KEY_PRICE,
        "price at zero supply is the base key price"
    );
}

#[test]
fn test_get_price_reflects_current_supply_on_linear_curve() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    setup_approved(&env, &client, &caller);
    let creator = register_creator(&env, &client);

    // Buy 3 keys so supply becomes 3 → price = base + slope * supply.
    let buyer = Address::generate(&env);
    for _ in 0..3 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &buyer, &quote.total_amount, &None);
    }

    let price = client.get_price(&creator, &caller);
    let expected = KEY_PRICE + SLOPE * 3;
    assert_eq!(
        price, expected,
        "price should be base + slope*supply on the linear curve"
    );
}

#[test]
fn test_get_price_rejects_unapproved_caller() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    let admin = Address::generate(&env);
    client.set_key_price(&admin, &KEY_PRICE);
    let creator = register_creator(&env, &client);

    // caller was never approved.
    let result = client.try_get_price(&creator, &caller);
    assert_eq!(result, Err(Ok(ContractError::CallerNotApproved)));
}

#[test]
fn test_get_price_rejects_removed_caller() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    let admin = setup_approved(&env, &client, &caller);
    let creator = register_creator(&env, &client);

    assert_eq!(client.get_price(&creator, &caller), KEY_PRICE);

    client.remove_approved_caller(&admin, &caller);

    let result = client.try_get_price(&creator, &caller);
    assert_eq!(result, Err(Ok(ContractError::CallerNotApproved)));
}

#[test]
fn test_get_price_rejects_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    setup_approved(&env, &client, &caller);

    let unknown = Address::generate(&env);
    let result = client.try_get_price(&unknown, &caller);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_get_price_rejects_missing_key_price() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    // Admin + allowlist are configured but no key price is set.
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.add_approved_caller(&admin, &caller);
    let creator = register_creator(&env, &client);

    let result = client.try_get_price(&creator, &caller);
    assert_eq!(result, Err(Ok(ContractError::KeyPriceNotSet)));
}

// ── get_twap_price ───────────────────────────────────────────────────────────

#[test]
fn test_get_twap_returns_current_price_when_window_is_zero() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    setup_approved(&env, &client, &caller);
    let creator = register_creator(&env, &client);

    let twap = client.get_twap_price(&creator, &0u32, &caller);
    assert_eq!(twap, KEY_PRICE);
}

#[test]
fn test_get_twap_returns_current_price_without_history() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    setup_approved(&env, &client, &caller);
    let creator = register_creator(&env, &client);

    let twap = client.get_twap_price(&creator, &200u32, &caller);
    assert_eq!(
        twap, KEY_PRICE,
        "with no recorded observations the TWAP falls back to the current price"
    );
}

#[test]
fn test_get_twap_is_time_weighted_average_over_window() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    setup_approved(&env, &client, &caller);
    let creator = register_creator(&env, &client);
    let buyer = Address::generate(&env);

    // Ledger 100: observe price at supply 0 → 100.
    set_ledger_sequence(&env, 100);
    assert_eq!(client.get_price(&creator, &caller), KEY_PRICE);

    // Buy one key (supply 1) and advance to ledger 200: observe price 110.
    set_ledger_sequence(&env, 200);
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);
    assert_eq!(client.get_price(&creator, &caller), KEY_PRICE + SLOPE);

    // Window of 200 ledgers ending at current ledger 300: TWAP =
    // (100 * 100 + 110 * 100) / 200 = 105.
    set_ledger_sequence(&env, 300);
    let twap = client.get_twap_price(&creator, &200u32, &caller);
    assert_eq!(twap, 105);
}

// ── allowlist admin functions ────────────────────────────────────────────────

#[test]
fn test_admin_can_add_and_remove_approved_callers() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let caller = Address::generate(&env);

    assert!(!client.is_approved_caller(&caller));

    client.add_approved_caller(&admin, &caller);
    assert!(client.is_approved_caller(&caller));

    client.remove_approved_caller(&admin, &caller);
    assert!(!client.is_approved_caller(&caller));
}

#[test]
fn test_add_approved_caller_is_idempotent() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let caller = Address::generate(&env);

    client.add_approved_caller(&admin, &caller);
    client.add_approved_caller(&admin, &caller);
    assert!(client.is_approved_caller(&caller));
}

#[test]
fn test_remove_approved_caller_is_idempotent_on_missing() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let caller = Address::generate(&env);

    // Removing an absent caller is a no-op and must not error.
    client.remove_approved_caller(&admin, &caller);
    assert!(!client.is_approved_caller(&caller));
}

#[test]
fn test_add_approved_caller_reverts_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let non_admin = Address::generate(&env);
    let caller = Address::generate(&env);

    let result = client.try_add_approved_caller(&non_admin, &caller);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_remove_approved_caller_reverts_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let non_admin = Address::generate(&env);
    let caller = Address::generate(&env);

    let result = client.try_remove_approved_caller(&non_admin, &caller);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ── price_queried event ──────────────────────────────────────────────────────

fn find_price_queried_event(env: &Env) -> Option<events::PriceQueriedEvent> {
    env.events()
        .all()
        .iter()
        .find(|(_, topics, _)| {
            let name: Symbol = topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| v.into_val(env))
                .unwrap_or(Symbol::new(env, "none"));
            name == events::PRICE_QUERIED_EVENT_NAME
        })
        .map(|(_, _, data)| data.into_val(env))
}

#[test]
fn test_get_price_emits_price_queried_event() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    setup_approved(&env, &client, &caller);
    let creator = register_creator(&env, &client);

    client.get_price(&creator, &caller);

    let event = find_price_queried_event(&env).expect("price_queried event should be emitted");
    assert_eq!(event.caller, caller);
    assert_eq!(event.creator, creator);
    assert_eq!(event.price, KEY_PRICE);
}

#[test]
fn test_get_twap_price_emits_price_queried_event() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    setup_approved(&env, &client, &caller);
    let creator = register_creator(&env, &client);

    let twap = client.get_twap_price(&creator, &50u32, &caller);

    let event = find_price_queried_event(&env).expect("price_queried event should be emitted");
    assert_eq!(event.caller, caller);
    assert_eq!(event.creator, creator);
    assert_eq!(event.price, twap);
}

#[test]
fn test_get_price_emits_event_on_every_call() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let caller = Address::generate(&env);
    setup_approved(&env, &client, &caller);
    let creator = register_creator(&env, &client);
    let buyer = Address::generate(&env);

    // The harness event log scopes to the most recent entrypoint call, so
    // verify each individual oracle call emits exactly one price_queried event.
    client.get_price(&creator, &caller);
    assert_eq!(count_price_queried_events(&env), 1);

    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);
    assert_eq!(
        count_price_queried_events(&env),
        0,
        "buy must not emit price_queried"
    );

    client.get_price(&creator, &caller);
    assert_eq!(
        count_price_queried_events(&env),
        1,
        "each oracle call emits exactly one price_queried event"
    );
}

fn count_price_queried_events(env: &Env) -> usize {
    env.events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            let name: Symbol = topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| v.into_val(env))
                .unwrap_or(Symbol::new(env, "none"));
            name == events::PRICE_QUERIED_EVENT_NAME
        })
        .count()
}
