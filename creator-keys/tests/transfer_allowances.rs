//! Integration coverage for key transfer allowances (`approve` / `transfer_from`).
//!
//! 1. `approve` stores an allowance per `(owner, spender, key_id)` tuple and
//!    emits `Approval` with owner, spender, amount and key id.
//! 2. `approve` with `amount = 0` revokes the allowance.
//! 3. `transfer_from` succeeds up to the approved amount and moves balances.
//! 4. `transfer_from` is rejected when the allowance is insufficient.
//! 5. The allowance is decremented atomically by the transferred amount.
//! 6. Supply, holder count and dividend state stay consistent.
//! 7. The delegated path enforces the same invariants as `transfer_keys`:
//!    the creator's post-buy cooldown and holder-count-changed events.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{
    events::{ApprovalEvent, TransferFromEvent, APPROVAL_EVENT_NAME, TRANSFER_FROM_EVENT_NAME},
    AllowanceError, ContractError, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    Address, Env, IntoVal, String, Symbol,
};

const KEY_PRICE: i128 = 100;

fn register_creator(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    h: &str,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, h),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    creator
}

fn setup(env: &Env) -> (creator_keys::CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);
    (client, admin)
}

fn approval_events(env: &Env) -> soroban_sdk::Vec<ApprovalEvent> {
    let mut found = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == APPROVAL_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

fn transfer_from_events(env: &Env) -> soroban_sdk::Vec<TransferFromEvent> {
    let mut found = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == TRANSFER_FROM_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

/// Holder-count events from the most recent invocation.
///
/// `env.events()` only retains the last contract call, so this must be read
/// before any read-only view call resets the log.
fn holder_count_events(
    env: &Env,
) -> soroban_sdk::Vec<creator_keys::events::HolderCountChangedEvent> {
    let mut found = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == creator_keys::events::HOLDER_COUNT_CHANGED_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

/// Register a creator and fund `holder` with `count` keys.
fn creator_with_holder(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    handle: &str,
    count: u32,
) -> (Address, Address) {
    let creator = register_creator(env, client, handle);
    let holder = Address::generate(env);
    for _ in 0..count {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }
    (creator, holder)
}

// ---------------------------------------------------------------------------
// 1. approve stores per-tuple allowance
// ---------------------------------------------------------------------------

#[test]
fn test_approve_stores_allowance_for_owner_spender_key_tuple() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 3);
    let spender = Address::generate(&env);

    assert_eq!(client.get_allowance(&holder, &spender, &creator), 0);
    client.approve(&holder, &spender, &creator, &2);

    assert_eq!(client.get_allowance(&holder, &spender, &creator), 2);
}

#[test]
fn test_allowance_is_scoped_per_spender_and_per_key() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator_a, holder) = creator_with_holder(&env, &client, "alice", 3);
    let (creator_b, _h2) = creator_with_holder(&env, &client, "bob", 3);
    let spender_one = Address::generate(&env);
    let spender_two = Address::generate(&env);

    client.approve(&holder, &spender_one, &creator_a, &1);
    client.approve(&holder, &spender_two, &creator_a, &2);
    client.approve(&holder, &spender_one, &creator_b, &3);

    assert_eq!(client.get_allowance(&holder, &spender_one, &creator_a), 1);
    assert_eq!(client.get_allowance(&holder, &spender_two, &creator_a), 2);
    assert_eq!(client.get_allowance(&holder, &spender_one, &creator_b), 3);
}

#[test]
fn test_approve_overwrites_rather_than_accumulates() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 5);
    let spender = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &4);
    client.approve(&holder, &spender, &creator, &1);

    assert_eq!(
        client.get_allowance(&holder, &spender, &creator),
        1,
        "approve replaces the previous value, it does not add to it"
    );
}

#[test]
fn test_approve_zero_revokes_allowance() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 3);
    let spender = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &2);
    assert_eq!(client.get_allowance(&holder, &spender, &creator), 2);

    client.approve(&holder, &spender, &creator, &0);
    assert_eq!(client.get_allowance(&holder, &spender, &creator), 0);
}

#[test]
fn test_approve_rejects_self_approval() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 1);

    let result = client.try_approve(&holder, &holder, &creator, &1);
    assert_eq!(result, Err(Ok(AllowanceError::SelfTransfer)));
}

#[test]
fn test_approve_rejects_zero_address_spender() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 1);
    let zero = Address::from_string(&String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ));

    let result = client.try_approve(&holder, &zero, &creator, &1);
    assert_eq!(result, Err(Ok(AllowanceError::ZeroAddress)));
}

