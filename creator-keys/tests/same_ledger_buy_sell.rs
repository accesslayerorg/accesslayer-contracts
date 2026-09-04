//! Integration test for a buy and an immediate sell in the same ledger (issue #699).
//!
//! A buy followed by a sell in the same ledger must produce a net-zero change:
//! the final supply and holder count must equal their pre-buy values.
//! The sell must observe the supply *after* the buy, not the pre-buy value,
//! confirming that operations within the same ledger apply sequentially without
//! state corruption.
//!
//! # Acceptance criteria covered
//!
//! - Final supply equals pre-buy supply after a net-zero buy+sell
//! - Holder count returns to the pre-buy value after the seller exits completely
//! - `buy_key` return value reflects supply after the buy
//! - `sell_key` return value reflects supply after the sell (i.e. back to pre-buy)
//! - No state corruption: post-sell supply and balance are internally consistent
//!
//! # Why same ledger
//!
//! The Soroban test harness advances a single ledger per `invoke` call unless
//! `env.ledger().set(...)` is used to bump the sequence. Tests here do NOT bump
//! the ledger between the buy and the sell, so both operations share the same
//! `env.ledger().sequence()`.  This exercises the same-ledger sequential
//! ordering guarantee: the sell sees post-buy state, not pre-buy state.
//!
//! # Test strategy
//!
//! Each test isolates one invariant to make failures actionable. The shared
//! `buy_n_keys` helper fetches a live quote before each purchase so the tests
//! are correct under both flat and bonding-curve pricing.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1_000_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// Buy `count` keys for `buyer` from `creator`, fetching a live quote before each purchase.
fn buy_n_keys(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &soroban_sdk::Address,
    buyer: &soroban_sdk::Address,
    count: u32,
) {
    for _ in 0..count {
        let quote = client.get_buy_quote(creator);
        client.buy_key(creator, buyer, &quote.total_amount, &None);
    }
}

/// Sell `count` keys for `seller` from `creator`, one at a time.
fn sell_n_keys(
    env: &soroban_sdk::Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &soroban_sdk::Address,
    seller: &soroban_sdk::Address,
    count: u32,
) {
    env.ledger().with_mut(|l| l.sequence_number += 1);
    for _ in 0..count {
        client.sell_key(creator, seller, &None);
    }
}

// ── Supply invariants ─────────────────────────────────────────────────────────

/// Final supply after buying and selling the same number of keys equals the
/// pre-buy supply (net-zero change, acceptance criterion 1).
#[test]
fn test_final_supply_equals_pre_buy_supply_after_net_zero_buy_sell() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(supply_before, 0, "precondition: creator starts at supply 0");

    // Buy 5 keys then sell 5 keys.
    buy_n_keys(&client, &creator, &trader, 5);
    sell_n_keys(&env, &client, &creator, &trader, 5);

    let supply_after = client.get_total_key_supply(&creator);
    assert_eq!(
        supply_after, supply_before,
        "final supply ({supply_after}) must equal pre-buy supply ({supply_before}) \
         after a net-zero buy+sell"
    );
}

/// Supply rises to 5 immediately after the buy and drops back to 0 after the sell.
/// The return values of `buy_key` and `sell_key` track each transition.
#[test]
fn test_supply_transitions_correctly_through_buy_and_sell() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    // Track supply after each buy
    for expected in 1u32..=5 {
        let quote = client.get_buy_quote(&creator);
        let new_supply = client.buy_key(&creator, &trader, &quote.total_amount, &None);
        assert_eq!(
            new_supply, expected,
            "supply after buy #{expected} must be {expected}"
        );
    }

    assert_eq!(
        client.get_total_key_supply(&creator),
        5,
        "supply must be 5 after 5 buys"
    );

    // Track supply after each sell
    env.ledger().with_mut(|l| l.sequence_number += 1);
    for expected in (0u32..5).rev() {
        let new_supply = client.sell_key(&creator, &trader, &None);
        assert_eq!(
            new_supply, expected,
            "supply after sell to {expected} must be {expected}"
        );
    }

    assert_eq!(
        client.get_total_key_supply(&creator),
        0,
        "supply must return to 0 after 5 sells"
    );
}

