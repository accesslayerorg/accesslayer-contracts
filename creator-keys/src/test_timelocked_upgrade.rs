//! Tests for the timelocked contract upgrade pattern.
//!
//! Covers the three gates an upgrade must clear â€” timelock delay, 2-of-N
//! multi-sig approval, and an unfrozen protocol â€” plus state preservation across
//! the logic swap and the `LogicUpgraded` event payload.
//!
//! Soroban has no `delegatecall`, so an EVM-style proxy cannot exist here. The
//! equivalent guarantee comes from the native WASM swap: storage is keyed to the
//! contract address rather than to the code, so state survives by construction.
//! These tests assert that property end-to-end by upgrading to a real build of
//! this contract and then continuing to use it.

use crate::{
    events, ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
    TimelockChangeType,
};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    vec, Address, Bytes, BytesN, Env, IntoVal, String, Symbol, TryFromVal, TryIntoVal, Vec,
};

/// A real build of this contract, used as the upgrade target so the swap
/// exercises a genuine executable replacement rather than a stub module.
///
/// Regenerate after changing the contract, otherwise the post-upgrade assertions
/// run against a stale build:
///   cargo build -p creator-keys --release --target wasm32v1-none
///   cp target/wasm32v1-none/release/creator_keys.wasm creator-keys/test_wasm/contract.wasm
const TEST_WASM: &[u8] = include_bytes!("../test_wasm/contract.wasm");

type Setup = (
    Env,
    CreatorKeysContractClient<'static>,
    Address,
    Vec<Address>,
    BytesN<32>,
);

fn setup() -> Setup {
    setup_inner(true)
}

/// Same as [`setup`] but leaves the multi-sig admin set unconfigured, which is
/// how a freshly deployed contract looks.
fn setup_without_multisig() -> Setup {
    setup_inner(false)
}

fn setup_inner(configure_multisig: bool) -> Setup {
    let env = Env::default();
    env.mock_all_auths();
    // Running the real 470KB build as the upgrade target costs far more than the
    // default test budget allows.
    env.cost_estimate().budget().reset_unlimited();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_treasury_address(&admin, &treasury);
    client.set_key_price(&admin, &1000i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_protocol_fee_recipient(&admin, &treasury);

    let members = vec![
        &env,
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    let member_list = members.clone();
    if configure_multisig {
        client.set_global_pause_admins(&admin, &members);
    }

    let target = env.deployer().upload_contract_wasm(TEST_WASM);

    (env, client, admin, member_list, target)
}

fn set_timestamp(env: &Env, timestamp: u64) {
    env.ledger().with_mut(|l| l.timestamp = timestamp);
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient, creator: &Address) {
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

/// Advances the clock to the moment `action_id` becomes executable.
fn elapse_timelock(env: &Env, client: &CreatorKeysContractClient, action_id: u32) {
    let action = client.get_action(&action_id).unwrap();
    set_timestamp(env, action.execution_not_before);
}

/// Appends every event from the **most recent** invocation whose first topic is
/// `expected_name` into `into`.
///
/// `env.events().all()` only surfaces the current top-level invocation, so
/// cross-invocation assertions must accumulate call by call rather than reading
/// the log once at the end.
fn collect_events<T>(env: &Env, expected_name: &Symbol, into: &mut Vec<T>)
where
    T: IntoVal<Env, soroban_sdk::Val> + TryFromVal<Env, soroban_sdk::Val>,
{
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().try_into_val(env).unwrap();
        if &name == expected_name {
            into.push_back(T::try_from_val(env, &data).unwrap());
        }
    }
}

/// Asserts the most recent invocation emitted exactly one `expected_name` event
/// and returns its payload.
fn sole_event<T>(env: &Env, expected_name: &Symbol) -> T
where
    T: IntoVal<Env, soroban_sdk::Val> + TryFromVal<Env, soroban_sdk::Val>,
{
    let mut found = Vec::new(env);
    collect_events(env, expected_name, &mut found);
    assert_eq!(found.len(), 1, "expected one {expected_name:?} event");
    found.get(0).unwrap()
}

// â”€â”€â”€ Proposal is inert until executed â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_upgrade_proposal_does_not_apply_immediately() {
    let (env, client, admin, _members, target) = setup();
    set_timestamp(&env, 1_000);

    let action_id = client.upgrade(&admin, &target);

    assert_eq!(action_id, 1);
    // Nothing has changed yet: the call only queues.
    assert_eq!(client.get_version(), 1);
    assert_eq!(client.get_logic_address(), None);
    assert_eq!(client.get_previous_wasm(), None);
    assert_eq!(client.get_upgrade_target(), Some(target.clone()));

    let action = client.get_action(&action_id).unwrap();
    assert_eq!(action.change_type, TimelockChangeType::Upgrade);
    assert_eq!(action.proposer, admin);
    assert_eq!(action.proposed_at, 1_000);
    assert_eq!(
        action.execution_not_before,
        1_000 + client.get_timelock_delay()
    );
    assert!(!action.executed);
    assert!(!action.cancelled);
}

#[test]
fn test_upgrade_never_applies_without_the_protocol_admin() {
    let (env, client, _admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let stranger = Address::generate(&env);

    assert_eq!(
        client.try_upgrade(&stranger, &target),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(client.get_upgrade_target(), None);
    assert_eq!(client.get_upgrade_approvals(&1u32), 0);
    assert!(members.iter().all(|m| m != stranger));
}

// â”€â”€â”€ Gate 1: timelock delay â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_execute_upgrade_before_delay_is_rejected() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);

    // Satisfy the multi-sig gate so only the timelock can be what rejects.
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    assert_eq!(client.get_upgrade_approvals(&action_id), 2);

    set_timestamp(&env, 1_000 + client.get_timelock_delay() - 1);
    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::TimelockNotElapsed))
    );
    assert_eq!(client.get_version(), 1);
    assert_eq!(client.get_logic_address(), None);
    // The action stays pending so it can still be executed later.
    assert!(!client.get_action(&action_id).unwrap().executed);
}

