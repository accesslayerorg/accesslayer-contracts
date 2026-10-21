//! Tests for get_retention_policy view and retention policy configuration (#724).

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{
    retention, ContractError, CreatorKeysContract, CreatorKeysContractClient, PartitionStrategy,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_get_retention_policy_unconfigured_returns_defaults_no_panic() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    // Call get_retention_policy before any admin configuration
    let policy = client.get_retention_policy();

    // Assert no panic and matches default canonical configuration
    assert_eq!(policy.retention_days, retention::DEFAULT_RETENTION_DAYS);
    assert_eq!(
        policy.partition_strategy,
        retention::DEFAULT_PARTITION_STRATEGY
    );
    assert_eq!(
        policy.compression_enabled,
        retention::DEFAULT_COMPRESSION_ENABLED
    );
    assert_eq!(policy.batch_size, retention::DEFAULT_BATCH_SIZE);
}

#[test]
fn test_get_retention_policy_returns_configured_values() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    let configured_days = 90u32;
    let configured_strategy = PartitionStrategy::Monthly;
    let configured_compression = false;
    let configured_batch = 500u32;

    client.set_retention_policy(
        &admin,
        &configured_days,
        &configured_strategy,
        &configured_compression,
        &configured_batch,
    );

    let policy = client.get_retention_policy();

    // Acceptance Criteria validations
    assert_eq!(policy.retention_days, configured_days);
    assert_eq!(policy.partition_strategy, configured_strategy);
    assert_eq!(policy.compression_enabled, configured_compression);
    assert_eq!(policy.batch_size, configured_batch);
}

#[test]
fn test_get_retention_policy_all_partition_strategies() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    let strategies = [
        PartitionStrategy::Daily,
        PartitionStrategy::Weekly,
        PartitionStrategy::Monthly,
        PartitionStrategy::Ledger,
    ];

    for strategy in strategies {
        client.set_retention_policy(&admin, &60u32, &strategy, &true, &250u32);
        let policy = client.get_retention_policy();
        assert_eq!(policy.partition_strategy, strategy);
    }
}

#[test]
fn test_get_retention_policy_compression_enabled_variants() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    // Test with compression enabled
    client.set_retention_policy(&admin, &45u32, &PartitionStrategy::Weekly, &true, &150u32);
    let policy_true = client.get_retention_policy();
    assert!(policy_true.compression_enabled);

    // Test with compression disabled
    client.set_retention_policy(&admin, &45u32, &PartitionStrategy::Weekly, &false, &150u32);
    let policy_false = client.get_retention_policy();
    assert!(!policy_false.compression_enabled);
}

#[test]
fn test_get_retention_policy_is_read_only_and_idempotent() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    client.set_retention_policy(&admin, &180u32, &PartitionStrategy::Ledger, &true, &1000u32);

    let first_read = client.get_retention_policy();
    let second_read = client.get_retention_policy();

    assert_eq!(first_read, second_read);
    assert_eq!(first_read.retention_days, 180);
    assert_eq!(first_read.partition_strategy, PartitionStrategy::Ledger);
    assert!(first_read.compression_enabled);
    assert_eq!(first_read.batch_size, 1000);
}

#[test]
fn test_get_retention_policy_updates_after_reconfiguration() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    // Initial configuration
    client.set_retention_policy(&admin, &30u32, &PartitionStrategy::Daily, &true, &100u32);
    let v1 = client.get_retention_policy();
    assert_eq!(v1.retention_days, 30);
    assert_eq!(v1.partition_strategy, PartitionStrategy::Daily);

    // Reconfiguration
    client.set_retention_policy(
        &admin,
        &365u32,
        &PartitionStrategy::Monthly,
        &false,
        &5000u32,
    );
    let v2 = client.get_retention_policy();
    assert_eq!(v2.retention_days, 365);
    assert_eq!(v2.partition_strategy, PartitionStrategy::Monthly);
    assert!(!v2.compression_enabled);
    assert_eq!(v2.batch_size, 5000);
    assert_ne!(v1, v2);
}

#[test]
fn test_set_retention_policy_unauthorized_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    let result = client.try_set_retention_policy(
        &non_admin,
        &90u32,
        &PartitionStrategy::Daily,
        &true,
        &100u32,
    );
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_set_retention_policy_zero_batch_size_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    let result =
        client.try_set_retention_policy(&admin, &90u32, &PartitionStrategy::Daily, &true, &0u32);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}
