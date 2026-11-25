//! Integration tests for the key ownership handover (#1098).
//!
//! Covers the acceptance criteria of the issue:
//! - `accept_transfer` is restricted to the proposed new owner
//! - `execute_transfer` reverts before the 24h timelock expires
//! - unaccepted proposals expire correctly after 48h
//! - `cancel_transfer` is restricted to the current creator
//! - `TransferProposed`, `TransferAccepted` and `TransferExecuted` are emitted
//!   with the correct key and wallet data

mod contract_test_env;

use contract_test_env::{
    assert_event, register_creator_keys, register_test_creator, set_ledger_sequence,
    set_test_timestamp, test_env_with_auths, DEFAULT_TEST_TIMESTAMP,
};
use creator_keys::{
    events,
    key_ownership_transfer::{ACCEPTANCE_WINDOW_SECS, EXECUTION_TIMELOCK_SECS},
    ContractError, CreatorKeysContractClient,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

const HOUR: u64 = 3_600;
/// Ledger sequence pinned by [`setup`] so event `ledger` fields are predictable.
const TEST_LEDGER: u32 = 4_242;

/// Registers the contract and a creator, pinning time and ledger sequence.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    set_test_timestamp(env, DEFAULT_TEST_TIMESTAMP);
    set_ledger_sequence(env, TEST_LEDGER);
    let (client, contract_id) = register_creator_keys(env);
    let creator = register_test_creator(env, &client, "alice");
    (client, contract_id, creator)
}

// ── happy path ────────────────────────────────────────────────────────────────

#[test]
fn test_full_handover_lifecycle() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);

    // Before any handover the registered creator manages the key.
    assert_eq!(client.get_key_owner(&creator), creator);

    let proposal_id = client.propose_transfer(&creator, &new_owner);
    assert_eq!(proposal_id, 0u32);

    let proposal = client.get_transfer_proposal(&proposal_id);
    assert_eq!(proposal.key_id, creator);
    assert_eq!(proposal.current_owner, creator);
    assert_eq!(proposal.new_owner, new_owner);
    assert_eq!(proposal.proposed_at, DEFAULT_TEST_TIMESTAMP);
    assert_eq!(
        proposal.expires_at,
        DEFAULT_TEST_TIMESTAMP + ACCEPTANCE_WINDOW_SECS
    );
    assert_eq!(proposal.accepted_at, None);
    assert_eq!(proposal.executable_at, None);

    // Unaccepted proposals cannot execute.
    assert_eq!(
        client.try_execute_transfer(&proposal_id),
        Err(Ok(ContractError::TransferNotAccepted))
    );
    assert_eq!(client.get_key_owner(&creator), creator);

    // The new owner accepts one hour later.
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + HOUR);
    let executable_at = client.accept_transfer(&proposal_id);
    assert_eq!(
        executable_at,
        DEFAULT_TEST_TIMESTAMP + HOUR + EXECUTION_TIMELOCK_SECS
    );
    let proposal = client.get_transfer_proposal(&proposal_id);
    assert_eq!(proposal.accepted_at, Some(DEFAULT_TEST_TIMESTAMP + HOUR));
    assert_eq!(proposal.executable_at, Some(executable_at));

    // Execution is blocked for the whole 24h timelock.
    set_test_timestamp(&env, executable_at - 1);
    assert_eq!(
        client.try_execute_transfer(&proposal_id),
        Err(Ok(ContractError::TimelockNotElapsed))
    );
    assert_eq!(client.get_key_owner(&creator), creator);

    // At exactly `executable_at` the handover can be finalised.
    set_test_timestamp(&env, executable_at);
    client.execute_transfer(&proposal_id);
    assert_eq!(client.get_key_owner(&creator), new_owner);

    // The proposal is consumed once executed.
    assert_eq!(
        client.try_get_transfer_proposal(&proposal_id),
        Err(Ok(ContractError::TransferProposalNotFound))
    );
    assert_eq!(
        client.try_execute_transfer(&proposal_id),
        Err(Ok(ContractError::TransferProposalNotFound))
    );
}

#[test]
fn test_get_key_owner_defaults_to_registered_creator() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let unregistered = Address::generate(&env);

    assert_eq!(client.get_key_owner(&creator), creator);
    assert_eq!(client.get_key_owner(&unregistered), unregistered);
}

#[test]
fn test_new_owner_can_hand_over_again_after_execution() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);
    let next_owner = Address::generate(&env);

    let first = client.propose_transfer(&creator, &new_owner);
    client.accept_transfer(&first);
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + EXECUTION_TIMELOCK_SECS);
    client.execute_transfer(&first);
    assert_eq!(client.get_key_owner(&creator), new_owner);

    // The wallet that took over now controls further handovers, not the
    // original creator.
    env.mock_auths(&[]);
    assert!(matches!(
        client.try_propose_transfer(&creator, &next_owner),
        Err(Err(_))
    ));

    env.mock_all_auths();
    let second = client.propose_transfer(&creator, &next_owner);
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, new_owner);

    client.accept_transfer(&second);
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + 2 * EXECUTION_TIMELOCK_SECS);
    client.execute_transfer(&second);
    assert_eq!(client.get_key_owner(&creator), next_owner);
}