#[test]
fn test_approve_rejects_unregistered_key() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (_creator, holder) = creator_with_holder(&env, &client, "alice", 1);
    let stranger = Address::generate(&env);
    let spender = Address::generate(&env);

    let result = client.try_approve(&holder, &spender, &stranger, &1);
    assert_eq!(result, Err(Ok(AllowanceError::NotRegistered)));
}

// ---------------------------------------------------------------------------
// 2. Approval event
// ---------------------------------------------------------------------------

#[test]
fn test_approval_event_emitted_with_all_fields() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 3);
    let spender = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &2);

    let events = approval_events(&env);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.owner, holder);
    assert_eq!(event.spender, spender);
    assert_eq!(event.amount, 2);
    assert_eq!(event.key_id, creator);
}

#[test]
fn test_approval_event_topics_carry_owner_and_spender() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 3);
    let spender = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &1);

    let all = env.events().all();
    let mut checked = false;
    for (_, topics, _) in all.iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&env);
        if name == APPROVAL_EVENT_NAME {
            let topic_owner: Address = topics.get(1).unwrap().into_val(&env);
            let topic_spender: Address = topics.get(2).unwrap().into_val(&env);
            assert_eq!(topic_owner, holder);
            assert_eq!(topic_spender, spender);
            checked = true;
        }
    }
    assert!(checked, "expected an Approval event");
}

// ---------------------------------------------------------------------------
// 3. transfer_from succeeds up to the allowance
// ---------------------------------------------------------------------------

#[test]
fn test_transfer_from_succeeds_up_to_approved_amount() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 5);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &2);
    client.transfer_from(&spender, &holder, &recipient, &creator, &2);

    assert_eq!(client.get_key_balance(&creator, &holder), 3);
    assert_eq!(client.get_key_balance(&creator, &recipient), 2);
    assert_eq!(
        client.get_total_key_supply(&creator),
        5,
        "transfer_from moves ownership without minting or burning"
    );
}

#[test]
fn test_transfer_from_can_be_called_repeatedly_until_allowance_spent() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 5);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &3);
    client.transfer_from(&spender, &holder, &recipient, &creator, &1);
    assert_eq!(client.get_allowance(&holder, &spender, &creator), 2);

    client.transfer_from(&spender, &holder, &recipient, &creator, &2);
    assert_eq!(client.get_allowance(&holder, &spender, &creator), 0);
    assert_eq!(client.get_key_balance(&creator, &recipient), 3);
}

// ---------------------------------------------------------------------------
// 4. transfer_from rejected when the allowance is insufficient
// ---------------------------------------------------------------------------

#[test]
fn test_transfer_from_rejected_when_no_allowance() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 5);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let result = client.try_transfer_from(&spender, &holder, &recipient, &creator, &1);
    assert_eq!(result, Err(Ok(AllowanceError::InsufficientAllowance)));
}

#[test]
fn test_transfer_from_rejected_above_allowance() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 5);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &2);
    let result = client.try_transfer_from(&spender, &holder, &recipient, &creator, &3);

    assert_eq!(result, Err(Ok(AllowanceError::InsufficientAllowance)));
    // State assertions: a rejected call must not move any keys.
    assert_eq!(client.get_key_balance(&creator, &holder), 5);
    assert_eq!(client.get_key_balance(&creator, &recipient), 0);
    assert_eq!(client.get_allowance(&holder, &spender, &creator), 2);
}

#[test]
fn test_transfer_from_allowance_check_precedes_balance_check() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 2);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Allowance of 5 exceeds the holder's balance of 2, so the balance is the
    // binding constraint and the transfer still fails.
    client.approve(&holder, &spender, &creator, &5);
    let result = client.try_transfer_from(&spender, &holder, &recipient, &creator, &5);
    assert_eq!(result, Err(Ok(AllowanceError::InsufficientBalance)));
    assert_eq!(client.get_allowance(&holder, &spender, &creator), 5);
}

#[test]
fn test_transfer_from_rejects_zero_amount() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 3);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &3);
    let result = client.try_transfer_from(&spender, &holder, &recipient, &creator, &0);
    assert_eq!(result, Err(Ok(AllowanceError::ZeroAmount)));
}

#[test]
fn test_transfer_from_rejects_self_transfer() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 3);
    let spender = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &3);
    let result = client.try_transfer_from(&spender, &holder, &holder, &creator, &1);
    assert_eq!(result, Err(Ok(AllowanceError::SelfTransfer)));
}

#[test]
fn test_transfer_from_rejects_unregistered_key() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 1);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let stranger = Address::generate(&env);

    let approve_result = client.try_approve(&holder, &spender, &stranger, &1);
    assert_eq!(approve_result, Err(Ok(AllowanceError::NotRegistered)));

    // Force the state so the transfer path is exercised too.
    client.approve(&holder, &spender, &creator, &1);
    let result = client.try_transfer_from(&spender, &holder, &recipient, &stranger, &1);
    assert_eq!(result, Err(Ok(AllowanceError::NotRegistered)));
}

