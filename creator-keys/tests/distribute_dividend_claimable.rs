//! `distribute_dividend_claimable` writes per-holder unclaimed balances (issue #857).

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, setup_holders,
    test_env_with_auths, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

#[test]
fn distribute_claimable_writes_proportional_unclaimed_balances() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let creator = register_test_creator(&env, &client, "alicecreator");

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let holders = [(alice.clone(), 3u32), (bob.clone(), 1u32)];
    let _supply = setup_holders(&env, &client, &creator, &holders);

    let amounts = soroban_sdk::vec![&env, alice.clone(), bob.clone()];
    client.distribute_dividend_claimable(&creator, &10_000i128, &amounts);

    // protocol 10%, creator 90% -> net = 9000; per_key = 9000/4 = 2250
    // alice = 3 * 2250 = 6750, bob = 1 * 2250 = 2250
    assert_eq!(client.get_unclaimed_dividend(&creator, &alice), 6_750);
    assert_eq!(client.get_unclaimed_dividend(&creator, &bob), 2_250);
}