// ── events ────────────────────────────────────────────────────────────────────

#[test]
fn test_transfer_events_emitted_with_key_and_wallet_data() {
    let env = test_env_with_auths();
    let (client, contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);

    let proposal_id = client.propose_transfer(&creator, &new_owner);
    assert_event(
        &env,
        &contract_id,
        events::TRANSFER_PROPOSED_EVENT_NAME,
        &events::TransferProposedEvent {
            proposal_id,
            key_id: creator.clone(),
            current_owner: creator.clone(),
            new_owner: new_owner.clone(),
            proposed_at: DEFAULT_TEST_TIMESTAMP,
            expires_at: DEFAULT_TEST_TIMESTAMP + ACCEPTANCE_WINDOW_SECS,
            ledger: TEST_LEDGER,
        },
    );

    let accepted_at = DEFAULT_TEST_TIMESTAMP + HOUR;
    set_test_timestamp(&env, accepted_at);
    client.accept_transfer(&proposal_id);
    assert_event(
        &env,
        &contract_id,
        events::TRANSFER_ACCEPTED_EVENT_NAME,
        &events::TransferAcceptedEvent {
            proposal_id,
            key_id: creator.clone(),
            current_owner: creator.clone(),
            new_owner: new_owner.clone(),
            accepted_at,
            executable_at: accepted_at + EXECUTION_TIMELOCK_SECS,
            ledger: TEST_LEDGER,
        },
    );

    set_test_timestamp(&env, accepted_at + EXECUTION_TIMELOCK_SECS);
    client.execute_transfer(&proposal_id);
    assert_event(
        &env,
        &contract_id,
        events::TRANSFER_EXECUTED_EVENT_NAME,
        &events::TransferExecutedEvent {
            proposal_id,
            key_id: creator.clone(),
            previous_owner: creator.clone(),
            new_owner: new_owner.clone(),
            executed_at: accepted_at + EXECUTION_TIMELOCK_SECS,
            ledger: TEST_LEDGER,
        },
    );
}

// ── propose ───────────────────────────────────────────────────────────────────

#[test]
fn test_propose_transfer_restricted_to_current_creator() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);

    // Without the current creator's signature the host rejects the call.
    env.mock_auths(&[]);
    assert!(matches!(
        client.try_propose_transfer(&creator, &new_owner),
        Err(Err(_))
    ));
    assert_eq!(
        client.try_get_transfer_proposal(&0u32),
        Err(Ok(ContractError::TransferProposalNotFound))
    );

    // Only the creator is asked to sign.
    env.mock_all_auths();
    client.propose_transfer(&creator, &new_owner);
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, creator);
}

#[test]
fn test_propose_transfer_validation() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);
    let unregistered = Address::generate(&env);

    // The key must be registered.
    assert_eq!(
        client.try_propose_transfer(&unregistered, &new_owner),
        Err(Ok(ContractError::NotRegistered))
    );

    // Handing the key over to the wallet that already manages it is a no-op.
    assert_eq!(
        client.try_propose_transfer(&creator, &creator),
        Err(Ok(ContractError::SelfTransfer))
    );

    // A key has at most one live proposal at a time.
    client.propose_transfer(&creator, &new_owner);
    assert_eq!(
        client.try_propose_transfer(&creator, &Address::generate(&env)),
        Err(Ok(ContractError::TransferAlreadyPending))
    );
}

// ── accept ────────────────────────────────────────────────────────────────────

#[test]
fn test_accept_transfer_restricted_to_proposed_new_owner() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);
    let proposal_id = client.propose_transfer(&creator, &new_owner);

    // A wallet other than `new_owner` cannot produce the required signature.
    env.mock_auths(&[]);
    assert!(matches!(
        client.try_accept_transfer(&proposal_id),
        Err(Err(_))
    ));

    // Nothing was recorded while the call was unauthorised.
    env.mock_all_auths();
    assert_eq!(client.get_transfer_proposal(&proposal_id).accepted_at, None);

    // The proposed new owner is the only required signer.
    client.accept_transfer(&proposal_id);
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, new_owner);
}

#[test]
fn test_accept_transfer_rejects_double_acceptance() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);

    let proposal_id = client.propose_transfer(&creator, &new_owner);
    client.accept_transfer(&proposal_id);
    assert_eq!(
        client.try_accept_transfer(&proposal_id),
        Err(Ok(ContractError::TransferAlreadyAccepted))
    );
}

