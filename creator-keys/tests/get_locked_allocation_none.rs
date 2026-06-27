//! Unit tests for `get_locked_allocation` returning `None` when a creator
//! was registered without a locked allocation, and `Some` when one was set.
//! Also asserts the expected error for a non-existent creator.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use creator_keys::LockedAllocation;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, String};

#[test]
fn test_get_locked_allocation_returns_none_when_not_set() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = register_test_creator(&env, &client, "alice");

    let result = client.get_locked_allocation(&creator);
    assert_eq!(
        result, None,
        "get_locked_allocation must return None for creator registered without locked allocation"
    );
}

#[test]
fn test_get_locked_allocation_returns_some_when_set() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "bob");
    let unlock_ledger: u32 = 5000;
    let amount: u32 = 100;

    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 1;
    env.ledger().set(ledger_info);

    client.register_creator(
        &creator,
        &handle,
        &Some(LockedAllocation {
            amount,
            unlock_ledger,
            claimed: false,
        }),
        &None,
     &None);

    let result = client.get_locked_allocation(&creator);
    assert!(
        result.is_some(),
        "get_locked_allocation must return Some when locked allocation was set"
    );

    let alloc = result.unwrap();
    assert_eq!(alloc.amount, amount, "locked amount must match");
    assert_eq!(
        alloc.unlock_ledger, unlock_ledger,
        "unlock_ledger must match"
    );
    assert!(!alloc.claimed, "allocation must not be claimed initially");
}

#[test]
fn test_get_locked_allocation_returns_none_for_nonexistent_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let unknown = Address::generate(&env);

    let result = client.get_locked_allocation(&unknown);
    assert_eq!(
        result, None,
        "get_locked_allocation must return None for a non-existent creator"
    );
}
