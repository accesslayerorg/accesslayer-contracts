#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, vec, Address, Env, String};

#[test]
fn test_treasury() {
    let env = Env::default();
    let contract_id = env.register(TreasuryContract, ());
    let client = TreasuryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let key_id = String::from_str(&env, "key123");

    // Collect fee
    client.collect_fee(&key_id, &1000);
    assert_eq!(client.get_treasury_balance(), 1000);

    client.collect_fee(&key_id, &500);
    assert_eq!(client.get_treasury_balance(), 1500);

    // Distribute
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    let recipients = vec![&env, recipient1.clone(), recipient2.clone()];
    let amounts = vec![&env, 700, 300];

    // Mock admin auth
    env.mock_all_auths();

    client.distribute(&recipients, &amounts);

    assert_eq!(client.get_treasury_balance(), 500);

    let history = client.get_distribution_history(&0).unwrap();
    assert_eq!(history.epoch, 0);
    assert_eq!(history.recipients, recipients);
    assert_eq!(history.amounts, amounts);
}

#[test]
#[should_panic(expected = "Insufficient balance")]
fn test_distribute_insufficient_balance() {
    let env = Env::default();
    let contract_id = env.register(TreasuryContract, ());
    let client = TreasuryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let key_id = String::from_str(&env, "key123");
    client.collect_fee(&key_id, &100);

    let recipient = Address::generate(&env);
    let recipients = vec![&env, recipient];
    let amounts = vec![&env, 200];

    env.mock_all_auths();

    client.distribute(&recipients, &amounts);
}