#[test]
fn test_delay_is_measured_from_the_delay_configured_at_proposal_time() {
    let (env, client, admin, _members, target) = setup();
    set_timestamp(&env, 1_000);
    client.set_timelock_delay(&admin, &3_600u64);

    let action_id = client.upgrade(&admin, &target);
    assert_eq!(
        client.get_action(&action_id).unwrap().execution_not_before,
        4_600
    );

    // Shrinking the delay afterwards must not shorten this proposal's window.
    client.set_timelock_delay(&admin, &1u64);
    set_timestamp(&env, 4_599);
    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::TimelockNotElapsed))
    );
}

// â”€â”€â”€ Gate 2: 2-of-N multi-sig approval â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_execute_upgrade_requires_two_distinct_approvals() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);
    elapse_timelock(&env, &client, action_id);

    // No approvals.
    assert_eq!(client.get_upgrade_approvals(&action_id), 0);
    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::UpgradeApprovalThresholdNotMet))
    );

    // One approval is still short of the 2-of-N threshold.
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    assert_eq!(client.get_upgrade_approvals(&action_id), 1);
    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::UpgradeApprovalThresholdNotMet))
    );

    // The same admin cannot supply the second signature.
    assert_eq!(
        client.try_approve_upgrade(&members.get(0).unwrap(), &action_id),
        Err(Ok(ContractError::AlreadyApproved))
    );
    assert_eq!(client.get_upgrade_approvals(&action_id), 1);

    // A second distinct admin clears the gate.
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    assert_eq!(client.get_upgrade_approvals(&action_id), 2);
    assert_eq!(client.get_version(), 1);
}

#[test]
fn test_approval_must_come_from_the_configured_multisig() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);
    let outsider = Address::generate(&env);

    assert_eq!(
        client.try_approve_upgrade(&outsider, &action_id),
        Err(Ok(ContractError::Unauthorized))
    );
    // The protocol admin is not implicitly a member of the multi-sig set.
    assert_eq!(
        client.try_approve_upgrade(&admin, &action_id),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(client.get_upgrade_approvals(&action_id), 0);
    assert!(members.iter().all(|m| m != admin));
}

#[test]
fn test_upgrade_cannot_execute_after_multisig_membership_rotates() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    elapse_timelock(&env, &client, action_id);

    // Membership rotates to an entirely different set before execution, so the
    // approvals already recorded no longer belong to anyone in the set.
    client.set_global_pause_admins(
        &admin,
        &vec![&env, Address::generate(&env), Address::generate(&env)],
    );
    assert_eq!(client.get_upgrade_approvals(&action_id), 0);
    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::UpgradeApprovalThresholdNotMet))
    );
    assert_eq!(client.get_version(), 1);
}

#[test]
fn test_upgrade_fails_closed_when_no_multisig_is_configured() {
    let (env, client, admin, members, target) = setup_without_multisig();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);
    elapse_timelock(&env, &client, action_id);

    assert_eq!(
        client.try_approve_upgrade(&members.get(0).unwrap(), &action_id),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(client.get_version(), 1);
}

