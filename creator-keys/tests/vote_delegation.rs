//! Integration tests for creator key delegation contract for proxy voting rights (Issue #1116).

mod contract_test_env;

use contract_test_env::{
    assert_event, register_creator_keys, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::{
    events::{self, PollError},
    ContractError,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    vec, Address, String,
};

const KEY_PRICE: i128 = 100;

fn poll_options(env: &soroban_sdk::Env) -> soroban_sdk::Vec<String> {
    vec![
        env,
        String::from_str(env, "Option A"),
        String::from_str(env, "Option B"),
    ]
}

fn register_test_creator(
    env: &soroban_sdk::Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    handle: &str,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    creator
}

#[test]
fn test_get_voting_weight_combined_for_multiple_delegators() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let delegator1 = Address::generate(&env);
    let delegator2 = Address::generate(&env);
    let delegate = Address::generate(&env);

    // delegator1 buys 3 keys, delegator2 buys 2 keys, delegate buys 4 keys
    for _ in 0..3 {
        client.buy_key(&creator, &delegator1, &KEY_PRICE, &None);
    }
    for _ in 0..2 {
        client.buy_key(&creator, &delegator2, &KEY_PRICE, &None);
    }
    for _ in 0..4 {
        client.buy_key(&creator, &delegate, &KEY_PRICE, &None);
    }

    assert_eq!(client.get_voting_weight(&delegator1, &creator), 3);
    assert_eq!(client.get_voting_weight(&delegator2, &creator), 2);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 4);

    // delegator1 delegates 3 keys to delegate
    client.delegate_votes(&delegator1, &creator, &delegate);

    // delegator1 weight drops to 0, delegate weight is 4 + 3 = 7
    assert_eq!(client.get_voting_weight(&delegator1, &creator), 0);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 7);

    // delegator2 delegates 2 keys to delegate
    client.delegate_votes(&delegator2, &creator, &delegate);

    // delegator2 weight drops to 0, delegate weight is 4 + 3 + 2 = 9
    assert_eq!(client.get_voting_weight(&delegator2, &creator), 0);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 9);
}

#[test]
fn test_delegate_cannot_redelegate_received_weight() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);
    let holder_c = Address::generate(&env);

    client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder_b, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder_c, &KEY_PRICE, &None);

    // A delegates to B
    client.delegate_votes(&holder_a, &creator, &holder_b);

    // B has received delegated weight; B cannot re-delegate to C
    let err = client.try_delegate_votes(&holder_b, &creator, &holder_c);
    assert_eq!(err, Err(Ok(ContractError::CannotReDelegate)));

    // Inverse check: A (who has delegated) cannot receive delegation from C
    let err2 = client.try_delegate_votes(&holder_c, &creator, &holder_a);
    assert_eq!(err2, Err(Ok(ContractError::CannotReDelegate)));
}

#[test]
fn test_revoke_delegation_returns_weight_to_holder() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    let delegate = Address::generate(&env);

    for _ in 0..5 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }
    for _ in 0..2 {
        client.buy_key(&creator, &delegate, &KEY_PRICE, &None);
    }

    client.delegate_votes(&holder, &creator, &delegate);
    assert_eq!(client.get_voting_weight(&holder, &creator), 0);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 7);

    // Revoke delegation
    client.revoke_delegation(&holder, &creator);

    // Weight is returned to holder, delegate's weight drops back to own balance
    assert_eq!(client.get_voting_weight(&holder, &creator), 5);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 2);
    assert_eq!(client.get_vote_delegate(&holder, &creator), None);
}

#[test]
fn test_governance_vote_uses_delegated_weight_and_prevents_double_counting() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let delegator = Address::generate(&env);
    let delegate = Address::generate(&env);

    // delegator has 10 keys, delegate has 5 keys
    for _ in 0..10 {
        client.buy_key(&creator, &delegator, &KEY_PRICE, &None);
    }
    for _ in 0..5 {
        client.buy_key(&creator, &delegate, &KEY_PRICE, &None);
    }

    client.delegate_votes(&delegator, &creator, &delegate);

    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Community vote"),
        &poll_options(&env),
        &100,
    );

    // Delegator cannot vote directly with delegated weight
    let res = client.try_cast_vote(&creator, &delegator, &poll_id, &0);
    assert_eq!(res, Err(Ok(PollError::Unauthorized)));

    // Delegate votes using own (5) + delegated (10) = 15 weight
    client.cast_vote(&creator, &delegate, &poll_id, &0);

    let result = client.get_poll_result(&creator, &poll_id);
    assert_eq!(result.vote_counts.get(0).unwrap(), 15);
    assert_eq!(result.total_weight, 15);

    // Re-verify delegator still cannot vote (no double voting)
    let res2 = client.try_cast_vote(&creator, &delegator, &poll_id, &1);
    assert_eq!(res2, Err(Ok(PollError::Unauthorized)));
}

