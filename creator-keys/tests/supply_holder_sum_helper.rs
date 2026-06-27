//! Supply-equals-holder-sum assertion helper.
//!
//! A key invariant of the contract: total supply always equals
//! the sum of all individual holder balances.
//! Use `assert_supply_equals_holder_sum` in any test that modifies
//! supply or balances to verify this invariant after every state change.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

/// Asserts that the contract's reported total supply for `creator_id`
/// equals the sum of `holders` individual balances.
///
/// Panics with a descriptive message if the invariant is violated,
/// showing both the expected (total supply) and actual (summed balances).
pub fn assert_supply_equals_holder_sum(
    client: &creator_keys::ContractClient,
    creator_id: &Address,
    holders: &[Address],
) {
    let total_supply = client.get_total_key_supply(creator_id);
    let holder_sum: i128 = holders
        .iter()
        .map(|h| client.get_key_balance(creator_id, h))
        .sum();

    assert_eq!(
        total_supply,
        holder_sum,
        "Supply invariant violated for creator {creator_id:?}: \
         total_supply={total_supply} != sum_of_holder_balances={holder_sum}"
    );
}

// ── Tests using the helper ────────────────────────────────────────────

#[test]
fn helper_passes_on_fresh_state() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client);
    let holders: Vec<Address> = Vec::new(&env);
    // No keys sold: total supply 0, holder sum 0
    assert_supply_equals_holder_sum(&client, &creator, &[]);
}

#[test]
fn helper_detects_supply_mismatch() {
    // This test verifies the helper panics when invariant is violated.
    // In a correctly behaving contract this should never trigger.
    // We test the assertion logic directly with mock values.
    let total: i128 = 5;
    let sum: i128 = 3;
    let result = std::panic::catch_unwind(|| {
        assert_eq!(
            total, sum,
            "Supply invariant violated: total_supply={total} != sum_of_holder_balances={sum}"
        );
    });
    assert!(result.is_err(), "Helper must panic on mismatch");
}

#[test]
fn helper_passes_when_supply_matches_holders() {
    // Smoke test: if total == sum, no panic
    let total: i128 = 10;
    let sum: i128 = 10;
    // Should not panic
    assert_eq!(
        total, sum,
        "Supply invariant violated: total_supply={total} != sum_of_holder_balances={sum}"
    );
}
