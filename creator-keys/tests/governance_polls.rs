use creator_keys::{events::PollError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    vec, Address, Env, String,
};

fn poll_options(env: &Env) -> soroban_sdk::Vec<String> {
    vec![
        env,
        String::from_str(env, "Yes"),
        String::from_str(env, "No"),
    ]
}

#[test]
fn creator_can_create_poll_and_view_empty_result() {
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

    let question = String::from_str(&env, "Should we launch premium content?");
    let options = poll_options(&env);
    let poll_id = client.create_poll(&creator, &question, &options, &10);

    assert_eq!(poll_id, 1);
    let result = client.get_poll_result(&creator, &poll_id);
    assert_eq!(result.question, question);
    assert_eq!(result.options.len(), 2);
    assert_eq!(result.vote_counts.get(0).unwrap(), 0);
    assert_eq!(result.vote_counts.get(1).unwrap(), 0);
    assert_eq!(result.total_weight, 0);
    assert!(!result.expired);
}

#[test]
fn holder_vote_uses_liquid_key_balance_as_weight() {
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
    client.buy_key(&creator, &holder, &100, &None);

    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Pick one"),
        &poll_options(&env),
        &10,
    );
    client.cast_vote(&creator, &holder, &poll_id, &0);

    let result = client.get_poll_result(&creator, &poll_id);
    assert_eq!(result.vote_counts.get(0).unwrap(), 2);
    assert_eq!(result.vote_counts.get(1).unwrap(), 0);
    assert_eq!(result.total_weight, 2);
}

#[test]
fn changing_vote_before_expiry_updates_tally() {
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
    client.buy_key(&creator, &holder, &100, &None);
    client.buy_key(&creator, &holder, &100, &None);

    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Pick one"),
        &poll_options(&env),
        &10,
    );

    client.cast_vote(&creator, &holder, &poll_id, &0);
    client.cast_vote(&creator, &holder, &poll_id, &1);

    let result = client.get_poll_result(&creator, &poll_id);
    assert_eq!(result.vote_counts.get(0).unwrap(), 0);
    assert_eq!(result.vote_counts.get(1).unwrap(), 3);
    assert_eq!(result.total_weight, 3);
}

#[test]
fn vote_after_expiry_reverts_with_poll_expired() {
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
        &String::from_str(&env, "Pick one"),
        &poll_options(&env),
        &1,
    );

    env.ledger().with_mut(|ledger| {
        ledger.sequence_number += 1;
    });

    let result = client.try_cast_vote(&creator, &holder, &poll_id, &0);
    assert_eq!(result, Err(Ok(PollError::PollExpired)));

    let result = client.get_poll_result(&creator, &poll_id);
    assert!(result.expired);
}

#[test]
fn non_holder_vote_reverts_with_not_a_holder() {
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

    let non_holder = Address::generate(&env);
    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Pick one"),
        &poll_options(&env),
        &10,
    );

    let result = client.try_cast_vote(&creator, &non_holder, &poll_id, &0);
    assert_eq!(result, Err(Ok(PollError::NotAHolder)));
}

#[test]
fn zero_liquid_balance_after_sell_reverts_with_not_a_holder() {
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
        &String::from_str(&env, "Pick one"),
        &poll_options(&env),
        &10,
    );

    let before = client.get_poll_result(&creator, &poll_id);

    client.sell_key(&creator, &holder, &None);

    let result = client.try_cast_vote(&creator, &holder, &poll_id, &0);
    assert_eq!(result, Err(Ok(PollError::NotAHolder)));

    let after = client.get_poll_result(&creator, &poll_id);
    assert_eq!(after.total_weight, before.total_weight);
    assert_eq!(
        after.vote_counts.get(0).unwrap(),
        before.vote_counts.get(0).unwrap()
    );
    assert_eq!(
        after.vote_counts.get(1).unwrap(),
        before.vote_counts.get(1).unwrap()
    );
}

#[test]
fn zero_liquid_balance_after_transfer_reverts_with_not_a_holder() {
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
    let recipient = Address::generate(&env);
    client.buy_key(&creator, &holder, &100, &None);
    client.buy_key(&creator, &holder, &100, &None);

    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Pick one"),
        &poll_options(&env),
        &10,
    );

    let before = client.get_poll_result(&creator, &poll_id);

    client.transfer_keys(&creator, &holder, &recipient, &2);

    let result = client.try_cast_vote(&creator, &holder, &poll_id, &0);
    assert_eq!(result, Err(Ok(PollError::NotAHolder)));

    let after = client.get_poll_result(&creator, &poll_id);
    assert_eq!(after.total_weight, before.total_weight);
    assert_eq!(
        after.vote_counts.get(0).unwrap(),
        before.vote_counts.get(0).unwrap()
    );
    assert_eq!(
        after.vote_counts.get(1).unwrap(),
        before.vote_counts.get(1).unwrap()
    );
}

#[test]
fn staked_keys_zero_liquid_balance_rejected() {
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
    let recipient = Address::generate(&env);
    client.buy_key(&creator, &holder, &100, &None);
    client.buy_key(&creator, &holder, &100, &None);
    client.buy_key(&creator, &holder, &100, &None);

    client.transfer_keys(&creator, &holder, &recipient, &3);

    assert_eq!(client.get_key_balance(&creator, &holder), 0);

    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Pick one"),
        &poll_options(&env),
        &10,
    );

    let result = client.try_cast_vote(&creator, &holder, &poll_id, &0);
    assert_eq!(result, Err(Ok(PollError::NotAHolder)));

    let result = client.get_poll_result(&creator, &poll_id);
    assert_eq!(result.total_weight, 0);
    assert_eq!(result.vote_counts.get(0).unwrap(), 0);
    assert_eq!(result.vote_counts.get(1).unwrap(), 0);
}

#[test]
fn invalid_vote_option_reverts() {
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
        &String::from_str(&env, "Pick one"),
        &poll_options(&env),
        &10,
    );

    let result = client.try_cast_vote(&creator, &holder, &poll_id, &2);
    assert_eq!(result, Err(Ok(PollError::InvalidOption)));
}