#[test]
fn test_events_emitted_on_delegate_and_revoke() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    let delegate = Address::generate(&env);

    for _ in 0..6 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }

    // 1. Delegate votes -> emits VotingDelegated
    client.delegate_votes(&holder, &creator, &delegate);

    assert_event(
        &env,
        &contract_id,
        events::VOTING_DELEGATED_EVENT_NAME,
        &events::VotingDelegated {
            holder: holder.clone(),
            delegate_wallet: delegate.clone(),
            key_id: creator.clone(),
            weight: 6,
        },
    );

    // 2. Revoke delegation -> emits DelegationRevoked
    client.revoke_delegation(&holder, &creator);

    assert_event(
        &env,
        &contract_id,
        events::VOTE_DELEGATION_REVOKED_EVENT_NAME,
        &events::DelegationRevoked {
            holder: holder.clone(),
            previous_delegate_wallet: delegate.clone(),
            key_id: creator.clone(),
            weight: 6,
        },
    );
}

#[test]
fn test_negative_self_delegation_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    let res = client.try_delegate_votes(&holder, &creator, &holder);
    assert_eq!(res, Err(Ok(ContractError::SelfDelegation)));
}

#[test]
fn test_negative_non_holder_delegating_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let non_holder = Address::generate(&env);
    let delegate = Address::generate(&env);

    let res = client.try_delegate_votes(&non_holder, &creator, &delegate);
    assert_eq!(res, Err(Ok(ContractError::NotAHolder)));
}

#[test]
fn test_negative_double_delegation_requires_revoke_first() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    let delegate1 = Address::generate(&env);
    let delegate2 = Address::generate(&env);

    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    client.delegate_votes(&holder, &creator, &delegate1);

    // Delegating again without revoking must fail with AlreadyDelegated
    let res = client.try_delegate_votes(&holder, &creator, &delegate2);
    assert_eq!(res, Err(Ok(ContractError::AlreadyDelegated)));
}

#[test]
fn test_negative_revoke_without_delegation_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    let res = client.try_revoke_delegation(&holder, &creator);
    assert_eq!(res, Err(Ok(ContractError::NoDelegation)));
}

#[test]
fn test_key_ownership_never_changes() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    let delegate = Address::generate(&env);

    for _ in 0..5 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }

    assert_eq!(client.get_key_balance(&creator, &holder), 5);
    assert_eq!(client.get_key_balance(&creator, &delegate), 0);

    client.delegate_votes(&holder, &creator, &delegate);

    // Ownership of underlying keys remains strictly with the holder
    assert_eq!(client.get_key_balance(&creator, &holder), 5);
    assert_eq!(client.get_key_balance(&creator, &delegate), 0);
}

#[test]
fn test_delegation_weight_adjusts_on_transfer_and_prunes_on_zero_balance() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let delegator = Address::generate(&env);
    let delegate = Address::generate(&env);
    let third_party = Address::generate(&env);

    for _ in 0..5 {
        client.buy_key(&creator, &delegator, &KEY_PRICE, &None);
    }

    client.delegate_votes(&delegator, &creator, &delegate);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 5);

    // Delegator transfers 2 keys to third_party
    client.transfer_keys(&creator, &delegator, &third_party, &2);

    // Delegator now has 3 keys; delegate's voting weight dynamically drops to 3
    assert_eq!(client.get_key_balance(&creator, &delegator), 3);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 3);
    assert_eq!(client.get_voting_weight(&third_party, &creator), 2);

    // Delegator transfers remaining 3 keys
    client.transfer_keys(&creator, &delegator, &third_party, &3);

    // Delegator now has 0 keys; delegate's voting weight drops to 0, delegation is cleared
    assert_eq!(client.get_key_balance(&creator, &delegator), 0);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 0);
    assert_eq!(client.get_vote_delegate(&delegator, &creator), None);
}

fn advance_sequence(env: &soroban_sdk::Env) {
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
}

#[test]
fn test_delegation_weight_adjusts_on_sell_and_prunes_on_full_exit() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let delegator = Address::generate(&env);
    let delegate = Address::generate(&env);

    client.buy_key(&creator, &delegator, &KEY_PRICE, &None);
    client.buy_key(&creator, &delegator, &KEY_PRICE, &None);

    client.delegate_votes(&delegator, &creator, &delegate);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 2);

    // Advance sequence past flash-loan guard ledger
    advance_sequence(&env);

    // Delegator sells 1 key
    client.sell_key(&creator, &delegator, &None);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 1);

    // Advance sequence again
    advance_sequence(&env);

    // Delegator sells last key
    client.sell_key(&creator, &delegator, &None);
    assert_eq!(client.get_voting_weight(&delegate, &creator), 0);
    assert_eq!(client.get_vote_delegate(&delegator, &creator), None);
}

#[test]
fn test_unauthorized_caller_cannot_delegate_or_revoke() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let holder = Address::generate(&env);
    let delegate = Address::generate(&env);

    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.delegate_votes(&holder, &creator, &delegate);

    // Revoke mock authorizations: calls requiring holder.require_auth() must now fail
    env.mock_auths(&[]);

    let res = client.try_delegate_votes(&holder, &creator, &delegate);
    assert!(
        res.is_err(),
        "delegate_votes must fail without holder authorization"
    );

    let res_revoke = client.try_revoke_delegation(&holder, &creator);
    assert!(
        res_revoke.is_err(),
        "revoke_delegation must fail without holder authorization"
    );
}
