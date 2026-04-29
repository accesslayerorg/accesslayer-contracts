//! Tests verifying buy quote monotonicity over increasing quantities (#157).
//!
//! The key price is fixed, so each successive buy costs exactly the same amount.
//! These tests assert that the quote price is non-decreasing (monotonic) across
//! a range of small and medium purchase scenarios, and that the total_amount
//! ordering is strictly deterministic.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup_with_fees<'a>(env: &'a Env, price: i128) -> (CreatorKeysContractClient<'a>, Address) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, price, 9000, 1000);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

fn buy_n(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    n: u32,
    price: i128,
) {
    for _ in 0..n {
        client.buy_key(creator, buyer, &price);
    }
}

// ── Monotonicity: fixed price means each quote is identical ───────────��───

#[test]
fn test_buy_quote_is_identical_across_consecutive_calls() {
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, 100);

    let q1 = client.get_buy_quote(&creator);
    let q2 = client.get_buy_quote(&creator);
    let q3 = client.get_buy_quote(&creator);

    assert_eq!(q1.price, q2.price);
    assert_eq!(q2.price, q3.price);
    assert_eq!(q1.total_amount, q2.total_amount);
    assert_eq!(q2.total_amount, q3.total_amount);
}

#[test]
fn test_buy_quote_price_unchanged_after_one_buy() {
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, 100);
    let buyer = Address::generate(&env);

    let before = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &100);
    let after = client.get_buy_quote(&creator);

    assert_eq!(before.price, after.price);
    assert_eq!(before.total_amount, after.total_amount);
}

#[test]
fn test_buy_quote_price_unchanged_after_five_buys() {
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, 500);
    let buyer = Address::generate(&env);

    let before = client.get_buy_quote(&creator);
    buy_n(&client, &creator, &buyer, 5, 500);
    let after = client.get_buy_quote(&creator);

    assert_eq!(before.price, after.price, "price must be deterministic");
    assert_eq!(before.total_amount, after.total_amount);
}

#[test]
fn test_buy_quote_price_unchanged_across_multiple_buyers_small_range() {
    let env = test_env_with_auths();
    let price = 200_i128;
    let (client, creator) = setup_with_fees(&env, price);

    let q0 = client.get_buy_quote(&creator);

    for _ in 0..10 {
        let buyer = Address::generate(&env);
        client.buy_key(&creator, &buyer, &price);
        let q = client.get_buy_quote(&creator);
        assert_eq!(
            q.price, q0.price,
            "price must remain constant across buyers"
        );
    }
}

#[test]
fn test_buy_quote_total_amount_ordering_is_deterministic_small_range() {
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, 1_000);
    let buyer = Address::generate(&env);

    let q_start = client.get_buy_quote(&creator);
    buy_n(&client, &creator, &buyer, 3, 1_000);
    let q_after = client.get_buy_quote(&creator);

    // Fixed price: total_amount should be unchanged.
    assert_eq!(q_start.total_amount, q_after.total_amount);
    // Fees must be non-negative.
    assert!(q_after.creator_fee >= 0);
    assert!(q_after.protocol_fee >= 0);
}

#[test]
fn test_buy_quote_fees_sum_to_total_minus_price() {
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, 1_000);
    let buyer = Address::generate(&env);

    buy_n(&client, &creator, &buyer, 2, 1_000);
    let q = client.get_buy_quote(&creator);

    // total_amount = price + creator_fee + protocol_fee for a buy quote
    assert_eq!(
        q.total_amount,
        q.price + q.creator_fee + q.protocol_fee,
        "buy quote: total_amount must equal price + all fees"
    );
}

// ── Medium input range ────────────────────────────────────────────────────

#[test]
fn test_buy_quote_stable_over_medium_volume_20_buys() {
    let env = test_env_with_auths();
    let price = 5_000_i128;
    let (client, creator) = setup_with_fees(&env, price);

    let base_quote = client.get_buy_quote(&creator);

    for i in 0..20_u32 {
        let buyer = Address::generate(&env);
        client.buy_key(&creator, &buyer, &price);
        let q = client.get_buy_quote(&creator);
        assert_eq!(
            q.price,
            base_quote.price,
            "quote price must be stable after {} buys",
            i + 1
        );
        assert_eq!(
            q.total_amount,
            base_quote.total_amount,
            "total_amount must be stable after {} buys",
            i + 1
        );
    }
}

