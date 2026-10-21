#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, TryFromVal,
};

fn setup_test() -> (Env, TwapOracleContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TwapOracleContract, ());
    let client = TwapOracleContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let key_id = Address::generate(&env);

    client.initialize(&admin);

    (env, client, admin, key_id)
}

#[test]
fn test_minimum_observation_guard_returns_twap_invalid_before_threshold_met() {
    let (env, client, admin, key_id) = setup_test();

    // 1. Zero observations: get_twap returns TWAPInvalid
    env.ledger().set_timestamp(1_000);
    let res_zero = client.try_get_twap(&key_id, &WINDOW_1H);
    assert_eq!(res_zero, Err(Ok(TwapOracleError::TWAPInvalid)));

    // 2. Exactly 1 observation recorded: still returns TWAPInvalid
    client.record_swap(&key_id, &100);
    let res_one = client.try_get_twap(&key_id, &WINDOW_1H);
    assert_eq!(res_one, Err(Ok(TwapOracleError::TWAPInvalid)));

    // Also helper views return TWAPInvalid before threshold met
    assert_eq!(
        client.try_get_twap_1h(&key_id),
        Err(Ok(TwapOracleError::TWAPInvalid))
    );
    assert_eq!(
        client.try_get_twap_4h(&key_id),
        Err(Ok(TwapOracleError::TWAPInvalid))
    );
    assert_eq!(
        client.try_get_twap_24h(&key_id),
        Err(Ok(TwapOracleError::TWAPInvalid))
    );

    // 3. Trade 2 recorded after elapsed time: now threshold (default 2) is met!
    env.ledger().set_timestamp(4_600); // +3,600s
    client.record_swap(&key_id, &200);

    let res_two = client.try_get_twap(&key_id, &WINDOW_1H);
    assert_eq!(res_two, Ok(Ok(100)));

    // 4. Custom threshold: raise min_observations to 4
    let key_id2 = Address::generate(&env);
    client.set_config(&admin, &key_id2, &4u32, &DEFAULT_MAX_WINDOW_SECONDS);

    // Observation 1 at t = 10_000
    env.ledger().set_timestamp(10_000);
    client.record_swap(&key_id2, &100);
    assert_eq!(
        client.try_get_twap(&key_id2, &WINDOW_1H),
        Err(Ok(TwapOracleError::TWAPInvalid))
    );

    // Observation 2 at t = 11_000
    env.ledger().set_timestamp(11_000);
    client.record_swap(&key_id2, &110);
    assert_eq!(
        client.try_get_twap(&key_id2, &WINDOW_1H),
        Err(Ok(TwapOracleError::TWAPInvalid))
    );

    // Observation 3 at t = 12_000
    env.ledger().set_timestamp(12_000);
    client.record_swap(&key_id2, &120);
    assert_eq!(
        client.try_get_twap(&key_id2, &WINDOW_1H),
        Err(Ok(TwapOracleError::TWAPInvalid))
    );

    // Observation 4 at t = 13_600 (threshold of 4 reached)
    env.ledger().set_timestamp(13_600);
    client.record_swap(&key_id2, &130);
    assert!(client.try_get_twap(&key_id2, &WINDOW_1H).is_ok());
}

