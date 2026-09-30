//! `claim_dividend_claimable` credits the full unclaimed amount (issue #857).

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, setup_holders,
    test_env_with_auths, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

#[test]
fn claim_credits_full_unclaimed_amount_and_zeroes_entry() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let creator = register_test_creator(&env, &client, "bobcreator");

    let alice = Address::generate(&env);
    let holders = [(alice.clone(), 1u32)];
    setup_holders(&env, &client, &creator, &holders);

    let amounts = soroban_sdk::vec![&env, alice.clone()];
    client.distribute_dividend_claimable(&creator, &1_000i128, &amounts);

    let before = client.get_unclaimed_dividend(&creator, &alice);
    assert!(before > 0);

    let claimed = client.claim_dividend_claimable(&creator, &alice);
    assert_eq!(claimed, before);
    assert_eq!(client.get_unclaimed_dividend(&creator, &alice), 0);
}
