//! Integration coverage for the complete governance proposal lifecycle.

use creator_keys::{
    events::{PollClosedEvent, PollError, POLL_CLOSED_EVENT_NAME},
    CreatorKeysContract, CreatorKeysContractClient,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    vec, Address, Env, IntoVal, String, Symbol,
};

const SEVEN_DAY_LEDGERS: u32 = (7 * 24 * 60 * 60) / 5;
const KEY_PRICE: i128 = 100;
const QUORUM_BPS: u32 = 1_000;

fn proposal_options(env: &Env) -> soroban_sdk::Vec<String> {
    vec![
        env,
        String::from_str(env, "Option A"),
        String::from_str(env, "Option B"),
    ]
}

#[test]
fn proposal_lifecycle_uses_snapshot_weights_quorum_and_closes() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &KEY_PRICE);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "governance"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Three holders provide 100 circulating keys with distinct vote weights.
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);
    let holder_c = Address::generate(&env);
    for _ in 0..5 {
        client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);
    }
    for _ in 0..3 {
        client.buy_key(&creator, &holder_b, &KEY_PRICE, &None);
    }
    for _ in 0..92 {
        client.buy_key(&creator, &holder_c, &KEY_PRICE, &None);
    }

    assert_eq!(client.get_total_key_supply(&creator), 100);
    client.set_quorum_bps(&creator, &QUORUM_BPS);

    let proposal_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Choose the next community investment"),
        &proposal_options(&env),
        &SEVEN_DAY_LEDGERS,
    );

    let initial_result = client.get_poll_result(&creator, &proposal_id);
    assert_eq!(initial_result.options.len(), 2);
    assert_eq!(initial_result.total_weight, 0);
    assert!(!initial_result.closed);
    assert!(!initial_result.expired);

    // Advance the ledger while the seven-day proposal is still active, then capture
    // two independent snapshots that remain below the 10% quorum threshold.
    env.ledger().with_mut(|ledger| ledger.sequence_number += 1);
    client.cast_vote_with_snapshot(&creator, &holder_a, &proposal_id, &0);
    client.cast_vote_with_snapshot(&creator, &holder_b, &proposal_id, &1);

    assert_eq!(
        client.get_vote_snapshot(&creator, &proposal_id, &holder_a),
        Some(5)
    );
    assert_eq!(
        client.get_vote_snapshot(&creator, &proposal_id, &holder_b),
        Some(3)
    );
    let before_quorum = client.get_poll_result(&creator, &proposal_id);
    assert_eq!(before_quorum.vote_counts.get(0).unwrap(), 5);
    assert_eq!(before_quorum.vote_counts.get(1).unwrap(), 3);
    assert_eq!(before_quorum.total_weight, 8);

    // Eight out of one hundred keys is below the configured 10% quorum.
    let early_close = client.try_close_proposal(&creator, &proposal_id);
    assert_eq!(early_close, Err(Ok(PollError::QuorumNotReached)));
    assert!(!client.get_poll_result(&creator, &proposal_id).closed);

    // The third snapshot supplies the remaining 92 keys and makes the
    // proposal quorum-eligible, with option A as the clear winner.
    client.cast_vote_with_snapshot(&creator, &holder_c, &proposal_id, &0);
    assert_eq!(
        client.get_vote_snapshot(&creator, &proposal_id, &holder_c),
        Some(92)
    );
    let final_result = client.close_proposal(&creator, &proposal_id);
    assert!(final_result.closed);
    assert_eq!(final_result.vote_counts.get(0).unwrap(), 97);
    assert_eq!(final_result.vote_counts.get(1).unwrap(), 3);
    assert_eq!(final_result.total_weight, 100);
    let winning_option =
        if final_result.vote_counts.get(0).unwrap() > final_result.vote_counts.get(1).unwrap() {
            0
        } else {
            1
        };
    assert_eq!(winning_option, 0);

    // The close event carries the final participation result and quorum flag.
    let close_events = env.events().all();
    let close_event = close_events
        .iter()
        .find(|(contract, topics, _)| {
            *contract == contract_id
                && topics
                    .get(0)
                    .map(|value| {
                        let event_name: Symbol = value.into_val(&env);
                        event_name == POLL_CLOSED_EVENT_NAME
                    })
                    .unwrap_or(false)
        })
        .expect("expected proposal closed event");
    let creator_topic: Address = close_event.1.get(1).unwrap().into_val(&env);
    let proposal_topic: u32 = close_event.1.get(2).unwrap().into_val(&env);
    let payload: PollClosedEvent = close_event.2.into_val(&env);
    assert_eq!(creator_topic, creator);
    assert_eq!(proposal_topic, proposal_id);
    assert_eq!(payload.creator_id, creator);
    assert_eq!(payload.poll_id, proposal_id);
    assert_eq!(payload.total_weight, 100);
    assert!(payload.quorum_reached);
}