#[test]
fn test_twap_computed_correctly_for_all_three_supported_windows() {
    let (env, client, _admin, key_id) = setup_test();

    // Timeline setup:
    // t = 10_000: Swap 1 at price = 100
    env.ledger().set_timestamp(10_000);
    client.record_swap(&key_id, &100);

    // t = 13_600 (+3,600s = 1h elapsed): Swap 2 at price = 200
    // Price was 100 for 3,600s -> cumulative product = 100 * 3,600 = 360,000
    env.ledger().set_timestamp(13_600);
    client.record_swap(&key_id, &200);

    // 1-hour TWAP from t=10_000 to t=13_600:
    // (360,000 - 0) / 3,600 = 100
    let twap_1h = client.get_twap(&key_id, &WINDOW_1H);
    assert_eq!(twap_1h, 100);
    assert_eq!(client.get_twap_1h(&key_id), 100);

    // t = 24_400 (+10,800s = 3h later, total 4h from t=10_000): Swap 3 at price = 300
    // Price was 200 for 10,800s -> added 200 * 10,800 = 2,160,000
    // Total cumulative at t=24_400 = 360,000 + 2,160,000 = 2,520,000
    env.ledger().set_timestamp(24_400);
    client.record_swap(&key_id, &300);

    // 4-hour TWAP (14,400s) from t=10_000 to t=24_400:
    // 2,520,000 / 14,400 = 175
    let twap_4h = client.get_twap(&key_id, &WINDOW_4H);
    assert_eq!(twap_4h, 175);
    assert_eq!(client.get_twap_4h(&key_id), 175);

    // 1-hour TWAP (3,600s) at t=24_400 (from t=20_800 to t=24_400, price was 200 continuously):
    // 200 * 3,600 / 3,600 = 200
    let twap_1h_recent = client.get_twap(&key_id, &WINDOW_1H);
    assert_eq!(twap_1h_recent, 200);
    assert_eq!(client.get_twap_1h(&key_id), 200);

    // Advance to t = 96_400 (total 86,400s = 24h from t=10_000): Swap 4 at price = 400
    // From t=24_400 to t=96_400 (72,000s), price was 300:
    // Added 300 * 72,000 = 21,600,000
    // Total cumulative at t=96_400 = 2,520,000 + 21,600,000 = 24,120,000
    env.ledger().set_timestamp(96_400);
    client.record_swap(&key_id, &400);

    // 24-hour TWAP (86,400s) from t=10_000 to t=96_400:
    // 24,120,000 / 86,400 = 279
    let twap_24h = client.get_twap(&key_id, &WINDOW_24H);
    assert_eq!(twap_24h, 279);
    assert_eq!(client.get_twap_24h(&key_id), 279);

    // Verify 4-hour TWAP at t=96_400 (from t=82_000 to t=96_400, price was 300 continuously):
    assert_eq!(client.get_twap_4h(&key_id), 300);

    // Verify 1-hour TWAP at t=96_400 (from t=92_800 to t=96_400, price was 300 continuously):
    assert_eq!(client.get_twap_1h(&key_id), 300);
}

#[test]
fn test_twap_unaffected_by_single_large_outlier_swap() {
    let (env, client, _admin, key_id) = setup_test();

    // Baseline: price is 100 for 3,590 seconds (99.7% of the 1-hour window)
    env.ledger().set_timestamp(1_000);
    client.record_swap(&key_id, &100);

    // An attacker executes an outlier swap at t = 4,590 (+3,590s), spiking spot price 100x to 10,000
    env.ledger().set_timestamp(4_590);
    client.record_swap(&key_id, &10_000);

    // Spot price is now 10,000 (100x baseline)
    assert_eq!(client.get_latest_price(&key_id), 10_000);

    // Query 1-hour TWAP at t = 4,600 (only 10 seconds after the spike)
    // Elapsed under price 100: 3,590s -> cumulative = 359,000
    // Elapsed under price 10,000: 10s -> cumulative = 100,000
    // Total cumulative = 459,000
    // 1-hour TWAP = 459,000 / 3,600 = 127
    env.ledger().set_timestamp(4_600);
    let twap = client.get_twap(&key_id, &WINDOW_1H);
    assert_eq!(twap, 127);

    // Despite a 10,000% spot price spike (from 100 to 10,000),
    // the TWAP only shifted from 100 to 127 (a tiny 27% increase),
    // successfully resisting price manipulation!
    assert!(twap < 130);

    // Also test same-block outlier swap (flash-loan / sandwich manipulation):
    let key_id2 = Address::generate(&env);
    env.ledger().set_timestamp(10_000);
    client.record_swap(&key_id2, &500);

    env.ledger().set_timestamp(13_600);
    client.record_swap(&key_id2, &500);

    // Same block trade at t = 13_600 spikes to 1_000_000
    client.record_swap(&key_id2, &1_000_000);

    // TWAP at t = 13_600 over 1h has exactly ZERO cumulative inflation from the same-block spike:
    let twap_same_block = client.get_twap(&key_id2, &WINDOW_1H);
    assert_eq!(twap_same_block, 500);
}

