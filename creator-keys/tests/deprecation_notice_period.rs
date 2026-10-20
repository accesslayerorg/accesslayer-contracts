//! Integration tests for the notice-period deprecation flow: retiring inactive
//! creator keys after a notice period, unwinding every remaining position at
//! the last traded price.
//!
//! Acceptance criteria verified:
//!
//! 1. `execute_deprecation` reverts before the notice period expires.
//! 2. All holder positions are unwound correctly at the last traded price.
//! 3. Holders can exit normally during the notice period.
//! 4. `DeprecationProposed` is emitted with the correct unwind date and reason.
//! 5. `DeprecationExecuted` is emitted with the total positions unwound and the
//!    amount distributed.

use creator_keys::{
    events, ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    Address, Env, IntoVal, String,
};

const SECONDS_PER_DAY: u64 = 86_400;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn setup() -> (Env, CreatorKeysContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    // Flat price of 100 per key, no slope so every key costs exactly 100.
    client.set_key_price(&admin, &100_i128);
    client.set_curve_slope(&admin, &0_i128);
    client.set_fee_config(&admin, &9_000_u32, &1_000_u32);
    // Disable circuit breaker so sequential buys never trip it.
    client.set_circuit_breaker_threshold(&admin, &10_000_u32);

    (env, client, admin)
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient, handle: &str) -> Address {
    let creator = Address::generate(env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
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

fn buy_one(client: &CreatorKeysContractClient, creator: &Address, buyer: &Address) {
    client.buy_key(creator, buyer, &10_000_i128, &None);
}

fn set_timestamp(env: &Env, timestamp: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = timestamp;
    env.ledger().set(ledger);
}

fn set_ledger_sequence(env: &Env, sequence: u32) {
    let mut ledger = env.ledger().get();
    ledger.sequence_number = sequence;
    env.ledger().set(ledger);
}

// ---------------------------------------------------------------------------
// 4. DeprecationProposed emitted with correct unwind date and reason
// ---------------------------------------------------------------------------

#[test]
fn test_propose_deprecation_emits_event_with_unwind_date_and_reason() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    buy_one(&client, &creator, &buyer);

    let start = 1_700_000_000u64;
    set_timestamp(&env, start);

    let proposal = client.propose_deprecation(
        &admin,
        &creator,
        &7u32,
        &String::from_str(&env, "inactive creator"),
    );

    let expected_unwind = start + 7 * SECONDS_PER_DAY;
    assert_eq!(proposal.unwind_at_timestamp, expected_unwind);
    assert_eq!(proposal.notice_period_days, 7);
    assert!(!proposal.executed);

    let all_events = env.events().all();
    let event = all_events
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .map(|v| {
                    let sym: soroban_sdk::Symbol = v.into_val(&env);
                    sym == events::DEPRECATION_PROPOSED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("DeprecationProposed event not found");

    let data: events::DeprecationProposedEvent = event.2.into_val(&env);
    assert_eq!(data.key_id, creator);
    assert_eq!(data.reason, String::from_str(&env, "inactive creator"));
    assert_eq!(data.notice_period_days, 7);
    assert_eq!(data.unwind_at_timestamp, expected_unwind);
}

// ---------------------------------------------------------------------------
// 1. execute_deprecation reverts before the notice period expires
// ---------------------------------------------------------------------------

#[test]
fn test_execute_deprecation_reverts_before_notice_period_expires() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "bob");
    let holder = Address::generate(&env);
    buy_one(&client, &creator, &holder);

    set_timestamp(&env, 1_700_000_000);
    client.propose_deprecation(&admin, &creator, &3u32, &String::from_str(&env, "retire"));

    // One day later: notice period is still active.
    set_timestamp(&env, 1_700_000_000 + SECONDS_PER_DAY);

    let result = client.try_execute_deprecation(&admin, &creator);
    assert_eq!(
        result,
        Err(Ok(ContractError::DeprecationNoticePeriodActive))
    );

    // Positions are untouched after the rejected call.
    assert_eq!(client.get_key_balance(&creator, &holder), 1);
    assert_eq!(client.get_creator_supply(&creator), 1);
}

#[test]
fn test_execute_deprecation_rejects_non_admin() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "brenda");
    let attacker = Address::generate(&env);

    set_timestamp(&env, 1_700_000_000);
    client.propose_deprecation(&admin, &creator, &1u32, &String::from_str(&env, "x"));
    set_timestamp(&env, 1_700_000_000 + SECONDS_PER_DAY);

    let result = client.try_execute_deprecation(&attacker, &creator);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_execute_deprecation_without_proposal_reverts() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "ben");

    let result = client.try_execute_deprecation(&admin, &creator);
    assert_eq!(result, Err(Ok(ContractError::NoDeprecationProposed)));
}

// ---------------------------------------------------------------------------
// 2. All holder positions unwound correctly at the last traded price
// ---------------------------------------------------------------------------

#[test]
fn test_execute_deprecation_unwinds_all_holders_at_last_traded_price() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "carol");
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    // Two buys each holder -> supply 4. With a flat curve every traded price is
    // 100, so the last traded price is 100.
    buy_one(&client, &creator, &holder_a);
    buy_one(&client, &creator, &holder_a);
    buy_one(&client, &creator, &holder_b);
    buy_one(&client, &creator, &holder_b);

    assert_eq!(client.get_creator_supply(&creator), 4);

    set_timestamp(&env, 1_700_000_000);
    client.propose_deprecation(&admin, &creator, &1u32, &String::from_str(&env, "unwind"));

    // Advance past the notice period.
    set_timestamp(&env, 1_700_000_000 + SECONDS_PER_DAY);

    let execution = client.execute_deprecation(&admin, &creator);

    assert_eq!(execution.total_positions_unwound, 4);
    assert_eq!(execution.unwind_price, 100);
    assert_eq!(execution.amount_distributed, 400);

    // All holder positions are burned and supply is zero.
    assert_eq!(client.get_key_balance(&creator, &holder_a), 0);
    assert_eq!(client.get_key_balance(&creator, &holder_b), 0);
    assert_eq!(client.get_creator_supply(&creator), 0);
}

