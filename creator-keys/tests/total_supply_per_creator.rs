//! Unit tests for `get_total_key_supply` scoping per creator (issue #701).
//!
//! The `get_total_key_supply` view must return the total keys in circulation
//! for a *specific* creator — not a global count across all creators. These tests
//! confirm that supply tracking is fully isolated per creator address and that
//! no operation on one creator's keys leaks into another creator's supply counter.
//!
//! # Acceptance criteria covered
//!
//! - Supply scoped correctly per creator
//! - Creator A supply unaffected by creator B buys
//! - Sell correctly decrements supply
//! - Unregistered creator returns `NotRegistered` from the checked view
//!
//! # Test strategy
//!
//! Each test below exercises one narrow invariant so failures pin-point exactly
//! which property broke. A shared `buy_n_keys` helper fetches a live quote
//! before every purchase so the tests work correctly under both flat and
//! bonding-curve pricing models.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::Ledger as _;

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

// ── Baseline: supply starts at zero ─────────────────────────────────────────

#[test]
fn test_supply_is_zero_immediately_after_creator_registration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");

    assert_eq!(
        client.get_total_key_supply(&creator),
        0,
        "supply must be 0 right after registration before any buys"
    );
}

// ── Issue spec: creator A — 10 buys ─────────────────────────────────────────

#[test]
fn test_supply_equals_ten_after_ten_buys_for_creator_a() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let buyer_a = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator_a, &buyer_a, 10);

    assert_eq!(
        client.get_total_key_supply(&creator_a),
        10,
        "supply must equal the number of keys bought"
    );
}

// ── Issue spec: creator B — 5 buys ──────────────────────────────────────────

#[test]
fn test_supply_equals_five_after_five_buys_for_creator_b() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_b = register_test_creator(&env, &client, "bob");
    let buyer_b = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator_b, &buyer_b, 5);

    assert_eq!(
        client.get_total_key_supply(&creator_b),
        5,
        "supply must equal the number of keys bought"
    );
}

// ── Issue spec: creator A supply unaffected by creator B buys ───────────────

#[test]
fn test_creator_a_supply_unaffected_by_creator_b_buys() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");
    let buyer_a = soroban_sdk::Address::generate(&env);
    let buyer_b = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator_a, &buyer_a, 10);
    assert_eq!(client.get_total_key_supply(&creator_a), 10);

    buy_n_keys(&client, &creator_b, &buyer_b, 5);
    assert_eq!(client.get_total_key_supply(&creator_b), 5);

    // After creator B's 5 buys, creator A's supply must still be exactly 10
    assert_eq!(
        client.get_total_key_supply(&creator_a),
        10,
        "creator A supply must not change when creator B keys are purchased"
    );
}

// ── Issue spec: creator B supply unaffected by creator A buys ───────────────

#[test]
fn test_creator_b_supply_unaffected_by_creator_a_buys() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");
    let buyer_a = soroban_sdk::Address::generate(&env);
    let buyer_b = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator_b, &buyer_b, 5);
    assert_eq!(client.get_total_key_supply(&creator_b), 5);

    buy_n_keys(&client, &creator_a, &buyer_a, 10);
    assert_eq!(client.get_total_key_supply(&creator_a), 10);

    // After creator A's 10 buys, creator B's supply must still be exactly 5
    assert_eq!(
        client.get_total_key_supply(&creator_b),
        5,
        "creator B supply must not change when creator A keys are purchased"
    );
}

// ── Issue spec: sell correctly decrements supply ─────────────────────────────

#[test]
fn test_sell_three_keys_decrements_creator_a_supply_from_ten_to_seven() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let buyer_a = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator_a, &buyer_a, 10);
    assert_eq!(client.get_total_key_supply(&creator_a), 10);

    for _ in 0..3 {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator_a, &buyer_a, &None);
    }

    assert_eq!(
        client.get_total_key_supply(&creator_a),
        7,
        "selling 3 keys from a supply of 10 must yield 7"
    );
}

// ── Sell on creator A does not affect creator B ──────────────────────────────

#[test]
fn test_selling_from_creator_a_does_not_affect_creator_b_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");
    let buyer_a = soroban_sdk::Address::generate(&env);
    let buyer_b = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator_a, &buyer_a, 10);
    buy_n_keys(&client, &creator_b, &buyer_b, 5);

    for _ in 0..3 {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator_a, &buyer_a, &None);
    }

    assert_eq!(client.get_total_key_supply(&creator_a), 7);
    assert_eq!(
        client.get_total_key_supply(&creator_b),
        5,
        "creator B supply must not change when creator A keys are sold"
    );
}

// ── Selling on creator B does not affect creator A ───────────────────────────

#[test]
fn test_selling_from_creator_b_does_not_affect_creator_a_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");
    let buyer_a = soroban_sdk::Address::generate(&env);
    let buyer_b = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator_a, &buyer_a, 10);
    buy_n_keys(&client, &creator_b, &buyer_b, 5);

    for _ in 0..3 {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator_b, &buyer_b, &None);
    }

    assert_eq!(client.get_total_key_supply(&creator_b), 2);
    assert_eq!(
        client.get_total_key_supply(&creator_a),
        10,
        "creator A supply must not change when creator B keys are sold"
    );
}

