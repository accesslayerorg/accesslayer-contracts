use creator_keys::{events::PollError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, vec, Address, Env, String};

fn poll_options(env: &Env) -> soroban_sdk::Vec<String> {
    vec![
        env,
        String::from_str(env, "Yes"),
        String::from_str(env, "No"),
    ]
}

#[test]
fn delegation_lifecycle() {
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

    let delegator = Address::generate(&env);
    let delegate = Address::generate(&env);

    assert_eq!(client.get_delegate(&creator, &delegator), None);

    client.delegate(&creator, &delegator, &delegate);
    assert_eq!(
        client.get_delegate(&creator, &delegator),
        Some(delegate.clone())
    );

    client.revoke_delegate(&creator, &delegator);
    assert_eq!(client.get_delegate(&creator, &delegator), None);
}

#[test]
fn delegated_voting_works() {
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

    let delegator1 = Address::generate(&env);
    let delegator2 = Address::generate(&env);
    let delegate = Address::generate(&env);

    client.buy_key(&creator, &delegator1, &100, &None);
    client.buy_key(&creator, &delegator1, &100, &None); // 2 keys
    client.buy_key(&creator, &delegator2, &100, &None); // 1 key
    client.buy_key(&creator, &delegate, &100, &None); // 1 key

    client.delegate(&creator, &delegator1, &delegate);
    client.delegate(&creator, &delegator2, &delegate);

    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Delegated vote poll"),
        &poll_options(&env),
        &10,
    );

    // delegate votes on behalf of delegators (delegator 1: 2, delegator 2: 1)
    client.cast_delegated_vote(
        &creator,
        &delegate,
        &vec![&env, delegator1.clone(), delegator2.clone()],
        &poll_id,
        &0,
    );
    // delegate votes on behalf of themselves
    client.cast_vote(&creator, &delegate, &poll_id, &0);

    let result = client.get_poll_result(&creator, &poll_id);
    // Total weight = 2 (d1) + 1 (d2) + 1 (delegate) = 4
    assert_eq!(result.vote_counts.get(0).unwrap(), 4);
    assert_eq!(result.total_weight, 4);
}

#[test]
fn delegator_cannot_double_vote() {
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

    let delegator = Address::generate(&env);
    let delegate = Address::generate(&env);

    client.buy_key(&creator, &delegator, &100, &None);
    client.delegate(&creator, &delegator, &delegate);

    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Should double voting be blocked?"),
        &poll_options(&env),
        &10,
    );

    // delegator attempting to vote directly should fail
    let res = client.try_cast_vote(&creator, &delegator, &poll_id, &0);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().unwrap(), PollError::Unauthorized);
}

#[test]
fn cast_delegated_vote_fails_if_not_delegated() {
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
    let random_delegate = Address::generate(&env);

    client.buy_key(&creator, &holder, &100, &None);

    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Poll"),
        &poll_options(&env),
        &10,
    );

    let res = client.try_cast_delegated_vote(
        &creator,
        &random_delegate,
        &vec![&env, holder.clone()],
        &poll_id,
        &0,
    );
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().unwrap(), PollError::Unauthorized);
}
