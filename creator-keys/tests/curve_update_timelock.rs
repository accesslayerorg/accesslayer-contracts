mod contract_test_env;

use contract_test_env::{
    assert_event, register_creator_keys, register_test_creator, set_key_price_for_tests,
    set_test_timestamp, test_env_with_auths,
};
use creator_keys::{
    curve_update::CURVE_UPDATE_DELAY_SECONDS,
    events::{
        CurveUpdateExecutedEvent, CurveUpdateProposedEvent, CURVE_UPDATE_EXECUTED_EVENT_NAME,
        CURVE_UPDATE_PROPOSED_EVENT_NAME,
    },
    ContractError,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

const START_TS: u64 = 1_700_000_000;

fn setup() -> (
    Env,
    creator_keys::CreatorKeysContractClient<'static>,
    Address,
    Address,
) {
    let env = test_env_with_auths();
    let env: &'static Env = Box::leak(Box::new(env));
    set_test_timestamp(env, START_TS);
    let (client, contract_id) = register_creator_keys(env);
    let admin = set_key_price_for_tests(env, &client, 100);
    client.set_curve_slope(&admin, &2);
    let creator = register_test_creator(env, &client, "curveowner");
    (env.clone(), client, contract_id, creator)
}

#[test]
fn curve_update_is_applied_after_48_hours_and_emits_values() {
    let (env, client, contract_id, creator) = setup();

    let proposal_id = client.propose_curve_update(&creator, &5, &200);
    let eta = START_TS + CURVE_UPDATE_DELAY_SECONDS;
    assert_event(
        &env,
        &contract_id,
        CURVE_UPDATE_PROPOSED_EVENT_NAME,
        &CurveUpdateProposedEvent {
            proposal_id,
            creator: creator.clone(),
            new_slope: 5,
            new_intercept: 200,
            eta,
        },
    );

    assert_eq!(client.query_price(&creator, &3), 106);
    set_test_timestamp(&env, eta - 1);
    assert_eq!(
        client.try_execute_curve_update(&proposal_id),
        Err(Ok(ContractError::TimelockNotElapsed))
    );
    assert_eq!(client.query_price(&creator, &3), 106);

    set_test_timestamp(&env, eta);
    client.execute_curve_update(&proposal_id);
    assert_event(
        &env,
        &contract_id,
        CURVE_UPDATE_EXECUTED_EVENT_NAME,
        &CurveUpdateExecutedEvent {
            proposal_id,
            creator: creator.clone(),
            old_slope: 2,
            old_intercept: 100,
            new_slope: 5,
            new_intercept: 200,
        },
    );
    assert_eq!(client.query_price(&creator, &3), 215);
}

#[test]
fn creator_can_cancel_a_pending_curve_update() {
    let (_env, client, _contract_id, creator) = setup();
    let proposal_id = client.propose_curve_update(&creator, &5, &200);

    client.cancel_curve_update(&proposal_id);
    assert_eq!(
        client.try_execute_curve_update(&proposal_id),
        Err(Ok(ContractError::ActionNotPending))
    );
}

#[test]
fn proposal_rejects_unregistered_keys_and_invalid_parameters() {
    let (env, client, _contract_id, creator) = setup();
    let unknown_key = Address::generate(&env);
    assert_eq!(
        client.try_propose_curve_update(&unknown_key, &1, &100),
        Err(Ok(ContractError::NotRegistered))
    );
    assert_eq!(
        client.try_propose_curve_update(&creator, &-1, &100),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    assert_eq!(
        client.try_propose_curve_update(&creator, &1, &0),
        Err(Ok(ContractError::NotPositiveAmount))
    );
}
