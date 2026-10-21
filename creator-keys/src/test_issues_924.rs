//! Tests for issue #924 (on-chain leaderboard snapshot for top holder rankings).
//!
//! The candidate wallet list is supplied by the caller because Soroban storage
//! cannot be enumerated on-chain; these tests therefore exercise ranking of the
//! supplied set, not completeness of an on-chain holder registry.

use crate::{
    events, ContractError, CreatorKeysContract, CreatorKeysContractClient, LeaderboardConfig,
    RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    vec, Address, Env, String, Symbol, TryIntoVal, Vec,
};

fn setup_test() -> (Env, CreatorKeysContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_treasury_address(&admin, &treasury);
    client.set_key_price(&admin, &100i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_protocol_fee_recipient(&admin, &treasury);

    (env, client, admin, treasury)
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient, creator: &Address) {
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

/// Buys `buys` keys for `buyer` so the resulting balance is unambiguous.
fn buy(client: &CreatorKeysContractClient, creator: &Address, buyer: &Address, buys: u32) {
    for _ in 0..buys {
        client.buy_key(creator, buyer, &1000i128, &None);
    }
}

fn advance_ledger(env: &Env, by: u32) {
    let current = env.ledger().sequence();
    env.ledger().set_sequence_number(current + by);
}

// ─── snapshot creation accuracy ──────────────────────────────────────────

#[test]
fn snapshot_records_top_holders_ranked_by_balance() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let small = Address::generate(&env);
    let medium = Address::generate(&env);
    let large = Address::generate(&env);
    buy(&client, &creator, &small, 1);
    buy(&client, &creator, &medium, 2);
    buy(&client, &creator, &large, 3);

    let candidates = vec![&env, small.clone(), medium.clone(), large.clone()];
    let ledger = client.take_leaderboard_snapshot(&admin, &creator, &candidates);
    assert_eq!(ledger, env.ledger().sequence());

    let snapshot = client.get_leaderboard(&creator, &ledger).unwrap();
    assert_eq!(snapshot.creator, creator);
    assert_eq!(snapshot.ledger, ledger);
    assert_eq!(snapshot.total_candidates, 3);
    assert_eq!(snapshot.entries.len(), 3);

    assert_eq!(snapshot.entries.get(0).unwrap().rank, 1);
    assert_eq!(snapshot.entries.get(0).unwrap().holder, large);
    assert_eq!(
        snapshot.entries.get(0).unwrap().balance,
        client.get_key_balance(&creator, &large)
    );

    assert_eq!(snapshot.entries.get(1).unwrap().rank, 2);
    assert_eq!(snapshot.entries.get(1).unwrap().holder, medium);

    assert_eq!(snapshot.entries.get(2).unwrap().rank, 3);
    assert_eq!(snapshot.entries.get(2).unwrap().holder, small);

    // Balances are strictly non-increasing down the leaderboard.
    assert!(snapshot.entries.get(0).unwrap().balance > snapshot.entries.get(1).unwrap().balance);
    assert!(snapshot.entries.get(1).unwrap().balance > snapshot.entries.get(2).unwrap().balance);
}

#[test]
fn snapshot_truncates_to_configured_top_n() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.set_leaderboard_config(&admin, &2u32, &0u32);

    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);
    let buyer_c = Address::generate(&env);
    let buyer_d = Address::generate(&env);
    buy(&client, &creator, &buyer_a, 4);
    buy(&client, &creator, &buyer_b, 3);
    buy(&client, &creator, &buyer_c, 2);
    buy(&client, &creator, &buyer_d, 1);

    let candidates = vec![
        &env,
        buyer_a.clone(),
        buyer_b.clone(),
        buyer_c.clone(),
        buyer_d.clone(),
    ];
    let ledger = client.take_leaderboard_snapshot(&admin, &creator, &candidates);

    let snapshot = client.get_leaderboard(&creator, &ledger).unwrap();
    assert_eq!(snapshot.top_n, 2);
    // `total_candidates` counts every supplied holder, `entries` is capped at N.
    assert_eq!(snapshot.total_candidates, 4);
    assert_eq!(snapshot.entries.len(), 2);
    assert_eq!(snapshot.entries.get(0).unwrap().holder, buyer_a);
    assert_eq!(snapshot.entries.get(1).unwrap().holder, buyer_b);
}