// ── Supply increments one-by-one on each buy ────────────────────────────────

#[test]
fn test_supply_increments_by_one_per_buy() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    for expected in 1u32..=5 {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &buyer, &quote.total_amount, &None);
        assert_eq!(
            client.get_total_key_supply(&creator),
            expected,
            "supply after buy #{expected} must be {expected}"
        );
    }
}

// ── Supply decrements one-by-one on each sell ───────────────────────────────

#[test]
fn test_supply_decrements_by_one_per_sell() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator, &buyer, 5);
    assert_eq!(client.get_total_key_supply(&creator), 5);

    for expected in (0u32..5).rev() {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &buyer, &None);
        assert_eq!(
            client.get_total_key_supply(&creator),
            expected,
            "supply after sell must be {expected}"
        );
    }
}

// ── Multiple buyers contribute to the same creator's supply ─────────────────

#[test]
fn test_supply_accumulates_across_multiple_buyers_for_same_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer_1 = soroban_sdk::Address::generate(&env);
    let buyer_2 = soroban_sdk::Address::generate(&env);
    let buyer_3 = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator, &buyer_1, 3);
    buy_n_keys(&client, &creator, &buyer_2, 4);
    buy_n_keys(&client, &creator, &buyer_3, 2);

    // Total supply = 3 + 4 + 2 = 9 regardless of how many buyers contributed
    assert_eq!(
        client.get_total_key_supply(&creator),
        9,
        "supply must be the sum across all buyers, not capped per buyer"
    );
}

// ── Three-creator isolation ──────────────────────────────────────────────────

#[test]
fn test_three_creators_maintain_independent_supply_counters() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");
    let creator_c = register_test_creator(&env, &client, "carol");
    let buyer = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator_a, &buyer, 4);
    buy_n_keys(&client, &creator_b, &buyer, 7);
    buy_n_keys(&client, &creator_c, &buyer, 2);

    assert_eq!(client.get_total_key_supply(&creator_a), 4);
    assert_eq!(client.get_total_key_supply(&creator_b), 7);
    assert_eq!(client.get_total_key_supply(&creator_c), 2);
}

// ── Selling all keys drops supply to zero ───────────────────────────────────

#[test]
fn test_supply_reaches_zero_after_selling_all_keys() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator, &buyer, 3);
    assert_eq!(client.get_total_key_supply(&creator), 3);

    for _ in 0..3 {
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &buyer, &None);
    }

    assert_eq!(
        client.get_total_key_supply(&creator),
        0,
        "supply must return to 0 after all keys are sold"
    );
}

// ── Issue spec: unregistered creator returns NotRegistered ──────────────────

#[test]
fn test_unregistered_creator_returns_not_registered_from_checked_view() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let unregistered = soroban_sdk::Address::generate(&env);

    // get_creator_supply (checked view) must fail with NotRegistered
    let result = client.try_get_creator_supply(&unregistered);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "checked supply view must return NotRegistered for an unregistered creator"
    );
}

#[test]
fn test_unregistered_creator_returns_zero_from_unchecked_view() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let unregistered = soroban_sdk::Address::generate(&env);

    // get_total_key_supply (unchecked view) returns 0 for unknown addresses,
    // consistent with the contract's safe-read design
    assert_eq!(
        client.get_total_key_supply(&unregistered),
        0,
        "unchecked supply view must return 0 for unregistered creator, not panic"
    );
}

// ── Supply is read-only (calling it repeatedly does not mutate state) ────────

#[test]
fn test_get_total_key_supply_is_idempotent_and_read_only() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator, &buyer, 5);

    let s1 = client.get_total_key_supply(&creator);
    let s2 = client.get_total_key_supply(&creator);
    let s3 = client.get_total_key_supply(&creator);

    assert_eq!(s1, 5);
    assert_eq!(s1, s2, "supply view must be idempotent (1st vs 2nd call)");
    assert_eq!(s2, s3, "supply view must be idempotent (2nd vs 3rd call)");
}

// ── Supply tracks holder balance sum ────────────────────────────────────────

#[test]
fn test_total_supply_equals_sum_of_all_holder_balances() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer_1 = soroban_sdk::Address::generate(&env);
    let buyer_2 = soroban_sdk::Address::generate(&env);

    buy_n_keys(&client, &creator, &buyer_1, 3);
    buy_n_keys(&client, &creator, &buyer_2, 2);

    // Sell 1 from buyer_1 so we have an asymmetric balance
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &buyer_1, &None);

    let total_supply = client.get_total_key_supply(&creator);
    let balance_1 = client.get_key_balance(&creator, &buyer_1);
    let balance_2 = client.get_key_balance(&creator, &buyer_2);

    assert_eq!(
        total_supply,
        balance_1 + balance_2,
        "total supply must equal the sum of every holder's individual balance"
    );
}