#[test]
fn test_twap_updated_event_emitted_on_each_swap_with_correct_cumulative_data() {
    let (env, client, _admin, key_id) = setup_test();

    // First swap at t = 1_000, price = 500
    env.ledger().set_timestamp(1_000);
    client.record_swap(&key_id, &500);

    // Inspect events
    let mut updated_events = soroban_sdk::Vec::new(&env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first_topic) = topics.get(0) {
            if Symbol::try_from_val(&env, &first_topic) == Ok(Symbol::new(&env, "TWAPUpdated")) {
                if let Ok(event) = TwapUpdatedEvent::try_from_val(&env, &data) {
                    updated_events.push_back(event);
                }
            }
        }
    }

    assert_eq!(updated_events.len(), 1);
    let event1 = updated_events.get(0).unwrap();
    assert_eq!(event1.key_id, key_id);
    assert_eq!(event1.price, 500);
    assert_eq!(event1.cumulative_price_time, 0);
    assert_eq!(event1.timestamp, 1_000);

    // Second swap at t = 2_000 (+1,000s), price = 600
    // Expected cumulative = 500 * 1,000 = 500,000
    env.ledger().set_timestamp(2_000);
    client.record_swap(&key_id, &600);

    let mut second_events = soroban_sdk::Vec::new(&env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first_topic) = topics.get(0) {
            if Symbol::try_from_val(&env, &first_topic) == Ok(Symbol::new(&env, "TWAPUpdated")) {
                if let Ok(event) = TwapUpdatedEvent::try_from_val(&env, &data) {
                    second_events.push_back(event);
                }
            }
        }
    }

    assert_eq!(second_events.len(), 1);
    let event2 = second_events.get(0).unwrap();
    assert_eq!(event2.key_id, key_id);
    assert_eq!(event2.price, 600);
    assert_eq!(event2.cumulative_price_time, 500_000);
    assert_eq!(event2.timestamp, 2_000);
}

#[test]
fn test_storage_pruned_correctly_beyond_maximum_window_boundary() {
    let (env, client, _admin, key_id) = setup_test();

    // Default max_window_seconds = 86_400 (24h)
    // Insert observations at various timestamps:
    // t = 1,000: price = 100
    env.ledger().set_timestamp(1_000);
    client.record_swap(&key_id, &100);

    // t = 2,000: price = 110
    env.ledger().set_timestamp(2_000);
    client.record_swap(&key_id, &110);

    // t = 5,000: price = 120
    env.ledger().set_timestamp(5_000);
    client.record_swap(&key_id, &120);

    // t = 10,000: price = 130
    env.ledger().set_timestamp(10_000);
    client.record_swap(&key_id, &130);

    // All 4 observations stored so far
    assert_eq!(client.get_observation_count(&key_id), 4);

    // Now advance time to t = 100,000 (99,000s later, well past the 86,400s max window)
    // Cutoff timestamp = 100,000 - 86,400 = 13,600.
    // At t = 100,000, swap at price = 200.
    // The observation at t = 10,000 is the boundary observation before cutoff 13,600.
    // Observations at t = 1,000, 2,000, and 5,000 are strictly older and must be PRUNED!
    env.ledger().set_timestamp(100_000);
    client.record_swap(&key_id, &200);

    let observations = client.get_observations(&key_id);
    // Pruned: only boundary observation (t=10,000) and current observation (t=100,000) retained
    assert_eq!(observations.len(), 2);
    assert_eq!(observations.get(0).unwrap().timestamp, 10_000);
    assert_eq!(observations.get(1).unwrap().timestamp, 100_000);

    // Verify 24h TWAP can still be computed accurately after pruning!
    // From cutoff (t=13,600) to now (t=100,000), price was 130 continuously for 86,400s:
    // 24h TWAP must be exactly 130!
    let twap_24h = client.get_twap_24h(&key_id);
    assert_eq!(twap_24h, 130);
}

