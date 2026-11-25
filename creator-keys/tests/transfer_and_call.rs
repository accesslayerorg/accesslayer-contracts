//! Integration tests for key-receiver callbacks.

mod contract_test_env;

use common::{KeyReceiver, KEY_RECEIVED_ACK};
use contract_test_env::{set_pricing_and_fees, test_env_with_auths};
use creator_keys::{
    events, ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
};
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    testutils::{Address as _, Events},
    Address, Bytes, Env, String, Symbol, TryIntoVal,
};

#[derive(Clone)]
#[contracttype]
enum ReceiverMode {
    Accept,
    Reject,
    Panic,
    Reenter(Address),
}

#[contracttype]
enum ReceiverKey {
    Mode,
    Received,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
struct ReceivedCallback {
    from: Address,
    key_id: Address,
    amount: u32,
    data: Bytes,
}

#[contract]
struct MockReceiver;

#[contractimpl]
impl MockReceiver {
    pub fn on_key_received(
        env: Env,
        from: Address,
        key_id: Address,
        amount: u32,
        data: Bytes,
    ) -> Symbol {
        <Self as KeyReceiver>::on_key_received(env, from, key_id, amount, data)
    }

    pub fn configure(env: Env, mode: ReceiverMode) {
        env.storage().instance().set(&ReceiverKey::Mode, &mode);
    }

    pub fn received(env: Env) -> Option<ReceivedCallback> {
        env.storage().instance().get(&ReceiverKey::Received)
    }
}

impl KeyReceiver for MockReceiver {
    fn on_key_received(
        env: Env,
        from: Address,
        key_id: Address,
        amount: u32,
        data: Bytes,
    ) -> Symbol {
        let mode: ReceiverMode = env
            .storage()
            .instance()
            .get(&ReceiverKey::Mode)
            .unwrap_or(ReceiverMode::Accept);
        match mode {
            ReceiverMode::Accept => {
                env.storage().instance().set(
                    &ReceiverKey::Received,
                    &ReceivedCallback {
                        from,
                        key_id,
                        amount,
                        data,
                    },
                );
                KEY_RECEIVED_ACK
            }
            ReceiverMode::Reject => symbol_short!("reject"),
            ReceiverMode::Panic => panic!("receiver rejected transfer"),
            ReceiverMode::Reenter(keys_contract) => {
                CreatorKeysContractClient::new(&env, &keys_contract)
                    .transfer_keys(&key_id, &from, &from, &amount);
                KEY_RECEIVED_ACK
            }
        }
    }
}

fn setup() -> (
    Env,
    CreatorKeysContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = test_env_with_auths();
    let keys_contract = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &keys_contract);
    set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
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
    client.buy_key(&creator, &sender, &100i128, &None);
    // `Address::generate` yields contract addresses; use a Stellar account
    // (G...) strkey so the account-recipient path skips the callback.
    let recipient = Address::from_str(
        &env,
        "GA37NRXDHG22SCVEEOKUAKJMP6EPAAXNVM3TBEFGBAXEPYEUCWLPWNMA",
    );
    (env, client, creator, sender, recipient, keys_contract)
}

fn register_receiver(env: &Env, mode: ReceiverMode) -> (Address, MockReceiverClient<'static>) {
    let receiver = env.register(MockReceiver, ());
    let client = MockReceiverClient::new(env, &receiver);
    client.configure(&mode);
    (receiver, client)
}

fn assert_balances(
    client: &CreatorKeysContractClient,
    creator: &Address,
    sender: &Address,
    recipient: &Address,
    sender_balance: u32,
    recipient_balance: u32,
) {
    assert_eq!(client.get_key_balance(creator, sender), sender_balance);
    assert_eq!(
        client.get_key_balance(creator, recipient),
        recipient_balance
    );
}

#[test]
fn contract_receiver_acknowledgement_accepts_transfer_and_emits_data_hash() {
    let (env, client, creator, sender, _, _) = setup();
    let (receiver, receiver_client) = register_receiver(&env, ReceiverMode::Accept);
    let data = Bytes::from_slice(&env, &[1, 2, 3]);

    client.transfer_and_call(&creator, &sender, &receiver, &1, &data);
    // Capture events before any other top-level call resets the event log.
    let (_, topics, payload) = env.events().all().last().unwrap();

    assert_balances(&client, &creator, &sender, &receiver, 0, 1);
    assert_eq!(
        receiver_client.received(),
        Some(ReceivedCallback {
            from: sender,
            key_id: creator.clone(),
            amount: 1,
            data: data.clone(),
        })
    );
    let event: events::TransferCalledEvent = payload.try_into_val(&env).unwrap();
    assert_eq!(event.creator_id, creator);
    assert_eq!(event.recipient, receiver);
    assert_eq!(event.data_hash, env.crypto().sha256(&data).to_bytes());
    let topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    assert_eq!(topic, Symbol::new(&env, events::TRANSFER_CALLED_EVENT_NAME));
}

#[test]
fn wrong_acknowledgement_reverts_transfer() {
    let (env, client, creator, sender, _, _) = setup();
    let (receiver, _) = register_receiver(&env, ReceiverMode::Reject);

    assert_eq!(
        client.try_transfer_and_call(&creator, &sender, &receiver, &1, &Bytes::new(&env)),
        Err(Ok(ContractError::TransferNotAccepted))
    );
    assert_balances(&client, &creator, &sender, &receiver, 1, 0);
}

#[test]
fn receiver_panic_reverts_transfer() {
    let (env, client, creator, sender, _, _) = setup();
    let (receiver, _) = register_receiver(&env, ReceiverMode::Panic);

    assert!(client
        .try_transfer_and_call(&creator, &sender, &receiver, &1, &Bytes::new(&env))
        .is_err());
    assert_balances(&client, &creator, &sender, &receiver, 1, 0);
}

#[test]
fn account_recipient_skips_callback_and_receives_keys() {
    let (env, client, creator, sender, recipient, _) = setup();

    client.transfer_and_call(
        &creator,
        &sender,
        &recipient,
        &1,
        &Bytes::from_slice(&env, &[7]),
    );

    assert_balances(&client, &creator, &sender, &recipient, 0, 1);
}

#[test]
fn reentrant_transfer_from_receiver_reverts_outer_transfer() {
    let (env, client, creator, sender, _, keys_contract) = setup();
    let (receiver, _) = register_receiver(&env, ReceiverMode::Reenter(keys_contract));

    assert!(client
        .try_transfer_and_call(&creator, &sender, &receiver, &1, &Bytes::new(&env))
        .is_err());
    assert_balances(&client, &creator, &sender, &receiver, 1, 0);
}

#[test]
fn oversized_callback_data_is_rejected_before_transfer() {
    let (env, client, creator, sender, _, _) = setup();
    let (receiver, _) = register_receiver(&env, ReceiverMode::Accept);
    let data = Bytes::from_slice(&env, &[0; 257]);

    assert_eq!(
        client.try_transfer_and_call(&creator, &sender, &receiver, &1, &data),
        Err(Ok(ContractError::BatchTooLarge))
    );
    assert_balances(&client, &creator, &sender, &receiver, 1, 0);
}
