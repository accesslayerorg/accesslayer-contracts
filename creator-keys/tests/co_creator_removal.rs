//! Tests for `remove_co_creator` (#791).

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_pricing_and_fees, test_env_with_auths};
use creator_keys::{events, CoCreatorConfig, FeatureError};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, String, Symbol,
};

const CO_CREATOR_SHARE_BPS: u32 = 3000;
const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

fn register_creator_with_co_creator(
    env: &soroban_sdk::Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    handle: &str,
) -> (Address, Address, CoCreatorConfig) {
    let creator = Address::generate(env);
    let co_creator = Address::generate(env);
    let config = CoCreatorConfig {
        address: co_creator.clone(),
        share_bps: CO_CREATOR_SHARE_BPS,
    };

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &None,
        &None,
        &None,
        &Some(config.clone()),
        &None,
    );

    (creator, co_creator, config)
}

#[test]
fn test_remove_co_creator_clears_config() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (creator, _co_creator, _config) = register_creator_with_co_creator(&env, &client, "alice");

    assert!(client.get_co_creator(&creator).is_some());

    client.remove_co_creator(&creator, &creator);

    assert_eq!(client.get_co_creator(&creator), None);
}

#[test]
fn test_remove_co_creator_emits_event_with_removed_address() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let (creator, co_creator, _config) = register_creator_with_co_creator(&env, &client, "alice");

    client.remove_co_creator(&creator, &creator);

    let mut found = false;
    for (contract, topics, data) in env.events().all().iter() {
        if contract != contract_id {
            continue;
        }
        let event_name: Symbol = topics
            .get(0)
            .expect("event should have a name topic")
            .into_val(&env);
        if event_name == events::CO_CREATOR_REMOVED_EVENT_NAME {
            let payload: events::CoCreatorRemovedEvent = data.clone().into_val(&env);
            assert_eq!(payload.creator_id, creator);
            assert_eq!(payload.co_creator, co_creator);
            found = true;
        }
    }
    assert!(found, "expected a CoCreatorRemoved event to be emitted");
}

#[test]
fn test_remove_co_creator_rejects_non_creator_caller() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (creator, _co_creator, _config) = register_creator_with_co_creator(&env, &client, "alice");
    let attacker = Address::generate(&env);

    let result = client.try_remove_co_creator(&creator, &attacker);
    assert_eq!(result, Err(Ok(FeatureError::Unauthorized)));
    assert!(client.get_co_creator(&creator).is_some());
}

#[test]
fn test_remove_co_creator_fails_when_none_configured() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "bob"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let result = client.try_remove_co_creator(&creator, &creator);
    assert_eq!(result, Err(Ok(FeatureError::NoCoCreatorSet)));
}

#[test]
fn test_remove_co_creator_restores_full_royalties_to_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let (creator, co_creator, _config) = register_creator_with_co_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Before removal: the creator fee (900 at price 1000, 90% creator_bps) splits
    // 30% to the co-creator (270) and 70% stays with the creator (630).
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    let creator_balance_before = client.get_creator_fee_balance(&creator);
    let co_creator_balance_before = client.get_co_creator_fee_balance(&creator, &co_creator);
    assert_eq!(creator_balance_before, 630);
    assert_eq!(co_creator_balance_before, 270);

    client.remove_co_creator(&creator, &creator);

    // After removal: the full creator fee (900, at the new supply-1 bonding
    // curve price under a zero curve slope, still 1000) goes to the creator;
    // the co-creator's balance is untouched.
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    let creator_balance_after = client.get_creator_fee_balance(&creator);
    let co_creator_balance_after = client.get_co_creator_fee_balance(&creator, &co_creator);
    assert_eq!(creator_balance_after, creator_balance_before + 900);
    assert_eq!(co_creator_balance_after, co_creator_balance_before);
}

#[test]
fn test_remove_co_creator_is_idempotent_failure_on_second_call() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (creator, _co_creator, _config) = register_creator_with_co_creator(&env, &client, "alice");

    client.remove_co_creator(&creator, &creator);

    let result = client.try_remove_co_creator(&creator, &creator);
    assert_eq!(result, Err(Ok(FeatureError::NoCoCreatorSet)));
}