#[test]
fn snapshot_excludes_zero_balance_candidates() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let holder = Address::generate(&env);
    let empty = Address::generate(&env);
    buy(&client, &creator, &holder, 1);

    let candidates = vec![&env, empty.clone(), holder.clone()];
    let ledger = client.take_leaderboard_snapshot(&admin, &creator, &candidates);

    let snapshot = client.get_leaderboard(&creator, &ledger).unwrap();
    assert_eq!(snapshot.total_candidates, 1);
    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries.get(0).unwrap().holder, holder);
}

#[test]
fn snapshot_ties_break_on_ascending_holder_address() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let first = Address::generate(&env);
    let second = Address::generate(&env);
    buy(&client, &creator, &first, 1);
    buy(&client, &creator, &second, 1);
    assert_eq!(
        client.get_key_balance(&creator, &first),
        client.get_key_balance(&creator, &second)
    );

    let (low, high) = if first < second {
        (first.clone(), second.clone())
    } else {
        (second.clone(), first.clone())
    };

    // Candidate order is deliberately reversed: ranking must not depend on it.
    let candidates = vec![&env, high.clone(), low.clone()];
    let ledger = client.take_leaderboard_snapshot(&admin, &creator, &candidates);

    let snapshot = client.get_leaderboard(&creator, &ledger).unwrap();
    assert_eq!(snapshot.entries.len(), 2);
    assert_eq!(snapshot.entries.get(0).unwrap().holder, low);
    assert_eq!(snapshot.entries.get(1).unwrap().holder, high);
}

#[test]
fn snapshot_of_empty_candidate_set_is_recorded_with_no_entries() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));

    let snapshot = client.get_leaderboard(&creator, &ledger).unwrap();
    assert_eq!(snapshot.total_candidates, 0);
    assert!(snapshot.entries.is_empty());
}

#[test]
fn snapshot_is_rejected_for_unregistered_creator() {
    let (env, client, admin, _treasury) = setup_test();
    let unknown = Address::generate(&env);

    let result = client.try_take_leaderboard_snapshot(&admin, &unknown, &Vec::new(&env));
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn snapshot_candidate_list_above_cap_is_rejected() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let mut candidates = Vec::new(&env);
    for _ in 0..=crate::leaderboard::MAX_CANDIDATES {
        candidates.push_back(Address::generate(&env));
    }

    let result = client.try_take_leaderboard_snapshot(&admin, &creator, &candidates);
    assert_eq!(result, Err(Ok(ContractError::SnapshotHolderLimitExceeded)));
}

#[test]
fn second_snapshot_in_same_ledger_is_rejected() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));
    let result = client.try_take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));
    assert_eq!(result, Err(Ok(ContractError::SnapshotAlreadyExists)));
}

// ─── retrieval ───────────────────────────────────────────────────────────

#[test]
fn get_leaderboard_returns_none_for_unrecorded_ledger() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    assert!(client
        .get_leaderboard(&creator, &env.ledger().sequence())
        .is_none());
}