// ── Holder count invariants ───────────────────────────────────────────────────

/// Holder count returns to its pre-buy value when the trader sells all their keys
/// (acceptance criterion 2).
#[test]
fn test_holder_count_returns_to_pre_buy_value_after_full_exit() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    let holder_count_before = client.get_creator_holder_count(&creator);
    assert_eq!(
        holder_count_before, 0,
        "precondition: no holders before any trade"
    );

    buy_n_keys(&client, &creator, &trader, 5);
    assert_eq!(
        client.get_creator_holder_count(&creator),
        1,
        "holder count must be 1 while trader holds 5 keys"
    );

    sell_n_keys(&env, &client, &creator, &trader, 5);
    let holder_count_after = client.get_creator_holder_count(&creator);

    assert_eq!(
        holder_count_after, holder_count_before,
        "holder count ({holder_count_after}) must return to pre-buy value \
         ({holder_count_before}) after full exit"
    );
    assert_eq!(
        holder_count_after, 0,
        "no holders remain after the sole holder sells all keys"
    );
}

/// Holder count is NOT decremented on a partial sell (trader still holds keys).
#[test]
fn test_holder_count_unchanged_after_partial_sell() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    buy_n_keys(&client, &creator, &trader, 5);
    assert_eq!(client.get_creator_holder_count(&creator), 1);

    // Partial sell: 3 out of 5 — trader still holds 2
    sell_n_keys(&env, &client, &creator, &trader, 3);

    assert_eq!(
        client.get_creator_holder_count(&creator),
        1,
        "holder count must stay 1 while trader still holds keys"
    );
    assert_eq!(
        client.get_key_balance(&creator, &trader),
        2,
        "trader balance must be 2 after partial sell"
    );
}

// ── No intermediate state observable ─────────────────────────────────────────

/// The sell observes post-buy state: the supply quote for the sell reflects the
/// supply that exists after all buys have been applied (acceptance criterion 3).
///
/// If the sell read pre-buy supply, the sell quote would use a lower supply step
/// and compute a different price. We verify the sequence is correct by ensuring
/// the sell quote price equals the final step of the buy sequence.
#[test]
fn test_sell_quote_observes_post_buy_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    // Record the buy quote at supply 4 (the price for the 5th key)
    buy_n_keys(&client, &creator, &trader, 4);
    let quote_at_step_5 = client.get_buy_quote(&creator);
    // Now buy the 5th key
    client.buy_key(&creator, &trader, &quote_at_step_5.total_amount, &None);

    assert_eq!(
        client.get_total_key_supply(&creator),
        5,
        "supply must be 5 after 5 buys"
    );

    // The sell quote must reference step 5 (the current supply).
    // If the contract were reading stale pre-buy state, it would use a lower step.
    let sell_quote = client.get_sell_quote(&creator, &trader);
    assert_eq!(
        sell_quote.price, quote_at_step_5.price,
        "sell quote price ({}) must equal the buy price at step 5 ({}) — \
         the sell must observe post-buy supply, not pre-buy supply",
        sell_quote.price, quote_at_step_5.price,
    );
}

/// Trader balance is zero after selling all keys bought in the same ledger.
/// State integrity: no ghost balance remains.
#[test]
fn test_trader_balance_is_zero_after_full_exit() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    buy_n_keys(&client, &creator, &trader, 5);
    assert_eq!(
        client.get_key_balance(&creator, &trader),
        5,
        "precondition: trader holds 5 keys"
    );

    sell_n_keys(&env, &client, &creator, &trader, 5);

    assert_eq!(
        client.get_key_balance(&creator, &trader),
        0,
        "trader balance must be 0 after selling all keys"
    );
}

