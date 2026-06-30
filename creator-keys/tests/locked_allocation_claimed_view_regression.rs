//! Regression test verifying `get_locked_allocation` returns claimed `true` after claim.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::LockedAllocation;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, String};

#[test]
fn test_get_locked_allocation_returns_claimed_true_after_claim() {
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
    );

    // Assert claimed field is false before claim
    let alloc_before = client.get_locked_allocation(&creator).unwrap();
    assert!(!alloc_before.claimed, "claimed field must be false before claim");

    // Advance to exactly unlock_ledger.
    ledger_info.sequence_number = unlock_ledger;
    env.ledger().set(ledger_info);

    // Claim the allocation
    client.claim_locked_allocation(&creator);

    // Call get_locked_allocation and assert claimed is true
    let alloc_after1 = client.get_locked_allocation(&creator).unwrap();
    assert!(alloc_after1.claimed, "claimed field must be true after successful claim");

    // Call again and assert it is still true
    let alloc_after2 = client.get_locked_allocation(&creator).unwrap();
    assert!(alloc_after2.claimed, "claimed field must remain true on subsequent reads");
}
