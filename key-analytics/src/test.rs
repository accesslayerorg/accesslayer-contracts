#![cfg(test)]

use crate::{
    AnalyticsSnapshotError, KeyAnalyticsContract, KeyAnalyticsContractClient, KeyMarketStatsView,
    SnapshotTaken, MAX_SNAPSHOT_RANGE_DAYS, SECONDS_PER_DAY, SNAPSHOT_TAKEN_EVENT_NAME,
};
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, TryFromVal,
};

// ── Mock Creator Keys Contract ────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum MockStorageKey {
    Stats(Address),
    ShouldFail(Address),
}

/// Stand-in for `creator-keys` exposing `get_key_market_stats`.
#[contract]
pub struct MockCreatorKeys;

#[contractimpl]
impl MockCreatorKeys {
    pub fn set_market_stats(env: Env, creator: Address, stats: KeyMarketStatsView) {
        env.storage()
            .instance()
            .set(&MockStorageKey::Stats(creator), &stats);
    }

    pub fn set_should_fail(env: Env, creator: Address, fail: bool) {
        env.storage()
            .instance()
            .set(&MockStorageKey::ShouldFail(creator), &fail);
    }

    pub fn get_key_market_stats(
        env: Env,
        creator: Address,
    ) -> Result<KeyMarketStatsView, AnalyticsSnapshotError> {
        let should_fail: bool = env
            .storage()
            .instance()
            .get(&MockStorageKey::ShouldFail(creator.clone()))
            .unwrap_or(false);
        if should_fail {
            return Err(AnalyticsSnapshotError::MetricFetchFailed);
        }

        env.storage()
            .instance()
            .get(&MockStorageKey::Stats(creator))
            .ok_or(AnalyticsSnapshotError::MetricFetchFailed)
    }
}

// ── Test Setup Helpers ────────────────────────────────────────────────────────

struct TestSetup<'a> {
    env: Env,
    client: KeyAnalyticsContractClient<'a>,
    contract_id: Address,
    mock_client: MockCreatorKeysClient<'a>,
}

fn setup_test<'a>(env: &'a Env) -> TestSetup<'a> {
    env.mock_all_auths();

    let mock_keys_id = env.register(MockCreatorKeys, ());
    let mock_client = MockCreatorKeysClient::new(env, &mock_keys_id);

    let contract_id = env.register(KeyAnalyticsContract, ());
    let client = KeyAnalyticsContractClient::new(env, &contract_id);

    client.initialize(&mock_keys_id);

    TestSetup {
        env: env.clone(),
        client,
        contract_id,
        mock_client,
    }
}

// ── Unit Tests ────────────────────────────────────────────────────────────────

#[test]
fn test_single_use_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let keys = Address::generate(&env);
    let contract_id = env.register(KeyAnalyticsContract, ());
    let client = KeyAnalyticsContractClient::new(&env, &contract_id);

    // Initial setup succeeds
    assert_eq!(client.get_creator_keys(), None);
    client.initialize(&keys);
    assert_eq!(client.get_creator_keys(), Some(keys.clone()));

    // Cannot initialize a second time
    let second_keys = Address::generate(&env);
    let res = client.try_initialize(&second_keys);
    assert_eq!(res, Err(Ok(AnalyticsSnapshotError::AlreadyInitialized)));
    assert_eq!(client.get_creator_keys(), Some(keys));
}

