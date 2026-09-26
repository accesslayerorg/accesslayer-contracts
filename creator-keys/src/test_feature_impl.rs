//! Integration tests for the four new features:
//!
//! 1. `holder_count` tracking with `HolderCountChanged` event
//! 2. `update_metadata()` `MetadataUpdated` event and `update_config()` admin function
//! 3. Snapshot mechanism — governance-only `take_snapshot_governance`, pruning, retention
//! 4. `batch_buy_v2()` — per-key slippage, `FeeCollected` events, cap

#![cfg(test)]

use crate::{
    ConfigUpdateParams, ContractError, CreatorKeysContract, CreatorKeysContractClient, KeyMetadata,
    RegisterCreatorParams,
};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, String};

// ---------------------------------------------------------------------------
// Shared test helpers
// ---------------------------------------------------------------------------

fn setup() -> (Env, CreatorKeysContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &100i128);
    client.set_curve_slope(&admin, &0i128); // flat curve for predictable prices
    client.set_fee_config(&admin, &9000u32, &1000u32);
    // Disable circuit-breaker so flat-curve tests don't trip it
    client.set_circuit_breaker_threshold(&admin, &10000u32);

    (env, client, admin)
}

fn register(env: &Env, client: &CreatorKeysContractClient, creator: &Address) {
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "testcreator"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

fn init_metadata(env: &Env, client: &CreatorKeysContractClient, creator: &Address) {
    client.initialise_key(
        creator,
        &KeyMetadata {
            name: String::from_str(env, "MyKey"),
            bio: String::from_str(env, "A test key"),
            avatar_uri: String::from_str(env, "https://img.example.com/key.png"),
        },
    );
}

// ---------------------------------------------------------------------------
// 1. holder_count tracking
// ---------------------------------------------------------------------------

#[test]
fn holder_count_is_zero_before_any_buys() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    register(&env, &client, &creator);

    assert_eq!(client.get_holder_count(&creator), 0);
    assert_eq!(client.get_creator_holder_count(&creator), 0);
}

#[test]
fn holder_count_increments_on_first_buy() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator);

    assert_eq!(client.get_holder_count(&creator), 0);
    client.buy_key(&creator, &buyer, &500i128, &None);
    assert_eq!(client.get_holder_count(&creator), 1);
}

#[test]
fn holder_count_does_not_double_count_repeat_buys() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator);

    client.buy_key(&creator, &buyer, &500i128, &None);
    client.buy_key(&creator, &buyer, &500i128, &None);
    // Still 1 unique holder, not 2.
    assert_eq!(client.get_holder_count(&creator), 1);
}

#[test]
fn holder_count_decrements_on_full_exit() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator);

    client.buy_key(&creator, &buyer, &500i128, &None);
    assert_eq!(client.get_holder_count(&creator), 1);

    // Advance ledger so flash-loan guard doesn't fire.
    env.ledger().with_mut(|l| l.sequence_number += 1);

    client.sell_key(&creator, &buyer, &None);
    assert_eq!(client.get_holder_count(&creator), 0);
}

#[test]
fn holder_count_partial_sell_does_not_decrement() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator);

    // Buy 2 keys via buy_keys
    client.buy_keys(&creator, &buyer, &2u32, &500i128, &None);
    assert_eq!(client.get_holder_count(&creator), 1);

    // Advance ledger.
    env.ledger().with_mut(|l| l.sequence_number += 1);

    // Sell only 1 — buyer still holds 1 key.
    client.sell_key(&creator, &buyer, &None);
    assert_eq!(client.get_holder_count(&creator), 1);
}

#[test]
fn holder_count_tracks_two_wallets_through_buys_and_full_exits() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);
    register(&env, &client, &creator);

    client.buy_key(&creator, &buyer_a, &500i128, &None);
    assert_eq!(client.get_holder_count(&creator), 1);

    client.buy_key(&creator, &buyer_b, &500i128, &None);
    assert_eq!(client.get_holder_count(&creator), 2);

    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.sell_key(&creator, &buyer_a, &None);
    assert_eq!(client.get_holder_count(&creator), 1);

    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.sell_key(&creator, &buyer_b, &None);
    assert_eq!(client.get_holder_count(&creator), 0);
}

// ---------------------------------------------------------------------------
// 2a. update_metadata — MetadataUpdated event and field updates
// ---------------------------------------------------------------------------

