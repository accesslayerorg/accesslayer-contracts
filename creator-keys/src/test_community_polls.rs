//! Tests for snapshot-weighted community polls (`community_polls.rs`).
use crate::{ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, String, Symbol, TryIntoVal, Vec,
};

struct Setup {
    env: Env,
    client: CreatorKeysContractClient<'static>,
    creator: Address,
}

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "pollcreator"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    Setup {
        env,
        client,
        creator,
    }
}

fn options(env: &Env, items: &[&str]) -> Vec<String> {
    let mut result = Vec::new(env);
    for item in items {
        result.push_back(String::from_str(env, item));
    }
    result
}

fn event_names(env: &Env) -> Vec<Symbol> {
    let mut found = Vec::new(env);
    for (_, topics, _) in env.events().all().iter() {
        if let Some(name) = topics.get(0).and_then(|t| t.try_into_val(env).ok()) {
            found.push_back(name);
        }
    }
    found
}

#[test]
fn create_snapshot_poll_rejects_non_creator() {
    let s = setup();
    let not_creator = Address::generate(&s.env);
    let opts = options(&s.env, &["Yes", "No"]);
    let err = s
        .client
        .try_create_snapshot_poll(
            &not_creator,
            &s.creator,
            &String::from_str(&s.env, "Ship v2?"),
            &opts,
            &3_600u64,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized);
}

#[test]
fn create_snapshot_poll_rejects_bad_option_count() {
    let s = setup();
    let one_option = options(&s.env, &["Only one"]);
    let err = s
        .client
        .try_create_snapshot_poll(
            &s.creator,
            &s.creator,
            &String::from_str(&s.env, "Ship v2?"),
            &one_option,
            &3_600u64,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InvalidPollOption);

    let five_options = options(&s.env, &["A", "B", "C", "D", "E"]);
    let err = s
        .client
        .try_create_snapshot_poll(
            &s.creator,
            &s.creator,
            &String::from_str(&s.env, "Ship v2?"),
            &five_options,
            &3_600u64,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::InvalidPollOption);
}

#[test]
fn vote_weight_is_fixed_at_snapshot_time() {
    let s = setup();
    let alice = Address::generate(&s.env);
    let bob = Address::generate(&s.env);
    s.client.buy_key(&s.creator, &alice, &100i128, &None);
    s.client.buy_key(&s.creator, &bob, &100i128, &None);

    let opts = options(&s.env, &["Yes", "No"]);
    let poll_id = s.client.create_snapshot_poll(
        &s.creator,
        &s.creator,
        &String::from_str(&s.env, "Ship v2?"),
        &opts,
        &3_600u64,
    );

    // Alice buys more keys *after* the snapshot; her vote weight must stay at
    // the balance she held when the poll was created, not her current balance.
    s.client.buy_key(&s.creator, &alice, &100i128, &None);

    s.client.cast_poll_vote(&alice, &poll_id, &0u32);
    s.client.cast_poll_vote(&bob, &poll_id, &1u32);

    let result = s.client.get_poll_results(&poll_id);
    assert_eq!(result.option_weights.get(0).unwrap(), 1);
    assert_eq!(result.option_counts.get(0).unwrap(), 1);
    assert_eq!(result.option_weights.get(1).unwrap(), 1);
    assert_eq!(result.option_counts.get(1).unwrap(), 1);
}

#[test]
fn duplicate_vote_is_rejected() {
    let s = setup();
    let alice = Address::generate(&s.env);
    s.client.buy_key(&s.creator, &alice, &100i128, &None);

    let opts = options(&s.env, &["Yes", "No"]);
    let poll_id = s.client.create_snapshot_poll(
        &s.creator,
        &s.creator,
        &String::from_str(&s.env, "Ship v2?"),
        &opts,
        &3_600u64,
    );

    s.client.cast_poll_vote(&alice, &poll_id, &0u32);
    let err = s
        .client
        .try_cast_poll_vote(&alice, &poll_id, &1u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::AlreadyVoted);
}

#[test]
fn non_holder_cannot_vote() {
    let s = setup();
    let holder = Address::generate(&s.env);
    let outsider = Address::generate(&s.env);
    s.client.buy_key(&s.creator, &holder, &100i128, &None);
    let opts = options(&s.env, &["Yes", "No"]);
    let poll_id = s.client.create_snapshot_poll(
        &s.creator,
        &s.creator,
        &String::from_str(&s.env, "Ship v2?"),
        &opts,
        &3_600u64,
    );

    let err = s
        .client
        .try_cast_poll_vote(&outsider, &poll_id, &0u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::NotAHolder);
}

#[test]
fn vote_after_expiry_is_rejected() {
    let s = setup();
    let alice = Address::generate(&s.env);
    s.client.buy_key(&s.creator, &alice, &100i128, &None);

    let opts = options(&s.env, &["Yes", "No"]);
    let poll_id = s.client.create_snapshot_poll(
        &s.creator,
        &s.creator,
        &String::from_str(&s.env, "Ship v2?"),
        &opts,
        &3_600u64,
    );

    s.env.ledger().set_timestamp(1_000 + 3_600);

    let err = s
        .client
        .try_cast_poll_vote(&alice, &poll_id, &0u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::PollClosed);
}

#[test]
fn close_snapshot_poll_locks_in_results_and_rejects_before_expiry() {
    let s = setup();
    let alice = Address::generate(&s.env);
    s.client.buy_key(&s.creator, &alice, &100i128, &None);

    let opts = options(&s.env, &["Yes", "No"]);
    let poll_id = s.client.create_snapshot_poll(
        &s.creator,
        &s.creator,
        &String::from_str(&s.env, "Ship v2?"),
        &opts,
        &3_600u64,
    );
    s.client.cast_poll_vote(&alice, &poll_id, &0u32);

    let err = s
        .client
        .try_close_snapshot_poll(&poll_id)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::DeadlinePassed);

    s.env.ledger().set_timestamp(1_000 + 3_600);
    let result = s.client.close_snapshot_poll(&poll_id);
    assert!(result.closed);
    assert_eq!(result.option_weights.get(0).unwrap(), 1);

    let err = s
        .client
        .try_close_snapshot_poll(&poll_id)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::ActionNotPending);
}

#[test]
fn poll_lifecycle_emits_all_three_events() {
    // The test harness's event log reflects only the most recent top-level
    // contract call, so each event is checked right after the call that
    // emits it rather than once at the end.
    let s = setup();
    let alice = Address::generate(&s.env);
    s.client.buy_key(&s.creator, &alice, &100i128, &None);

    let opts = options(&s.env, &["Yes", "No"]);
    let poll_id = s.client.create_snapshot_poll(
        &s.creator,
        &s.creator,
        &String::from_str(&s.env, "Ship v2?"),
        &opts,
        &3_600u64,
    );
    assert!(event_names(&s.env).contains(&symbol_short!("poll_new")));

    s.client.cast_poll_vote(&alice, &poll_id, &0u32);
    assert!(event_names(&s.env).contains(&symbol_short!("poll_vote")));

    s.env.ledger().set_timestamp(1_000 + 3_600);
    s.client.close_snapshot_poll(&poll_id);
    assert!(event_names(&s.env).contains(&symbol_short!("poll_end")));
}
