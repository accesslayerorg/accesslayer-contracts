#![cfg(test)]

use crate::{
    NonceConsumed, NonceRegistryContract, NonceRegistryContractClient, NonceRegistryError,
    NONCE_CONSUMED_EVENT_NAME,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol, TryFromVal, Vec,
};

fn setup() -> (Env, NonceRegistryContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(NonceRegistryContract, ());
    let client = NonceRegistryContractClient::new(&env, &contract_id);
    let wallet = Address::generate(&env);
    (env, client, wallet)
}

fn to_vec(env: &Env, values: &[u64]) -> Vec<u64> {
    let mut out = Vec::new(env);
    for value in values {
        out.push_back(*value);
    }
    out
}

fn nonce_consumed_events(env: &Env) -> Vec<NonceConsumed> {
    let topic = Symbol::new(env, NONCE_CONSUMED_EVENT_NAME);
    let mut matching = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            if Symbol::try_from_val(env, &first) == Ok(topic.clone()) {
                if let Ok(event) = NonceConsumed::try_from_val(env, &data) {
                    matching.push_back(event);
                }
            }
        }
    }
    matching
}

#[test]
fn nonce_is_unused_until_consumed() {
    let (_env, client, wallet) = setup();

    assert!(!client.is_nonce_used(&wallet, &7));

    client.consume_nonce(&wallet, &7);

    assert!(client.is_nonce_used(&wallet, &7));
    assert!(!client.is_nonce_used(&wallet, &8));
}

#[test]
fn consuming_a_nonce_twice_reverts_with_nonce_already_used() {
    let (_env, client, wallet) = setup();

    client.consume_nonce(&wallet, &1);

    assert_eq!(
        client.try_consume_nonce(&wallet, &1),
        Err(Ok(NonceRegistryError::NonceAlreadyUsed))
    );
}

#[test]
fn nonce_state_is_tracked_per_wallet() {
    let (env, client, wallet) = setup();
    let other = Address::generate(&env);

    client.consume_nonce(&wallet, &42);

    assert!(client.is_nonce_used(&wallet, &42));
    assert!(!client.is_nonce_used(&other, &42));
}

#[test]
fn nonces_have_no_ordering() {
    let (_env, client, wallet) = setup();

    client.consume_nonce(&wallet, &u64::MAX);
    client.consume_nonce(&wallet, &0);
    client.consume_nonce(&wallet, &5);

    assert!(client.is_nonce_used(&wallet, &u64::MAX));
    assert!(client.is_nonce_used(&wallet, &0));
    assert!(client.is_nonce_used(&wallet, &5));
    assert!(!client.is_nonce_used(&wallet, &1));
}

#[test]
fn nonce_consumed_event_is_emitted_on_first_consumption() {
    let (env, client, wallet) = setup();

    client.consume_nonce(&wallet, &9);

    let events = nonce_consumed_events(&env);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.wallet, wallet);
    assert_eq!(event.nonce, 9);

    // The wallet is indexed as the second topic so relayers can filter per account.
    let (_, topics, _) = env.events().all().get(0).unwrap();
    assert_eq!(topics.len(), 2);
    assert_eq!(
        Address::try_from_val(&env, &topics.get(1).unwrap()),
        Ok(wallet.clone())
    );
}

#[test]
fn failed_consumption_emits_no_event() {
    let (env, client, wallet) = setup();

    client.consume_nonce(&wallet, &3);

    assert!(client.try_consume_nonce(&wallet, &3).is_err());

    // Only the rejected invocation is in scope: it emitted nothing.
    assert_eq!(nonce_consumed_events(&env).len(), 0);
}

#[test]
fn bulk_invalidate_marks_every_provided_nonce_consumed() {
    let (env, client, wallet) = setup();

    let burned = client.bulk_invalidate(&wallet, &to_vec(&env, &[10, 20, 30]));

    assert_eq!(burned, 3);
    assert_eq!(nonce_consumed_events(&env).len(), 3);
    for nonce in [10u64, 20, 30] {
        assert!(client.is_nonce_used(&wallet, &nonce));
    }
    assert!(!client.is_nonce_used(&wallet, &40));
}

#[test]
fn bulk_invalidate_skips_nonces_that_are_already_consumed() {
    let (env, client, wallet) = setup();

    client.consume_nonce(&wallet, &2);
    let burned = client.bulk_invalidate(&wallet, &to_vec(&env, &[1, 2, 3]));

    assert_eq!(burned, 2);
    // One event per newly burned nonce; the already consumed one is skipped.
    assert_eq!(nonce_consumed_events(&env).len(), 2);
    for nonce in [1u64, 2, 3] {
        assert!(client.is_nonce_used(&wallet, &nonce));
    }

    // Re-running the same list is a no-op instead of a failure.
    assert_eq!(
        client.bulk_invalidate(&wallet, &to_vec(&env, &[1, 2, 3])),
        0
    );
    assert_eq!(nonce_consumed_events(&env).len(), 0);
}

#[test]
fn bulk_invalidate_requires_wallet_authorization() {
    let env = Env::default();
    let contract_id = env.register(NonceRegistryContract, ());
    let client = NonceRegistryContractClient::new(&env, &contract_id);
    let wallet = Address::generate(&env);
    let attacker = Address::generate(&env);

    // No auth mocked at all: the wallet's authorization cannot be satisfied.
    assert!(client
        .try_bulk_invalidate(&wallet, &to_vec(&env, &[1, 2]))
        .is_err());
    assert!(!client.is_nonce_used(&wallet, &1));

    // Auth for a different address does not grant wallet access either.
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &attacker,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "bulk_invalidate",
            args: (wallet.clone(), to_vec(&env, &[1, 2])).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(client
        .try_bulk_invalidate(&wallet, &to_vec(&env, &[1, 2]))
        .is_err());
    assert!(!client.is_nonce_used(&wallet, &1));
}

#[test]
fn bulk_invalidate_only_affects_the_calling_wallet() {
    let env = Env::default();
    let contract_id = env.register(NonceRegistryContract, ());
    let client = NonceRegistryContractClient::new(&env, &contract_id);
    let wallet = Address::generate(&env);
    let other = Address::generate(&env);

    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &wallet,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "bulk_invalidate",
            args: (wallet.clone(), to_vec(&env, &[4, 5])).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    assert_eq!(client.bulk_invalidate(&wallet, &to_vec(&env, &[4, 5])), 2);
    assert!(client.is_nonce_used(&wallet, &4));
    // The same numeric nonce for another wallet stays unconsumed.
    assert!(!client.is_nonce_used(&other, &4));
}
