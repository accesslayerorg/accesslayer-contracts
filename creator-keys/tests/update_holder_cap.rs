//! Integration tests for `update_holder_cap` (issue #862).
//!
//! Once a creator configures a cap via `set_holder_cap`, they can only ever
//! tighten it via `update_holder_cap`: the new value must sit between
//! `HOLDER_CAP_MIN_BPS` (100 bps / 1%) and the currently stored cap, and a
//! non-creator caller must be rejected with `Unauthorized`.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_key_price_for_tests};
use creator_keys::{constants, events, ContractError};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 100;
const INITIAL_CAP_BPS: u32 = 2500;

fn setup(env: &Env) -> (creator_keys::CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    client.set_holder_cap(&creator, &Some(INITIAL_CAP_BPS));
    (client, creator)
}

#[test]
fn test_holder_cap_reduced_successfully() {
    let env = contract_test_env::test_env_with_auths();
    let (client, creator) = setup(&env);

    let result = client.try_update_holder_cap(&creator, &creator, &1000);
    assert_eq!(result, Ok(Ok(())));
    assert_eq!(
        client.get_holder_cap(&creator),
        Some(1000),
        "cap must be tightened to the requested lower value"
    );
}

#[test]
fn test_holder_cap_same_value_is_not_an_increase() {
    let env = contract_test_env::test_env_with_auths();
    let (client, creator) = setup(&env);

    let result = client.try_update_holder_cap(&creator, &creator, &INITIAL_CAP_BPS);
    assert_eq!(result, Ok(Ok(())));
    assert_eq!(client.get_holder_cap(&creator), Some(INITIAL_CAP_BPS));
}

#[test]
fn test_holder_cap_can_reduce_to_minimum() {
    let env = contract_test_env::test_env_with_auths();
    let (client, creator) = setup(&env);

    let result = client.try_update_holder_cap(&creator, &creator, &100);
    assert_eq!(result, Ok(Ok(())));
    assert_eq!(client.get_holder_cap(&creator), Some(100));
}

#[test]
fn test_update_holder_cap_above_current_reverts_with_cap_cannot_increase() {
    let env = contract_test_env::test_env_with_auths();
    let (client, creator) = setup(&env);

    let result = client.try_update_holder_cap(&creator, &creator, &(INITIAL_CAP_BPS + 1));
    assert_eq!(
        result,
        Err(Ok(ContractError::CapCannotIncrease)),
        "a cap above the currently stored value must be rejected"
    );
    assert_eq!(
        client.get_holder_cap(&creator),
        Some(INITIAL_CAP_BPS),
        "failed update must not change the stored cap"
    );
}

#[test]
fn test_update_holder_cap_below_minimum_reverts_with_cap_too_low() {
    let env = contract_test_env::test_env_with_auths();
    let (client, creator) = setup(&env);

    let result = client.try_update_holder_cap(&creator, &creator, &99);
    assert_eq!(
        result,
        Err(Ok(ContractError::CapTooLow)),
        "a cap below 100 bps (1%) must be rejected"
    );
    assert_eq!(
        client.get_holder_cap(&creator),
        Some(INITIAL_CAP_BPS),
        "failed update must not change the stored cap"
    );
}

#[test]
fn test_update_holder_cap_non_creator_reverts_with_unauthorized() {
    let env = contract_test_env::test_env_with_auths();
    let (client, creator) = setup(&env);

    let attacker = Address::generate(&env);
    let result = client.try_update_holder_cap(&creator, &attacker, &1000);
    assert_eq!(
        result,
        Err(Ok(ContractError::Unauthorized)),
        "a non-creator caller must be rejected"
    );
    assert_eq!(
        client.get_holder_cap(&creator),
        Some(INITIAL_CAP_BPS),
        "failed unauthorized update must not change the stored cap"
    );
}

#[test]
fn test_update_holder_cap_without_existing_cap_reverts() {
    let env = contract_test_env::test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    let result = client.try_update_holder_cap(&creator, &creator, &1000);
    assert_eq!(
        result,
        Err(Ok(ContractError::HolderCapNotSet)),
        "there must be a configured cap to tighten"
    );
}

#[test]
fn test_update_holder_cap_persists_to_storage_with_ttl() {
    let env = contract_test_env::test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    client.set_holder_cap(&creator, &Some(INITIAL_CAP_BPS));
    client.update_holder_cap(&creator, &creator, &1000);

    let stored: Option<u32> = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get(&constants::storage::holder_cap_bps(&creator))
    });
    assert_eq!(
        stored,
        Some(1000),
        "tightened cap must round-trip via persistent storage"
    );
}

#[test]
fn test_update_holder_cap_emits_event_with_old_and_new_values() {
    let env = contract_test_env::test_env_with_auths();
    let (client, creator) = setup(&env);

    env.events().all();

    client.update_holder_cap(&creator, &creator, &1000);

    let event_log = env.events().all();
    let mut emitted = false;
    for (_, topics, data) in event_log.iter() {
        let is_target = topics
            .get(0)
            .map(|t| {
                let name: Symbol = t.into_val(&env);
                name == events::HOLDER_CAP_UPDATED_EVENT_NAME
            })
            .unwrap_or(false);
        if !is_target {
            continue;
        }
        let payload: events::HolderCapUpdatedEvent = data.into_val(&env);
        assert_eq!(
            payload.key_id, creator,
            "event must carry the creator's address as the key_id"
        );
        assert_eq!(
            payload.old_cap_bps, INITIAL_CAP_BPS,
            "event must carry the pre-update cap"
        );
        assert_eq!(
            payload.new_cap_bps, 1000,
            "event must carry the tightened cap"
        );
        emitted = true;
    }
    assert!(
        emitted,
        "holder_cap_updated event must be emitted on a successful update"
    );
}
