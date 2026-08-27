//! Unit tests for the `BatchTransferCompleted` event emitted by `batch_transfer_keys`.
//!
//! Each test asserts exactly one field of the event payload so that a regression
//! on any single field produces a focused, descriptive failure.
//!
//! Event shape emitted by `batch_transfer_keys`:
//! - topics: `(BATCH_TRANSFER_COMPLETED_EVENT_NAME, creator, from)`
//! - data:   `BatchTransferCompletedEvent { creator_id, from, transfers, total_transferred, ledger }`

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, set_ledger_sequence, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Vec,
};

const KEY_PRICE: i128 = 100;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;
const TEST_LEDGER: u32 = 77;

fn setup_batch_transfer(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
) -> (Address, Address, Address, Address) {
    set_pricing_and_fees(env, client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    set_ledger_sequence(env, TEST_LEDGER);

    let creator = Address::generate(env);
    let sender = Address::generate(env);
    let r1 = Address::generate(env);
    let r2 = Address::generate(env);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    for _ in 0..3 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }

    let transfers = Vec::from_array(env, [(r1.clone(), 2u32), (r2.clone(), 1u32)]);
    client.batch_transfer_keys(&creator, &sender, &transfers);

    (creator, sender, r1, r2)
}

fn last_batch_transfer_event(env: &Env) -> events::BatchTransferCompletedEvent {
    let event_log = env.events().all();
    let (_, _, data) = event_log.last().expect("at least one event must exist");
    data.into_val(env)
}

#[test]
fn test_batch_transfer_event_creator_id_field() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (creator, _, _, _) = setup_batch_transfer(&env, &client);

    let payload = last_batch_transfer_event(&env);
    assert_eq!(
        payload.creator_id, creator,
        "creator_id field must match the creator address"
    );
}

#[test]
fn test_batch_transfer_event_from_field() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (_, sender, _, _) = setup_batch_transfer(&env, &client);

    let payload = last_batch_transfer_event(&env);
    assert_eq!(
        payload.from, sender,
        "from field must match the sender address"
    );
}

#[test]
fn test_batch_transfer_event_total_transferred_field() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_batch_transfer(&env, &client);

    let payload = last_batch_transfer_event(&env);
    assert_eq!(
        payload.total_transferred, 3,
        "total_transferred must equal the sum of all quantities (2 + 1 = 3)"
    );
}

#[test]
fn test_batch_transfer_event_transfers_length() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_batch_transfer(&env, &client);

    let payload = last_batch_transfer_event(&env);
    assert_eq!(
        payload.transfers.len(),
        2,
        "transfers vec must contain exactly 2 entries"
    );
}

#[test]
fn test_batch_transfer_event_transfers_quantities() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (_, _, r1, r2) = setup_batch_transfer(&env, &client);

    let payload = last_batch_transfer_event(&env);
    let (ref addr0, qty0) = payload.transfers.get(0).unwrap();
    let (ref addr1, qty1) = payload.transfers.get(1).unwrap();

    assert_eq!(*addr0, r1, "first transfer recipient must be r1");
    assert_eq!(qty0, 2u32, "first transfer quantity must be 2");
    assert_eq!(*addr1, r2, "second transfer recipient must be r2");
    assert_eq!(qty1, 1u32, "second transfer quantity must be 1");
}

#[test]
fn test_batch_transfer_event_ledger_field_matches_set_sequence() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_batch_transfer(&env, &client);

    let payload = last_batch_transfer_event(&env);
    assert_eq!(
        payload.ledger, TEST_LEDGER,
        "ledger field must match the ledger sequence at the time of the call"
    );
}

#[test]
fn test_batch_transfer_no_event_emitted_on_failure() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: soroban_sdk::String::from_str(&env, "bob"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.buy_key(&creator, &sender, &KEY_PRICE, &None);

    let events_before = env.events().all().len();

    // This must revert: sender only holds 1 key but requests 5.
    let transfers = Vec::from_array(&env, [(recipient.clone(), 5u32)]);
    let _ = client.try_batch_transfer_keys(&creator, &sender, &transfers);

    let events_after = env.events().all().len();
    assert_eq!(
        events_before, events_after,
        "no event must be emitted when batch_transfer_keys reverts"
    );
}