#[test]
fn test_take_daily_snapshot_success_and_metrics_stored_exactly() {
    let env = Env::default();
    let setup = setup_test(&env);
    setup
        .env
        .ledger()
        .set_timestamp(1_000 * SECONDS_PER_DAY + 500);

    let key_id = Address::generate(&setup.env);
    let stats = KeyMarketStatsView {
        creator: key_id.clone(),
        supply: 40,
        buy_price: 2_500_000,
        sell_price: 2_400_000,
        holder_count: 15,
        volume: 50_000_000,
    };
    setup.mock_client.set_market_stats(&key_id, &stats);

    let snapshot = setup.client.take_daily_snapshot(&key_id);

    assert_eq!(snapshot.key_id, key_id);
    assert_eq!(snapshot.date, 1_000);
    assert_eq!(snapshot.price, 2_500_000);
    assert_eq!(snapshot.volume, 50_000_000);
    assert_eq!(snapshot.holder_count, 15);
    // market_cap = buy_price * supply = 2_500_000 * 40 = 100_000_000
    assert_eq!(snapshot.market_cap, 100_000_000);

    // Verify stored snapshot is retrieved accurately via get_snapshot
    let stored = setup.client.get_snapshot(&key_id, &1_000);
    assert_eq!(stored, snapshot);
}

#[test]
fn test_duplicate_snapshot_same_day_fails_with_snapshot_exists() {
    let env = Env::default();
    let setup = setup_test(&env);
    setup
        .env
        .ledger()
        .set_timestamp(2_000 * SECONDS_PER_DAY + 10);

    let key_id = Address::generate(&setup.env);
    let stats = KeyMarketStatsView {
        creator: key_id.clone(),
        supply: 10,
        buy_price: 1_000_000,
        sell_price: 950_000,
        holder_count: 5,
        volume: 10_000_000,
    };
    setup.mock_client.set_market_stats(&key_id, &stats);

    // First snapshot succeeds
    let first = setup.client.take_daily_snapshot(&key_id);
    assert_eq!(first.date, 2_000);

    // Second snapshot on the same day must fail with SnapshotExists
    let second = setup.client.try_take_daily_snapshot(&key_id);
    assert_eq!(second, Err(Ok(AnalyticsSnapshotError::SnapshotExists)));
}

#[test]
fn test_snapshot_next_day_succeeds() {
    let env = Env::default();
    let setup = setup_test(&env);
    let start_ts = 500 * SECONDS_PER_DAY;
    setup.env.ledger().set_timestamp(start_ts);

    let key_id = Address::generate(&setup.env);
    let stats_day1 = KeyMarketStatsView {
        creator: key_id.clone(),
        supply: 10,
        buy_price: 1_000,
        sell_price: 900,
        holder_count: 2,
        volume: 5_000,
    };
    setup.mock_client.set_market_stats(&key_id, &stats_day1);

    // Day 1
    let snap1 = setup.client.take_daily_snapshot(&key_id);
    assert_eq!(snap1.date, 500);
    assert_eq!(snap1.price, 1_000);

    // Advance to next day
    setup
        .env
        .ledger()
        .set_timestamp(start_ts + SECONDS_PER_DAY + 300);

    let stats_day2 = KeyMarketStatsView {
        creator: key_id.clone(),
        supply: 15,
        buy_price: 1_500,
        sell_price: 1_400,
        holder_count: 3,
        volume: 12_000,
    };
    setup.mock_client.set_market_stats(&key_id, &stats_day2);

    // Day 2 snapshot succeeds
    let snap2 = setup.client.take_daily_snapshot(&key_id);
    assert_eq!(snap2.date, 501);
    assert_eq!(snap2.price, 1_500);
    assert_eq!(snap2.market_cap, 15 * 1_500);

    // Both days are retrievable
    assert_eq!(setup.client.get_snapshot(&key_id, &500), snap1);
    assert_eq!(setup.client.get_snapshot(&key_id, &501), snap2);
}