#[test]
fn update_metadata_emits_event_on_change() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    register(&env, &client, &creator);
    init_metadata(&env, &client, &creator);

    // Should succeed and (implicitly) emit event.
    client.update_metadata(
        &creator,
        &Some(String::from_str(&env, "NewName")),
        &None,
        &None,
    );

    let stored = client.get_key_metadata(&creator).unwrap();
    assert_eq!(stored.name, String::from_str(&env, "NewName"));
    assert_eq!(stored.bio, String::from_str(&env, "A test key")); // unchanged
}

#[test]
fn update_metadata_none_fields_leave_data_intact() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    register(&env, &client, &creator);
    init_metadata(&env, &client, &creator);

    client.update_metadata(&creator, &None, &None, &None);

    let stored = client.get_key_metadata(&creator).unwrap();
    assert_eq!(stored.name, String::from_str(&env, "MyKey"));
}

#[test]
fn update_metadata_non_creator_rejected() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let attacker = Address::generate(&env);
    register(&env, &client, &creator);
    init_metadata(&env, &client, &creator);

    // Attacker has no metadata, so returns NotRegistered.
    let result = client.try_update_metadata(
        &attacker,
        &Some(String::from_str(&env, "Hacked")),
        &None,
        &None,
    );
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

// ---------------------------------------------------------------------------
// 2b. update_config — admin-only, validation, ConfigUpdated event
// ---------------------------------------------------------------------------

#[test]
fn update_config_succeeds_with_valid_params() {
    let (_env, client, admin) = setup();

    client.update_config(
        &admin,
        &ConfigUpdateParams {
            creator_bps: 8000,
            protocol_bps: 2000,
            curve_slope: 50,
        },
    );

    // New fee config should be readable.
    let fee_view = client.get_protocol_fee_view();
    assert_eq!(fee_view.creator_bps, 8000);
    assert_eq!(fee_view.protocol_bps, 2000);

    // Curve slope should be updated.
    assert_eq!(client.get_curve_slope(), 50);
}

