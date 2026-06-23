use creator_keys::{config, constants, ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

fn setup() -> (Env, CreatorKeysContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    (env, client, contract_id)
}

fn advance_ledgers(env: &Env, ledgers: u32) {
    env.ledger().with_mut(|li| {
        li.sequence_number += ledgers;
    });
}

fn creator_ttl(env: &Env, contract_id: &Address, creator: &Address) -> u32 {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .get_ttl(&constants::storage::creator(creator))
    })
}

fn holder_ttl(env: &Env, contract_id: &Address, creator: &Address, holder: &Address) -> u32 {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .get_ttl(&constants::storage::key_balance(creator, holder))
    })
}

fn fee_config_ttl(env: &Env, contract_id: &Address) -> u32 {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .get_ttl(&constants::storage::FEE_CONFIG)
    })
}

#[test]
fn registration_sets_initial_creator_ttl() {
    let (env, client, contract_id) = setup();
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle);

    let ttl = client.get_creator_ttl_remaining(&creator);
    assert!(ttl > 0);
    assert_eq!(ttl, creator_ttl(&env, &contract_id, &creator));
}

#[test]
fn buy_extends_creator_holder_and_fee_config_ttls() {
    let (env, client, contract_id) = setup();
    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);
    client.set_fee_config(&admin, &9000, &1000);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    advance_ledgers(
        &env,
        config::CREATOR_TTL_LEDGERS - config::CREATOR_TTL_THRESHOLD + 1,
    );
    let creator_before = creator_ttl(&env, &contract_id, &creator);
    let fee_before = fee_config_ttl(&env, &contract_id);

    client.buy_key(&creator, &buyer, &100);

    let creator_after = creator_ttl(&env, &contract_id, &creator);
    let holder_after = holder_ttl(&env, &contract_id, &creator, &buyer);
    let fee_after = fee_config_ttl(&env, &contract_id);

    assert!(creator_after > creator_before);
    assert!(holder_after > 0);
    assert!(fee_after > fee_before);
}

#[test]
fn sell_extends_creator_holder_and_fee_config_ttls() {
    let (env, client, contract_id) = setup();
    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);
    client.set_fee_config(&admin, &9000, &1000);

    let creator = Address::generate(&env);
    let seller = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);
    client.buy_key(&creator, &seller, &100);

    advance_ledgers(
        &env,
        config::CREATOR_TTL_LEDGERS - config::CREATOR_TTL_THRESHOLD + 1,
    );
    let creator_before = creator_ttl(&env, &contract_id, &creator);
    let holder_before = holder_ttl(&env, &contract_id, &creator, &seller);
    let fee_before = fee_config_ttl(&env, &contract_id);

    client.sell_key(&creator, &seller);

    let creator_after = creator_ttl(&env, &contract_id, &creator);
    let holder_after = holder_ttl(&env, &contract_id, &creator, &seller);
    let fee_after = fee_config_ttl(&env, &contract_id);

    assert!(creator_after > creator_before);
    assert!(holder_after > holder_before);
    assert!(fee_after > fee_before);
}

#[test]
fn failed_buy_does_not_extend_creator_ttl() {
    let (env, client, contract_id) = setup();
    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let before = creator_ttl(&env, &contract_id, &creator);
    let result = client.try_buy_key(&creator, &buyer, &99);
    let after = creator_ttl(&env, &contract_id, &creator);

    assert_eq!(result, Err(Ok(ContractError::InsufficientPayment)));
    assert_eq!(after, before);
}

#[test]
fn failed_sell_does_not_extend_creator_ttl() {
    let (env, client, contract_id) = setup();
    let creator = Address::generate(&env);
    let seller = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let before = creator_ttl(&env, &contract_id, &creator);
    let result = client.try_sell_key(&creator, &seller);
    let after = creator_ttl(&env, &contract_id, &creator);

    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
    assert_eq!(after, before);
}