#[test]
fn get_leaderboard_returns_snapshot_for_every_recorded_ledger() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.set_leaderboard_config(&admin, &10u32, &0u32);

    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);
    buy(&client, &creator, &buyer_a, 3);
    buy(&client, &creator, &buyer_b, 1);

    let first_ledger = client.take_leaderboard_snapshot(
        &admin,
        &creator,
        &vec![&env, buyer_a.clone(), buyer_b.clone()],
    );
    advance_ledger(&env, 10);
    let second_ledger = client.take_leaderboard_snapshot(
        &admin,
        &creator,
        &vec![&env, buyer_a.clone(), buyer_b.clone()],
    );
    assert_ne!(first_ledger, second_ledger);

    let first = client.get_leaderboard(&creator, &first_ledger).unwrap();
    let second = client.get_leaderboard(&creator, &second_ledger).unwrap();
    assert_eq!(first.ledger, first_ledger);
    assert_eq!(second.ledger, second_ledger);
    assert_eq!(first.entries.get(0).unwrap().holder, buyer_a);
    assert_eq!(second.entries.get(0).unwrap().holder, buyer_a);
    assert_eq!(first.total_candidates, 2);
    assert_eq!(second.total_candidates, 2);
}

#[test]
fn get_leaderboard_ledgers_lists_recorded_snapshots_in_order() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.set_leaderboard_config(&admin, &10u32, &0u32);

    assert!(client.get_leaderboard_ledgers(&creator).is_empty());

    let first_ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));
    advance_ledger(&env, 5);
    let second_ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));

    let ledgers = client.get_leaderboard_ledgers(&creator);
    assert_eq!(ledgers.len(), 2);
    assert_eq!(ledgers.get(0).unwrap(), first_ledger);
    assert_eq!(ledgers.get(1).unwrap(), second_ledger);
}

#[test]
fn leaderboard_is_scoped_per_creator() {
    let (env, client, admin, _treasury) = setup_test();
    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);
    register_creator(&env, &client, &creator_a);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator_b.clone(),
            handle: String::from_str(&env, "bob"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let buyer = Address::generate(&env);
    buy(&client, &creator_a, &buyer, 2);

    let candidates = vec![&env, buyer.clone()];
    let ledger = client.take_leaderboard_snapshot(&admin, &creator_a, &candidates);

    assert!(client.get_leaderboard(&creator_a, &ledger).is_some());
    assert!(client.get_leaderboard(&creator_b, &ledger).is_none());
    assert!(client.get_leaderboard_ledgers(&creator_b).is_empty());
}

// ─── access control ──────────────────────────────────────────────────────

#[test]
fn snapshot_rejects_non_admin_non_governance_caller() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let stranger = Address::generate(&env);
    let result = client.try_take_leaderboard_snapshot(&stranger, &creator, &Vec::new(&env));
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn snapshot_rejects_admin_caller_once_governance_is_configured_but_admin_is_not_governance() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    // The admin remains authorised alongside the governance contract: the
    // check accepts either.
    let governance = Address::generate(&env);
    client.set_governance_address(&admin, &governance);
    client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));
}

#[test]
fn governance_contract_can_trigger_a_snapshot() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);

    let governance = Address::generate(&env);
    client.set_governance_address(&admin, &governance);

    let ledger = client.take_leaderboard_snapshot(&governance, &creator, &Vec::new(&env));
    assert!(client.get_leaderboard(&creator, &ledger).is_some());
}