#[test]
fn test_input_validation_and_unsupported_windows() {
    let (env, client, _admin, key_id) = setup_test();

    // Non-positive swap price rejected
    env.ledger().set_timestamp(1_000);
    assert_eq!(
        client.try_record_swap(&key_id, &0),
        Err(Ok(TwapOracleError::NotPositiveAmount))
    );
    assert_eq!(
        client.try_record_swap(&key_id, &-50),
        Err(Ok(TwapOracleError::NotPositiveAmount))
    );

    client.record_swap(&key_id, &100);
    env.ledger().set_timestamp(5_000);
    client.record_swap(&key_id, &200);

    // Window = 0 rejected with TWAPInvalid
    assert_eq!(
        client.try_get_twap(&key_id, &0),
        Err(Ok(TwapOracleError::TWAPInvalid))
    );

    // Window > max_window_seconds (86_400) rejected with TWAPInvalid
    assert_eq!(
        client.try_get_twap(&key_id, &(WINDOW_24H + 1)),
        Err(Ok(TwapOracleError::TWAPInvalid))
    );
}

#[test]
fn test_admin_initialization_and_config_management() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TwapOracleContract, ());
    let client = TwapOracleContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let key_id = Address::generate(&env);

    // Initial admin set
    assert_eq!(client.get_admin(), None);
    assert!(client.try_initialize(&admin).is_ok());
    assert_eq!(client.get_admin(), Some(admin.clone()));

    // Re-initialization reverts
    assert_eq!(
        client.try_initialize(&admin),
        Err(Ok(TwapOracleError::AlreadyInitialized))
    );

    // Default configuration returned
    let default_config = client.get_config(&key_id);
    assert_eq!(default_config.min_observations, DEFAULT_MIN_OBSERVATIONS);
    assert_eq!(
        default_config.max_window_seconds,
        DEFAULT_MAX_WINDOW_SECONDS
    );

    // Admin updates config
    assert!(client
        .try_set_config(&admin, &key_id, &5u32, &43_200u64)
        .is_ok());
    let custom_config = client.get_config(&key_id);
    assert_eq!(custom_config.min_observations, 5);
    assert_eq!(custom_config.max_window_seconds, 43_200);

    // Unauthorized non-admin cannot update config
    let rogue = Address::generate(&env);
    assert_eq!(
        client.try_set_config(&rogue, &key_id, &3u32, &86_400u64),
        Err(Ok(TwapOracleError::Unauthorized))
    );

    // Invalid config parameters rejected
    assert_eq!(
        client.try_set_config(&admin, &key_id, &0u32, &86_400u64),
        Err(Ok(TwapOracleError::TWAPInvalid))
    );
    assert_eq!(
        client.try_set_config(&admin, &key_id, &2u32, &0u64),
        Err(Ok(TwapOracleError::TWAPInvalid))
    );

    // Admin rotation
    let new_admin = Address::generate(&env);
    assert!(client.try_set_admin(&admin, &new_admin).is_ok());
    assert_eq!(client.get_admin(), Some(new_admin.clone()));

    // Old admin can no longer configure
    assert_eq!(
        client.try_set_config(&admin, &key_id, &3u32, &86_400u64),
        Err(Ok(TwapOracleError::Unauthorized))
    );
}

#[test]
fn test_views_and_aliases() {
    let (env, client, _admin, key_id) = setup_test();

    env.ledger().set_timestamp(1_000);
    // Use record_trade alias
    client.record_trade(&key_id, &250);

    assert_eq!(client.get_latest_price(&key_id), 250);
    assert_eq!(client.get_observation_count(&key_id), 1);

    env.ledger().set_timestamp(3_000); // +2,000s
                                       // Use update alias
    client.update(&key_id, &350);

    assert_eq!(client.get_latest_price(&key_id), 350);
    assert_eq!(client.get_observation_count(&key_id), 2);

    // Cumulative at t=3,000: 250 * 2,000 = 500,000
    assert_eq!(client.get_cumulative_price_time(&key_id), 500_000);

    // Advance timestamp without trade to t=4,000 (+1,000s under price 350)
    // Cumulative at t=4,000: 500,000 + 350 * 1,000 = 850,000
    env.ledger().set_timestamp(4_000);
    assert_eq!(client.get_cumulative_price_time(&key_id), 850_000);
}
