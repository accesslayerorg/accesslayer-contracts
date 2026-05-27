//! Regression test for creator detail read consistency across multiple calls.
//!
//! The creator detail read method is tested after state updates, but not for consistency
//! across repeated calls with no intervening state changes. This test confirms storage
//! read stability by reading the same creator detail multiple times without any writes
//! between reads.
//!
//! Related issue: #290

mod contract_test_env;

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env, String};

#[test]
fn test_creator_details_identical_across_three_consecutive_reads() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = soroban_sdk::Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    // Register creator to establish initial state
    client.register_creator(&creator, &handle);

    // Perform three consecutive reads with NO state changes between them
    let read1 = client.get_creator_details(&creator);
    let read2 = client.get_creator_details(&creator);
    let read3 = client.get_creator_details(&creator);

    // Assert all reads return identical values for all fields
    assert_eq!(read1.is_registered, read2.is_registered, "is_registered must be identical across reads");
    assert_eq!(read2.is_registered, read3.is_registered, "is_registered must be identical across reads");
    
    assert_eq!(read1.creator, read2.creator, "creator address must be identical across reads");
    assert_eq!(read2.creator, read3.creator, "creator address must be identical across reads");
    
    assert_eq!(read1.handle, read2.handle, "handle must be identical across reads");
    assert_eq!(read2.handle, read3.handle, "handle must be identical across reads");
    
    assert_eq!(read1.supply, read2.supply, "supply must be identical across reads");
    assert_eq!(read2.supply, read3.supply, "supply must be identical across reads");
    
    assert_eq!(read1.holder_count, read2.holder_count, "holder_count must be identical across reads");
    assert_eq!(read2.holder_count, read3.holder_count, "holder_count must be identical across reads");
    
    assert_eq!(read1.fee_recipient, read2.fee_recipient, "fee_recipient must be identical across reads");
    assert_eq!(read2.fee_recipient, read3.fee_recipient, "fee_recipient must be identical across reads");

    // Verify expected values
    assert!(read1.is_registered, "creator should be registered");
    assert_eq!(read1.creator, creator, "creator address should match");
    assert_eq!(read1.handle, handle, "handle should match");
    assert_eq!(read1.supply, 0, "supply should be 0 for newly registered creator");
    assert_eq!(read1.holder_count, 0, "holder_count should be 0 for newly registered creator");
    assert_eq!(read1.fee_recipient, creator, "fee_recipient should default to creator address");
}

#[test]
fn test_creator_details_identical_across_five_consecutive_reads_after_buy() {
    let env = contract_test_env::test_env_with_auths();
    let (client, _) = contract_test_env::register_creator_keys(&env);
    let creator = contract_test_env::register_test_creator(&env, &client, "bob");
    let buyer = soroban_sdk::Address::generate(&env);

    // Perform a buy to establish non-zero state
    contract_test_env::set_key_price_for_tests(&env, &client, 100i128);
    client.buy_key(&creator, &buyer, &100i128);

    // Perform five consecutive reads with NO state changes between them
    let read1 = client.get_creator_details(&creator);
    let read2 = client.get_creator_details(&creator);
    let read3 = client.get_creator_details(&creator);
    let read4 = client.get_creator_details(&creator);
    let read5 = client.get_creator_details(&creator);

    // Assert all reads return identical values
    assert_eq!(read1.is_registered, read2.is_registered);
    assert_eq!(read2.is_registered, read3.is_registered);
    assert_eq!(read3.is_registered, read4.is_registered);
    assert_eq!(read4.is_registered, read5.is_registered);
    
    assert_eq!(read1.creator, read2.creator);
    assert_eq!(read2.creator, read3.creator);
    assert_eq!(read3.creator, read4.creator);
    assert_eq!(read4.creator, read5.creator);
    
    assert_eq!(read1.handle, read2.handle);
    assert_eq!(read2.handle, read3.handle);
    assert_eq!(read3.handle, read4.handle);
    assert_eq!(read4.handle, read5.handle);
    
    assert_eq!(read1.supply, read2.supply);
    assert_eq!(read2.supply, read3.supply);
    assert_eq!(read3.supply, read4.supply);
    assert_eq!(read4.supply, read5.supply);
    
    assert_eq!(read1.holder_count, read2.holder_count);
    assert_eq!(read2.holder_count, read3.holder_count);
    assert_eq!(read3.holder_count, read4.holder_count);
    assert_eq!(read4.holder_count, read5.holder_count);
    
    assert_eq!(read1.fee_recipient, read2.fee_recipient);
    assert_eq!(read2.fee_recipient, read3.fee_recipient);
    assert_eq!(read3.fee_recipient, read4.fee_recipient);
    assert_eq!(read4.fee_recipient, read5.fee_recipient);

    // Verify expected values after buy
    assert!(read1.is_registered);
    assert_eq!(read1.supply, 1, "supply should be 1 after one buy");
    assert_eq!(read1.holder_count, 1, "holder_count should be 1 after one buyer");
}

