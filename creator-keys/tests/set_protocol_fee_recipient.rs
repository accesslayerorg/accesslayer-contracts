//! Tests for the `set_protocol_fee_recipient` entrypoint.
//!
//! Verifies that:
//! - a zero address is rejected with `ContractError::ZeroAddress`
//! - a valid non-zero address is accepted and stored

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_set_protocol_fee_recipient_rejects_zero_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let zero_str = String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero_addr = Address::from_string(&zero_str);

    let result = client.try_set_protocol_fee_recipient(&admin, &zero_addr);
    assert_eq!(
        result,
        Err(Ok(ContractError::ZeroAddress)),
        "zero address must be rejected"
    );

    // Confirm nothing was stored.
    assert_eq!(
        client.get_protocol_fee_recipient(),
        None,
        "protocol fee recipient should remain unset after rejection"
    );
}

#[test]
fn test_set_protocol_fee_recipient_accepts_valid_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);

    let result = client.try_set_protocol_fee_recipient(&admin, &recipient);
    assert_eq!(result, Ok(Ok(())), "valid address should be accepted");

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(recipient),
        "stored recipient should match"
    );
}

#[test]
fn test_set_protocol_fee_recipient_idempotent() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_protocol_fee_recipient(&admin, &recipient);
    // Setting the same value again should be a no-op.
    client.set_protocol_fee_recipient(&admin, &recipient);

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(recipient),
        "recipient unchanged after idempotent set"
    );
}