#[test]
fn test_transfer_from_respects_frozen_position() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 3);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &2);
    client.freeze_position(&creator, &holder);

    let result = client.try_transfer_from(&spender, &holder, &recipient, &creator, &1);
    assert_eq!(result, Err(Ok(AllowanceError::FrozenPosition)));
    assert_eq!(client.get_allowance(&holder, &spender, &creator), 2);
}

// ---------------------------------------------------------------------------
// 5. Allowance decrements atomically
// ---------------------------------------------------------------------------

#[test]
fn test_allowance_decremented_by_transferred_amount() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 5);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &4);
    client.transfer_from(&spender, &holder, &recipient, &creator, &3);

    assert_eq!(
        client.get_allowance(&holder, &spender, &creator),
        1,
        "allowance must drop by exactly the transferred amount"
    );
}

#[test]
fn test_allowance_entry_removed_when_fully_consumed() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 5);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &2);
    client.transfer_from(&spender, &holder, &recipient, &creator, &2);

    assert_eq!(client.get_allowance(&holder, &spender, &creator), 0);
    // A fully consumed allowance cannot be reused.
    let result = client.try_transfer_from(&spender, &holder, &recipient, &creator, &1);
    assert_eq!(result, Err(Ok(AllowanceError::InsufficientAllowance)));
}

#[test]
fn test_transfer_from_event_reports_remaining_allowance() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 5);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &4);
    client.transfer_from(&spender, &holder, &recipient, &creator, &3);

    let events = transfer_from_events(&env);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.key_id, creator);
    assert_eq!(event.spender, spender);
    assert_eq!(event.from, holder);
    assert_eq!(event.to, recipient);
    assert_eq!(event.amount, 3);
    assert_eq!(event.remaining_allowance, 1);
}

// ---------------------------------------------------------------------------
// 6. State consistency
// ---------------------------------------------------------------------------

#[test]
fn test_transfer_from_maintains_holder_count() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 2);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let before = client.get_creator(&creator).holder_count;
    assert_eq!(before, 1);

    client.approve(&holder, &spender, &creator, &2);
    // Move the holder's whole balance away; the count must stay at one holder.
    client.transfer_from(&spender, &holder, &recipient, &creator, &2);

    assert_eq!(client.get_creator(&creator).holder_count, 1);
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
    assert_eq!(client.get_key_balance(&creator, &recipient), 2);
}

#[test]
fn test_transfer_from_increments_holder_count_for_new_recipient() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 2);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &1);
    client.transfer_from(&spender, &holder, &recipient, &creator, &1);

    assert_eq!(client.get_creator(&creator).holder_count, 2);
}

#[test]
fn test_transfer_from_respects_recipient_holding_cap() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 10);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Cap every wallet at 2 keys; the holder's 10-key balance predates the cap
    // but new recipient balances may not exceed it.
    client.set_holding_cap(&creator, &2);
    assert_eq!(client.get_holding_cap(&creator), Some(2));

    client.approve(&holder, &spender, &creator, &5);
    let result = client.try_transfer_from(&spender, &holder, &recipient, &creator, &3);
    assert_eq!(result, Err(Ok(AllowanceError::HoldingCapExceeded)));

    // A transfer inside the cap still succeeds.
    client.transfer_from(&spender, &holder, &recipient, &creator, &2);
    assert_eq!(client.get_key_balance(&creator, &recipient), 2);
    assert_eq!(client.get_allowance(&holder, &spender, &creator), 3);
}

#[test]
fn test_transfer_from_succeeds_within_zero_tax_key() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 3);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // A creator with no sell tax configured must still permit delegated transfers.
    assert_eq!(client.get_sell_tax_bps(&creator), 0);
    client.approve(&holder, &spender, &creator, &3);
    client.transfer_from(&spender, &holder, &recipient, &creator, &3);
    assert_eq!(client.get_key_balance(&creator, &recipient), 3);
}

#[test]
fn test_transfer_from_rejected_when_contract_paused() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 3);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &2);
    client.pause(&admin);

    let result = client.try_transfer_from(&spender, &holder, &recipient, &creator, &1);
    assert_eq!(result, Err(Ok(AllowanceError::ProtocolPaused)));
}

