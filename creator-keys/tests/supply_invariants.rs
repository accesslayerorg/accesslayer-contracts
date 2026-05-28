//! Invariant tests for total supply after mixed buy/sell trades (#167).
//!
//! Asserts that supply conservation rules hold across all mixed trade sequences:
//!   - total supply never goes negative
//!   - sum of individual holder balances equals total supply
//!   - supply decrements correctly on sells
//!   - supply is consistent across multiple participants

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup<'a>(env: &'a Env, price: i128) -> (CreatorKeysContractClient<'a>, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, price);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

// ── Supply conservation: buy then sell returns to prior state ───────────���─

#[test]
fn test_supply_buy_then_sell_returns_to_zero() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);
    let buyer = Address::generate(&env);

    assert_eq!(client.get_total_key_supply(&creator), 0);
    client.buy_key(&creator, &buyer, &100);
    assert_eq!(client.get_total_key_supply(&creator), 1);
    client.sell_key(&creator, &buyer);
    assert_eq!(client.get_total_key_supply(&creator), 0);
}

#[test]
fn test_supply_buy_two_sell_one_conserves_supply() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);
    let buyer = Address::generate(&env);

    client.buy_key(&creator, &buyer, &100);
    client.buy_key(&creator, &buyer, &100);
    assert_eq!(client.get_total_key_supply(&creator), 2);

    client.sell_key(&creator, &buyer);
    assert_eq!(client.get_total_key_supply(&creator), 1);
}

#[test]
fn test_supply_alternating_buys_and_sells() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);
    let buyer = Address::generate(&env);

    // buy → sell → buy → sell: supply must be 0 at end
    client.buy_key(&creator, &buyer, &100);
    client.sell_key(&creator, &buyer);
    client.buy_key(&creator, &buyer, &100);
    client.sell_key(&creator, &buyer);

    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);
}

// ── Multi-participant scenarios ───────────────────────────────────────────

#[test]
fn test_supply_three_buyers_sum_equals_total() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let b3 = Address::generate(&env);

    client.buy_key(&creator, &b1, &100);
    client.buy_key(&creator, &b2, &100);
    client.buy_key(&creator, &b3, &100);

    let bal1 = client.get_key_balance(&creator, &b1);
    let bal2 = client.get_key_balance(&creator, &b2);
    let bal3 = client.get_key_balance(&creator, &b3);
    let total = client.get_total_key_supply(&creator);

    assert_eq!(
        bal1 + bal2 + bal3,
        total,
        "sum of individual balances must equal total supply"
    );
}

#[test]
fn test_supply_multiple_buys_per_holder_sum_equals_total() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    client.buy_key(&creator, &b1, &100);
    client.buy_key(&creator, &b1, &100);
    client.buy_key(&creator, &b2, &100);

    let bal1 = client.get_key_balance(&creator, &b1);
    let bal2 = client.get_key_balance(&creator, &b2);
    let total = client.get_total_key_supply(&creator);

    assert_eq!(
        bal1 + bal2,
        total,
        "sum of per-holder balances must equal total supply"
    );
    assert_eq!(bal1, 2);
    assert_eq!(bal2, 1);
}

#[test]
fn test_supply_mixed_trades_three_participants() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let carol = Address::generate(&env);

    // Alice buys 2, Bob buys 1, Carol buys 2
    client.buy_key(&creator, &alice, &100);
    client.buy_key(&creator, &alice, &100);
    client.buy_key(&creator, &bob, &100);
    client.buy_key(&creator, &carol, &100);
    client.buy_key(&creator, &carol, &100);

    assert_eq!(client.get_total_key_supply(&creator), 5);

    // Alice sells 1, Carol sells 2
    client.sell_key(&creator, &alice);
    client.sell_key(&creator, &carol);
    client.sell_key(&creator, &carol);

    let bal_alice = client.get_key_balance(&creator, &alice);
    let bal_bob = client.get_key_balance(&creator, &bob);
    let bal_carol = client.get_key_balance(&creator, &carol);
    let total = client.get_total_key_supply(&creator);

    assert_eq!(bal_alice + bal_bob + bal_carol, total);
    assert_eq!(total, 2); // alice:1 + bob:1 + carol:0
    assert_eq!(bal_alice, 1);
    assert_eq!(bal_bob, 1);
    assert_eq!(bal_carol, 0);
}

#[test]
fn test_supply_never_goes_below_zero_after_all_sells() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100);
    client.buy_key(&creator, &buyer, &100);
    client.buy_key(&creator, &buyer, &100);
    client.sell_key(&creator, &buyer);
    client.sell_key(&creator, &buyer);
    client.sell_key(&creator, &buyer);

    assert_eq!(client.get_total_key_supply(&creator), 0);
}

// ── Supply isolation across creators ─────────────────────────────────────

#[test]
fn test_supply_changes_for_one_creator_do_not_affect_another() {
    let env = test_env_with_auths();
    let (client, creator_a) = setup(&env, 100);
    let creator_b = register_test_creator(&env, &client, "bob");

    let buyer = Address::generate(&env);

    client.buy_key(&creator_a, &buyer, &100);
    client.buy_key(&creator_a, &buyer, &100);
    client.sell_key(&creator_a, &buyer);

    // creator_b supply untouched
    assert_eq!(client.get_total_key_supply(&creator_b), 0);
    assert_eq!(client.get_total_key_supply(&creator_a), 1);
}

// ── Holder count mirrors supply conservation ──────────────────────────────

#[test]
fn test_holder_count_reflects_mixed_trade_correctly() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);

    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);

    client.buy_key(&creator, &b1, &100);
    client.buy_key(&creator, &b2, &100);
    assert_eq!(client.get_creator_holder_count(&creator), 2);

    client.sell_key(&creator, &b1);
    assert_eq!(client.get_creator_holder_count(&creator), 1);

    client.sell_key(&creator, &b2);
    assert_eq!(client.get_creator_holder_count(&creator), 0);
    assert_eq!(client.get_total_key_supply(&creator), 0);
}

#[test]
fn test_holder_count_unchanged_when_holder_still_has_keys() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env, 100);
    let buyer = Address::generate(&env);

    client.buy_key(&creator, &buyer, &100);
    client.buy_key(&creator, &buyer, &100);
    client.sell_key(&creator, &buyer);

    // Buyer still holds 1 key — holder count must stay at 1
    assert_eq!(client.get_creator_holder_count(&creator), 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);
}