#[test]
fn update_config_rejects_non_admin() {
    let (env, client, _admin) = setup();
    let attacker = Address::generate(&env);

    let result = client.try_update_config(
        &attacker,
        &ConfigUpdateParams {
            creator_bps: 8000,
            protocol_bps: 2000,
            curve_slope: 0,
        },
    );
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn update_config_rejects_invalid_fee_bps_exceeding_10000() {
    let (_env, client, admin) = setup();

    let result = client.try_update_config(
        &admin,
        &ConfigUpdateParams {
            creator_bps: 9000,
            protocol_bps: 2000, // sum = 11000 > 10000
            curve_slope: 0,
        },
    );
    assert_eq!(result, Err(Ok(ContractError::InvalidFeeConfig)));
}

#[test]
fn update_config_rejects_negative_curve_slope() {
    let (_env, client, admin) = setup();

    let result = client.try_update_config(
        &admin,
        &ConfigUpdateParams {
            creator_bps: 9000,
            protocol_bps: 1000,
            curve_slope: -1,
        },
    );
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn update_config_zero_slope_is_valid() {
    let (_env, client, admin) = setup();

    client.update_config(
        &admin,
        &ConfigUpdateParams {
            creator_bps: 9000,
            protocol_bps: 1000,
            curve_slope: 0,
        },
    );
    assert_eq!(client.get_curve_slope(), 0);
}

// ---------------------------------------------------------------------------
// 3. Snapshot mechanism — governance-only access and pruning
// ---------------------------------------------------------------------------

#[test]
fn take_snapshot_governance_requires_governance_caller() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    register(&env, &client, &creator);

    // No governance address set — any caller is rejected with Unauthorized.
    let attacker = Address::generate(&env);
    let result = client.try_take_snapshot_governance(
        &attacker,
        &creator,
        &1u32,
        &soroban_sdk::Vec::new(&env),
    );
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn take_snapshot_governance_succeeds_for_governance_caller() {
    let (env, client, admin) = setup();
    let creator = Address::generate(&env);
    let governance = Address::generate(&env);
    register(&env, &client, &creator);

    client.set_governance_address(&admin, &governance);

    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &500i128, &None);

    let mut holders = soroban_sdk::Vec::new(&env);
    holders.push_back(holder.clone());

    client.take_snapshot_governance(&governance, &creator, &1u32, &holders);

    // Verify snapshot was stored.
    let meta = client.get_snapshot_meta(&creator, &1u32).unwrap();
    assert_eq!(meta.total_holders, 1);

    let balance = client.get_snapshot_balance(&creator, &1u32, &holder);
    assert_eq!(balance, 1);
}

#[test]
fn take_snapshot_governance_rejects_non_governance_caller_after_governance_set() {
    let (env, client, admin) = setup();
    let creator = Address::generate(&env);
    let governance = Address::generate(&env);
    register(&env, &client, &creator);

    client.set_governance_address(&admin, &governance);

    // Admin is NOT governance — should be rejected.
    let result =
        client.try_take_snapshot_governance(&admin, &creator, &1u32, &soroban_sdk::Vec::new(&env));
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn take_snapshot_governance_rejects_duplicate_snapshot_id() {
    let (env, client, admin) = setup();
    let creator = Address::generate(&env);
    let governance = Address::generate(&env);
    register(&env, &client, &creator);
    client.set_governance_address(&admin, &governance);

    client.take_snapshot_governance(&governance, &creator, &1u32, &soroban_sdk::Vec::new(&env));

    let result = client.try_take_snapshot_governance(
        &governance,
        &creator,
        &1u32,
        &soroban_sdk::Vec::new(&env),
    );
    assert_eq!(result, Err(Ok(ContractError::SnapshotAlreadyExists)));
}

#[test]
fn snapshot_pruned_after_retention_window() {
    let (env, client, admin) = setup();
    let creator = Address::generate(&env);
    let governance = Address::generate(&env);
    register(&env, &client, &creator);
    client.set_governance_address(&admin, &governance);

    // Set a short retention window of 5 ledgers.
    client.set_snapshot_retention(&admin, &5u32);

    // Take snapshot 1 at ledger 1.
    client.take_snapshot_governance(&governance, &creator, &1u32, &soroban_sdk::Vec::new(&env));

    // Advance by 10 ledgers (past the 5-ledger retention window).
    env.ledger().with_mut(|l| l.sequence_number += 10);

    // Take snapshot 2; this should trigger pruning of snapshot 1.
    client.take_snapshot_governance(&governance, &creator, &2u32, &soroban_sdk::Vec::new(&env));

    // Snapshot 1 should have been pruned (meta returns None).
    let pruned_meta = client.get_snapshot_meta(&creator, &1u32);
    assert_eq!(
        pruned_meta, None,
        "snapshot 1 should be pruned after retention window"
    );

    // Snapshot 2 should still exist.
    assert!(client.get_snapshot_meta(&creator, &2u32).is_some());
}

#[test]
fn snapshot_not_pruned_within_retention_window() {
    let (env, client, admin) = setup();
    let creator = Address::generate(&env);
    let governance = Address::generate(&env);
    register(&env, &client, &creator);
    client.set_governance_address(&admin, &governance);

    // Set retention window of 100 ledgers.
    client.set_snapshot_retention(&admin, &100u32);

    client.take_snapshot_governance(&governance, &creator, &1u32, &soroban_sdk::Vec::new(&env));

    // Advance only 5 ledgers — still well inside the window.
    env.ledger().with_mut(|l| l.sequence_number += 5);

    client.take_snapshot_governance(&governance, &creator, &2u32, &soroban_sdk::Vec::new(&env));

    // Snapshot 1 should NOT be pruned.
    assert!(
        client.get_snapshot_meta(&creator, &1u32).is_some(),
        "snapshot 1 should not be pruned within retention window"
    );
}

#[test]
fn get_governance_address_returns_set_value() {
    let (env, client, admin) = setup();
    let governance = Address::generate(&env);

    assert_eq!(client.get_governance_address(), None);
    client.set_governance_address(&admin, &governance);
    assert_eq!(client.get_governance_address(), Some(governance));
}

#[test]
fn set_snapshot_retention_round_trips() {
    let (_env, client, admin) = setup();

    assert_eq!(client.get_snapshot_retention(), 0);
    client.set_snapshot_retention(&admin, &200u32);
    assert_eq!(client.get_snapshot_retention(), 200);
}

// ---------------------------------------------------------------------------
// 4. batch_buy_v2 — slippage, FeeCollected events, cap
// ---------------------------------------------------------------------------

#[test]
fn batch_buy_v2_succeeds_single_order_no_slippage() {
    let (env, client, admin) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator);

    // Set protocol fee for treasury.
    let treasury = Address::generate(&env);
    client.set_protocol_fee(&admin, &Some(100u32), &treasury);

    let mut orders = soroban_sdk::Vec::new(&env);
    orders.push_back((creator.clone(), 1u32, None::<i128>));

    let results = client.batch_buy_v2(&buyer, &orders);
    assert_eq!(results.len(), 1);
    assert_eq!(results.get(0).unwrap().quantity, 1);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
}

#[test]
fn batch_buy_v2_multiple_orders_succeed() {
    let (env, client, _admin) = setup();
    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator_a);
    register(&env, &client, &creator_b);

    let mut orders = soroban_sdk::Vec::new(&env);
    orders.push_back((creator_a.clone(), 1u32, None::<i128>));
    orders.push_back((creator_b.clone(), 1u32, None::<i128>));

    let results = client.batch_buy_v2(&buyer, &orders);
    assert_eq!(results.len(), 2);
    assert_eq!(client.get_key_balance(&creator_a, &buyer), 1);
    assert_eq!(client.get_key_balance(&creator_b, &buyer), 1);
}

