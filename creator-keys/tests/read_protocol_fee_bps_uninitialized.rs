//! Integration tests for `read_protocol_fee_bps` helper on uninitialized and initialized contract environments.
//!
//! Verifies that calling `read_protocol_fee_bps` panics with a descriptive message
//! containing 'uninitialized' and identifying terms when called prior to initialization,
//! and returns the stored protocol fee basis points when initialized.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{read_protocol_fee_bps, CreatorKeysContract};
use soroban_sdk::{testutils::Address as _, Env};

#[test]
#[should_panic(
    expected = "read_protocol_fee_bps: contract is uninitialized (protocol_fee_bps not set)"
)]
fn test_read_protocol_fee_bps_panics_when_uninitialized() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());

    env.as_contract(&contract_id, || {
        read_protocol_fee_bps(&env);
    });
}

#[test]
fn test_read_protocol_fee_bps_succeeds_when_initialized() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);

    let admin = soroban_sdk::Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_fee_config(&admin, &9000, &1000);

    let bps = env.as_contract(&contract_id, || read_protocol_fee_bps(&env));
    assert_eq!(
        bps, 1000,
        "must return stored protocol fee bps when initialized"
    );
}
