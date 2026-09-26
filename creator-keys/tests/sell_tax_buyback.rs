//! Integration coverage for the configurable sell tax and buyback pool.
//!
//! 1. `set_sell_tax_bps` stores a per-key tax and is restricted to the creator
//!    within the admin ceiling.
//! 2. The tax is deducted from every sell's proceeds.
//! 3. The collected tax is forwarded to the buyback pool in the same call.
//! 4. `SellTaxCollected` is emitted with the amount and pool address.
//! 5. `get_buyback_pool_balance` reports the accumulated pool balance.
//! 6. A zero-tax key deducts nothing and leaves the pool untouched.
//!
//! Proceeds depend on the protocol fee split, so the deduction tests measure the
//! untaxed baseline from an identically configured second creator and compare
//! against it rather than hardcoding fee-derived amounts.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{
    events::{SellTaxCollectedEvent, SELL_TAX_COLLECTED_EVENT_NAME},
    ContractError, CreatorKeysContractClient, RegisterCreatorParams, SellTaxError,
    MAX_SELL_TAX_BPS,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    Address, Env, IntoVal, String, Symbol,
};

const KEY_PRICE: i128 = 100_000;
const CREATOR_BPS: u32 = 2_000;
const PROTOCOL_BPS: u32 = 1_000;

type Client<'a> = CreatorKeysContractClient<'a>;

fn setup(env: &Env) -> (Client<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);
    client.set_fee_config(&admin, &CREATOR_BPS, &PROTOCOL_BPS);
    (client, admin)
}

fn register_creator(env: &Env, client: &Client<'_>, handle: &str) -> Address {
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

/// Moves the ledger forward so a sell never lands in the ledger of its buy.
fn advance(env: &Env) {
    env.ledger()
        .set_sequence_number(env.ledger().sequence() + 10);
}

/// Register a creator and give `holder` one key on a fresh ledger.
fn creator_with_holder(env: &Env, client: &Client<'_>, handle: &str) -> (Address, Address) {
    let creator = register_creator(env, client, handle);
    let holder = Address::generate(env);
    advance(env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    (creator, holder)
}

fn zero_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ))
}

fn tax_events(env: &Env) -> soroban_sdk::Vec<SellTaxCollectedEvent> {
    let mut found = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == SELL_TAX_COLLECTED_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

/// Everything a single `sell_key` call reported, captured in one pass.
///
/// `env.events()` only retains the most recent invocation, so every assertion
/// about a sell has to read it before making another contract call.
struct SellObservation {
    proceeds: i128,
    tax_events: Vec<SellTaxCollectedEvent>,
}

fn observe_sell(env: &Env) -> SellObservation {
    let mut proceeds = None;
    let mut taxes = std::vec::Vec::new();
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == creator_keys::events::SELL_EVENT_NAME {
            let payload: creator_keys::events::KeysSoldEvent = data.into_val(env);
            proceeds = Some(payload.proceeds);
        } else if name == SELL_TAX_COLLECTED_EVENT_NAME {
            taxes.push(data.into_val(env));
        }
    }
    SellObservation {
        proceeds: proceeds.expect("no sell event was emitted"),
        tax_events: taxes,
    }
}

/// Proceeds reported by the `sell` event for the most recent sell.
fn last_sell_proceeds(env: &Env) -> i128 {
    observe_sell(env).proceeds
}

/// Untaxed baseline proceeds, measured from a creator with no tax configured.
fn baseline_proceeds(env: &Env, client: &Client<'_>, handle: &str) -> i128 {
    let (creator, holder) = creator_with_holder(env, client, handle);
    advance(env);
    client.sell_key(&creator, &holder, &None);
    observe_sell(env).proceeds
}

// ---------------------------------------------------------------------------
// 1. set_sell_tax_bps
// ---------------------------------------------------------------------------

#[test]
fn test_sell_tax_defaults_to_zero() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    assert_eq!(client.get_sell_tax_bps(&creator), 0);
    assert_eq!(client.get_buyback_pool_balance(), (0, None));
}

#[test]
fn test_set_sell_tax_bps_stores_creator_tax() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    client.set_sell_tax_bps(&creator, &creator, &500);
    assert_eq!(client.get_sell_tax_bps(&creator), 500);
}

