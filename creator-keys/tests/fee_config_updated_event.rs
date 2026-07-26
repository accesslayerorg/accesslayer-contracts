//! Integration test verifying structured log event for admin fee bps update (#589)

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal,
};

#[test]
fn test_fee_config_updated_event_emitted() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);

    // Initial set (no old config)
    client.set_fee_config(&admin, &9000, &1000);

    let event_log = env.events().all();
    assert!(!event_log.is_empty());

    let (_, topics, data) = event_log.last().unwrap();
    let event_name: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(event_name, events::FEE_CONFIG_UPDATED_EVENT_NAME);

    let emitted_event: events::FeeConfigUpdatedEvent = data.into_val(&env);
    assert_eq!(emitted_event.old_bps, 0);
    assert_eq!(emitted_event.new_bps, 1000);
    assert_eq!(emitted_event.updated_at_ledger, env.ledger().sequence());

    // Update fee config
    client.set_fee_config(&admin, &9500, &500);

    let event_log = env.events().all();
    let (_, topics, data) = event_log.last().unwrap();
    let event_name: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(event_name, events::FEE_CONFIG_UPDATED_EVENT_NAME);

    let emitted_event: events::FeeConfigUpdatedEvent = data.into_val(&env);
    assert_eq!(emitted_event.old_bps, 1000);
    assert_eq!(emitted_event.new_bps, 500);
    assert_eq!(emitted_event.updated_at_ledger, env.ledger().sequence());
}