#[test]
fn test_approve_upgrade_rejects_non_upgrade_actions() {
    let (env, client, admin, members, _target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.propose_action(&admin, &TimelockChangeType::Fee, &Bytes::new(&env));

    assert_eq!(
        client.try_approve_upgrade(&members.get(0).unwrap(), &action_id),
        Err(Ok(ContractError::InvalidChangeType))
    );
    assert_eq!(client.get_upgrade_approvals(&action_id), 0);
}

#[test]
fn test_approve_upgrade_rejects_missing_or_settled_actions() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);

    assert_eq!(
        client.try_approve_upgrade(&members.get(0).unwrap(), &404u32),
        Err(Ok(ContractError::ProposalNotFound))
    );

    let action_id = client.upgrade(&admin, &target);
    client.cancel_action(&admin, &action_id);
    assert_eq!(
        client.try_approve_upgrade(&members.get(0).unwrap(), &action_id),
        Err(Ok(ContractError::ActionNotPending))
    );
}

#[test]
fn test_approvals_do_not_leak_to_a_later_upgrade() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let first = client.upgrade(&admin, &target);
    client.approve_upgrade(&members.get(0).unwrap(), &first);
    client.approve_upgrade(&members.get(1).unwrap(), &first);
    assert_eq!(client.get_upgrade_approvals(&first), 2);
    client.cancel_action(&admin, &first);

    let second = client.upgrade(&admin, &target);
    assert_ne!(first, second);
    assert_eq!(client.get_upgrade_approvals(&second), 0);

    elapse_timelock(&env, &client, second);
    assert_eq!(
        client.try_execute_action(&admin, &second),
        Err(Ok(ContractError::UpgradeApprovalThresholdNotMet))
    );
}

// â”€â”€â”€ Gate 3: emergency freeze â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_upgrade_execution_blocked_while_frozen() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    elapse_timelock(&env, &client, action_id);

    // 2-of-N emergency freeze: the first vote alone does not halt anything.
    client.global_pause(&members.get(0).unwrap());
    assert!(!client.get_global_trading_paused());
    client.global_pause(&members.get(1).unwrap());
    assert!(client.get_global_trading_paused());

    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::ContractFrozen))
    );
    assert_eq!(client.get_version(), 1);
    assert_eq!(client.get_logic_address(), None);
    assert!(!client.get_action(&action_id).unwrap().executed);

    // Lifting the freeze lets the queued upgrade proceed.
    client.global_resume(&members.get(0).unwrap());
    client.global_resume(&members.get(1).unwrap());
    assert!(!client.get_global_trading_paused());
    client.execute_action(&admin, &action_id);
    assert_eq!(client.get_version(), 2);
}

#[test]
fn test_upgrade_cannot_be_queued_while_frozen() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    client.global_pause(&members.get(0).unwrap());
    client.global_pause(&members.get(1).unwrap());

    assert_eq!(
        client.try_upgrade(&admin, &target),
        Err(Ok(ContractError::ContractFrozen))
    );
    assert_eq!(client.get_upgrade_target(), None);

    client.global_resume(&members.get(0).unwrap());
    client.global_resume(&members.get(1).unwrap());
    let action_id = client.upgrade(&admin, &target);
    assert_eq!(client.get_upgrade_target(), Some(target));
    assert!(!client.get_action(&action_id).unwrap().executed);
}

#[test]
fn test_upgrade_blocked_while_single_admin_pause_is_active() {
    let (_env, client, admin, _members, target) = setup();
    client.pause(&admin);
    assert!(client.get_is_paused());

    assert_eq!(
        client.try_upgrade(&admin, &target),
        Err(Ok(ContractError::ContractFrozen))
    );

    client.unpause(&admin);
    assert!(!client.get_is_paused());
    client.upgrade(&admin, &target);
}

// â”€â”€â”€ Payload validation â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_malformed_upgrade_payload_is_rejected() {
    let (env, client, admin, members, _target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.propose_action(
        &admin,
        &TimelockChangeType::Upgrade,
        &Bytes::from_slice(&env, &[1u8; 8]),
    );
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    elapse_timelock(&env, &client, action_id);

    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::InvalidUpgradePayload))
    );
    assert_eq!(client.get_version(), 1);
    assert!(!client.get_action(&action_id).unwrap().executed);
}

#[test]
fn test_empty_upgrade_payload_is_rejected() {
    let (env, client, admin, members, _target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.propose_action(&admin, &TimelockChangeType::Upgrade, &Bytes::new(&env));
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    elapse_timelock(&env, &client, action_id);

    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::InvalidUpgradePayload))
    );
}

