//! Runs the example against the real `creator-keys` contract, so a protocol
//! change that breaks an integrator also breaks this suite in CI.

use crate::{
    ConsumerContract, ConsumerContractClient, ConsumerError, Receipt, ACCESS_EVENT, RECEIVED_EVENT,
};
use common::KEY_RECEIVED_ACK;
use creator_keys::{
    events::{TransferCalledEvent, TRANSFER_CALLED_EVENT_NAME},
    ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Bytes, Env, String, Symbol, TryFromVal, Val,
};

struct Setup {
    env: Env,
    keys: CreatorKeysContractClient<'static>,
    keys_id: Address,
    creator: Address,
    consumer: ConsumerContractClient<'static>,
    consumer_id: Address,
}

/// Deploys `creator-keys` with a price and fee split, registers one creator and
/// deploys the consumer for that creator's key.
fn setup(min_balance: u32) -> Setup {
    let env = Env::default();
    env.mock_all_auths();

    let keys_id = env.register(CreatorKeysContract, ());
    let keys = CreatorKeysContractClient::new(&env, &keys_id);
    let admin = Address::generate(&env);
    keys.set_protocol_admin(&admin, &admin);
    keys.set_key_price(&admin, &100i128);
    keys.set_fee_config(&admin, &9000u32, &1000u32);
    let creator = register_creator(&env, &keys, "alice");

    let consumer_id = env.register(
        ConsumerContract,
        (keys_id.clone(), creator.clone(), min_balance),
    );
    let consumer = ConsumerContractClient::new(&env, &consumer_id);

    Setup {
        env,
        keys,
        keys_id,
        creator,
        consumer,
        consumer_id,
    }
}

fn register_creator(env: &Env, keys: &CreatorKeysContractClient, handle: &str) -> Address {
    let creator = Address::generate(env);
    keys.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    creator
}

/// Buys `count` keys for `holder`, paying exactly what the live quote asks.
fn buy(keys: &CreatorKeysContractClient, creator: &Address, holder: &Address, count: u32) {
    for _ in 0..count {
        let quote = keys.get_buy_quote(creator);
        keys.buy_key(creator, holder, &quote.total_amount, &None);
    }
}

/// Returns the data of the first event from `source` whose first topic is
/// `name`. Events must be read right after the call that emitted them: the
/// test environment clears them at the next top-level invocation.
fn event_data(env: &Env, source: &Address, name: Symbol) -> Option<(Val, Val)> {
    env.events()
        .all()
        .iter()
        .find_map(|(contract, topics, data)| {
            let topic0 = Symbol::try_from_val(env, &topics.get(0)?).ok()?;
            let topic1 = topics.get(1)?;
            (contract == *source && topic0 == name).then_some((topic1, data))
        })
}

// --- Gating on key balance ---------------------------------------------------

#[test]
fn holder_with_enough_keys_passes_the_gate() {
    let s = setup(2);
    let holder = Address::generate(&s.env);
    buy(&s.keys, &s.creator, &holder, 2);

    assert_eq!(s.consumer.enter(&holder), 2);
    let (topic, data) = event_data(&s.env, &s.consumer_id, ACCESS_EVENT).unwrap();
    assert_eq!(Address::try_from_val(&s.env, &topic).unwrap(), holder);
    assert_eq!(u32::try_from_val(&s.env, &data).unwrap(), 2);

    assert!(s.consumer.has_access(&holder));
    assert_eq!(s.consumer.entries(&holder), 1);
}

#[test]
fn holder_below_the_minimum_is_rejected() {
    let s = setup(2);
    let holder = Address::generate(&s.env);
    buy(&s.keys, &s.creator, &holder, 1);

    assert_eq!(
        s.consumer.try_enter(&holder),
        Err(Ok(ConsumerError::InsufficientKeys))
    );
    assert!(!s.consumer.has_access(&holder));
    assert_eq!(s.consumer.entries(&holder), 0);
}

#[test]
fn gate_requires_the_callers_signature() {
    let s = setup(1);
    let holder = Address::generate(&s.env);
    buy(&s.keys, &s.creator, &holder, 1);
    s.env.set_auths(&[]);

    assert!(s.consumer.try_enter(&holder).is_err());
    assert_eq!(s.consumer.entries(&holder), 0);
}

