//! Regression test: locked allocation claim must revert before unlock_ledger
//! and succeed exactly at unlock_ledger.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{ContractError, LockedAllocation};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, String};

#[test]
fn test_claim_locked_allocation_reverts_at_every_ledger_before_unlock() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    let unlock_ledger: u32 = 1000;

    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 1;
    env.ledger().set(ledger_info.clone());

    client.register_creator(
        &creator,
        &handle,
        &Some(LockedAllocation {
            amount: 50,
            unlock_ledger,
            claimed: false,
        }),
        &None,
        &None,
        &None,
        &None,
    );

    // Immediately after registration — must revert.
    let result = client.try_claim_locked_allocation(&creator);
    assert_eq!(
        result,
        Err(Ok(ContractError::AllocationLocked)),
        "claim must revert immediately after registration"
    );

    // At unlock_ledger - 1 — must still revert.
    ledger_info.sequence_number = unlock_ledger - 1;
    env.ledger().set(ledger_info);

    let result = client.try_claim_locked_allocation(&creator);
    assert_eq!(
        result,
        Err(Ok(ContractError::AllocationLocked)),
        "claim must revert at unlock_ledger minus one"
    );
}

#[test]
fn test_claim_locked_allocation_succeeds_at_unlock_ledger() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    let unlock_ledger: u32 = 1000;
    let amount: u32 = 50;

    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 1;
    env.ledger().set(ledger_info.clone());

    client.register_creator(
        &creator,
        &handle,
        &Some(LockedAllocation {
            amount,
            unlock_ledger,
            claimed: false,
        }),
        &None,
        &None,
        &None,
        &None,
    );

    // Advance to exactly unlock_ledger.
    ledger_info.sequence_number = unlock_ledger;
    env.ledger().set(ledger_info);

    client.claim_locked_allocation(&creator);

    let stored = client.get_locked_allocation(&creator).unwrap();
    assert!(stored.claimed, "allocation must be marked as claimed");

    let balance = client.get_key_balance(&creator, &creator);
    assert_eq!(
        balance, amount,
        "keys must be transferred to creator balance"
    );
}