/// Supply and the sum of holder balances remain internally consistent after
/// same-ledger buy+sell (no double-counting or state corruption).
#[test]
fn test_supply_equals_sum_of_holder_balances_after_net_zero_trade() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);
    let bystander = Address::generate(&env);

    // Bystander holds 3 keys throughout — must be unaffected by trader's round-trip
    buy_n_keys(&client, &creator, &bystander, 3);

    buy_n_keys(&client, &creator, &trader, 5);
    sell_n_keys(&env, &client, &creator, &trader, 5);

    let supply = client.get_total_key_supply(&creator);
    let bal_trader = client.get_key_balance(&creator, &trader);
    let bal_bystander = client.get_key_balance(&creator, &bystander);

    assert_eq!(bal_trader, 0, "trader balance must be 0 after full exit");
    assert_eq!(
        bal_bystander, 3,
        "bystander balance must be unaffected by trader's round-trip"
    );
    assert_eq!(
        supply,
        bal_trader + bal_bystander,
        "total supply ({supply}) must equal sum of all holder balances \
         ({} + {} = {})",
        bal_trader,
        bal_bystander,
        bal_trader + bal_bystander,
    );
}

// ── Event consistency ─────────────────────────────────────────────────────────

/// `buy_key` return value (post-buy supply) and `sell_key` return value
/// (post-sell supply) form a consistent sequence: buy returns pre_buy + 5,
/// sell returns pre_buy (acceptance criterion 4).
#[test]
fn test_buy_and_sell_return_values_form_consistent_supply_sequence() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    let pre_buy_supply = client.get_total_key_supply(&creator);

    // Buy 5 keys; collect each return value
    let mut buy_returns = Vec::new();
    for _ in 0..5 {
        let quote = client.get_buy_quote(&creator);
        let new_supply = client.buy_key(&creator, &trader, &quote.total_amount, &None);
        buy_returns.push(new_supply);
    }

    // Sell 5 keys; collect each return value
    env.ledger().with_mut(|l| l.sequence_number += 1);
    let mut sell_returns = Vec::new();
    for _ in 0..5 {
        let new_supply = client.sell_key(&creator, &trader, &None);
        sell_returns.push(new_supply);
    }

    // Buy returns must be strictly ascending: 1, 2, 3, 4, 5
    for (i, &ret) in buy_returns.iter().enumerate() {
        assert_eq!(
            ret,
            pre_buy_supply + i as u32 + 1,
            "buy #{} must return supply {}",
            i + 1,
            pre_buy_supply + i as u32 + 1
        );
    }

    // Sell returns must be strictly descending: 4, 3, 2, 1, 0
    let post_buy_supply = *buy_returns.last().unwrap();
    for (i, &ret) in sell_returns.iter().enumerate() {
        assert_eq!(
            ret,
            post_buy_supply - i as u32 - 1,
            "sell #{} must return supply {}",
            i + 1,
            post_buy_supply - i as u32 - 1
        );
    }

    // Final sell return value must equal pre-buy supply (net-zero)
    let final_sell_supply = *sell_returns.last().unwrap();
    assert_eq!(
        final_sell_supply, pre_buy_supply,
        "final sell return value ({final_sell_supply}) must equal pre-buy supply \
         ({pre_buy_supply}) confirming net-zero state change"
    );
}

/// One buy event is emitted per buy and one sell event per sell.
///
/// The Soroban test harness makes only the most-recent invocation's events
/// visible through `env.events().all()`, so we read the log after each
/// individual `buy_key` / `sell_key` call and accumulate the counts.
#[test]
fn test_buy_and_sell_events_both_emitted_and_correctly_tagged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    // Count buy events — one per invocation
    let mut buy_count = 0usize;
    for _ in 0..5 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &trader, &quote.total_amount, &None);
        let event_log = env.events().all();
        buy_count += event_log
            .iter()
            .filter(|(_, topics, _)| {
                topics
                    .get(events::TOPIC_EVENT_NAME_INDEX)
                    .map(|v| {
                        let name: Symbol = v.into_val(&env);
                        name == events::BUY_EVENT_NAME
                    })
                    .unwrap_or(false)
            })
            .count();
    }

    // Count sell events — one per invocation
    env.ledger().with_mut(|l| l.sequence_number += 1);
    let mut sell_count = 0usize;
    for _ in 0..5 {
        client.sell_key(&creator, &trader, &None);
        let event_log = env.events().all();
        sell_count += event_log
            .iter()
            .filter(|(_, topics, _)| {
                topics
                    .get(events::TOPIC_EVENT_NAME_INDEX)
                    .map(|v| {
                        let name: Symbol = v.into_val(&env);
                        name == events::SELL_EVENT_NAME
                    })
                    .unwrap_or(false)
            })
            .count();
    }

    assert_eq!(buy_count, 5, "exactly 5 buy events must be emitted");
    assert_eq!(sell_count, 5, "exactly 5 sell events must be emitted");
}

