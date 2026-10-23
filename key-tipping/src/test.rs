#![cfg(test)]

use crate::{KeyTippingContract, KeyTippingContractClient, TipError};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, Env, String,
};

fn setup(
    env: &Env,
) -> (
    KeyTippingContractClient<'_>,
    Address,
    Address,
    Address,
    Address,
) {
    env.mock_all_auths();
    let contract_id = env.register(KeyTippingContract, ());
    let client = KeyTippingContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let creator = Address::generate(env);
    let token_id = env
        .register_stellar_asset_contract_v2(Address::generate(env))
        .address();
    client.initialize(&admin, &token_id, &treasury);
    (client, token_id, treasury, creator, admin)
}

#[test]
fn tip_transfers_fee_records_history_and_updates_total() {
    let env = Env::default();
    let (client, token_id, treasury, creator, _) = setup(&env);
    let key = Address::generate(&env);
    let tipper = Address::generate(&env);
    client.set_creator(&key, &creator);
    StellarAssetClient::new(&env, &token_id).mint(&tipper, &1_000);

    let receipt = client.send_tip(
        &key,
        &tipper,
        &1_000,
        &Some(String::from_str(&env, "thanks")),
    );
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(receipt.amount, 980);
    assert_eq!(receipt.protocol_fee, 20);
    assert_eq!(token.balance(&creator), 980);
    assert_eq!(token.balance(&treasury), 20);
    assert_eq!(client.get_total_tips(&key), 980);
    let history = client.get_tip_history(&key, &0, &10);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap(), receipt);
}

#[test]
fn unknown_keys_nonpositive_amounts_and_invalid_pages_are_rejected() {
    let env = Env::default();
    let (client, _, _, _, _) = setup(&env);
    let key = Address::generate(&env);
    let tipper = Address::generate(&env);
    assert_eq!(
        client.try_send_tip(&key, &tipper, &10, &None),
        Err(Ok(TipError::UnknownKey))
    );
    assert_eq!(
        client.try_send_tip(&key, &tipper, &0, &None),
        Err(Ok(TipError::InvalidAmount))
    );
    assert_eq!(
        client.try_get_tip_history(&key, &0, &0),
        Err(Ok(TipError::InvalidPage))
    );
    assert_eq!(
        client.try_get_tip_history(&key, &0, &101),
        Err(Ok(TipError::InvalidPage))
    );
}

#[test]
fn history_pages_are_newest_first_and_offset_from_newest() {
    let env = Env::default();
    let (client, token_id, _, creator, _) = setup(&env);
    let key = Address::generate(&env);
    let tipper = Address::generate(&env);
    client.set_creator(&key, &creator);
    StellarAssetClient::new(&env, &token_id).mint(&tipper, &3_000);
    client.send_tip(&key, &tipper, &100, &None);
    client.send_tip(&key, &tipper, &200, &None);
    client.send_tip(&key, &tipper, &300, &None);

    let newest = client.get_tip_history(&key, &0, &2);
    assert_eq!(newest.len(), 2);
    assert_eq!(newest.get(0).unwrap().amount, 294);
    assert_eq!(newest.get(1).unwrap().amount, 196);
    let older = client.get_tip_history(&key, &2, &2);
    assert_eq!(older.len(), 1);
    assert_eq!(older.get(0).unwrap().amount, 98);
}