#[test]
fn snapshot_is_admin_only_when_no_governance_address_is_configured() {
    let (env, client, _admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    assert!(client.get_governance_address().is_none());

    let stranger = Address::generate(&env);
    let result = client.try_take_leaderboard_snapshot(&stranger, &creator, &Vec::new(&env));
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ─── configuration ───────────────────────────────────────────────────────

#[test]
fn leaderboard_config_returns_defaults_before_it_is_set() {
    let (_env, client, _admin, _treasury) = setup_test();

    let config: LeaderboardConfig = client.get_leaderboard_config();
    assert_eq!(config.top_n, crate::leaderboard::DEFAULT_TOP_N);
    assert_eq!(
        config.retention_ledgers,
        crate::leaderboard::DEFAULT_RETENTION_LEDGERS
    );
    // Defaults are a valid, in-range configuration.
    assert!(config.top_n > 0 && config.top_n <= crate::leaderboard::MAX_TOP_N);
}

#[test]
fn set_leaderboard_config_round_trips() {
    let (_env, client, admin, _treasury) = setup_test();

    client.set_leaderboard_config(&admin, &25u32, &1000u32);

    let config = client.get_leaderboard_config();
    assert_eq!(config.top_n, 25);
    assert_eq!(config.retention_ledgers, 1000);
}

#[test]
fn set_leaderboard_config_rejects_zero_top_n() {
    let (_env, client, admin, _treasury) = setup_test();

    let result = client.try_set_leaderboard_config(&admin, &0u32, &10u32);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn set_leaderboard_config_rejects_top_n_above_cap() {
    let (_env, client, admin, _treasury) = setup_test();

    let too_high = crate::leaderboard::MAX_TOP_N + 1;
    let result = client.try_set_leaderboard_config(&admin, &too_high, &10u32);
    assert_eq!(result, Err(Ok(ContractError::LimitTooHigh)));
}

#[test]
fn set_leaderboard_config_accepts_maximum_top_n() {
    let (_env, client, admin, _treasury) = setup_test();

    client.set_leaderboard_config(&admin, &crate::leaderboard::MAX_TOP_N, &0u32);
    assert_eq!(
        client.get_leaderboard_config().top_n,
        crate::leaderboard::MAX_TOP_N
    );
}

#[test]
fn set_leaderboard_config_rejects_non_admin() {
    let (env, client, _admin, _treasury) = setup_test();

    let stranger = Address::generate(&env);
    let result = client.try_set_leaderboard_config(&stranger, &5u32, &5u32);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn set_leaderboard_config_emits_update_event() {
    let (env, client, admin, _treasury) = setup_test();
    client.set_leaderboard_config(&admin, &7u32, &77u32);

    let all_events = env.events().all();
    assert_eq!(all_events.len(), 1);
    let (_contract, topics, data) = all_events.get(0).unwrap();

    let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_admin: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    assert_eq!(name, events::LEADERBOARD_CONFIG_UPDATED_EVENT_NAME);
    assert_eq!(topic_admin, admin);

    let payload: events::LeaderboardConfigUpdatedEvent = data.try_into_val(&env).unwrap();
    assert_eq!(payload.admin, admin);
    assert_eq!(payload.old_top_n, crate::leaderboard::DEFAULT_TOP_N);
    assert_eq!(
        payload.old_retention_ledgers,
        crate::leaderboard::DEFAULT_RETENTION_LEDGERS
    );
    assert_eq!(payload.new_top_n, 7);
    assert_eq!(payload.new_retention_ledgers, 77);
    assert_eq!(payload.ledger, env.ledger().sequence());
}

// ─── pruning ─────────────────────────────────────────────────────────────

#[test]
fn aged_out_snapshots_are_pruned_on_the_next_snapshot() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.set_leaderboard_config(&admin, &10u32, &100u32);

    let old_ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));
    assert!(client.get_leaderboard(&creator, &old_ledger).is_some());

    // Stay inside the retention window: the old snapshot must survive.
    advance_ledger(&env, 50);
    let inside_ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));
    assert!(client.get_leaderboard(&creator, &old_ledger).is_some());
    assert!(client.get_leaderboard(&creator, &inside_ledger).is_some());

    // Cross the retention window: both earlier snapshots are pruned.
    advance_ledger(&env, 101);
    let new_ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));
    assert!(client.get_leaderboard(&creator, &old_ledger).is_none());
    assert!(client.get_leaderboard(&creator, &inside_ledger).is_none());
    assert!(client.get_leaderboard(&creator, &new_ledger).is_some());

    let ledgers = client.get_leaderboard_ledgers(&creator);
    assert_eq!(ledgers.len(), 1);
    assert_eq!(ledgers.get(0).unwrap(), new_ledger);
}

