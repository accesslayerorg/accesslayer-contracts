//! Integration tests for restoring a creator's state after a RESTORING
//! lifecycle transition (issue #709).
//!
//! The RESTORING lifecycle covers the period when a creator's archived state is
//! being copied back to active storage. During this window the contract keeps
//! serving read calls while trading writes are gated until restoration
//! completes. These tests confirm:
//!
//! - archiving a creator transitions its manifest through
//!   `Archived -> Restoring` under protocol-admin control
//! - reads (`get_buy_quote`, `get_key_balance`, profile views) succeed and
//!   return current values during RESTORING
//! - buys panic with `StateRestoring` during the RESTORING window
//! - a buy succeeds immediately after restoration completes
//! - the restored state matches the pre-archive snapshot exactly
//!
//! # Scope note
//!
//! Issue #709 references a RESTORING lifecycle that did not exist in this
//! contract yet, so this change also introduces the minimal feature surface it
//! tests against: `archive_creator`, `begin_creator_restore`,
//! `complete_creator_restore`, `get_creator_lifecycle`, the
//! [`creator_keys::CreatorLifecycleState`] enum, and the appended error codes
//! 40–42 (`CreatorArchived`, `StateRestoring`, `InvalidLifecycleTransition`).

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{events, ContractError, CreatorLifecycleState};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// Counts events whose name topic equals `name`.
fn count_named_events(env: &Env, name: Symbol) -> usize {
    env.events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            let topic: Symbol = topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .expect("event topic tuple must contain an event name")
                .into_val(env);
            topic == name
        })
        .count()
}

struct LifecycleFixture<'a> {
    client: creator_keys::CreatorKeysContractClient<'a>,
    admin: Address,
    creator: Address,
    holder: Address,
}

fn setup(env: &Env) -> LifecycleFixture<'_> {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");

    // Give the creator live state: one holder with keys so reads have values
    // to serve during the RESTORING window.
    let holder = Address::generate(env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    LifecycleFixture {
        client,
        admin,
        creator,
        holder,
    }
}

/// Drives a fixture into the RESTORING state via archive -> begin restore.
fn transition_to_restoring(env: &Env, fx: &LifecycleFixture<'_>) {
    fx.client.archive_creator(&fx.admin, &fx.creator);
    assert_eq!(
        fx.client.get_creator_lifecycle(&fx.creator),
        CreatorLifecycleState::Archived
    );
    fx.client.begin_creator_restore(&fx.admin, &fx.creator);
    assert_eq!(
        fx.client.get_creator_lifecycle(&fx.creator),
        CreatorLifecycleState::Restoring
    );
    let _ = env; // kept for signature symmetry with future ledger-based checks
}

// ---------------------------------------------------------------------------
// Read calls succeed during RESTORING and return current values
// ---------------------------------------------------------------------------

#[test]
fn test_reads_succeed_and_return_current_values_during_restoring() {
    let env = test_env_with_auths();
    let fx = setup(&env);

    let quote_before = fx.client.get_buy_quote(&fx.creator);
    let balance_before = fx.client.get_key_balance(&fx.creator, &fx.holder);
    let details_before = fx.client.get_creator_details(&fx.creator);

    transition_to_restoring(&env, &fx);

    // Price and balance reads keep serving current values mid-restoration.
    let quote_during = fx.client.get_buy_quote(&fx.creator);
    assert_eq!(quote_during.price, quote_before.price);
    assert_eq!(quote_during.total_amount, quote_before.total_amount);
    assert_eq!(
        fx.client.get_key_balance(&fx.creator, &fx.holder),
        balance_before
    );
    assert_eq!(
        fx.client.get_creator_details(&fx.creator).supply,
        details_before.supply
    );
}

// ---------------------------------------------------------------------------
// Buy panics with StateRestoring during RESTORING; succeeds right after
// ---------------------------------------------------------------------------

#[test]
fn test_buy_panics_with_state_restoring_then_succeeds_after_completion() {
    let env = test_env_with_auths();
    let fx = setup(&env);

    let supply_before = fx.client.get_total_key_supply(&fx.creator);
    transition_to_restoring(&env, &fx);

    // Buy is gated during the RESTORING window.
    let buyer = Address::generate(&env);
    let result = fx
        .client
        .try_buy_key(&fx.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::StateRestoring)),
        "buy must panic with StateRestoring during the RESTORING window"
    );
    assert_eq!(fx.client.get_total_key_supply(&fx.creator), supply_before);
    assert_eq!(fx.client.get_key_balance(&fx.creator, &buyer), 0);

    // Complete the restoration: the very next buy succeeds.
    fx.client.complete_creator_restore(&fx.admin, &fx.creator);
    assert_eq!(
        fx.client.get_creator_lifecycle(&fx.creator),
        CreatorLifecycleState::Active
    );

    let supply = fx.client.buy_key(&fx.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(supply, supply_before + 1);
    assert_eq!(fx.client.get_key_balance(&fx.creator, &buyer), 1);
}

// ---------------------------------------------------------------------------
// Restored state matches pre-archive values
// ---------------------------------------------------------------------------

#[test]
fn test_restored_state_matches_pre_archive_values() {
    let env = test_env_with_auths();
    let fx = setup(&env);

    let snapshot_pre = capture_snapshot(&fx.client, &fx.creator, &fx.holder);
    let fee_balance_pre = fx.client.get_creator_fee_balance(&fx.creator);
    let handle_pre = fx.client.get_creator_details(&fx.creator).handle;

    transition_to_restoring(&env, &fx);
    fx.client.complete_creator_restore(&fx.admin, &fx.creator);

    let snapshot_post = capture_snapshot(&fx.client, &fx.creator, &fx.holder);
    snapshot_pre.assert_unchanged(&snapshot_post);
    assert_eq!(
        fx.client.get_creator_fee_balance(&fx.creator),
        fee_balance_pre
    );
    assert_eq!(
        fx.client.get_creator_details(&fx.creator).handle,
        handle_pre
    );
}

