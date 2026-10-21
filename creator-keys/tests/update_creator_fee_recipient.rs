//! Unit tests for `update_creator_fee_recipient` (#681).
//!
//! Covers:
//! - valid address update persists in contract state
//! - zero address is rejected with `ContractError::ZeroAddress`
//! - caller that is not the current fee recipient is rejected
//! - successive updates store the second address
//! - old recipient is absent from state after a successful update

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use creator_keys::{
    constants, ContractError, CreatorKeysContract, CreatorKeysContractClient, CreatorProfile,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn zero_address(env: &Env) -> Address {
    let zero_str = String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    Address::from_string(&zero_str)
}

#[test]
fn test_update_creator_fee_recipient_valid_address_persists() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let new_recipient = Address::generate(&env);

    let result = client.try_update_creator_fee_recipient(&creator, &new_recipient);
    assert_eq!(result, Ok(Ok(())), "valid address update should succeed");

    assert_eq!(
        client.get_creator_fee_recipient(&creator),
        new_recipient,
        "new recipient must be persisted in contract state"
    );
    assert_eq!(
        client.get_creator(&creator).fee_recipient,
        new_recipient,
        "creator profile fee_recipient must match the updated address"
    );
}

#[test]
fn test_update_creator_fee_recipient_rejects_zero_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let zero_addr = zero_address(&env);

    let result = client.try_update_creator_fee_recipient(&creator, &zero_addr);
    assert_eq!(
        result,
        Err(Ok(ContractError::ZeroAddress)),
        "zero address must be rejected (invalid recipient)"
    );

    assert_eq!(
        client.get_creator_fee_recipient(&creator),
        creator,
        "fee recipient must remain the creator after zero-address rejection"
    );
}

#[test]
fn test_update_creator_fee_recipient_unauthorized_caller_reverts() {
    // No mock_all_auths: current fee recipient must authorize the update.
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let current_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    let profile = CreatorProfile {
        creator: creator.clone(),
        handle: String::from_str(&env, "alice"),
        supply: 0,
        holder_count: 0,
        fee_recipient: current_recipient.clone(),
        registered_at: 0,
    };

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::creator(&creator), &profile);
    });

    let result = client.try_update_creator_fee_recipient(&creator, &new_recipient);
    assert!(
        result.is_err(),
        "caller that is not the authorized fee recipient must be rejected"
    );

    assert_eq!(
        client.get_creator_fee_recipient(&creator),
        current_recipient,
        "fee recipient must not change when unauthorized call is rejected"
    );
}

#[test]
fn test_update_creator_fee_recipient_second_update_overwrites_first() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let first_recipient = Address::generate(&env);
    let second_recipient = Address::generate(&env);

    client.update_creator_fee_recipient(&creator, &first_recipient);
    assert_eq!(
        client.get_creator_fee_recipient(&creator),
        first_recipient,
        "first update should store the first recipient"
    );

    client.update_creator_fee_recipient(&creator, &second_recipient);
    assert_eq!(
        client.get_creator_fee_recipient(&creator),
        second_recipient,
        "second update must overwrite the first recipient"
    );
}

#[test]
fn test_update_creator_fee_recipient_old_recipient_absent_after_update() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let old_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    client.update_creator_fee_recipient(&creator, &old_recipient);
    assert_eq!(client.get_creator_fee_recipient(&creator), old_recipient);

    client.update_creator_fee_recipient(&creator, &new_recipient);

    let stored = client.get_creator_fee_recipient(&creator);
    assert_eq!(stored, new_recipient);
    assert_ne!(
        stored, old_recipient,
        "old recipient address must be absent from state after update"
    );
    assert_ne!(
        client.get_creator(&creator).fee_recipient,
        old_recipient,
        "creator profile must no longer reference the old recipient"
    );
}