// â”€â”€â”€ Cancellation â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_cancelled_upgrade_clears_target_and_approvals() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    assert_eq!(client.get_upgrade_approvals(&action_id), 2);
    assert_eq!(client.get_upgrade_target(), Some(target));

    client.cancel_action(&admin, &action_id);

    assert!(client.get_action(&action_id).unwrap().cancelled);
    assert_eq!(client.get_upgrade_target(), None);
    assert_eq!(client.get_upgrade_approvals(&action_id), 0);

    elapse_timelock(&env, &client, action_id);
    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::ActionNotPending))
    );
    assert_eq!(client.get_version(), 1);
}

// â”€â”€â”€ Successful upgrade: state preservation and events â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_upgrade_applies_and_preserves_all_contract_state() {
    let (env, client, admin, members, target) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    register_creator(&env, &client, &creator);
    client.buy_key(&creator, &buyer, &1000i128, &None);

    let supply_before = client.get_total_key_supply(&creator);
    let balance_before = client.get_key_balance(&creator, &buyer);
    let holders_before = client.get_creator_holder_count(&creator);
    let fee_bps_before = client.get_protocol_fee_view();
    assert_eq!(supply_before, 1);
    assert_eq!(balance_before, 1);
    assert_eq!(holders_before, 1);

    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    elapse_timelock(&env, &client, action_id);
    client.execute_action(&admin, &action_id);

    // The new logic build is in effect.
    assert_eq!(client.get_version(), 2);
    assert_eq!(client.get_logic_address(), Some(target.clone()));
    assert_eq!(client.get_upgrade_target(), None);
    assert_eq!(client.get_upgrade_approvals(&action_id), 0);
    assert!(client.get_action(&action_id).unwrap().executed);

    // Every piece of pre-existing state survived the swap untouched.
    assert_eq!(client.get_total_key_supply(&creator), supply_before);
    assert_eq!(client.get_key_balance(&creator, &buyer), balance_before);
    assert_eq!(client.get_creator_holder_count(&creator), holders_before);
    let fee_bps_after = client.get_protocol_fee_view();
    assert_eq!(fee_bps_after.creator_bps, fee_bps_before.creator_bps);
    assert_eq!(fee_bps_after.protocol_bps, fee_bps_before.protocol_bps);
    assert_eq!(client.get_creator(&creator).supply, supply_before);

    // The upgraded contract still accepts writes: register and buy for a new
    // creator, which starts from zero supply.
    let creator_two = Address::generate(&env);
    register_creator(&env, &client, &creator_two);
    client.buy_key(&creator_two, &buyer, &1000i128, &None);
    assert_eq!(client.get_key_balance(&creator_two, &buyer), 1);
    // The original holder is unaffected by the new creator's trade.
    assert_eq!(client.get_key_balance(&creator, &buyer), balance_before);
}

#[test]
fn test_logic_upgraded_event_carries_old_and_new_hash() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    elapse_timelock(&env, &client, action_id);
    let executed_at = client.get_action(&action_id).unwrap().execution_not_before;
    client.execute_action(&admin, &action_id);

    let payload: events::LogicUpgradedEvent = sole_event(&env, &events::LOGIC_UPGRADED_EVENT_NAME);
    assert_eq!(payload.action_id, action_id);
    // First recorded upgrade: there is no prior build to report.
    assert_eq!(payload.old_wasm_hash, None);
    assert_eq!(payload.new_wasm_hash, target);
    assert_eq!(payload.old_version, 1);
    assert_eq!(payload.new_version, 2);
    assert_eq!(payload.executed_at, executed_at);
}

#[test]
fn test_legacy_upgrade_executed_event_is_still_emitted() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    elapse_timelock(&env, &client, action_id);
    client.execute_action(&admin, &action_id);

    let payload: events::UpgradeExecutedEvent =
        sole_event(&env, &events::UPGRADE_EXECUTED_EVENT_NAME);
    assert_eq!(payload.old_version, 1);
    assert_eq!(payload.new_version, 2);
}

#[test]
fn test_upgrade_approved_event_reports_running_count() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);

    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    let first: events::UpgradeApprovedEvent =
        sole_event(&env, &events::UPGRADE_APPROVED_EVENT_NAME);
    assert_eq!(first.action_id, action_id);
    assert_eq!(first.admin, members.get(0).unwrap());
    assert_eq!(first.approvals, 1);
    assert_eq!(first.threshold, 2);
    assert_eq!(first.approved_at, 1_000);

    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    let second: events::UpgradeApprovedEvent =
        sole_event(&env, &events::UPGRADE_APPROVED_EVENT_NAME);
    assert_eq!(second.admin, members.get(1).unwrap());
    // The event carries the running count so indexers need no off-chain tally.
    assert_eq!(second.approvals, 2);
}

