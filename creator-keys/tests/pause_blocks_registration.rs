//! Regression test: pause must block new registrations but not existing reads.

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, test_env_with_auths,
    set_key_price_for_tests, set_protocol_fee_bps,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn pause_blocks_new_creator_registration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);

    // Pause the contract
    client.pause();

    // New registration must be rejected
    let new_creator = Address::generate(&env);
    let result = client.try_register_creator(&new_creator);
    assert!(
        result.is_err(),
        "Registration must fail when contract is paused"
    );
}

#[test]
fn pause_does_not_block_existing_creator_reads() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 100i128);
    set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);

    // Register creator BEFORE pause
    let creator = register_test_creator(&env, &client);
    let supply_before = client.get_total_key_supply(&creator);

    // Pause the contract
    client.pause();

    // Existing reads must still work
    let supply_after = client.get_total_key_supply(&creator);
    assert_eq!(
        supply_before, supply_after,
        "Reads must not be blocked by pause"
    );

    // Unpausing should restore full functionality
    client.unpause();
    let supply_resumed = client.get_total_key_supply(&creator);
    assert_eq!(supply_before, supply_resumed);
}
