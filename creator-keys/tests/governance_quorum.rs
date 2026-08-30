use creator_keys::{
    events::{
        PollClosedEvent, PollError, QuorumUpdatedEvent, POLL_CLOSED_EVENT_NAME,
        QUORUM_UPDATED_EVENT_NAME,
    },
    CreatorKeysContract, CreatorKeysContractClient,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    vec, Address, Env, IntoVal, String, Symbol,
};

fn poll_options(env: &Env) -> soroban_sdk::Vec<String> {
    vec![
        env,
        String::from_str(env, "Option A"),
        String::from_str(env, "Option B"),
    ]
}

#[test]
fn test_set_quorum_bps_success_and_event() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    assert_eq!(client.get_quorum_bps(&creator), 0);

    // Set quorum to 1500 bps (15%)
    client.set_quorum_bps(&creator, &1500);

    // Verify quorum_updated event immediately following invocation
    let events1 = env.events().all();
    let (_, topics0, data0) = events1.last().expect("expected event from set_quorum_bps");
    let topic_sym0: Symbol = topics0.get(0).unwrap().into_val(&env);
    let topic_creator0: Address = topics0.get(1).unwrap().into_val(&env);
    let payload0: QuorumUpdatedEvent = data0.into_val(&env);
    assert_eq!(topic_sym0, QUORUM_UPDATED_EVENT_NAME);
    assert_eq!(topic_creator0, creator);
    assert_eq!(payload0.creator, creator);
    assert_eq!(payload0.quorum_bps, 1500);

    assert_eq!(client.get_quorum_bps(&creator), 1500);

    // Update quorum to 3000 bps (30%)
    client.set_quorum_bps(&creator, &3000);

    // Verify quorum_updated event immediately following second invocation
    let events2 = env.events().all();
    let (_, topics1, data1) = events2.last().expect("expected event from set_quorum_bps");
    let topic_sym1: Symbol = topics1.get(0).unwrap().into_val(&env);
    let topic_creator1: Address = topics1.get(1).unwrap().into_val(&env);
    let payload1: QuorumUpdatedEvent = data1.into_val(&env);
    assert_eq!(topic_sym1, QUORUM_UPDATED_EVENT_NAME);
    assert_eq!(topic_creator1, creator);
    assert_eq!(payload1.creator, creator);
    assert_eq!(payload1.quorum_bps, 3000);

    assert_eq!(client.get_quorum_bps(&creator), 3000);
}

#[test]
fn test_set_quorum_bps_bounds_validation() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Quorum above 5000 bps (50%) should panic with QuorumTooHigh
    let err_too_high = client.try_set_quorum_bps(&creator, &5001);
    assert_eq!(err_too_high, Err(Ok(PollError::QuorumTooHigh)));

    let err_max_exceeded = client.try_set_quorum_bps(&creator, &10000);
    assert_eq!(err_max_exceeded, Err(Ok(PollError::QuorumTooHigh)));

    // Quorum below 100 bps (1%) should panic with QuorumTooLow
    let err_too_low = client.try_set_quorum_bps(&creator, &99);
    assert_eq!(err_too_low, Err(Ok(PollError::QuorumTooLow)));

    let err_zero = client.try_set_quorum_bps(&creator, &0);
    assert_eq!(err_zero, Err(Ok(PollError::QuorumTooLow)));

    // Minimum and maximum valid bounds (100 and 5000) succeed
    assert_eq!(client.try_set_quorum_bps(&creator, &100), Ok(Ok(())));
    assert_eq!(client.get_quorum_bps(&creator), 100);

    assert_eq!(client.try_set_quorum_bps(&creator, &5000), Ok(Ok(())));
    assert_eq!(client.get_quorum_bps(&creator), 5000);
}

#[test]
fn test_set_quorum_bps_unauthorized() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);

    // Setting quorum for an unregistered creator returns NotRegistered
    env.mock_all_auths();
    let err_unregistered = client.try_set_quorum_bps(&unregistered_creator, &2000);
    assert_eq!(err_unregistered, Err(Ok(PollError::NotRegistered)));
}