#[test]
fn test_second_upgrade_reports_the_previous_build() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let mut upgraded: Vec<events::LogicUpgradedEvent> = Vec::new(&env);

    let first = client.upgrade(&admin, &target);
    client.approve_upgrade(&members.get(0).unwrap(), &first);
    client.approve_upgrade(&members.get(1).unwrap(), &first);
    elapse_timelock(&env, &client, first);
    client.execute_action(&admin, &first);
    collect_events(&env, &events::LOGIC_UPGRADED_EVENT_NAME, &mut upgraded);

    assert_eq!(client.get_version(), 2);
    assert_eq!(client.get_logic_address(), Some(target.clone()));
    // Nothing preceded the first upgrade, so there is no rollback target yet.
    assert_eq!(client.get_previous_wasm(), None);

    let second = client.upgrade(&admin, &target);
    client.approve_upgrade(&members.get(0).unwrap(), &second);
    client.approve_upgrade(&members.get(2).unwrap(), &second);
    elapse_timelock(&env, &client, second);
    client.execute_action(&admin, &second);
    collect_events(&env, &events::LOGIC_UPGRADED_EVENT_NAME, &mut upgraded);

    assert_eq!(client.get_version(), 3);
    // The build that was live before this upgrade is now retained for rollback.
    assert_eq!(client.get_previous_wasm(), Some(target.clone()));
    assert_eq!(client.get_logic_address(), Some(target.clone()));

    // Both upgrades reported; the second names the build it replaced.
    assert_eq!(upgraded.len(), 2);
    let payload = upgraded.get(1).unwrap();
    assert_eq!(payload.action_id, second);
    assert_eq!(payload.old_wasm_hash, Some(target));
    assert_eq!(payload.old_version, 2);
    assert_eq!(payload.new_version, 3);
}

#[test]
fn test_upgraded_action_cannot_be_executed_twice() {
    let (env, client, admin, members, target) = setup();
    set_timestamp(&env, 1_000);
    let action_id = client.upgrade(&admin, &target);
    client.approve_upgrade(&members.get(0).unwrap(), &action_id);
    client.approve_upgrade(&members.get(1).unwrap(), &action_id);
    elapse_timelock(&env, &client, action_id);
    client.execute_action(&admin, &action_id);
    assert_eq!(client.get_version(), 2);

    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::ActionNotPending))
    );
    assert_eq!(client.get_version(), 2);
}

// ─── Non-upgrade change types ────────────────────────────────────────────

#[test]
fn test_non_upgrade_change_types_are_retired_without_applying_anything() {
    let (env, client, admin, _members, _target) = setup();
    set_timestamp(&env, 1_000);
    let treasury_before = client.get_treasury_address();
    let slope_before = client.get_curve_slope();
    let fee_before = client.get_protocol_trade_fee();
    let target_before = client.get_upgrade_target();

    // A malformed payload must not be able to abort the contract: `Fee`,
    // `CurveExponent` and `Treasury` actions ignore their payload entirely, so
    // there is no decode to fail. All three are proposed at the same timestamp
    // so a single clock advance makes them all executable.
    let change_types = vec![
        &env,
        TimelockChangeType::Fee,
        TimelockChangeType::CurveExponent,
        TimelockChangeType::Treasury,
    ];
    let mut action_ids: Vec<u32> = Vec::new(&env);
    for i in 0..change_types.len() {
        action_ids.push_back(client.propose_action(
            &admin,
            &change_types.get(i).unwrap(),
            &Bytes::from_slice(&env, &[0xffu8; 7]),
        ));
    }

    set_timestamp(&env, 1_000 + client.get_timelock_delay());
    for i in 0..action_ids.len() {
        let action_id = action_ids.get(i).unwrap();
        client.execute_action(&admin, &action_id);
        assert!(
            client.get_action(&action_id).unwrap().executed,
            "action {action_id} should still retire on schedule"
        );
    }

    // Nothing was applied, and in particular the upgrade machinery was untouched.
    assert_eq!(client.get_treasury_address(), treasury_before);
    assert_eq!(client.get_curve_slope(), slope_before);
    assert_eq!(client.get_protocol_trade_fee(), fee_before);
    assert_eq!(client.get_upgrade_target(), target_before);
    assert_eq!(client.get_version(), 1);
    assert_eq!(client.get_logic_address(), None);
}