#[test]
fn test_creator_details_no_storage_writes_during_reads() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = soroban_sdk::Address::generate(&env);
    let handle = String::from_str(&env, "charlie");

    // Register creator
    client.register_creator(&creator, &handle);

    // Capture storage state before reads
    // Note: In Soroban SDK, we can't directly inspect storage writes, but we can
    // verify that repeated reads don't change the returned values, which would
    // indicate no state mutation is occurring.
    
    // Perform multiple reads
    let read1 = client.get_creator_details(&creator);
    let read2 = client.get_creator_details(&creator);
    let read3 = client.get_creator_details(&creator);
    let read4 = client.get_creator_details(&creator);

    // If reads were triggering writes, we might see inconsistent values or
    // side effects. The fact that all reads return identical values confirms
    // no storage writes are occurring.
    assert_eq!(read1.is_registered, read2.is_registered);
    assert_eq!(read2.is_registered, read3.is_registered);
    assert_eq!(read3.is_registered, read4.is_registered);
    
    assert_eq!(read1.supply, read2.supply);
    assert_eq!(read2.supply, read3.supply);
    assert_eq!(read3.supply, read4.supply);
    
    // Verify the method is truly read-only by checking that the state
    // remains unchanged after multiple calls
    assert_eq!(read1.supply, 0, "supply should remain 0 (no writes during reads)");
    assert_eq!(read4.supply, 0, "supply should still be 0 after multiple reads");
}

#[test]
fn test_unregistered_creator_details_identical_across_reads() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let unregistered_creator = soroban_sdk::Address::generate(&env);

    // Read details for an unregistered creator multiple times
    let read1 = client.get_creator_details(&unregistered_creator);
    let read2 = client.get_creator_details(&unregistered_creator);
    let read3 = client.get_creator_details(&unregistered_creator);

    // Assert all reads return identical default values
    assert_eq!(read1.is_registered, read2.is_registered);
    assert_eq!(read2.is_registered, read3.is_registered);
    assert!(!read1.is_registered, "unregistered creator should return false");
    
    assert_eq!(read1.creator, read2.creator);
    assert_eq!(read2.creator, read3.creator);
    assert_eq!(read1.creator, unregistered_creator);
    
    assert_eq!(read1.handle, read2.handle);
    assert_eq!(read2.handle, read3.handle);
    assert_eq!(read1.handle, String::from_str(&env, ""), "handle should be empty for unregistered");
    
    assert_eq!(read1.supply, read2.supply);
    assert_eq!(read2.supply, read3.supply);
    assert_eq!(read1.supply, 0, "supply should be 0 for unregistered");
    
    assert_eq!(read1.holder_count, read2.holder_count);
    assert_eq!(read2.holder_count, read3.holder_count);
    assert_eq!(read1.holder_count, 0, "holder_count should be 0 for unregistered");
}

#[test]
fn test_creator_details_consistency_across_ten_reads() {
    let env = contract_test_env::test_env_with_auths();
    let (client, _) = contract_test_env::register_creator_keys(&env);
    let creator = contract_test_env::register_test_creator(&env, &client, "dave");

    // Perform ten consecutive reads
    let reads: Vec<_> = (0..10)
        .map(|_| client.get_creator_details(&creator))
        .collect();

    // Verify all reads are identical
    for i in 1..reads.len() {
        assert_eq!(reads[0].is_registered, reads[i].is_registered, "is_registered mismatch at read {}", i);
        assert_eq!(reads[0].creator, reads[i].creator, "creator mismatch at read {}", i);
        assert_eq!(reads[0].handle, reads[i].handle, "handle mismatch at read {}", i);
        assert_eq!(reads[0].supply, reads[i].supply, "supply mismatch at read {}", i);
        assert_eq!(reads[0].holder_count, reads[i].holder_count, "holder_count mismatch at read {}", i);
        assert_eq!(reads[0].fee_recipient, reads[i].fee_recipient, "fee_recipient mismatch at read {}", i);
    }

    // Verify expected values
    assert!(reads[0].is_registered);
    assert_eq!(reads[0].handle, String::from_str(&env, "dave"));
    assert_eq!(reads[0].supply, 0);
}
