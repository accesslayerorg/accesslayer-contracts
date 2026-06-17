use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_get_protocol_admin_returns_none_initially() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let result = client.get_protocol_admin();
    assert_eq!(result, None);
}

#[test]
fn test_get_protocol_admin_returns_set_address() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &new_admin);

    let result = client.get_protocol_admin();
    assert_eq!(result, Some(new_admin));
}

#[test]
fn test_get_protocol_admin_persists_across_reads() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &new_admin);

    let first_read = client.get_protocol_admin();
    let second_read = client.get_protocol_admin();
    assert_eq!(first_read, second_read);
    assert_eq!(first_read, Some(new_admin));
}

#[test]
fn test_get_protocol_admin_is_isolated_from_treasury() {
    // Ensures the admin read path does not bleed into treasury storage
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    // Only set treasury, not protocol admin
    client.set_treasury_address(&admin, &treasury);

    let result = client.get_protocol_admin();
    assert_eq!(result, None); // admin storage must be untouched
}

#[test]
fn test_get_protocol_admin_overwrite_returns_latest() {
    // Admin address can be updated and the read returns the most recent value
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let first_admin = Address::generate(&env);
    let second_admin = Address::generate(&env);

    client.set_protocol_admin(&admin, &first_admin);
    assert_eq!(client.get_protocol_admin(), Some(first_admin));

    client.set_protocol_admin(&admin, &second_admin);
    assert_eq!(client.get_protocol_admin(), Some(second_admin));
}

#[test]
fn test_protocol_admin_unchanged_after_fee_config_update() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let protocol_admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &protocol_admin);

    let before = client.get_protocol_admin();
    client.set_fee_config(&admin, &8000u32, &2000u32);
    let after = client.get_protocol_admin();

    assert_eq!(before, Some(protocol_admin));
    assert_eq!(
        after, before,
        "fee config update must not overwrite protocol admin storage"
    );
}
