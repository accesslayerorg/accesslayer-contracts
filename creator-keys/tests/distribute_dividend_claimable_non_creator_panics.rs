//! `distribute_dividend_claimable` requires creator auth (issue #857).

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, setup_holders,
    test_env_with_auths, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

#[test]
fn non_creator_call_is_rejected() {
    let env = test_env_with_auths();

    let (client, _id) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "carolcreator");
    let alice = Address::generate(&env);
    let holders = [(alice.clone(), 1u32)];
    setup_holders(&env, &client, &creator, &holders);

    env.mock_auths(&[]);

    let amounts = soroban_sdk::vec![&env, alice.clone()];
    let result = client.try_distribute_dividend_claimable(&creator, &1_000i128, &amounts);
    assert!(result.is_err());
}
