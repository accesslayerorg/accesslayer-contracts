//! Unit tests: only current fee recipient can rotate to a new address.

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, test_env_with_auths,
    set_key_price_for_tests, set_protocol_fee_bps,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn fee_recipient_rotation_rejected_from_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client);
    let new_recipient = Address::generate(&env);

    // Calling from the creator address (not the fee recipient) must fail
    let result = client.try_update_creator_fee_recipient(&creator, &creator, &new_recipient);
    assert_eq!(
        result,
        Err(Ok(ContractError::Unauthorized)),
        "Creator must not be able to rotate fee recipient without being the current recipient"
    );
}

#[test]
fn fee_recipient_rotation_rejected_from_random_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client);
    let random = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    let result = client.try_update_creator_fee_recipient(&creator, &random, &new_recipient);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}
