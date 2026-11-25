//! Integration coverage for the append-only per-creator holder registry.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::constants::storage;
use creator_keys::{CREATOR_TTL_LEDGERS, TTL_MIN_EXTENSION_LEDGERS};
use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _, Ledger as _},
    Address, Env, Vec,
};

const KEY_PRICE: i128 = 100;

fn buy_one(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
) {
    let quote = client.get_buy_quote(creator);
    client.buy_key(creator, buyer, &quote.total_amount, &None);
}

fn registry_ttl(env: &Env, contract_id: &Address, creator: &Address) -> u32 {
    let key = storage::holder_registry(creator);
    env.as_contract(contract_id, || env.storage().persistent().get_ttl(&key))
}

#[test]
fn records_buyers_once_and_keeps_zero_balance_holders() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, 9_000, 1_000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    assert!(client.get_holder_registry(&creator).is_empty());
    buy_one(&client, &creator, &buyer);
    buy_one(&client, &creator, &buyer);
    buy_one(&client, &creator, &buyer);

    let registry = client.get_holder_registry(&creator);
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.get(0), Some(buyer.clone()));

    let recipient = Address::generate(&env);
    client.transfer_keys(&creator, &buyer, &recipient, &1);
    client.transfer_keys(&creator, &buyer, &recipient, &1);

    let batch_recipient = Address::generate(&env);
    let mut transfers = Vec::new(&env);
    transfers.push_back((batch_recipient.clone(), 1));
    client.batch_transfer_keys(&creator, &recipient, &transfers);

    let spender = Address::generate(&env);
    let delegated_recipient = Address::generate(&env);
    client.approve(&buyer, &spender, &creator, &1);
    client.transfer_from(&spender, &buyer, &delegated_recipient, &creator, &1);

    let registry = client.get_holder_registry(&creator);
    assert_eq!(registry.len(), 4);
    assert_eq!(registry.get(0), Some(buyer.clone()));
    assert_eq!(registry.get(1), Some(recipient));
    assert_eq!(registry.get(2), Some(batch_recipient));
    assert_eq!(registry.get(3), Some(delegated_recipient));
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);
}

#[test]
fn registry_reads_and_writes_extend_ttl() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    env.deployer().extend_ttl(
        contract_id.clone(),
        CREATOR_TTL_LEDGERS,
        CREATOR_TTL_LEDGERS,
    );
    set_pricing_and_fees(&env, &client, KEY_PRICE, 9_000, 1_000);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    buy_one(&client, &creator, &buyer);

    assert!(
        registry_ttl(&env, &contract_id, &creator) >= CREATOR_TTL_LEDGERS - 1,
        "the registry write should extend the entry to the full TTL window"
    );

    let mut ledger = env.ledger().get();
    ledger.sequence_number += CREATOR_TTL_LEDGERS - 100;
    env.ledger().set(ledger);
    let before_read = registry_ttl(&env, &contract_id, &creator);
    assert!(before_read < TTL_MIN_EXTENSION_LEDGERS);

    assert_eq!(client.get_holder_registry(&creator).get(0), Some(buyer));
    let after_read = registry_ttl(&env, &contract_id, &creator);
    assert!(
        after_read >= TTL_MIN_EXTENSION_LEDGERS,
        "reading the registry should refresh its TTL: before={before_read}, after={after_read}"
    );
}
