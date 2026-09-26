mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::events::{pause_expiry_set_topics, PAUSE_EXPIRY_SET_EVENT_NAME};
use creator_keys::{ContractError, PauseState};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    vec, Address, IntoVal,
};

const BASE_PRICE: i128 = 100;

#[test]
fn test_pause_with_expiry_requires_admin_and_valid_duration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let admin = Address::generate(&env);
    client.set_multisig_admins(&creator, &vec![&env, admin.clone()]);

    let outsider = Address::generate(&env);
    assert_eq!(
        client.try_pause_with_expiry(&creator, &outsider, &10),
        Err(Ok(ContractError::Unauthorized))
    );

    assert_eq!(
        client.try_pause_with_expiry(&creator, &admin, &0),
        Err(Ok(ContractError::PauseTooLong))
    );
    assert_eq!(
        client.try_pause_with_expiry(&creator, &admin, &17_281),
        Err(Ok(ContractError::PauseTooLong))
    );
}

#[test]
fn test_pause_with_expiry_blocks_trading_until_expiry() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, BASE_PRICE);
    let creator = register_test_creator(&env, &client, "alice");
    let admin = Address::generate(&env);
    client.set_multisig_admins(&creator, &vec![&env, admin.clone()]);

    client.pause_with_expiry(&creator, &admin, &10);

    let state = client
        .get_pause_state(&creator)
        .expect("pause state should be set");
    assert!(state.trading_paused);
    assert_eq!(state.pause_expires_at, env.ledger().sequence() + 10);

    let buyer = Address::generate(&env);
    assert_eq!(
        client.try_buy_key(&creator, &buyer, &BASE_PRICE, &None),
        Err(Ok(ContractError::GlobalTradingHalted))
    );

    let mut ledger = env.ledger().get();
    ledger.sequence_number = env.ledger().sequence() + 11;
    env.ledger().set(ledger);

    let result = client.try_buy_key(&creator, &buyer, &BASE_PRICE, &None);
    assert!(
        result.is_ok(),
        "buy should succeed after expiry: {result:?}"
    );
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
}

#[test]
fn test_pause_with_expiry_emits_event_with_key_and_expiry() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");
    let admin = Address::generate(&env);
    client.set_multisig_admins(&creator, &vec![&env, admin.clone()]);

    client.pause_with_expiry(&creator, &admin, &12);

    let (_, data) = env
        .events()
        .all()
        .iter()
        .rev()
        .find_map(|(_, topics, data)| {
            let name: soroban_sdk::Symbol = topics.get(0)?.into_val(&env);
            if name == PAUSE_EXPIRY_SET_EVENT_NAME {
                let key_id: Address = topics.get(1)?.into_val(&env);
                if key_id == creator {
                    return Some((topics, data));
                }
            }
            None
        })
        .expect("pause_expiry_set event not found");

    let payload: creator_keys::events::PauseExpirySetEvent = data.into_val(&env);
    assert_eq!(payload.key_id, creator);
    assert_eq!(payload.pause_expires_at, env.ledger().sequence() + 12);
    assert!(
        env.events()
            .all()
            .iter()
            .any(|(_, topics, _)| { topics == pause_expiry_set_topics(&creator).into_val(&env) }),
        "event topics should match pause_expiry_set_topics"
    );
}

#[test]
fn test_pause_state_is_cleared_and_resumes_when_expired() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, BASE_PRICE);
    let creator = register_test_creator(&env, &client, "alice");
    let admin = Address::generate(&env);
    client.set_multisig_admins(&creator, &vec![&env, admin.clone()]);

    client.pause_with_expiry(&creator, &admin, &1);
    let state_before: PauseState = client.get_pause_state(&creator).unwrap();
    assert!(state_before.trading_paused);

    let mut ledger = env.ledger().get();
    ledger.sequence_number = state_before.pause_expires_at + 1;
    env.ledger().set(ledger);

    let state_after = client.get_pause_state(&creator).unwrap();
    assert!(state_after.trading_paused);
    assert!(
        state_after.pause_expires_at < env.ledger().sequence()
            || state_after.pause_expires_at == env.ledger().sequence()
    );

    let buyer = Address::generate(&env);
    let result = client.try_buy_key(&creator, &buyer, &BASE_PRICE, &None);
    assert!(
        result.is_ok(),
        "buy should resume when ledger reaches expiry"
    );
}