#[test]
fn batch_buy_v2_slippage_check_passes_within_bound() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator);

    // Price is 100 (flat curve). max_price = 200 — should pass.
    let mut orders = soroban_sdk::Vec::new(&env);
    orders.push_back((creator.clone(), 1u32, Some(200i128)));

    let results = client.batch_buy_v2(&buyer, &orders);
    assert_eq!(results.len(), 1);
}

#[test]
fn batch_buy_v2_slippage_check_fails_when_price_exceeds_max() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator);

    // Price is 100. max_price = 50 — should fail.
    let mut orders = soroban_sdk::Vec::new(&env);
    orders.push_back((creator.clone(), 1u32, Some(50i128)));

    let result = client.try_batch_buy_v2(&buyer, &orders);
    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));
}

#[test]
fn batch_buy_v2_empty_orders_rejected() {
    let (env, client, _admin) = setup();
    let buyer = Address::generate(&env);

    let result = client.try_batch_buy_v2(&buyer, &soroban_sdk::Vec::new(&env));
    assert_eq!(result, Err(Ok(ContractError::BatchClaimExceedsLimit)));
}

#[test]
fn batch_buy_v2_exceeds_cap_rejected() {
    let (env, client, _admin) = setup();
    let buyer = Address::generate(&env);

    // 6 orders (MAX_BATCH_BUY_SIZE = 5).
    let mut orders = soroban_sdk::Vec::new(&env);
    for _ in 0..6 {
        let creator = Address::generate(&env);
        register(&env, &client, &creator);
        orders.push_back((creator, 1u32, None::<i128>));
    }

    let result = client.try_batch_buy_v2(&buyer, &orders);
    assert_eq!(result, Err(Ok(ContractError::BatchClaimExceedsLimit)));
}

#[test]
fn batch_buy_v2_zero_quantity_rejected() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator);

    let mut orders = soroban_sdk::Vec::new(&env);
    orders.push_back((creator.clone(), 0u32, None::<i128>));

    let result = client.try_batch_buy_v2(&buyer, &orders);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn batch_buy_v2_increments_holder_count() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator);

    assert_eq!(client.get_holder_count(&creator), 0);

    let mut orders = soroban_sdk::Vec::new(&env);
    orders.push_back((creator.clone(), 2u32, None::<i128>));
    client.batch_buy_v2(&buyer, &orders);

    // Buyer entered for the first time — holder count = 1.
    assert_eq!(client.get_holder_count(&creator), 1);
}

#[test]
fn batch_buy_v2_multi_order_slippage_independent_per_order() {
    let (env, client, _admin) = setup();
    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);
    let buyer = Address::generate(&env);
    register(&env, &client, &creator_a);
    register(&env, &client, &creator_b);

    // creator_a has generous max_price (passes), creator_b has tight max_price (fails).
    let mut orders = soroban_sdk::Vec::new(&env);
    orders.push_back((creator_a.clone(), 1u32, Some(500i128))); // passes (price=100)
    orders.push_back((creator_b.clone(), 1u32, Some(50i128))); // fails (price=100 > 50)

    let result = client.try_batch_buy_v2(&buyer, &orders);
    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));

    // Because Soroban tx atomicity rolls back, neither order should have landed.
    assert_eq!(client.get_key_balance(&creator_a, &buyer), 0);
    assert_eq!(client.get_key_balance(&creator_b, &buyer), 0);
}
