//! Tests for KeyScoreUpdated event emission.

mod contract_test_env;

use contract_test_env::{add_fresh_caller, setup_with_admin, test_env_with_auths};
use key_reputation::{
    events::{KeyScoreUpdatedEvent, KEY_SCORE_UPDATED_NAME},
    METRIC_TRADE_VOLUME,
};
use soroban_sdk::{testutils::Address as _, testutils::Events as _, vec, IntoVal};

#[test]
fn test_key_score_updated_event_emitted_on_update() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    client.update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &500u32);

    let events = env.events().all();
    assert!(!events.is_empty(), "expected at least one event");

    // Find our KeyScoreUpdated event.
    let matching: soroban_sdk::Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            if let Some(name) = topics.get(0) {
                name == KEY_SCORE_UPDATED_NAME.into_val(&env)
            } else {
                false
            }
        })
        .collect();

    assert!(
        !matching.is_empty(),
        "KeyScoreUpdated event must be emitted"
    );
}

#[test]
fn test_key_score_updated_event_carries_correct_metric() {
    let env = test_env_with_auths();
    let (client, _, admin) = setup_with_admin(&env);
    let caller = add_fresh_caller(&env, &client, &admin);
    let key_id = soroban_sdk::Address::generate(&env);

    client.update_key_score(&caller, &key_id, &METRIC_TRADE_VOLUME, &700u32);

    let events = env.events().all();
    let (_, _, data) = events.last().expect("at least one event");

    let payload: KeyScoreUpdatedEvent = data.into_val(&env);
    assert_eq!(payload.key_id, key_id);
    assert_eq!(payload.metric, METRIC_TRADE_VOLUME);
    assert!(payload.composite_score <= 1000);
}
