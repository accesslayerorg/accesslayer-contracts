mod contract_test_env;

use contract_test_env::{
    compute_expected_holder_dividend, distribute_test_dividend, register_creator_keys,
    register_test_creator, set_pricing_and_fees, test_env_with_auths, DEFAULT_CREATOR_BPS,
    DEFAULT_PROTOCOL_BPS,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, Vec};

fn numbered_handle(i: u32) -> String {
    format!("creator_{i:03}")
}

#[test]
fn test_batch_claim_dividend_happy_path() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");
    let holder = Address::generate(&env);

    client.buy_key(&creator_a, &holder, &100, &None);
    client.buy_key(&creator_b, &holder, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator_a, &distributor, 10_000);
    distribute_test_dividend(&client, &creator_b, &distributor, 20_000);

    let creators = Vec::from_array(&env, [creator_a.clone(), creator_b.clone()]);
    let results = client.batch_claim_dividend(&creators, &holder);

    assert_eq!(results.len(), 2);

    let expected_a = compute_expected_holder_dividend(10_000, 1, 1, DEFAULT_PROTOCOL_BPS);
    let expected_b = compute_expected_holder_dividend(20_000, 1, 1, DEFAULT_PROTOCOL_BPS);

    assert_eq!(results.get(0).unwrap().creator, creator_a);
    assert_eq!(results.get(0).unwrap().amount_claimed, expected_a);
    assert_eq!(results.get(1).unwrap().creator, creator_b);
    assert_eq!(results.get(1).unwrap().amount_claimed, expected_b);
}

#[test]
fn test_batch_claim_zero_claimable_returns_zero_no_revert() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");
    let holder = Address::generate(&env);

    client.buy_key(&creator_a, &holder, &100, &None);
    client.buy_key(&creator_b, &holder, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator_a, &distributor, 10_000);
    // No distribution for creator_b

    let creators = Vec::from_array(&env, [creator_a.clone(), creator_b.clone()]);
    let results = client.batch_claim_dividend(&creators, &holder);

    assert_eq!(results.len(), 2);
    assert!(results.get(0).unwrap().amount_claimed > 0);
    assert_eq!(results.get(1).unwrap().amount_claimed, 0);
}

#[test]
fn test_batch_claim_zeroes_claimable_after_claim() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");
    let holder = Address::generate(&env);

    client.buy_key(&creator_a, &holder, &100, &None);
    client.buy_key(&creator_b, &holder, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator_a, &distributor, 10_000);
    distribute_test_dividend(&client, &creator_b, &distributor, 20_000);

    let creators = Vec::from_array(&env, [creator_a.clone(), creator_b.clone()]);
    client.batch_claim_dividend(&creators, &holder);

    assert_eq!(client.get_claimable_dividend(&creator_a, &holder), 0);
    assert_eq!(client.get_claimable_dividend(&creator_b, &holder), 0);
}

#[test]
fn test_batch_claim_exceeds_limit_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let holder = Address::generate(&env);

    let mut creators = Vec::new(&env);
    for i in 0..21u32 {
        let creator = register_test_creator(&env, &client, &numbered_handle(i));
        creators.push_back(creator);
    }

    let result = client.try_batch_claim_dividend(&creators, &holder);
    assert_eq!(result, Err(Ok(ContractError::BatchClaimExceedsLimit)));
}

#[test]
fn test_batch_claim_while_paused_fails() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator, &distributor, 10_000);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.pause(&admin);

    let creators = Vec::from_array(&env, [creator.clone()]);
    let result = client.try_batch_claim_dividend(&creators, &holder);
    assert_eq!(result, Err(Ok(ContractError::ProtocolPaused)));
}

#[test]
fn test_batch_claim_empty_list_returns_empty() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let holder = Address::generate(&env);
    let creators: Vec<Address> = Vec::new(&env);
    let results = client.batch_claim_dividend(&creators, &holder);

    assert_eq!(results.len(), 0);
}

#[test]
fn test_batch_claim_individual_failure_does_not_revert_others() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let creator_a = register_test_creator(&env, &client, "alice");
    let creator_b = register_test_creator(&env, &client, "bob");
    let creator_c = register_test_creator(&env, &client, "charlie");
    let holder = Address::generate(&env);

    client.buy_key(&creator_a, &holder, &100, &None);
    client.buy_key(&creator_b, &holder, &100, &None);
    client.buy_key(&creator_c, &holder, &100, &None);

    let distributor = Address::generate(&env);
    distribute_test_dividend(&client, &creator_a, &distributor, 10_000);
    // No distribution for creator_b — zero claimable
    distribute_test_dividend(&client, &creator_c, &distributor, 30_000);

    let creators = Vec::from_array(
        &env,
        [creator_a.clone(), creator_b.clone(), creator_c.clone()],
    );
    let results = client.batch_claim_dividend(&creators, &holder);

    assert_eq!(results.len(), 3);

    let expected_a = compute_expected_holder_dividend(10_000, 1, 1, DEFAULT_PROTOCOL_BPS);
    let expected_c = compute_expected_holder_dividend(30_000, 1, 1, DEFAULT_PROTOCOL_BPS);

    assert_eq!(results.get(0).unwrap().amount_claimed, expected_a);
    assert_eq!(results.get(1).unwrap().amount_claimed, 0);
    assert_eq!(results.get(2).unwrap().amount_claimed, expected_c);
}

#[test]
fn test_batch_claim_at_limit_of_20_succeeds() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        100,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );

    let holder = Address::generate(&env);

    let mut creators = Vec::new(&env);
    for i in 0..20u32 {
        let creator = register_test_creator(&env, &client, &numbered_handle(i));
        client.buy_key(&creator, &holder, &100, &None);
        creators.push_back(creator);
    }

    let results = client.batch_claim_dividend(&creators, &holder);
    assert_eq!(results.len(), 20);
}