#[test]
fn test_set_sell_tax_bps_accepts_exact_ceiling() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    client.set_sell_tax_bps(&creator, &creator, &MAX_SELL_TAX_BPS);
    assert_eq!(client.get_sell_tax_bps(&creator), MAX_SELL_TAX_BPS);
}

#[test]
fn test_set_sell_tax_bps_rejects_above_ceiling() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    let result = client.try_set_sell_tax_bps(&creator, &creator, &(MAX_SELL_TAX_BPS + 1));
    assert_eq!(result, Err(Ok(SellTaxError::TaxExceedsMax)));
    assert_eq!(
        client.get_sell_tax_bps(&creator),
        0,
        "a rejected update must not persist"
    );
}

#[test]
fn test_set_sell_tax_bps_rejects_non_creator_caller() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let impostor = Address::generate(&env);

    let result = client.try_set_sell_tax_bps(&impostor, &creator, &100);
    assert_eq!(result, Err(Ok(SellTaxError::Unauthorized)));
    assert_eq!(client.get_sell_tax_bps(&creator), 0);
}

#[test]
fn test_set_sell_tax_bps_rejects_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let stranger = Address::generate(&env);

    let result = client.try_set_sell_tax_bps(&stranger, &stranger, &100);
    assert_eq!(result, Err(Ok(SellTaxError::NotRegistered)));
}

#[test]
fn test_set_sell_tax_bps_zero_disables_tax() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    client.set_sell_tax_bps(&creator, &creator, &400);
    client.set_sell_tax_bps(&creator, &creator, &0);
    assert_eq!(client.get_sell_tax_bps(&creator), 0);
}

#[test]
fn test_sell_tax_is_scoped_per_creator() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let alice = register_creator(&env, &client, "alice");
    let bob = register_creator(&env, &client, "bobby");

    client.set_sell_tax_bps(&alice, &alice, &300);
    assert_eq!(client.get_sell_tax_bps(&alice), 300);
    assert_eq!(client.get_sell_tax_bps(&bob), 0);
}

// ---------------------------------------------------------------------------
// 2 + 3. Deduction and pool forwarding
// ---------------------------------------------------------------------------

#[test]
fn test_sell_tax_deducted_from_proceeds() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let base = baseline_proceeds(&env, &client, "baseline");
    assert!(
        base > 0,
        "the fee split must leave the seller some proceeds"
    );

    let (creator, holder) = creator_with_holder(&env, &client, "alice");
    let tax_bps = 1_000_u32;
    client.set_sell_tax_bps(&creator, &creator, &tax_bps);
    advance(&env);
    client.sell_key(&creator, &holder, &None);

    let expected_tax = base * tax_bps as i128 / 10_000;
    assert_eq!(last_sell_proceeds(&env), base - expected_tax);
}

#[test]
fn test_tax_forwarded_to_buyback_pool_atomically() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let pool = Address::generate(&env);
    client.set_buyback_pool_address(&admin, &pool);
    let base = baseline_proceeds(&env, &client, "baseline");

    let (creator, holder) = creator_with_holder(&env, &client, "alice");
    let tax_bps = 1_000_u32;
    client.set_sell_tax_bps(&creator, &creator, &tax_bps);
    assert_eq!(client.get_buyback_pool_balance(), (0, Some(pool.clone())));

    advance(&env);
    client.sell_key(&creator, &holder, &None);

    let expected_tax = base * tax_bps as i128 / 10_000;
    let (balance, reported_pool) = client.get_buyback_pool_balance();
    assert_eq!(
        balance, expected_tax,
        "the pool is credited in the same call as the sell"
    );
    assert_eq!(reported_pool, Some(pool));
}

#[test]
fn test_pool_balance_accumulates_across_creators_and_sells() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    client.set_buyback_pool_address(&admin, &Address::generate(&env));
    let base = baseline_proceeds(&env, &client, "baseline");

    let (alice, alice_holder) = creator_with_holder(&env, &client, "alice");
    let (bob, bob_holder) = creator_with_holder(&env, &client, "bobby");
    client.set_sell_tax_bps(&alice, &alice, &1_000);
    client.set_sell_tax_bps(&bob, &bob, &500);

    advance(&env);
    client.sell_key(&alice, &alice_holder, &None);
    let after_first = client.get_buyback_pool_balance().0;
    advance(&env);
    client.sell_key(&bob, &bob_holder, &None);
    let after_second = client.get_buyback_pool_balance().0;

    let alice_tax = base * 1_000 / 10_000;
    let bob_tax = base * 500 / 10_000;
    assert_eq!(after_first, alice_tax);
    assert_eq!(after_second, alice_tax + bob_tax);
}