#[test]
fn test_acceptance_window_still_open_one_second_before_deadline() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);

    let proposal_id = client.propose_transfer(&creator, &new_owner);
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + ACCEPTANCE_WINDOW_SECS - 1);
    client.accept_transfer(&proposal_id);
    assert_eq!(
        client.get_transfer_proposal(&proposal_id).accepted_at,
        Some(DEFAULT_TEST_TIMESTAMP + ACCEPTANCE_WINDOW_SECS - 1)
    );
}

// ── expiry ────────────────────────────────────────────────────────────────────

#[test]
fn test_unaccepted_proposal_expires_after_48h() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);

    let proposal_id = client.propose_transfer(&creator, &new_owner);
    let deadline = DEFAULT_TEST_TIMESTAMP + ACCEPTANCE_WINDOW_SECS;
    set_test_timestamp(&env, deadline);

    // The acceptance window has closed.
    assert_eq!(
        client.try_accept_transfer(&proposal_id),
        Err(Ok(ContractError::TransferWindowExpired))
    );
    // An expired proposal can never execute either.
    assert_eq!(
        client.try_execute_transfer(&proposal_id),
        Err(Ok(ContractError::TransferNotAccepted))
    );
    // Ownership never moved.
    assert_eq!(client.get_key_owner(&creator), creator);

    // The creator can still clear the expired proposal.
    client.cancel_transfer(&proposal_id);
    assert_eq!(
        client.try_get_transfer_proposal(&proposal_id),
        Err(Ok(ContractError::TransferProposalNotFound))
    );

    // And raise a fresh proposal afterwards.
    let second = client.propose_transfer(&creator, &new_owner);
    assert_eq!(second, proposal_id + 1);
}

#[test]
fn test_expired_proposal_is_replaced_by_a_new_proposal() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);
    let other = Address::generate(&env);

    let stale = client.propose_transfer(&creator, &new_owner);
    set_test_timestamp(&env, DEFAULT_TEST_TIMESTAMP + ACCEPTANCE_WINDOW_SECS);

    // The stale, never-accepted proposal no longer blocks a new one.
    let fresh = client.propose_transfer(&creator, &other);
    assert_eq!(fresh, stale + 1);
    assert_eq!(
        client.try_get_transfer_proposal(&stale),
        Err(Ok(ContractError::TransferProposalNotFound))
    );
    assert_eq!(client.get_transfer_proposal(&fresh).new_owner, other);
    assert_eq!(client.get_key_owner(&creator), creator);
}

// ── execute ───────────────────────────────────────────────────────────────────

#[test]
fn test_execute_transfer_reverts_before_timelock_expires() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);

    let proposal_id = client.propose_transfer(&creator, &new_owner);
    let accepted_at = DEFAULT_TEST_TIMESTAMP + HOUR;
    set_test_timestamp(&env, accepted_at);
    let executable_at = client.accept_transfer(&proposal_id);

    set_test_timestamp(&env, executable_at - 1);
    assert_eq!(
        client.try_execute_transfer(&proposal_id),
        Err(Ok(ContractError::TimelockNotElapsed))
    );
    assert_eq!(client.get_key_owner(&creator), creator);

    // Execution needs no signature: anyone may push an accepted proposal
    // through once its timelock has elapsed.
    env.mock_auths(&[]);
    set_test_timestamp(&env, executable_at);
    client.execute_transfer(&proposal_id);
    assert_eq!(client.get_key_owner(&creator), new_owner);
}

// ── cancel ────────────────────────────────────────────────────────────────────

#[test]
fn test_cancel_transfer_restricted_to_current_creator() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);

    let proposal_id = client.propose_transfer(&creator, &new_owner);

    // Without the current creator's signature the host rejects the call.
    env.mock_auths(&[]);
    assert!(matches!(
        client.try_cancel_transfer(&proposal_id),
        Err(Err(_))
    ));
    assert!(client
        .get_transfer_proposal(&proposal_id)
        .accepted_at
        .is_none());

    // The creator cancels, and is the only required signer.
    env.mock_all_auths();
    client.cancel_transfer(&proposal_id);
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, creator);

    assert_eq!(
        client.try_get_transfer_proposal(&proposal_id),
        Err(Ok(ContractError::TransferProposalNotFound))
    );
    assert_eq!(client.get_key_owner(&creator), creator);
}

#[test]
fn test_current_creator_can_cancel_after_acceptance() {
    let env = test_env_with_auths();
    let (client, _contract_id, creator) = setup(&env);
    let new_owner = Address::generate(&env);

    let proposal_id = client.propose_transfer(&creator, &new_owner);
    client.accept_transfer(&proposal_id);

    // Acceptance does not lock the handover in: the creator can still cancel
    // before execution, so the new owner cannot force the takeover.
    client.cancel_transfer(&proposal_id);
    assert_eq!(
        client.try_execute_transfer(&proposal_id),
        Err(Ok(ContractError::TransferProposalNotFound))
    );
    assert_eq!(client.get_key_owner(&creator), creator);
}
