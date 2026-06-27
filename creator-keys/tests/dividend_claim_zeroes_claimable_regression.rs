//! Regression test: dividend claim zeroes claimable balance after successful withdrawal.
//!
//! After a holder successfully claims their dividend, their claimable balance must
//! be set to exactly zero. `get_claimable_dividend` must return zero after a claim,
//! not the previously claimed amount. A second claim attempt must revert.
//!
//! General dividend coverage lives in `claim_dividend.rs`. This file pins the specific
//! invariant that `get_claimable_dividend` returns 0 after a successful claim so it
//! cannot regress silently.

mod contract_test_env;

use contract_test_env::{
    assert_claimable, distribute_test_dividend, register_creator_keys, register_test_creator,
    set_pricing_and_fees, test_env_with_auths, DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_claimable_balance_is_zero_after_successful_claim() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    client.claim_dividend(&creator, &buyer);

    // Claimable must be zero after claiming — not the previously claimed amount.
    assert_claimable(&client, &creator, &buyer, 0);
}

#[test]
fn test_second_claim_attempt_reverts_after_claimable_zeroed() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    client.claim_dividend(&creator, &buyer);

    // A second claim must fail because claimable is now zero.
    let result = client.try_claim_dividend(&creator, &buyer);
    assert_eq!(
        result,
        Err(Ok(ContractError::NoDividendClaimable)),
        "second claim must fail with NoDividendClaimable after claimable is zeroed"
    );
}
