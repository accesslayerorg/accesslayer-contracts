//! `claim_dividend_claimable` with no balance returns NoDividendClaimable (issue #857).

mod contract_test_env;
use contract_test_env::{register_creator_keys, test_env_with_auths};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

#[test]
fn claim_with_no_balance_errors() {
    let env = test_env_with_auths();
    let (client, _id) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let holder = Address::generate(&env);

    let result = client.try_claim_dividend_claimable(&creator, &holder);
    assert!(result.is_err());
}
