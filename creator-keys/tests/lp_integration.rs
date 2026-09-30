use creator_keys::{CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_set_lp_contract_address_succeeds_for_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let lp_address = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_lp_contract_address(&admin, &lp_address);

    // Verify the address was stored (we can't directly read it without a getter,
    // but the call succeeding indicates it was stored)
}

#[test]
fn test_set_lp_contract_address_reverts_for_non_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let lp_address = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);

    let result = client.try_set_lp_contract_address(&non_admin, &lp_address);
    assert!(
        result.is_err(),
        "non-admin should not be able to set LP address"
    );
}

#[test]
fn test_set_lp_allocation_bps_succeeds_for_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_lp_allocation_bps(&admin, &1000u32); // 10%

    // Call succeeding indicates the value was stored
}

#[test]
fn test_set_lp_allocation_bps_reverts_for_non_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);

    let result = client.try_set_lp_allocation_bps(&non_admin, &1000u32);
    assert!(
        result.is_err(),
        "non-admin should not be able to set LP allocation"
    );
}

#[test]
fn test_set_lp_allocation_bps_reverts_over_max() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);

    // 10001 bps exceeds 10000 (100%)
    let result = client.try_set_lp_allocation_bps(&admin, &10001u32);
    assert!(result.is_err(), "allocation over 100% should revert");
}

#[test]
fn test_set_lp_allocation_bps_accepts_zero() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_lp_allocation_bps(&admin, &0u32); // 0% should be valid

    // Call succeeding indicates 0% is accepted
}

#[test]
fn test_set_lp_allocation_bps_accepts_max() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_lp_allocation_bps(&admin, &10000u32); // 100% should be valid

    // Call succeeding indicates 100% is accepted
}

#[test]
fn test_lp_hook_skipped_when_address_not_set() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &1000i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "test"),
        },
        &None, // locked_allocation
        &None, // max_supply
        &None, // max_keys_per_wallet
        &None, // curve_preset
        &None, // co_creator
        &None, // whitelist
    );

    // Set allocation but no LP address - hook should be skipped
    client.set_lp_allocation_bps(&admin, &1000u32);

    // Buy should succeed without LP event
    let result = client.try_buy_key(&creator, &buyer, &1000i128, &None);
    assert!(result.is_ok(), "buy should succeed when LP address not set");
}