/// Boundary: the gate reads the balance live, so access moves with the keys.
#[test]
fn access_follows_the_keys_after_a_transfer() {
    let s = setup(1);
    let alice = Address::generate(&s.env);
    let bob = Address::generate(&s.env);
    buy(&s.keys, &s.creator, &alice, 1);
    assert_eq!(s.consumer.enter(&alice), 1);

    s.keys.transfer_keys(&s.creator, &alice, &bob, &1);

    assert_eq!(
        s.consumer.try_enter(&alice),
        Err(Ok(ConsumerError::InsufficientKeys))
    );
    assert_eq!(s.consumer.enter(&bob), 1);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn constructor_rejects_a_zero_minimum() {
    setup(0);
}

// --- Reading the quote -------------------------------------------------------

#[test]
fn current_quote_matches_the_protocol_quote() {
    let s = setup(1);
    let holder = Address::generate(&s.env);
    buy(&s.keys, &s.creator, &holder, 3);

    let ours = s.consumer.current_quote();
    let theirs = s.keys.get_buy_quote(&s.creator);
    assert_eq!(ours.price, theirs.price);
    assert_eq!(ours.creator_fee, theirs.creator_fee);
    assert_eq!(ours.protocol_fee, theirs.protocol_fee);
    assert_eq!(ours.total_amount, theirs.total_amount);
    assert!(ours.total_amount > 0);
}

#[test]
fn quote_for_an_unregistered_key_is_unavailable() {
    let s = setup(1);
    let unregistered = Address::generate(&s.env);
    let consumer_id = s
        .env
        .register(ConsumerContract, (s.keys_id.clone(), unregistered, 1u32));
    let consumer = ConsumerContractClient::new(&s.env, &consumer_id);

    assert_eq!(
        consumer.try_current_quote(),
        Err(Ok(ConsumerError::QuoteUnavailable))
    );
}

#[test]
fn quote_before_a_price_is_set_is_unavailable() {
    let env = Env::default();
    env.mock_all_auths();
    let keys_id = env.register(CreatorKeysContract, ());
    let key_id = Address::generate(&env);
    let consumer_id = env.register(ConsumerContract, (keys_id, key_id, 1u32));
    let consumer = ConsumerContractClient::new(&env, &consumer_id);

    assert_eq!(
        consumer.try_current_quote(),
        Err(Ok(ConsumerError::QuoteUnavailable))
    );
}

// --- Receiving keys ----------------------------------------------------------

#[test]
fn transfer_and_call_delivers_keys_and_runs_the_callback() {
    let s = setup(1);
    let sender = Address::generate(&s.env);
    buy(&s.keys, &s.creator, &sender, 2);
    let data = Bytes::from_slice(&s.env, b"deposit");

    s.keys
        .transfer_and_call(&s.creator, &sender, &s.consumer_id, &2, &data);

    // Decode the protocol's `transfer_called` event and the consumer's own event
    // before the next call clears the event buffer.
    let (_, payload) = event_data(
        &s.env,
        &s.keys_id,
        Symbol::new(&s.env, TRANSFER_CALLED_EVENT_NAME),
    )
    .unwrap();
    let called = TransferCalledEvent::try_from_val(&s.env, &payload).unwrap();
    assert_eq!(called.creator_id, s.creator);
    assert_eq!(called.from, sender);
    assert_eq!(called.recipient, s.consumer_id);
    assert_eq!(called.amount, 2);
    assert_eq!(called.data_hash, s.env.crypto().sha256(&data).to_bytes());

    let (topic, amount) = event_data(&s.env, &s.consumer_id, RECEIVED_EVENT).unwrap();
    assert_eq!(Address::try_from_val(&s.env, &topic).unwrap(), sender);
    assert_eq!(u32::try_from_val(&s.env, &amount).unwrap(), 2);

    assert_eq!(s.consumer.held_keys(), 2);
    assert_eq!(s.keys.get_key_balance(&s.creator, &sender), 0);
    assert_eq!(
        s.consumer.last_receipt(),
        Some(Receipt {
            from: sender,
            amount: 2,
            data,
            ledger: s.env.ledger().sequence(),
        })
    );
}

#[test]
fn transfer_of_a_different_key_is_refused_and_reverted() {
    let s = setup(1);
    let other_creator = register_creator(&s.env, &s.keys, "bob");
    let sender = Address::generate(&s.env);
    buy(&s.keys, &other_creator, &sender, 1);

    assert_eq!(
        s.keys.try_transfer_and_call(
            &other_creator,
            &sender,
            &s.consumer_id,
            &1,
            &Bytes::new(&s.env),
        ),
        Err(Ok(ContractError::TransferNotAccepted))
    );
    assert_eq!(s.keys.get_key_balance(&other_creator, &sender), 1);
    assert_eq!(s.keys.get_key_balance(&other_creator, &s.consumer_id), 0);
    assert_eq!(s.consumer.last_receipt(), None);
}

/// The callback's arguments are untrusted: calling it directly records a
/// receipt but moves no keys, which is why `held_keys` reads the protocol.
#[test]
fn a_direct_callback_call_proves_nothing() {
    let s = setup(1);
    let mallory = Address::generate(&s.env);

    let ack = s
        .consumer
        .on_key_received(&mallory, &s.creator, &1_000, &Bytes::new(&s.env));

    assert_eq!(ack, KEY_RECEIVED_ACK);
    assert_eq!(s.consumer.last_receipt().unwrap().amount, 1_000);
    assert_eq!(s.consumer.held_keys(), 0);
    assert!(!s.consumer.has_access(&mallory));
}