#[test]
fn test_close_poll_below_quorum_reverts_quorum_not_reached() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Buy 10 keys total across 2 holders (circulating supply = 10)
    let holder1 = Address::generate(&env);
    let holder2 = Address::generate(&env);
    client.buy_key(&creator, &holder1, &100, &None);
    for _ in 0..9 {
        client.buy_key(&creator, &holder2, &100, &None);
    }

    // Set 25% quorum requirement (2500 bps = 2.5 keys out of 10)
    client.set_quorum_bps(&creator, &2500);

    // Create a poll
    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Expand to new chains?"),
        &poll_options(&env),
        &10,
    );

    // Only holder1 votes with 1 key (10% participation, below 25% quorum)
    client.cast_vote(&creator, &holder1, &poll_id, &0);

    let result_before = client.get_poll_result(&creator, &poll_id);
    assert_eq!(result_before.total_weight, 1);
    assert!(!result_before.closed);

    // Attempting to close the proposal fails with QuorumNotReached
    let close_res = client.try_close_poll(&creator, &poll_id);
    assert_eq!(close_res, Err(Ok(PollError::QuorumNotReached)));

    // Poll remains open and unclosed
    let result_after = client.get_poll_result(&creator, &poll_id);
    assert_eq!(result_after.total_weight, 1);
    assert!(!result_after.closed);
}

#[test]
fn test_close_poll_meeting_quorum_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Circulating supply = 10 keys
    let holder1 = Address::generate(&env);
    let holder2 = Address::generate(&env);
    for _ in 0..3 {
        client.buy_key(&creator, &holder1, &100, &None);
    }
    for _ in 0..7 {
        client.buy_key(&creator, &holder2, &100, &None);
    }

    // Set 25% quorum requirement (2500 bps)
    client.set_quorum_bps(&creator, &2500);

    // Create a poll
    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Expand to new chains?"),
        &poll_options(&env),
        &10,
    );

    // Holder1 votes with 3 keys (30% participation, meets 25% quorum)
    client.cast_vote(&creator, &holder1, &poll_id, &0);

    // Close poll succeeds
    let close_res = client.close_poll(&creator, &poll_id);
    assert_eq!(close_res.total_weight, 3);
    assert!(close_res.closed);

    // Verify poll_closed event
    let mut found_closed_event = false;
    for (contract, topics, data) in env.events().all().iter() {
        if contract == contract_id {
            let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
            if topic0 == POLL_CLOSED_EVENT_NAME {
                let topic1: Address = topics.get(1).unwrap().into_val(&env);
                let topic2: u32 = topics.get(2).unwrap().into_val(&env);
                let payload: PollClosedEvent = data.into_val(&env);
                assert_eq!(topic1, creator);
                assert_eq!(topic2, poll_id);
                assert_eq!(payload.creator_id, creator);
                assert_eq!(payload.poll_id, poll_id);
                assert_eq!(payload.total_weight, 3);
                assert!(payload.quorum_reached);
                found_closed_event = true;
            }
        }
    }
    assert!(found_closed_event, "expected PollClosedEvent in events");

    // Result view shows closed
    let poll_result = client.get_poll_result(&creator, &poll_id);
    assert!(poll_result.closed);

    // Voting on a closed poll is rejected with AlreadyClosed
    let vote_after_close = client.try_cast_vote(&creator, &holder2, &poll_id, &1);
    assert_eq!(vote_after_close, Err(Ok(PollError::AlreadyClosed)));

    // Re-closing already closed poll returns AlreadyClosed
    let reclose_res = client.try_close_poll(&creator, &poll_id);
    assert_eq!(reclose_res, Err(Ok(PollError::AlreadyClosed)));
}

#[test]
fn test_close_poll_without_quorum_configured() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &100, &None);

    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Default quorum test"),
        &poll_options(&env),
        &10,
    );

    client.cast_vote(&creator, &holder, &poll_id, &0);

    // Closes successfully with default (0 bps) quorum
    let result = client.close_poll(&creator, &poll_id);
    assert!(result.closed);
    assert_eq!(result.total_weight, 1);
}
