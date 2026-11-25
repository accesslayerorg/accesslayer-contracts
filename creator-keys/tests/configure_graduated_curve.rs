//! Graduated bonding curve configuration before the first sale (#829).
//!
//! Covers exponent selection per supply tier, the base exponent below the first
//! milestone, the emitted configuration event, and the `CurveAlreadyActive`
//! panic once keys have been sold.

mod contract_test_env;

use contract_test_env::{
    assert_event, register_creator_keys, register_test_creator, set_curve_slope,
    set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::events::{GraduatedCurveConfiguredEvent, GRADUATED_CURVE_CONFIGURED_EVENT_NAME};
use creator_keys::{CreatorKeysContractClient, CurveConfigError};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

const KEY_PRICE: i128 = 1_000;
const CURVE_SLOPE: i128 = 10;

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let (client, contract_id) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, KEY_PRICE, 9_000, 1_000);
    set_curve_slope(env, &client, CURVE_SLOPE);
    let creator = register_test_creator(env, &client, "graduated");
    (client, contract_id, creator)
}

fn milestones(env: &Env, pairs: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut out = Vec::new(env);
    for pair in pairs {
        out.push_back(*pair);
    }
    out
}

#[test]
fn supply_below_first_milestone_keeps_the_base_exponent() {
    let env = test_env_with_auths();
    let (client, _, creator) = setup(&env);
    client.configure_graduated_curve(&creator, &milestones(&env, &[(100, 2)]));

    // Below the milestone: base exponent 1 -> 1000 + 10 * 50.
    assert_eq!(client.query_price(&creator, &50), 1_500);
    assert_eq!(client.get_graduated_exponent(&creator, &50), Some(1));

    // At and above the milestone: exponent 2.
    assert_eq!(client.get_graduated_exponent(&creator, &100), Some(2));
    // 1000 + 10 * 100 (first band) + 10 * 50^2 (steep band).
    assert_eq!(client.query_price(&creator, &150), 27_000);
}

#[test]
fn exponent_steps_up_at_each_configured_milestone() {
    let env = test_env_with_auths();
    let (client, _, creator) = setup(&env);
    client.configure_graduated_curve(&creator, &milestones(&env, &[(10, 2), (20, 3)]));

    assert_eq!(client.get_graduated_exponent(&creator, &0), Some(1));
    assert_eq!(client.get_graduated_exponent(&creator, &9), Some(1));
    assert_eq!(client.get_graduated_exponent(&creator, &10), Some(2));
    assert_eq!(client.get_graduated_exponent(&creator, &19), Some(2));
    assert_eq!(client.get_graduated_exponent(&creator, &20), Some(3));
    assert_eq!(client.get_graduated_exponent(&creator, &1_000), Some(3));
}

#[test]
fn configuration_is_emitted_with_every_milestone_pair() {
    let env = test_env_with_auths();
    let (client, contract_id, creator) = setup(&env);
    let configured = milestones(&env, &[(100, 2), (500, 3)]);
    client.configure_graduated_curve(&creator, &configured);

    assert_event(
        &env,
        &contract_id,
        GRADUATED_CURVE_CONFIGURED_EVENT_NAME,
        &GraduatedCurveConfiguredEvent {
            creator: creator.clone(),
            milestones: configured.clone(),
            ledger: env.ledger().sequence(),
        },
    );

    assert_eq!(client.get_graduated_curve(&creator), Some(configured));
}

#[test]
fn configuring_after_a_sale_panics_with_curve_already_active() {
    let env = test_env_with_auths();
    let (client, _, creator) = setup(&env);
    let buyer = Address::generate(&env);

    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let result = client.try_configure_graduated_curve(&creator, &milestones(&env, &[(100, 2)]));
    assert_eq!(
        result.err().unwrap().unwrap(),
        CurveConfigError::CurveAlreadyActive
    );
}

#[test]
fn exponents_outside_one_to_five_are_rejected() {
    let env = test_env_with_auths();
    let (client, _, creator) = setup(&env);

    for exponent in [0u32, 6u32] {
        let result =
            client.try_configure_graduated_curve(&creator, &milestones(&env, &[(100, exponent)]));
        assert_eq!(
            result.err().unwrap().unwrap(),
            CurveConfigError::InvalidExponent
        );
    }
}

#[test]
fn non_ascending_thresholds_are_rejected() {
    let env = test_env_with_auths();
    let (client, _, creator) = setup(&env);

    let result =
        client.try_configure_graduated_curve(&creator, &milestones(&env, &[(500, 2), (300, 3)]));
    assert_eq!(
        result.err().unwrap().unwrap(),
        CurveConfigError::ThresholdNotAscending
    );
}
