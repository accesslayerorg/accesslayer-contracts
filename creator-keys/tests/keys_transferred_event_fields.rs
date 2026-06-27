//! Unit tests for the `KeysTransferred` event emitted on successful peer-to-peer transfer.
//!
//! Each test asserts exactly one field of the event payload independently so that a
//! regression on any single field produces a focused, descriptive failure.
//!
//! Event shape emitted by `transfer_keys`:
//! - topics: `(KEYS_TRANSFERRED_EVENT_NAME, creator, from)`
//! - data:   `KeysTransferredEvent { creator_id, from, to, amount, ledger }`

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, set_ledger_sequence, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String,
};

const KEY_PRICE: i128 = 100;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;
const TRANSFER_AMOUNT: u32 = 1;
const TEST_LEDGER: u32 = 42;

fn setup_transfer(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
) -> (Address, Address, Address) {
    set_pricing_and_fees(env, client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    set_ledger_sequence(env, TEST_LEDGER);

    let creator = Address::generate(env);
    let sender = Address::generate(env);
    let recipient = Address::generate(env);

    client.register_creator(&creator, &String::from_str(env, "alice"), &None, &None);
    client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    client.transfer_keys(&creator, &sender, &recipient, &TRANSFER_AMOUNT);

    (creator, sender, recipient)
}

fn last_event_data(env: &Env) -> events::KeysTransferredEvent {
    let event_log = env.events().all();
    let (_, _, data) = event_log.last().expect("at least one event must exist");
    data.into_val(env)
}

#[test]
fn test_keys_transferred_event_creator_id_field() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (creator, _, _) = setup_transfer(&env, &client);

    let payload = last_event_data(&env);
    assert_eq!(
        payload.creator_id, creator,
        "creator_id field must match the creator address"
    );
}

#[test]
fn test_keys_transferred_event_from_field() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (_, sender, _) = setup_transfer(&env, &client);

    let payload = last_event_data(&env);
    assert_eq!(
        payload.from, sender,
        "from field must match the sender address"
    );
}

#[test]
fn test_keys_transferred_event_to_field() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (_, _, recipient) = setup_transfer(&env, &client);

    let payload = last_event_data(&env);
    assert_eq!(
        payload.to, recipient,
        "to field must match the recipient address"
    );
}

#[test]
fn test_keys_transferred_event_amount_field() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_transfer(&env, &client);

    let payload = last_event_data(&env);
    assert_eq!(
        payload.amount, TRANSFER_AMOUNT,
        "amount field must equal the number of keys transferred"
    );
}

#[test]
fn test_keys_transferred_event_ledger_field_is_nonzero() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_transfer(&env, &client);

    let payload = last_event_data(&env);
    assert!(
        payload.ledger > 0,
        "ledger field must be a positive non-zero sequence number"
    );
}