#[test]
fn test_two_different_keys_same_day_are_independent() {
    let env = Env::default();
    let setup = setup_test(&env);
    setup.env.ledger().set_timestamp(300 * SECONDS_PER_DAY);

    let key_a = Address::generate(&setup.env);
    let key_b = Address::generate(&setup.env);

    let stats_a = KeyMarketStatsView {
        creator: key_a.clone(),
        supply: 20,
        buy_price: 500,
        sell_price: 450,
        holder_count: 8,
        volume: 20_000,
    };
    let stats_b = KeyMarketStatsView {
        creator: key_b.clone(),
        supply: 80,
        buy_price: 3_000,
        sell_price: 2_900,
        holder_count: 40,
        volume: 800_000,
    };
    setup.mock_client.set_market_stats(&key_a, &stats_a);
    setup.mock_client.set_market_stats(&key_b, &stats_b);

    // Both succeed independently on day 300
    let snap_a = setup.client.take_daily_snapshot(&key_a);
    let snap_b = setup.client.take_daily_snapshot(&key_b);

    assert_eq!(snap_a.price, 500);
    assert_eq!(snap_b.price, 3_000);

    // Key A duplicate fails, Key B can still be read
    assert_eq!(
        setup.client.try_take_daily_snapshot(&key_a),
        Err(Ok(AnalyticsSnapshotError::SnapshotExists))
    );
    assert_eq!(setup.client.get_snapshot(&key_b, &300), snap_b);
}

#[test]
fn test_get_snapshot_for_missing_date() {
    let env = Env::default();
    let setup = setup_test(&env);

    let key_id = Address::generate(&setup.env);
    let res = setup.client.try_get_snapshot(&key_id, &42);
    assert_eq!(res, Err(Ok(AnalyticsSnapshotError::SnapshotNotFound)));
}

#[test]
fn test_get_snapshot_range_full_partial_single_gaps_and_order() {
    let env = Env::default();
    let setup = setup_test(&env);

    let key_id = Address::generate(&setup.env);

    // Record snapshots on days 10, 11, and 13 (day 12 is skipped as a gap)
    let days = [10u64, 11u64, 13u64];
    for (i, &day) in days.iter().enumerate() {
        setup.env.ledger().set_timestamp(day * SECONDS_PER_DAY);
        let stats = KeyMarketStatsView {
            creator: key_id.clone(),
            supply: (i as u32 + 1) * 10,
            buy_price: (i as i128 + 1) * 100,
            sell_price: 90,
            holder_count: (i as u32 + 1) * 2,
            volume: 1_000 * (i as i128 + 1),
        };
        setup.mock_client.set_market_stats(&key_id, &stats);
        setup.client.take_daily_snapshot(&key_id);
    }

    // 1. Full range covering gaps: [10, 13]
    let full = setup.client.get_snapshot_range(&key_id, &10, &13);
    assert_eq!(full.len(), 3);
    assert_eq!(full.get(0).unwrap().date, 10);
    assert_eq!(full.get(1).unwrap().date, 11);
    assert_eq!(full.get(2).unwrap().date, 13); // Day 12 omitted, ascending order preserved

    // 2. Partial range: [11, 12]
    let partial = setup.client.get_snapshot_range(&key_id, &11, &12);
    assert_eq!(partial.len(), 1);
    assert_eq!(partial.get(0).unwrap().date, 11);

    // 3. Single day range: [11, 11]
    let single = setup.client.get_snapshot_range(&key_id, &11, &11);
    assert_eq!(single.len(), 1);
    assert_eq!(single.get(0).unwrap().date, 11);

    // 4. Missing day single range: [12, 12]
    let gap_only = setup.client.get_snapshot_range(&key_id, &12, &12);
    assert_eq!(gap_only.len(), 0);
}

#[test]
fn test_get_snapshot_range_invalid_bounds_and_span_limit() {
    let env = Env::default();
    let setup = setup_test(&env);

    let key_id = Address::generate(&setup.env);

    // from_date > to_date
    let res_inv = setup.client.try_get_snapshot_range(&key_id, &25, &20);
    assert_eq!(res_inv, Err(Ok(AnalyticsSnapshotError::InvalidRange)));

    // Exceeds max span (MAX_SNAPSHOT_RANGE_DAYS = 90)
    // Span = to - from + 1. If from = 1, to = 91 -> span = 91 > 90
    let res_too_large =
        setup
            .client
            .try_get_snapshot_range(&key_id, &1, &(1 + MAX_SNAPSHOT_RANGE_DAYS));
    assert_eq!(
        res_too_large,
        Err(Ok(AnalyticsSnapshotError::RangeTooLarge))
    );

    // Exactly 90 days span should be valid (from = 1, to = 90 -> span = 90)
    let res_ok = setup
        .client
        .get_snapshot_range(&key_id, &1, &MAX_SNAPSHOT_RANGE_DAYS);
    assert_eq!(res_ok.len(), 0); // No snapshots, but query itself is valid
}

