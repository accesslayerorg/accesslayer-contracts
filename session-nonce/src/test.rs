#![cfg(test)]

use crate::{
    ActionExecuted, DataKey, SessionNonceContract, SessionNonceContractClient, SessionNonceError,
    ACTION_EXECUTED_EVENT_NAME,
};
use soroban_sdk::{
    testutils::{Address as _, Events, MockAuth, MockAuthInvoke},
    Address, Env, IntoVal, Symbol, TryFromVal, Vec,
};

fn setup() -> (Env, Address, SessionNonceContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SessionNonceContract, ());
    let client = SessionNonceContractClient::new(&env, &contract_id);
    let wallet = Address::generate(&env);
    (env, contract_id, client, wallet)
}

fn action_executed_events(env: &Env) -> Vec<(Address, ActionExecuted)> {
    let topic = Symbol::new(env, ACTION_EXECUTED_EVENT_NAME);
    let mut matching = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if topics.len() != 2 {
            continue;
        }
        let first = topics.get(0).unwrap();
        if Symbol::try_from_val(env, &first) != Ok(topic.clone()) {
            continue;
        }
        let topic_wallet = Address::try_from_val(env, &topics.get(1).unwrap()).unwrap();
        let event = ActionExecuted::try_from_val(env, &data).unwrap();
        matching.push_back((topic_wallet, event));
    }
    matching
}

#[test]
fn get_nonce_starts_at_zero_for_every_wallet() {
    let (env, _, client, wallet) = setup();
    let other = Address::generate(&env);
    assert_eq!(client.get_nonce(&wallet), 0);
    assert_eq!(client.get_nonce(&other), 0);
}

#[test]
fn consume_nonce_increments_and_returns_next_nonce() {
    let (_, _, client, wallet) = setup();
    assert_eq!(client.consume_nonce(&wallet, &0), 1);
    assert_eq!(client.get_nonce(&wallet), 1);
    assert_eq!(client.consume_nonce(&wallet, &1), 2);
    assert_eq!(client.consume_nonce(&wallet, &2), 3);
    assert_eq!(client.get_nonce(&wallet), 3);
}

#[test]
fn nonces_are_tracked_per_wallet() {
    let (env, _, client, wallet) = setup();
    let other = Address::generate(&env);
    client.consume_nonce(&wallet, &0);
    client.consume_nonce(&wallet, &1);
    assert_eq!(client.get_nonce(&wallet), 2);
    assert_eq!(client.get_nonce(&other), 0);
    // The other wallet's nonce 0 is still valid.
    assert_eq!(client.consume_nonce(&other, &0), 1);
}

#[test]
fn replay_of_consumed_nonce_returns_nonce_already_used() {
    let (env, _, client, wallet) = setup();
    client.consume_nonce(&wallet, &0);
    assert_eq!(
        client.try_consume_nonce(&wallet, &0),
        Err(Ok(SessionNonceError::NonceAlreadyUsed))
    );
    // Rejected replay leaves state and events untouched.
    assert_eq!(client.get_nonce(&wallet), 1);
    assert_eq!(action_executed_events(&env).len(), 0);
}

#[test]
fn out_of_order_nonce_is_rejected() {
    let (_, _, client, wallet) = setup();
    assert_eq!(
        client.try_consume_nonce(&wallet, &5),
        Err(Ok(SessionNonceError::NonceAlreadyUsed))
    );
    assert_eq!(client.get_nonce(&wallet), 0);
}

#[test]
fn action_executed_event_includes_consumed_nonce() {
    let (env, _, client, wallet) = setup();
    client.consume_nonce(&wallet, &0);
    let events = action_executed_events(&env);
    assert_eq!(events.len(), 1);
    let (topic_wallet, event) = events.get(0).unwrap();
    assert_eq!(topic_wallet, wallet);
    assert_eq!(
        event,
        ActionExecuted {
            wallet: wallet.clone(),
            nonce: 0,
        }
    );

    client.consume_nonce(&wallet, &1);
    let events = action_executed_events(&env);
    assert_eq!(events.len(), 1);
    assert_eq!(events.get(0).unwrap().1.nonce, 1);
}

#[test]
fn competing_submissions_of_same_nonce_only_one_succeeds() {
    // Two relayers race to submit actions signed with the same nonce. Soroban
    // serialises the invocations, so exactly one wins and the other is
    // rejected without advancing the counter a second time.
    let (_, _, client, wallet) = setup();
    let first = client.try_consume_nonce(&wallet, &0);
    let second = client.try_consume_nonce(&wallet, &0);
    assert_eq!(first, Ok(Ok(1)));
    assert_eq!(second, Err(Ok(SessionNonceError::NonceAlreadyUsed)));
    assert_eq!(client.get_nonce(&wallet), 1);
}

#[test]
fn consume_nonce_rejects_overflow() {
    let (env, contract_id, client, wallet) = setup();
    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&DataKey::Nonce(wallet.clone()), &u64::MAX);
    });
    assert_eq!(
        client.try_consume_nonce(&wallet, &u64::MAX),
        Err(Ok(SessionNonceError::NonceOverflow))
    );
    assert_eq!(client.get_nonce(&wallet), u64::MAX);
}

#[test]
fn consume_nonce_requires_wallet_authorization() {
    let env = Env::default();
    let contract_id = env.register(SessionNonceContract, ());
    let client = SessionNonceContractClient::new(&env, &contract_id);
    let wallet = Address::generate(&env);
    let attacker = Address::generate(&env);

    // No auth at all.
    assert!(client.try_consume_nonce(&wallet, &0).is_err());
    assert_eq!(client.get_nonce(&wallet), 0);

    // Auth from a different address does not authorize the wallet.
    env.mock_auths(&[MockAuth {
        address: &attacker,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "consume_nonce",
            args: (wallet.clone(), 0u64).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(client.try_consume_nonce(&wallet, &0).is_err());
    assert_eq!(client.get_nonce(&wallet), 0);

    // The wallet's own authorization succeeds.
    env.mock_auths(&[MockAuth {
        address: &wallet,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "consume_nonce",
            args: (wallet.clone(), 0u64).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert_eq!(client.consume_nonce(&wallet, &0), 1);
}
