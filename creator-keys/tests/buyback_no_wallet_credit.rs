//! Regression test for creator buyback not crediting any wallet key balance (#444).
//!
//! A buyback burns keys from total supply without crediting any wallet. This test
//! confirms that after a buyback no wallet's balance has increased, distinguishing
//! it from a regular buy.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

fn setup(env: &soroban_sdk::Env) -> (creator_keys::CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

/// Records all holder balances for a creator.
fn record_balances(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    holders: &[&Address],
) -> Vec<u32> {
    holders
        .iter()
        .map(|h| client.get_key_balance(creator, h))
        .collect()
}

#[test]
fn test_buyback_does_not_increase_any_wallet_balance() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    // Create multiple holders with different balances
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);
    let holder_c = Address::generate(&env);

    // holder_a buys 3 keys
    client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);

    // holder_b buys 2 keys
    client.buy_key(&creator, &holder_b, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder_b, &KEY_PRICE, &None);

    // holder_c buys 1 key
    client.buy_key(&creator, &holder_c, &KEY_PRICE, &None);

    let holders = vec![&holder_a, &holder_b, &holder_c];
    let balances_before = record_balances(&client, &creator, &holders);

    // Creator buys 2 keys
    client.buy_key(&creator, &creator, &KEY_PRICE, &None);
    client.buy_key(&creator, &creator, &KEY_PRICE, &None);

    let supply_before = client.get_total_key_supply(&creator);
    let total_balance_before: u32 = balances_before.iter().sum();

    // Creator buyback 2 keys from their own balance
    let total_cost = client.get_buyback_quote(&creator, &2);
    client.buyback(&creator, &creator, &2, &total_cost, &None);

    // Check balances after buyback
    let balances_after = record_balances(&client, &creator, &holders);
    let supply_after = client.get_total_key_supply(&creator);
    let total_balance_after: u32 = balances_after.iter().sum();

    // No holder's balance should have increased
    for (i, (before, after)) in balances_before
        .iter()
        .zip(balances_after.iter())
        .enumerate()
    {
        assert!(
            *after <= *before,
            "holder {} balance increased from {} to {} after buyback",
            i,
            before,
            after
        );
    }

    // Total supply should have decreased by the buyback amount
    assert_eq!(
        supply_after,
        supply_before - 2,
        "total supply should decrease by buyback amount"
    );

    // Total balances across all holders should remain unchanged
    // (since the buyback only affects the creator's own balance)
    assert_eq!(
        total_balance_after, total_balance_before,
        "total balances across all holders should not change"
    );
}

#[test]
fn test_buyback_creator_balance_decreases_no_others_change() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    // Create holders
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    // holder_a buys 2 keys
    client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);

    // holder_b buys 1 key
    client.buy_key(&creator, &holder_b, &KEY_PRICE, &None);

    // Creator buys 3 keys
    client.buy_key(&creator, &creator, &KEY_PRICE, &None);
    client.buy_key(&creator, &creator, &KEY_PRICE, &None);
    client.buy_key(&creator, &creator, &KEY_PRICE, &None);

    // Record all balances
    let creator_balance_before = client.get_key_balance(&creator, &creator);
    let holder_a_balance_before = client.get_key_balance(&creator, &holder_a);
    let holder_b_balance_before = client.get_key_balance(&creator, &holder_b);
    let supply_before = client.get_total_key_supply(&creator);

    // Creator buyback 2 keys
    let total_cost = client.get_buyback_quote(&creator, &2);
    client.buyback(&creator, &creator, &2, &total_cost, &None);

    // Check balances
    let creator_balance_after = client.get_key_balance(&creator, &creator);
    let holder_a_balance_after = client.get_key_balance(&creator, &holder_a);
    let holder_b_balance_after = client.get_key_balance(&creator, &holder_b);
    let supply_after = client.get_total_key_supply(&creator);

    // Creator's balance should decrease by buyback amount
    assert_eq!(
        creator_balance_after,
        creator_balance_before - 2,
        "creator balance should decrease by buyback amount"
    );

    // Other holders' balances should remain unchanged
    assert_eq!(
        holder_a_balance_after, holder_a_balance_before,
        "holder_a balance should not change"
    );
    assert_eq!(
        holder_b_balance_after, holder_b_balance_before,
        "holder_b balance should not change"
    );

    // Supply should decrease by buyback amount
    assert_eq!(
        supply_after,
        supply_before - 2,
        "total supply should decrease by buyback amount"
    );
}

#[test]
fn test_buyback_supply_exact_decrease() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    // Creator buys 5 keys
    for _ in 0..5 {
        client.buy_key(&creator, &creator, &KEY_PRICE, &None);
    }

    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(supply_before, 5);

    // Buyback 3 keys
    let total_cost = client.get_buyback_quote(&creator, &3);
    client.buyback(&creator, &creator, &3, &total_cost, &None);

    let supply_after = client.get_total_key_supply(&creator);
    assert_eq!(
        supply_after,
        supply_before - 3,
        "supply should decrease by exact buyback amount"
    );
}

#[test]
fn test_full_buyback_no_wallets_affected() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    // Create a holder
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    // Creator buys 3 keys
    client.buy_key(&creator, &creator, &KEY_PRICE, &None);
    client.buy_key(&creator, &creator, &KEY_PRICE, &None);
    client.buy_key(&creator, &creator, &KEY_PRICE, &None);

    let holder_balance_before = client.get_key_balance(&creator, &holder);
    let _supply_before = client.get_total_key_supply(&creator);

    // Buyback all creator keys (3)
    let total_cost = client.get_buyback_quote(&creator, &3);
    client.buyback(&creator, &creator, &3, &total_cost, &None);

    let holder_balance_after = client.get_key_balance(&creator, &holder);
    let supply_after = client.get_total_key_supply(&creator);

    // Holder's balance should be unchanged
    assert_eq!(
        holder_balance_after, holder_balance_before,
        "holder balance should not change after creator's full buyback"
    );

    // Supply should only reflect the holder's keys
    assert_eq!(
        supply_after, 1,
        "supply should equal remaining holder's keys"
    );
}