// ---------------------------------------------------------------------------
// 5. DeprecationExecuted emitted with positions unwound and amount distributed
// ---------------------------------------------------------------------------

#[test]
fn test_execute_deprecation_emits_event_with_totals() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "dave");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder);
    buy_one(&client, &creator, &holder);
    buy_one(&client, &creator, &holder);

    set_timestamp(&env, 1_700_000_000);
    client.propose_deprecation(&admin, &creator, &1u32, &String::from_str(&env, "test"));
    set_timestamp(&env, 1_700_000_000 + SECONDS_PER_DAY);

    client.execute_deprecation(&admin, &creator);

    let all_events = env.events().all();
    let event = all_events
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .map(|v| {
                    let sym: soroban_sdk::Symbol = v.into_val(&env);
                    sym == events::DEPRECATION_EXECUTED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("DeprecationExecuted event not found");

    let data: events::DeprecationExecutedEvent = event.2.into_val(&env);
    assert_eq!(data.key_id, creator);
    assert_eq!(data.total_positions_unwound, 3);
    assert_eq!(data.amount_distributed, 300);
    assert_eq!(data.unwind_price, 100);
}

// ---------------------------------------------------------------------------
// 3. Holders can exit normally during the notice period
// ---------------------------------------------------------------------------

#[test]
fn test_holder_can_exit_during_notice_period() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "erin");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder);
    buy_one(&client, &creator, &holder);

    set_timestamp(&env, 1_700_000_000);
    client.propose_deprecation(&admin, &creator, &5u32, &String::from_str(&env, "notice"));

    // Still inside the notice period: a normal sell succeeds at market rate.
    // Advance the ledger past the flash-loan guard window (~720 ledgers).
    set_ledger_sequence(&env, 1_000);
    set_timestamp(&env, 1_700_000_000 + 2 * SECONDS_PER_DAY);
    let new_supply = client.sell_key(&creator, &holder, &None);
    assert_eq!(new_supply, 1);
    assert_eq!(client.get_key_balance(&creator, &holder), 1);

    // The remaining position is unwound when the period elapses.
    set_timestamp(&env, 1_700_000_000 + 5 * SECONDS_PER_DAY);
    let execution = client.execute_deprecation(&admin, &creator);
    assert_eq!(execution.total_positions_unwound, 1);
    assert_eq!(execution.amount_distributed, 100);
}

// ---------------------------------------------------------------------------
// New buys are rejected once a deprecation is proposed
// ---------------------------------------------------------------------------

#[test]
fn test_buy_rejected_after_deprecation_proposed() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "frank");
    let buyer = Address::generate(&env);

    set_timestamp(&env, 1_700_000_000);
    client.propose_deprecation(&admin, &creator, &1u32, &String::from_str(&env, "x"));

    let result = client.try_buy_key(&creator, &buyer, &10_000_i128, &None);
    assert_eq!(result, Err(Ok(ContractError::KeyDeprecated)));
}

// ---------------------------------------------------------------------------
// Proposal validation
// ---------------------------------------------------------------------------

#[test]
fn test_propose_deprecation_rejects_invalid_notice_period() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "gina");

    let zero =
        client.try_propose_deprecation(&admin, &creator, &0u32, &String::from_str(&env, "x"));
    assert_eq!(zero, Err(Ok(ContractError::InvalidNoticePeriod)));

    let too_long =
        client.try_propose_deprecation(&admin, &creator, &366u32, &String::from_str(&env, "x"));
    assert_eq!(too_long, Err(Ok(ContractError::InvalidNoticePeriod)));
}

#[test]
fn test_propose_deprecation_rejects_duplicate_proposal() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "hank");

    set_timestamp(&env, 1_700_000_000);
    client.propose_deprecation(&admin, &creator, &1u32, &String::from_str(&env, "first"));

    let result =
        client.try_propose_deprecation(&admin, &creator, &2u32, &String::from_str(&env, "again"));
    assert_eq!(result, Err(Ok(ContractError::DeprecationAlreadyProposed)));
}

#[test]
fn test_execute_deprecation_rejects_second_execution() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "ivan");
    let holder = Address::generate(&env);
    buy_one(&client, &creator, &holder);

    set_timestamp(&env, 1_700_000_000);
    client.propose_deprecation(&admin, &creator, &1u32, &String::from_str(&env, "x"));
    set_timestamp(&env, 1_700_000_000 + SECONDS_PER_DAY);

    client.execute_deprecation(&admin, &creator);

    let result = client.try_execute_deprecation(&admin, &creator);
    assert_eq!(result, Err(Ok(ContractError::DeprecationAlreadyExecuted)));
}

#[test]
fn test_get_deprecation_proposal_returns_none_before_proposal() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "jane");

    assert_eq!(client.get_deprecation_proposal(&creator), None);

    set_timestamp(&env, 1_700_000_000);
    // A registered key that has never been proposed returns None.
    assert!(client.get_deprecation_proposal(&creator).is_none());
}
