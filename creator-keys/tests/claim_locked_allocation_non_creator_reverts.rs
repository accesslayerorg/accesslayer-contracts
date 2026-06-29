//! Unit tests for time-locked allocation claiming by a non-creator wallet.
//!
//! The contract enforces per-creator allocation ownership: `claim_locked_allocation(creator)`
//! calls `creator.require_auth()` and looks up the allocation keyed by that address.
//! A wallet that is not the registered creator has no allocation stored under its address
//! and will receive `NotRegistered`, confirming it cannot access another creator's locked keys.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{ContractError, LockedAllocation};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, String};

const UNLOCK_LEDGER: u32 = 100;
const ALLOCATION_AMOUNT: u32 = 50;

fn setup_creator_with_locked_allocation(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
) -> Address {
    let creator = Address::generate(env);
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 1;
    env.ledger().set(ledger_info);
    client.register_creator(
        &creator,
        &String::from_str(env, "alice"),
        &Some(LockedAllocation {
            amount: ALLOCATION_AMOUNT,
            unlock_ledger: UNLOCK_LEDGER,
            claimed: false,
        }),
        &None,
        &None,
        &None,
    );
    creator
}

fn advance_past_unlock(env: &Env) {
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = UNLOCK_LEDGER + 1;
    env.ledger().set(ledger_info);
}

#[test]
fn test_non_creator_claim_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let _creator = setup_creator_with_locked_allocation(&env, &client);
    advance_past_unlock(&env);

    let non_creator = Address::generate(&env);
    let result = client.try_claim_locked_allocation(&non_creator);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "non-creator must not be able to claim another creator's allocation"
    );
}

#[test]
fn test_locked_keys_remain_unclaimed_after_failed_non_creator_attempt() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = setup_creator_with_locked_allocation(&env, &client);
    advance_past_unlock(&env);

    let non_creator = Address::generate(&env);
    let _ = client.try_claim_locked_allocation(&non_creator);

    let stored = client
        .get_locked_allocation(&creator)
        .expect("creator allocation must still exist");
    assert!(
        !stored.claimed,
        "allocation must remain unclaimed after a failed non-creator attempt"
    );
}

#[test]
fn test_creator_can_claim_after_failed_non_creator_attempt() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = setup_creator_with_locked_allocation(&env, &client);
    advance_past_unlock(&env);

    let non_creator = Address::generate(&env);
    let _ = client.try_claim_locked_allocation(&non_creator);

    client.claim_locked_allocation(&creator);

    let stored = client
        .get_locked_allocation(&creator)
        .expect("creator allocation must exist after claim");
    assert!(
        stored.claimed,
        "allocation must be marked claimed after creator claims"
    );

    let balance = client.get_key_balance(&creator, &creator);
    assert_eq!(
        balance, ALLOCATION_AMOUNT,
        "creator must receive the locked key allocation"
    );
}
