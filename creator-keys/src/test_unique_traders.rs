//! Unique trader analytics (#985).
//!
//! The per-creator counting already existed behind `get_analytics`. These tests
//! cover the additions: the `get_unique_trader_count` and `has_traded` views,
//! and the `UniqueTraderAdded` event that fires exactly once per wallet.

use super::*;
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{Address, Env, String, Symbol, TryIntoVal};

/// Register a contract with an admin and a flat curve, as `test.rs` does.
/// Duplicated rather than imported: `test.rs`'s helpers are private to that
/// module.
fn setup_env_with_creator<'a>(env: &'a Env) -> (CreatorKeysContractClient<'a>, Address, Address) {
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_fee_config(&admin, &9000, &1000);
    client.set_key_price(&admin, &1000i128);
    client.set_curve_slope(&admin, &0i128);

    let creator = Address::generate(env);
    client.register_creator(
        &crate::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "testcreator"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    (client, admin, creator)
}

/// Count the `uniq_trd` events published by the most recent contract call.
///
/// `env.events().all()` holds only the latest invocation's events, so this
/// answers "did the call I just made emit one?" rather than giving a running
/// total. Assert with it immediately after the call under test.
///
/// Topics come back as raw `Val`, which has no `PartialEq`, so the first topic
/// is converted to a `Symbol` before comparing. A topic that is not a symbol
/// belongs to some other event and simply does not match.
fn unique_trader_events(env: &Env) -> u32 {
    env.events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|topic| {
                    let name: Result<Symbol, _> = topic.try_into_val(env);
                    name.ok()
                })
                .is_some_and(|name| name == events::UNIQUE_TRADER_ADDED_EVENT_NAME)
        })
        .count() as u32
}

/// Move past the flash-loan guard, which rejects a sell in the buy's ledger.
fn next_ledger(env: &Env) {
    env.ledger()
        .set_sequence_number(env.ledger().sequence() + 1);
}

#[test]
fn test_unique_trader_count_view_starts_at_zero() {
    let env = Env::default();
    let (client, _admin, creator) = setup_env_with_creator(&env);

    assert_eq!(client.get_unique_trader_count(&creator), 0u64);
}

#[test]
fn test_unique_trader_count_view_matches_analytics() {
    let env = Env::default();
    let (client, _admin, creator) = setup_env_with_creator(&env);
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    client.buy_key(&creator, &b1, &1000i128, &None);
    client.buy_key(&creator, &b2, &2000i128, &None);

    assert_eq!(client.get_unique_trader_count(&creator), 2u64);
    assert_eq!(
        client.get_unique_trader_count(&creator),
        client.get_analytics(&creator).unique_traders,
    );
}

#[test]
fn test_unique_trader_count_unchanged_by_repeat_trades() {
    let env = Env::default();
    let (client, _admin, creator) = setup_env_with_creator(&env);
    let buyer = Address::generate(&env);

    client.buy_key(&creator, &buyer, &1000i128, &None);
    assert_eq!(client.get_unique_trader_count(&creator), 1u64);

    client.buy_key(&creator, &buyer, &2000i128, &None);
    next_ledger(&env);
    client.sell_key(&creator, &buyer, &None);

    assert_eq!(client.get_unique_trader_count(&creator), 1u64);
    assert_eq!(client.get_analytics(&creator).trade_count, 3u64);
}

#[test]
fn test_has_traded_false_for_untouched_wallet() {
    let env = Env::default();
    let (client, _admin, creator) = setup_env_with_creator(&env);
    let stranger = Address::generate(&env);

    assert!(!client.has_traded(&creator, &stranger));
}

#[test]
fn test_has_traded_true_after_first_buy() {
    let env = Env::default();
    let (client, _admin, creator) = setup_env_with_creator(&env);
    let buyer = Address::generate(&env);

    assert!(!client.has_traded(&creator, &buyer));
    client.buy_key(&creator, &buyer, &1000i128, &None);
    assert!(client.has_traded(&creator, &buyer));
}

#[test]
fn test_has_traded_is_per_wallet() {
    let env = Env::default();
    let (client, _admin, creator) = setup_env_with_creator(&env);
    let trader = Address::generate(&env);
    let bystander = Address::generate(&env);

    client.buy_key(&creator, &trader, &1000i128, &None);

    assert!(client.has_traded(&creator, &trader));
    assert!(!client.has_traded(&creator, &bystander));
}

#[test]
fn test_has_traded_stays_true_after_selling_out() {
    let env = Env::default();
    let (client, _admin, creator) = setup_env_with_creator(&env);
    let buyer = Address::generate(&env);

    client.buy_key(&creator, &buyer, &1000i128, &None);
    next_ledger(&env);
    client.sell_key(&creator, &buyer, &None);

    // The flag records that a trade happened, not that a balance is held.
    assert!(client.has_traded(&creator, &buyer));
}

#[test]
fn test_unique_trader_event_emitted_once_per_wallet() {
    let env = Env::default();
    let (client, _admin, creator) = setup_env_with_creator(&env);
    let buyer = Address::generate(&env);

    client.buy_key(&creator, &buyer, &1000i128, &None);
    assert_eq!(
        unique_trader_events(&env),
        1,
        "first buy from a wallet must emit the event"
    );

    // Neither a repeat buy nor a sell from the same wallet re-emits.
    client.buy_key(&creator, &buyer, &2000i128, &None);
    assert_eq!(
        unique_trader_events(&env),
        0,
        "a repeat buy must not re-emit"
    );

    next_ledger(&env);
    client.sell_key(&creator, &buyer, &None);
    assert_eq!(unique_trader_events(&env), 0, "a sell must not re-emit");

    assert_eq!(client.get_unique_trader_count(&creator), 1u64);
}

#[test]
fn test_unique_trader_event_emitted_for_each_new_wallet() {
    let env = Env::default();
    let (client, _admin, creator) = setup_env_with_creator(&env);
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let b3 = Address::generate(&env);

    // Checked per call: the helper only sees the latest invocation's events.
    client.buy_key(&creator, &b1, &1000i128, &None);
    assert_eq!(unique_trader_events(&env), 1);

    client.buy_key(&creator, &b2, &2000i128, &None);
    assert_eq!(unique_trader_events(&env), 1);

    client.buy_key(&creator, &b3, &3000i128, &None);
    assert_eq!(unique_trader_events(&env), 1);

    assert_eq!(client.get_unique_trader_count(&creator), 3u64);
}

#[test]
fn test_unique_trader_count_after_bulk_trades() {
    let env = Env::default();
    let (client, _admin, creator) = setup_env_with_creator(&env);

    // Ten wallets, each trading twice — the count must track wallets, not trades.
    // The crate is no_std, so this is a soroban Vec rather than a std one.
    let mut wallets = soroban_sdk::Vec::new(&env);
    for _ in 0..10 {
        wallets.push_back(Address::generate(&env));
    }

    for wallet in wallets.iter() {
        client.buy_key(&creator, &wallet, &10_000i128, &None);
    }
    for wallet in wallets.iter() {
        client.buy_key(&creator, &wallet, &10_000i128, &None);
    }

    assert_eq!(client.get_unique_trader_count(&creator), 10u64);
    assert_eq!(client.get_analytics(&creator).trade_count, 20u64);

    for wallet in wallets.iter() {
        assert!(client.has_traded(&creator, &wallet));
    }
}