#[test]
fn test_zero_tax_key_collects_nothing() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    client.set_buyback_pool_address(&admin, &Address::generate(&env));
    let base = baseline_proceeds(&env, &client, "baseline");

    let (creator, holder) = creator_with_holder(&env, &client, "alice");
    client.set_sell_tax_bps(&creator, &creator, &0);
    advance(&env);
    client.sell_key(&creator, &holder, &None);
    let sell = observe_sell(&env);

    assert!(sell.tax_events.is_empty(), "no tax event for a zero tax");
    assert_eq!(
        sell.proceeds, base,
        "the seller receives the full untaxed proceeds"
    );
    assert_eq!(
        client.get_buyback_pool_balance().0,
        0,
        "a zero tax must leave the pool untouched"
    );
}

#[test]
fn test_key_without_configured_tax_collects_nothing() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    client.set_buyback_pool_address(&admin, &Address::generate(&env));
    let base = baseline_proceeds(&env, &client, "baseline");

    // No set_sell_tax_bps call at all.
    let (creator, holder) = creator_with_holder(&env, &client, "alice");
    advance(&env);
    client.sell_key(&creator, &holder, &None);
    let sell = observe_sell(&env);

    assert!(sell.tax_events.is_empty());
    assert_eq!(sell.proceeds, base);
    assert_eq!(client.get_sell_tax_bps(&creator), 0);
    assert_eq!(client.get_buyback_pool_balance().0, 0);
}

#[test]
fn test_tax_too_small_to_floor_collects_nothing() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    // A 1 bp tax on a price of 9 floors to 0, so nothing is collected.
    client.set_key_price(&admin, &9);
    client.set_buyback_pool_address(&admin, &Address::generate(&env));

    let creator = register_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    advance(&env);
    client.buy_key(&creator, &holder, &9, &None);
    client.set_sell_tax_bps(&creator, &creator, &1);
    advance(&env);
    client.sell_key(&creator, &holder, &None);
    let sell = observe_sell(&env);

    assert!(sell.tax_events.is_empty());
    // Price 9, no trade fee, creator fee floors to 1, so 8 is the untaxed payout.
    assert_eq!(sell.proceeds, 8, "the seller keeps the untaxed 8");
    assert_eq!(client.get_buyback_pool_balance().0, 0);
}

// ---------------------------------------------------------------------------
// 4. SellTaxCollected event
// ---------------------------------------------------------------------------

#[test]
fn test_sell_tax_collected_event_emitted_with_amount_and_pool() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let pool = Address::generate(&env);
    client.set_buyback_pool_address(&admin, &pool);
    let base = baseline_proceeds(&env, &client, "baseline");

    let (creator, holder) = creator_with_holder(&env, &client, "alice");
    let tax_bps = 1_000_u32;
    client.set_sell_tax_bps(&creator, &creator, &tax_bps);
    advance(&env);
    client.sell_key(&creator, &holder, &None);
    let sell = observe_sell(&env);

    assert_eq!(sell.tax_events.len(), 1);
    let event = &sell.tax_events[0];
    let expected_tax = base * tax_bps as i128 / 10_000;
    assert_eq!(event.amount, expected_tax);
    assert_eq!(event.pool, pool);
    assert_eq!(event.creator, creator);
    assert_eq!(event.seller, holder);
    assert_eq!(event.tax_bps, tax_bps);
    assert_eq!(event.gross_proceeds, base);
    assert_eq!(event.net_proceeds, base - expected_tax);
    assert_eq!(event.pool_balance, expected_tax);
    assert_eq!(event.ledger, env.ledger().sequence());
}

#[test]
fn test_sell_tax_collected_event_reports_zero_pool_when_unassigned() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice");
    client.set_sell_tax_bps(&creator, &creator, &500);
    advance(&env);
    client.sell_key(&creator, &holder, &None);

    let event = tax_events(&env).get(0).unwrap();
    // Until an admin assigns a pool the tax is held against the zero address.
    assert_eq!(event.pool, zero_address(&env));
}