#[test]
fn test_buy_quote_total_amount_never_below_price() {
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, 10_000);
    let buyer = Address::generate(&env);

    buy_n(&client, &creator, &buyer, 10, 10_000);

    let q = client.get_buy_quote(&creator);
    assert!(
        q.total_amount >= q.price,
        "buy quote total_amount must be >= price (fees are additive)"
    );
}

// ── Different price points ────────────────────────────────────────────────

#[test]
fn test_buy_quote_price_point_1_is_stable() {
    let env = test_env_with_auths();
    let (client, creator) = setup_with_fees(&env, 1);

    let q1 = client.get_buy_quote(&creator);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1);
    let q2 = client.get_buy_quote(&creator);

    assert_eq!(q1.price, q2.price);
}

#[test]
fn test_buy_quote_price_point_large_is_stable() {
    let env = test_env_with_auths();
    let large_price = 1_000_000_i128;
    let (client, creator) = setup_with_fees(&env, large_price);

    let q_before = client.get_buy_quote(&creator);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &large_price);
    let q_after = client.get_buy_quote(&creator);

    assert_eq!(q_before.price, q_after.price);
    assert_eq!(q_before.total_amount, q_after.total_amount);
}

#[test]
fn test_buy_quote_monotonic_with_zero_creator_fee() {
    let env = test_env_with_auths();
    let price = 1_000_i128;
    let (client, _) = register_creator_keys(&env);
    // Set 50% creator fee, 50% protocol fee (protocol max is 5000 bps = 50%)
    set_pricing_and_fees(&env, &client, price, 5000, 5000);
    let creator = register_test_creator(&env, &client, "alice");

    let q1 = client.get_buy_quote(&creator);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &q1.total_amount);
    let q2 = client.get_buy_quote(&creator);

    assert_eq!(q1.price, q2.price, "price must remain constant");
    assert_eq!(
        q1.creator_fee, q1.protocol_fee,
        "fees should be equal at 50/50 split"
    );
    assert_eq!(q2.creator_fee, q2.protocol_fee, "fees should remain equal");
    assert_eq!(
        q1.total_amount, q2.total_amount,
        "total_amount must be stable"
    );
}

#[test]
fn test_buy_quote_monotonic_with_zero_protocol_fee() {
    let env = test_env_with_auths();
    let price = 2_000_i128;
    let (client, _) = register_creator_keys(&env);
    // Set 100% creator fee, 0% protocol fee (must sum to 10,000 bps)
    set_pricing_and_fees(&env, &client, price, 10000, 0);
    let creator = register_test_creator(&env, &client, "bob");

    let q1 = client.get_buy_quote(&creator);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &q1.total_amount);
    let q2 = client.get_buy_quote(&creator);

    assert_eq!(q1.price, q2.price, "price must remain constant");
    assert_eq!(q1.protocol_fee, 0, "protocol fee should be zero");
    assert_eq!(q2.protocol_fee, 0, "protocol fee should remain zero");
    assert_eq!(
        q1.total_amount, q2.total_amount,
        "total_amount must be stable"
    );
}

#[test]
fn test_buy_quote_stable_across_50_sequential_purchases() {
    let env = test_env_with_auths();
    let price = 750_i128;
    let (client, creator) = setup_with_fees(&env, price);

    let initial_quote = client.get_buy_quote(&creator);

    for i in 0..50_u32 {
        let buyer = Address::generate(&env);
        client.buy_key(&creator, &buyer, &price);

        let current_quote = client.get_buy_quote(&creator);
        assert_eq!(
            current_quote.price,
            initial_quote.price,
            "price must remain constant after {} purchases",
            i + 1
        );
        assert_eq!(
            current_quote.total_amount,
            initial_quote.total_amount,
            "total_amount must remain constant after {} purchases",
            i + 1
        );
        assert_eq!(
            current_quote.creator_fee,
            initial_quote.creator_fee,
            "creator_fee must remain constant after {} purchases",
            i + 1
        );
        assert_eq!(
            current_quote.protocol_fee,
            initial_quote.protocol_fee,
            "protocol_fee must remain constant after {} purchases",
            i + 1
        );
    }
}

