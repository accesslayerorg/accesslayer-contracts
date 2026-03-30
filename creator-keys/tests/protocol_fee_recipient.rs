//! Tests for get_protocol_fee_recipient read-only method.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_get_protocol_fee_recipient_returns_none_when_unset() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    assert_eq!(client.get_protocol_fee_recipient(), None);
}

#[test]
fn test_get_protocol_fee_recipient_is_read_only() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    // Multiple calls should return the same result without mutating state
    let r1 = client.get_protocol_fee_recipient();
    let r2 = client.get_protocol_fee_recipient();
    assert_eq!(r1, r2);
    assert_eq!(r1, None); // Should be None when unset
}

#[test]
fn test_get_protocol_fee_recipient_returns_stored_address() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let recipient = Address::generate(&env);

    // Manually set the protocol fee recipient in storage to test the getter
    env.as_contract(&contract_id, || {
        use creator_keys::constants::storage::PROTOCOL_FEE_RECIPIENT;
        env.storage().persistent().set(&PROTOCOL_FEE_RECIPIENT, &recipient);
    });

    let result = client.get_protocol_fee_recipient();
    assert_eq!(result, Some(recipient));
}