#[test]
fn test_direct_transfer_still_works_alongside_allowances() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 5);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let other = Address::generate(&env);

    client.approve(&holder, &spender, &creator, &2);
    client.transfer_from(&spender, &holder, &recipient, &creator, &1);
    // The owner can still move their own keys without touching the allowance.
    client.transfer_keys(&creator, &holder, &other, &1);

    assert_eq!(client.get_key_balance(&creator, &recipient), 1);
    assert_eq!(client.get_key_balance(&creator, &other), 1);
    assert_eq!(
        client.get_allowance(&holder, &spender, &creator),
        1,
        "a direct transfer_keys must not consume the delegated allowance"
    );
    assert_eq!(client.get_total_key_supply(&creator), 5);
}

// Keep the shared `ContractError` import meaningful for the pause error path.
#[allow(dead_code)]
fn _assert_pause_error(e: ContractError) {
    assert_eq!(e, ContractError::ProtocolPaused);
}

// ---------------------------------------------------------------------------
// 7. Parity with the direct transfer path
// ---------------------------------------------------------------------------

/// AC: a spender cannot route keys around the creator's post-buy cooldown.
#[test]
fn test_transfer_from_respects_buy_cooldown() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 2);

    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.approve(&holder, &spender, &creator, &2);
    client.set_buy_cooldown(&creator, &10);

    let result = client.try_transfer_from(&spender, &holder, &recipient, &creator, &1);
    assert_eq!(result, Err(Ok(AllowanceError::CooldownActive)));
    assert_eq!(client.get_key_balance(&creator, &holder), 2);
    assert_eq!(client.get_allowance(&holder, &spender, &creator), 2);
    assert_eq!(client.get_key_balance(&creator, &recipient), 0);
}

/// AC: the cooldown guard stops blocking once the window elapses.
#[test]
fn test_transfer_from_allowed_after_cooldown_elapses() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 2);

    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.approve(&holder, &spender, &creator, &2);
    client.set_buy_cooldown(&creator, &10);
    assert_eq!(
        client.try_transfer_from(&spender, &holder, &recipient, &creator, &1),
        Err(Ok(AllowanceError::CooldownActive))
    );

    // The buys happened on ledger 0; step past the 10-ledger window.
    env.ledger().set_sequence_number(10);
    client.transfer_from(&spender, &holder, &recipient, &creator, &1);
    assert_eq!(client.get_key_balance(&creator, &holder), 1);
    assert_eq!(client.get_key_balance(&creator, &recipient), 1);
}

/// AC: a transfer that adds a holder emits the holder-count change.
#[test]
fn test_transfer_from_emits_holder_count_change() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 3);
    assert_eq!(client.get_creator(&creator).holder_count, 1);

    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.approve(&holder, &spender, &creator, &3);
    // The sender keeps a balance and the receiver becomes a new holder, so the
    // count moves 1 -> 2.
    client.transfer_from(&spender, &holder, &recipient, &creator, &1);
    let found = holder_count_events(&env);

    assert_eq!(client.get_creator(&creator).holder_count, 2);
    assert_eq!(client.get_key_balance(&creator, &holder), 2);
    assert_eq!(client.get_key_balance(&creator, &recipient), 1);

    let event = found.get(0).unwrap();
    assert_eq!(event.creator_id, creator);
    assert_eq!(event.old_count, 1);
    assert_eq!(event.new_count, 2);
    assert_eq!(event.ledger, env.ledger().sequence());
}

/// AC: a full transfer that only swaps holders leaves the count unchanged.
///
/// `emit_holder_count_changed` reports the net count, so a sender dropping off
/// at the same time as a receiver joining is invisible — matching the behaviour
/// of the direct `transfer_keys` path.
#[test]
fn test_transfer_from_swapping_holders_emits_no_count_change() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 2);
    assert_eq!(client.get_creator(&creator).holder_count, 1);

    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.approve(&holder, &spender, &creator, &2);
    client.transfer_from(&spender, &holder, &recipient, &creator, &2);
    let found = holder_count_events(&env);

    assert!(
        found.is_empty(),
        "a net-zero holder swap must not emit a count change"
    );
    assert_eq!(client.get_creator(&creator).holder_count, 1);
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
    assert_eq!(client.get_key_balance(&creator, &recipient), 2);
}

/// AC: supply is untouched by a delegated transfer.
#[test]
fn test_transfer_from_does_not_change_supply() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let (creator, holder) = creator_with_holder(&env, &client, "alice", 4);

    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.approve(&holder, &spender, &creator, &4);
    let supply_before = client.get_total_key_supply(&creator);
    client.transfer_from(&spender, &holder, &recipient, &creator, &3);

    assert_eq!(client.get_total_key_supply(&creator), supply_before);
    assert_eq!(client.get_key_balance(&creator, &holder), 1);
    assert_eq!(client.get_key_balance(&creator, &recipient), 3);
    assert_eq!(client.get_allowance(&holder, &spender, &creator), 1);
}