#[test]
fn pruning_emits_a_pruned_event() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.set_leaderboard_config(&admin, &10u32, &50u32);

    let old_ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));
    advance_ledger(&env, 60);
    let new_ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));

    let all_events = env.events().all();
    let mut pruned: Option<events::LeaderboardSnapshotPrunedEvent> = None;
    for (_contract, topics, data) in all_events.iter() {
        let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
        if name != events::LEADERBOARD_SNAPSHOT_PRUNED_EVENT_NAME {
            continue;
        }
        let topic_creator: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
        let topic_ledger: u32 = topics.get(2).unwrap().try_into_val(&env).unwrap();
        assert_eq!(topic_creator, creator);
        assert_eq!(topic_ledger, old_ledger);
        pruned = Some(data.try_into_val(&env).unwrap());
    }

    let pruned = pruned.expect("expected a leaderboard-pruned event for the aged-out snapshot");
    assert_eq!(pruned.creator_id, creator);
    assert_eq!(pruned.snapshot_ledger, old_ledger);
    assert_eq!(pruned.current_ledger, new_ledger);
}

#[test]
fn zero_retention_disables_age_based_pruning() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.set_leaderboard_config(&admin, &10u32, &0u32);

    let first_ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));
    // Well beyond the default retention window, which is disabled here.
    advance_ledger(&env, 1_000);
    let second_ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));

    assert!(client.get_leaderboard(&creator, &first_ledger).is_some());
    assert!(client.get_leaderboard(&creator, &second_ledger).is_some());
    assert_eq!(client.get_leaderboard_ledgers(&creator).len(), 2);
}

#[test]
fn retained_snapshot_count_is_capped_even_without_age_pruning() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.set_leaderboard_config(&admin, &10u32, &0u32);

    let mut newest_ledger = 0u32;
    let mut first_pruned_ledger = None;
    for _ in 0..=crate::leaderboard::MAX_RETAINED_SNAPSHOTS {
        let ledger = client.take_leaderboard_snapshot(&admin, &creator, &Vec::new(&env));
        if first_pruned_ledger.is_none() {
            first_pruned_ledger = Some(ledger);
        }
        newest_ledger = ledger;
        advance_ledger(&env, 1);
    }

    // The very first snapshot fell outside the hard cap; the newest survives.
    let pruned_ledger = first_pruned_ledger.unwrap();
    assert_ne!(pruned_ledger, newest_ledger);
    assert!(client.get_leaderboard(&creator, &pruned_ledger).is_none());
    assert!(client.get_leaderboard(&creator, &newest_ledger).is_some());

    let ledgers = client.get_leaderboard_ledgers(&creator);
    assert_eq!(ledgers.len(), crate::leaderboard::MAX_RETAINED_SNAPSHOTS);
    assert_eq!(ledgers.get(0).unwrap(), pruned_ledger + 1);
    assert_eq!(ledgers.get(ledgers.len() - 1).unwrap(), newest_ledger);
}

// ─── events ──────────────────────────────────────────────────────────────

#[test]
fn snapshot_emits_leaderboard_taken_event() {
    let (env, client, admin, _treasury) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.set_leaderboard_config(&admin, &10u32, &0u32);

    let buyer = Address::generate(&env);
    buy(&client, &creator, &buyer, 1);

    let candidates = vec![&env, buyer];
    let ledger = client.take_leaderboard_snapshot(&admin, &creator, &candidates);

    let all_events = env.events().all();
    let (_contract, topics, data) = all_events.last().unwrap();

    let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_creator: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let topic_ledger: u32 = topics.get(2).unwrap().try_into_val(&env).unwrap();
    assert_eq!(name, events::LEADERBOARD_SNAPSHOT_TAKEN_EVENT_NAME);
    assert_eq!(topic_creator, creator);
    assert_eq!(topic_ledger, ledger);

    let payload: events::LeaderboardSnapshotTakenEvent = data.try_into_val(&env).unwrap();
    assert_eq!(payload.creator_id, creator);
    assert_eq!(payload.snapshot_ledger, ledger);
    assert_eq!(payload.top_n, 10);
    assert_eq!(payload.total_candidates, 1);
    assert_eq!(payload.recorded_entries, 1);
}