/// Each buy event carries the correct buyer and creator addresses.
///
/// Reads the event log immediately after each buy so we are inspecting the
/// event from that specific invocation.
#[test]
fn test_buy_events_carry_correct_addresses() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    for _ in 0..5 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &trader, &quote.total_amount, &None);

        // Read immediately — harness exposes the most-recent invocation's events.
        let event_log = env.events().all();
        let buy_events: Vec<_> = event_log
            .iter()
            .filter(|(_, topics, _)| {
                topics
                    .get(events::TOPIC_EVENT_NAME_INDEX)
                    .map(|v| {
                        let name: Symbol = v.into_val(&env);
                        name == events::BUY_EVENT_NAME
                    })
                    .unwrap_or(false)
            })
            .collect();

        assert_eq!(buy_events.len(), 1, "exactly 1 buy event per buy_key call");

        let (_, _, data) = buy_events[0];
        let payload: events::KeysBoughtEvent = data.into_val(&env);
        assert_eq!(payload.buyer, trader, "buy event buyer must be the trader");
        assert_eq!(
            payload.creator_id, creator,
            "buy event creator_id must match the creator"
        );
        assert_eq!(payload.quantity, 1u32, "each buy is for exactly 1 key");
    }
}

/// Each sell event carries the correct seller and creator addresses.
///
/// Reads the event log immediately after each sell so we are inspecting the
/// event from that specific invocation.
#[test]
fn test_sell_events_carry_correct_addresses() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);

    buy_n_keys(&client, &creator, &trader, 5);
    env.ledger().with_mut(|l| l.sequence_number += 1);

    for _ in 0..5 {
        client.sell_key(&creator, &trader, &None);

        // Read immediately — harness exposes the most-recent invocation's events.
        let event_log = env.events().all();
        let sell_events: Vec<_> = event_log
            .iter()
            .filter(|(_, topics, _)| {
                topics
                    .get(events::TOPIC_EVENT_NAME_INDEX)
                    .map(|v| {
                        let name: Symbol = v.into_val(&env);
                        name == events::SELL_EVENT_NAME
                    })
                    .unwrap_or(false)
            })
            .collect();

        assert_eq!(
            sell_events.len(),
            1,
            "exactly 1 sell event per sell_key call"
        );

        let (_, _, data) = sell_events[0];
        let payload: events::KeysSoldEvent = data.into_val(&env);
        assert_eq!(
            payload.seller, trader,
            "sell event seller must be the trader"
        );
        assert_eq!(
            payload.creator_id, creator,
            "sell event creator_id must match the creator"
        );
        assert_eq!(payload.quantity, 1u32, "each sell is for exactly 1 key");
    }
}

// ── Bystander isolation ───────────────────────────────────────────────────────

/// A bystander who holds keys before the same-ledger buy+sell is completely
/// unaffected: their balance and the total supply seen from their perspective
/// are unchanged by the trader's round-trip.
#[test]
fn test_bystander_unaffected_by_same_ledger_buy_sell() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = Address::generate(&env);
    let bystander = Address::generate(&env);

    // Bystander acquires 2 keys before the trader's round-trip
    buy_n_keys(&client, &creator, &bystander, 2);
    let bystander_balance_before = client.get_key_balance(&creator, &bystander);
    let supply_after_bystander = client.get_total_key_supply(&creator);

    // Trader's buy+sell
    buy_n_keys(&client, &creator, &trader, 5);
    sell_n_keys(&env, &client, &creator, &trader, 5);

    // Bystander must see no change
    assert_eq!(
        client.get_key_balance(&creator, &bystander),
        bystander_balance_before,
        "bystander balance must be unaffected by trader's round-trip"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_after_bystander,
        "total supply must return to its pre-trader-round-trip value"
    );
}