#[test]
fn test_snapshot_taken_event_topics_and_payload() {
    let env = Env::default();
    let setup = setup_test(&env);
    setup.env.ledger().set_timestamp(700 * SECONDS_PER_DAY);

    let key_id = Address::generate(&setup.env);
    let stats = KeyMarketStatsView {
        creator: key_id.clone(),
        supply: 25,
        buy_price: 4_000,
        sell_price: 3_800,
        holder_count: 9,
        volume: 75_000,
    };
    setup.mock_client.set_market_stats(&key_id, &stats);

    setup.client.take_daily_snapshot(&key_id);

    let events = setup.env.events().all();
    assert!(!events.is_empty());

    let (contract, topics, data) = events.last().unwrap();
    assert_eq!(contract, setup.contract_id);

    // Topic 0: Event Name
    let event_name: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&setup.env);
    assert_eq!(event_name, SNAPSHOT_TAKEN_EVENT_NAME);

    // Topic 1: Key ID
    let event_key: Address = topics.get(1).unwrap().into_val(&setup.env);
    assert_eq!(event_key, key_id);

    // Topic 2: Date
    let event_date: u64 = topics.get(2).unwrap().into_val(&setup.env);
    assert_eq!(event_date, 700);

    // Payload verification: (price, volume, holder_count, market_cap)
    let payload = SnapshotTaken::try_from_val(&setup.env, &data).unwrap();
    assert_eq!(payload.price, 4_000);
    assert_eq!(payload.volume, 75_000);
    assert_eq!(payload.holder_count, 9);
    assert_eq!(payload.market_cap, 100_000); // 4_000 * 25
}

#[test]
fn test_metric_source_failure_and_unconfigured() {
    let env = Env::default();
    let setup = setup_test(&env);
    setup.env.ledger().set_timestamp(100 * SECONDS_PER_DAY);

    let key_id = Address::generate(&setup.env);

    // 1. Metric source returns failure (key not registered)
    let res_fail = setup.client.try_take_daily_snapshot(&key_id);
    assert_eq!(res_fail, Err(Ok(AnalyticsSnapshotError::MetricFetchFailed)));

    // 2. Metric source configured to explicitly fail
    setup.mock_client.set_should_fail(&key_id, &true);
    let res_fail_flag = setup.client.try_take_daily_snapshot(&key_id);
    assert_eq!(
        res_fail_flag,
        Err(Ok(AnalyticsSnapshotError::MetricFetchFailed))
    );

    // 3. Metric source unconfigured contract
    let uninit_id = env.register(KeyAnalyticsContract, ());
    let uninit_client = KeyAnalyticsContractClient::new(&env, &uninit_id);
    let res_uninit = uninit_client.try_take_daily_snapshot(&key_id);
    assert_eq!(res_uninit, Err(Ok(AnalyticsSnapshotError::NotInitialized)));
}

#[test]
fn test_arithmetic_overflow_protection() {
    let env = Env::default();
    let setup = setup_test(&env);
    setup.env.ledger().set_timestamp(50 * SECONDS_PER_DAY);

    let key_id = Address::generate(&setup.env);
    let stats = KeyMarketStatsView {
        creator: key_id.clone(),
        supply: u32::MAX,
        buy_price: i128::MAX,
        sell_price: i128::MAX,
        holder_count: 1,
        volume: 0,
    };
    setup.mock_client.set_market_stats(&key_id, &stats);

    let res = setup.client.try_take_daily_snapshot(&key_id);
    assert_eq!(res, Err(Ok(AnalyticsSnapshotError::ArithmeticOverflow)));
}