// ---------------------------------------------------------------------------
// Sell is gated too; archived state gates trades with CreatorArchived
// ---------------------------------------------------------------------------

#[test]
fn test_sell_is_gated_during_restoring() {
    let env = test_env_with_auths();
    let fx = setup(&env);

    transition_to_restoring(&env, &fx);

    let result = fx.client.try_sell_key(&fx.creator, &fx.holder, &None);
    assert_eq!(result, Err(Ok(ContractError::StateRestoring)));
    assert_eq!(fx.client.get_total_key_supply(&fx.creator), 1);
}

#[test]
fn test_trades_are_gated_while_archived() {
    let env = test_env_with_auths();
    let fx = setup(&env);

    fx.client.archive_creator(&fx.admin, &fx.creator);

    let buyer = Address::generate(&env);
    let buy_result = fx
        .client
        .try_buy_key(&fx.creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(buy_result, Err(Ok(ContractError::CreatorArchived)));

    let sell_result = fx.client.try_sell_key(&fx.creator, &fx.holder, &None);
    assert_eq!(sell_result, Err(Ok(ContractError::CreatorArchived)));
    assert_eq!(fx.client.get_total_key_supply(&fx.creator), 1);
}

// ---------------------------------------------------------------------------
// Admin authorization and strict transition validation
// ---------------------------------------------------------------------------

#[test]
fn test_non_admin_cannot_drive_lifecycle_transitions() {
    let env = test_env_with_auths();
    let fx = setup(&env);

    let attacker = Address::generate(&env);

    let archive_result = fx.client.try_archive_creator(&attacker, &fx.creator);
    assert_eq!(archive_result, Err(Ok(ContractError::Unauthorized)));

    let begin_result = fx.client.try_begin_creator_restore(&attacker, &fx.creator);
    assert_eq!(begin_result, Err(Ok(ContractError::Unauthorized)));

    let complete_result = fx
        .client
        .try_complete_creator_restore(&attacker, &fx.creator);
    assert_eq!(complete_result, Err(Ok(ContractError::Unauthorized)));

    assert_eq!(
        fx.client.get_creator_lifecycle(&fx.creator),
        CreatorLifecycleState::Active
    );
}

#[test]
fn test_invalid_lifecycle_transitions_are_rejected() {
    let env = test_env_with_auths();
    let fx = setup(&env);

    // begin_restore on an Active creator is invalid.
    let begin_active = fx.client.try_begin_creator_restore(&fx.admin, &fx.creator);
    assert_eq!(
        begin_active,
        Err(Ok(ContractError::InvalidLifecycleTransition))
    );

    // complete_restore on an Active creator is invalid.
    let complete_active = fx
        .client
        .try_complete_creator_restore(&fx.admin, &fx.creator);
    assert_eq!(
        complete_active,
        Err(Ok(ContractError::InvalidLifecycleTransition))
    );

    // Valid path: Archived -> Restoring, then completion is final.
    fx.client.archive_creator(&fx.admin, &fx.creator);
    let complete_archived = fx
        .client
        .try_complete_creator_restore(&fx.admin, &fx.creator);
    assert_eq!(
        complete_archived,
        Err(Ok(ContractError::InvalidLifecycleTransition))
    );

    fx.client.begin_creator_restore(&fx.admin, &fx.creator);
    let begin_twice = fx.client.try_begin_creator_restore(&fx.admin, &fx.creator);
    assert_eq!(
        begin_twice,
        Err(Ok(ContractError::InvalidLifecycleTransition))
    );
}

#[test]
fn test_archive_unregistered_creator_fails() {
    let env = test_env_with_auths();
    let fx = setup(&env);

    let stranger = Address::generate(&env);
    let result = fx.client.try_archive_creator(&fx.admin, &stranger);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
    assert_eq!(
        fx.client.get_creator_lifecycle(&stranger),
        CreatorLifecycleState::Active
    );
}

// ---------------------------------------------------------------------------
// Lifecycle events are emitted for each transition
// ---------------------------------------------------------------------------

#[test]
fn test_lifecycle_events_are_emitted_for_each_transition() {
    let env = test_env_with_auths();
    let fx = setup(&env);

    let archived_events_before = count_named_events(&env, events::CREATOR_ARCHIVED_EVENT_NAME);
    fx.client.archive_creator(&fx.admin, &fx.creator);
    assert_eq!(
        count_named_events(&env, events::CREATOR_ARCHIVED_EVENT_NAME),
        archived_events_before + 1
    );

    let begun_events_before = count_named_events(&env, events::CREATOR_RESTORE_BEGUN_EVENT_NAME);
    fx.client.begin_creator_restore(&fx.admin, &fx.creator);
    assert_eq!(
        count_named_events(&env, events::CREATOR_RESTORE_BEGUN_EVENT_NAME),
        begun_events_before + 1
    );

    let done_events_before = count_named_events(&env, events::CREATOR_RESTORE_DONE_EVENT_NAME);
    fx.client.complete_creator_restore(&fx.admin, &fx.creator);
    assert_eq!(
        count_named_events(&env, events::CREATOR_RESTORE_DONE_EVENT_NAME),
        done_events_before + 1
    );
}