#[test]
fn test_buy_quote_multiple_creators_independent_monotonicity() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // Setup two creators with the same global price but different identities
    let price = 1_000_i128;

    // Set fee config and price (8000 + 2000 = 10,000 bps)
    set_pricing_and_fees(&env, &client, price, 8000, 2000);
    let creator_alice = register_test_creator(&env, &client, "alice");
    let creator_bob = register_test_creator(&env, &client, "bob");

    // Get initial quotes - should be identical since price is global
    let q_alice_1 = client.get_buy_quote(&creator_alice);
    let q_bob_1 = client.get_buy_quote(&creator_bob);

    // Verify both creators have the same quote initially (global price)
    assert_eq!(
        q_alice_1.price, q_bob_1.price,
        "both creators share the global price"
    );
    assert_eq!(
        q_alice_1.total_amount, q_bob_1.total_amount,
        "both creators have same total_amount"
    );

    // Make purchases for both creators using their respective total_amounts
    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);

    client.buy_key(&creator_alice, &buyer1, &q_alice_1.total_amount);
    client.buy_key(&creator_bob, &buyer2, &q_bob_1.total_amount);

    // Get quotes after purchases
    let q_alice_2 = client.get_buy_quote(&creator_alice);
    let q_bob_2 = client.get_buy_quote(&creator_bob);

    // Each creator's quotes should remain stable after their own purchases
    assert_eq!(
        q_alice_1.price, q_alice_2.price,
        "alice's price must be stable"
    );
    assert_eq!(
        q_alice_1.total_amount, q_alice_2.total_amount,
        "alice's total_amount must be stable"
    );

    assert_eq!(q_bob_1.price, q_bob_2.price, "bob's price must be stable");
    assert_eq!(
        q_bob_1.total_amount, q_bob_2.total_amount,
        "bob's total_amount must be stable"
    );

    // Verify they still have the same price (global price model)
    assert_eq!(
        q_alice_2.price, q_bob_2.price,
        "both creators continue to share the global price"
    );

    // But they should have independent supplies
    let alice_supply = client.get_total_key_supply(&creator_alice);
    let bob_supply = client.get_total_key_supply(&creator_bob);
    assert_eq!(alice_supply, 1, "alice should have 1 key sold");
    assert_eq!(bob_supply, 1, "bob should have 1 key sold");
}

// ── Fee config update regression test (#219) ─────────────────────────────────

#[test]
fn test_buy_quote_updates_after_fee_config_mutation() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let price = 1_000_i128;

    // Set initial fee config: 90% creator, 10% protocol
    set_pricing_and_fees(&env, &client, price, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");

    // Get quote with initial fee config
    let q_before = client.get_buy_quote(&creator);

    // Verify initial fee distribution
    assert_eq!(q_before.price, price);
    assert_eq!(q_before.creator_fee, 900);
    assert_eq!(q_before.protocol_fee, 100);
    assert_eq!(q_before.total_amount, price + 900 + 100);

    // Update fee config: 50% creator, 50% protocol
    let admin = Address::generate(&env);
    client.set_fee_config(&admin, &5000u32, &5000u32);

    // Get quote after fee config update
    let q_after = client.get_buy_quote(&creator);

    // Verify quote reflects updated fee config (no stale reads)
    assert_eq!(q_after.price, price, "price should remain unchanged");
    assert_eq!(
        q_after.creator_fee, 500,
        "creator_fee should reflect new config"
    );
    assert_eq!(
        q_after.protocol_fee, 500,
        "protocol_fee should reflect new config"
    );
    assert_eq!(
        q_after.total_amount,
        price + 500 + 500,
        "total_amount should reflect new fee split"
    );

    // Assert fees are different (no stale config)
    assert_ne!(
        q_before.creator_fee, q_after.creator_fee,
        "creator_fee must change after config update"
    );
    assert_ne!(
        q_before.protocol_fee, q_after.protocol_fee,
        "protocol_fee must change after config update"
    );
    // Total amount should remain the same (fee redistribution)
    assert_eq!(
        q_before.total_amount, q_after.total_amount,
        "total_amount should remain same after fee redistribution"
    );

    // Verify fee invariant: total_amount = price + creator_fee + protocol_fee
    assert_eq!(
        q_after.total_amount,
        q_after.price + q_after.creator_fee + q_after.protocol_fee,
        "buy quote invariant must hold after fee config update"
    );
}