#[test]
fn test_sell_tax_collected_event_topics_carry_creator_and_seller() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice");
    client.set_sell_tax_bps(&creator, &creator, &500);
    advance(&env);
    client.sell_key(&creator, &holder, &None);

    let mut checked = false;
    for (_, topics, _) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&env);
        if name == SELL_TAX_COLLECTED_EVENT_NAME {
            let topic_creator: Address = topics.get(1).unwrap().into_val(&env);
            let topic_seller: Address = topics.get(2).unwrap().into_val(&env);
            assert_eq!(topic_creator, creator);
            assert_eq!(topic_seller, holder);
            checked = true;
        }
    }
    assert!(checked, "expected a SellTaxCollected event");
}

#[test]
fn test_sell_tax_update_event_reports_old_and_new() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    client.set_sell_tax_bps(&creator, &creator, &200);
    client.set_sell_tax_bps(&creator, &creator, &600);

    let mut found = soroban_sdk::Vec::new(&env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&env);
        if name == creator_keys::events::SELL_TAX_UPDATED_EVENT_NAME {
            found.push_back(data.into_val(&env));
        }
    }
    let event: creator_keys::events::SellTaxUpdatedEvent = found.get(0).unwrap();
    assert_eq!(event.old_tax_bps, 200);
    assert_eq!(event.new_tax_bps, 600);
}

// ---------------------------------------------------------------------------
// 5. Buyback pool address
// ---------------------------------------------------------------------------

#[test]
fn test_set_buyback_pool_address_requires_admin() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let impostor = Address::generate(&env);
    let pool = Address::generate(&env);

    let result = client.try_set_buyback_pool_address(&impostor, &pool);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_set_buyback_pool_address_rejects_zero_address() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);

    let result = client.try_set_buyback_pool_address(&admin, &zero_address(&env));
    assert_eq!(result, Err(Ok(ContractError::ZeroAddress)));
}

#[test]
fn test_buyback_pool_balance_is_read_only() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let pool = Address::generate(&env);
    client.set_buyback_pool_address(&admin, &pool);

    let first = client.get_buyback_pool_balance();
    let second = client.get_buyback_pool_balance();
    assert_eq!(first, second);
    assert_eq!(first, (0, Some(pool)));
}

// ---------------------------------------------------------------------------
// 6. Regression guards
// ---------------------------------------------------------------------------

#[test]
fn test_tax_does_not_change_supply_or_balances() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice");
    client.set_sell_tax_bps(&creator, &creator, &1_000);

    let supply_before = client.get_total_key_supply(&creator);
    advance(&env);
    client.sell_key(&creator, &holder, &None);

    assert_eq!(client.get_total_key_supply(&creator), supply_before - 1);
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
}

#[test]
fn test_tax_reconciles_net_plus_tax_equals_gross() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice");
    client.set_sell_tax_bps(&creator, &creator, &100);
    advance(&env);
    client.sell_key(&creator, &holder, &None);
    let sell = observe_sell(&env);

    assert_eq!(sell.tax_events.len(), 1);
    let event = &sell.tax_events[0];
    assert_eq!(event.tax_bps, 100);
    assert_eq!(
        event.net_proceeds + event.amount,
        event.gross_proceeds,
        "net proceeds plus tax must equal gross proceeds"
    );
    assert_eq!(
        sell.proceeds, event.net_proceeds,
        "the sell event must report the post-tax amount the seller received"
    );
    assert_eq!(client.get_buyback_pool_balance().0, event.amount);
}

#[test]
fn test_tax_applies_to_every_sell() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    for _ in 0..3 {
        advance(&env);
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }
    let base = baseline_proceeds(&env, &client, "baseline");

    let tax_bps = 1_000_u32;
    client.set_sell_tax_bps(&creator, &creator, &tax_bps);

    // Every one of the holder's three sells is taxed and credited.
    for round in 0..3 {
        advance(&env);
        client.sell_key(&creator, &holder, &None);
        let expected_tax = base * tax_bps as i128 / 10_000;
        assert_eq!(
            client.get_buyback_pool_balance().0,
            expected_tax * (round as i128 + 1),
            "round {} must credit the pool again",
            round
        );
    }
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
}
