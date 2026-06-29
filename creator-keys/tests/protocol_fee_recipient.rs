//! Tests for get_protocol_fee_recipient read-only method.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
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

    let r1 = client.get_protocol_fee_recipient();
    let r2 = client.get_protocol_fee_recipient();
    assert_eq!(r1, r2);
    assert_eq!(r1, None);
}

#[test]
fn test_get_protocol_fee_recipient_returns_stored_address() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let recipient = Address::generate(&env);

    env.as_contract(&contract_id, || {
        use creator_keys::constants::storage::PROTOCOL_FEE_RECIPIENT;
        env.storage()
            .persistent()
            .set(&PROTOCOL_FEE_RECIPIENT, &recipient);
    });

    let result = client.get_protocol_fee_recipient();
    assert_eq!(result, Some(recipient));
}

#[test]
fn test_get_protocol_fee_recipient_reflects_set_entrypoint() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_protocol_fee_recipient(&admin, &recipient);

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(recipient),
        "read should return the address written by set_protocol_fee_recipient"
    );
}

#[test]
fn test_get_protocol_fee_recipient_reflects_update_entrypoint() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let original = Address::generate(&env);
    let updated = Address::generate(&env);

    // Register the admin so assert_is_admin passes in update_protocol_fee_recipient.
    client.set_protocol_admin(&admin, &admin);
    client.set_protocol_fee_recipient(&admin, &original);
    client.update_protocol_fee_recipient(&admin, &updated);

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(updated),
        "read should return the new address after update"
    );
}

#[test]
fn test_get_protocol_fee_recipient_tracks_overwrites() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let first = Address::generate(&env);
    let second = Address::generate(&env);

    client.set_protocol_fee_recipient(&admin, &first);
    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(first),
        "should return first recipient"
    );

    client.set_protocol_fee_recipient(&admin, &second);
    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(second),
        "should return second recipient after overwrite"
    );
}
