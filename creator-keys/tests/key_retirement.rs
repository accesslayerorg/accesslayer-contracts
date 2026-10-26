//! Integration tests for the admin-funded key retirement contract (Issue #1111).
//!
//! Acceptance criteria verified:
//!
//! 1. `execute_retirement` reverts when buyback pool is insufficient to cover all supply.
//! 2. All outstanding supply is bought back correctly at `buyback_price`.
//! 3. Holders can sell normally during the 7-day notice window.
//! 4. `RetirementProposed` is emitted with correct price and reason.
//! 5. `RetirementExecuted` is emitted with total supply bought back and amount paid.
//!
//! Additional cases:
//! - `propose_retirement` rejects non-admins.
//! - `propose_retirement` rejects zero/negative buyback price.
//! - `propose_retirement` rejects an already-proposed key.
//! - `execute_retirement` reverts before the 7-day window elapses.
//! - `execute_retirement` rejects non-admins.
//! - `execute_retirement` reverts when no proposal exists.
//! - `fund_retirement_buyback` rejects non-admins and zero amounts.
//! - `get_retirement_proposal` and `get_retirement_pool_balance` return correct values.

use creator_keys::{
    events, ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    Address, Env, IntoVal, String,
};

const SECONDS_PER_DAY: u64 = 86_400;
const NOTICE_WINDOW_SECS: u64 = 7 * SECONDS_PER_DAY;

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
    // Flat price of 100 per key; no slope so every key costs exactly 100.
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

fn set_timestamp(env: &Env, ts: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = ts;
    env.ledger().set(ledger);
}

// ---------------------------------------------------------------------------
// 4. RetirementProposed emitted with correct price and reason
// ---------------------------------------------------------------------------

#[test]
fn test_retirement_proposed_event_fields() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "alice");

    let start = 1_700_000_000u64;
    set_timestamp(&env, start);

    let reason = String::from_str(&env, "shutting down alice key");
    client.propose_retirement(&admin, &creator, &200_i128, &reason);

    // Find the RetirementProposed event.
    let all_events = env.events().all();
    let event = all_events
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .map(|v| {
                    let sym: soroban_sdk::Symbol = v.into_val(&env);
                    sym == events::RETIREMENT_PROPOSED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("RetirementProposed event not found");

    let data: events::RetirementProposedEvent = event.2.into_val(&env);
    assert_eq!(data.key_id, creator);
    assert_eq!(data.buyback_price, 200_i128);
    assert_eq!(
        data.reason,
        String::from_str(&env, "shutting down alice key")
    );
    assert_eq!(data.window_ends_at, start + NOTICE_WINDOW_SECS);
}

// ---------------------------------------------------------------------------
// 3. Holders can sell normally during the 7-day notice window
// ---------------------------------------------------------------------------

#[test]
fn test_holders_can_sell_during_notice_window() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "bob");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder);
    assert_eq!(client.get_creator_supply(&creator), 1);

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(
        &admin,
        &creator,
        &100_i128,
        &String::from_str(&env, "retire"),
    );

    // Advance to day 3 — still inside the window.
    // Also advance the ledger sequence past the flash-loan guard window (~720 ledgers).
    set_timestamp(&env, 1_700_000_000 + 3 * SECONDS_PER_DAY);
    {
        let mut ledger = env.ledger().get();
        ledger.sequence_number = 1_000;
        env.ledger().set(ledger);
    }

    // Normal sell should succeed during notice window.
    client.sell_key(&creator, &holder, &None);
    assert_eq!(client.get_creator_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
}

// ---------------------------------------------------------------------------
// 1. execute_retirement reverts when pool insufficient
// ---------------------------------------------------------------------------

#[test]
fn test_execute_retirement_reverts_when_pool_insufficient() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "charlie");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder); // supply = 1, needs 100 in pool

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(
        &admin,
        &creator,
        &100_i128,
        &String::from_str(&env, "retire"),
    );

    // Fund only 50 — insufficient for 1 key @ 100.
    client.fund_retirement_buyback(&admin, &creator, &50_i128);

    // Advance past the 7-day window.
    set_timestamp(&env, 1_700_000_000 + NOTICE_WINDOW_SECS + 1);

    let result = client.try_execute_retirement(&admin, &creator);
    assert_eq!(result, Err(Ok(ContractError::RetirementPoolInsufficient)));

    // Supply is untouched after the reverted call.
    assert_eq!(client.get_creator_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &holder), 1);
}

// ---------------------------------------------------------------------------
// 2. All outstanding supply bought back correctly at buyback_price
// ---------------------------------------------------------------------------

#[test]
fn test_execute_retirement_buys_back_all_supply() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "dana");
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    buy_one(&client, &creator, &holder_a); // supply = 1
    buy_one(&client, &creator, &holder_a); // supply = 2
    buy_one(&client, &creator, &holder_b); // supply = 3

    // 3 keys × 100 = 300 required in pool.
    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(
        &admin,
        &creator,
        &100_i128,
        &String::from_str(&env, "retire dana"),
    );
    client.fund_retirement_buyback(&admin, &creator, &300_i128);

    // Advance past 7-day window.
    set_timestamp(&env, 1_700_000_000 + NOTICE_WINDOW_SECS + 1);

    let result = client.execute_retirement(&admin, &creator);

    assert_eq!(result.total_supply_bought_back, 3);
    assert_eq!(result.total_amount_paid, 300_i128);
    assert_eq!(result.buyback_price, 100_i128);

    // Supply is now zero.
    assert_eq!(client.get_creator_supply(&creator), 0);

    // Holder balances cleared.
    assert_eq!(client.get_key_balance(&creator, &holder_a), 0);
    assert_eq!(client.get_key_balance(&creator, &holder_b), 0);
}

// ---------------------------------------------------------------------------
// 5. RetirementExecuted emitted with correct fields
// ---------------------------------------------------------------------------

#[test]
fn test_retirement_executed_event_fields() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "eve");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder);

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(&admin, &creator, &100_i128, &String::from_str(&env, "test"));
    client.fund_retirement_buyback(&admin, &creator, &100_i128);

    set_timestamp(&env, 1_700_000_000 + NOTICE_WINDOW_SECS + 1);
    client.execute_retirement(&admin, &creator);

    let all_events = env.events().all();
    let event = all_events
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .map(|v| {
                    let sym: soroban_sdk::Symbol = v.into_val(&env);
                    sym == events::RETIREMENT_EXECUTED_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .expect("RetirementExecuted event not found");

    let data: events::RetirementExecutedEvent = event.2.into_val(&env);
    assert_eq!(data.key_id, creator);
    assert_eq!(data.total_supply_bought_back, 1);
    assert_eq!(data.total_amount_paid, 100_i128);
    assert_eq!(data.buyback_price, 100_i128);
}

// ---------------------------------------------------------------------------
// execute_retirement reverts before the 7-day window elapses
// ---------------------------------------------------------------------------

#[test]
fn test_execute_retirement_reverts_before_window_elapses() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "frank");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder);

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(
        &admin,
        &creator,
        &100_i128,
        &String::from_str(&env, "retire"),
    );
    client.fund_retirement_buyback(&admin, &creator, &100_i128);

    // Advance only 3 days — still inside the 7-day window.
    set_timestamp(&env, 1_700_000_000 + 3 * SECONDS_PER_DAY);

    let result = client.try_execute_retirement(&admin, &creator);
    assert_eq!(result, Err(Ok(ContractError::RetirementWindowActive)));
}

// ---------------------------------------------------------------------------
// propose_retirement rejects non-admin
// ---------------------------------------------------------------------------

#[test]
fn test_propose_retirement_rejects_non_admin() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "grace");
    let attacker = Address::generate(&env);

    let result =
        client.try_propose_retirement(&attacker, &creator, &100_i128, &String::from_str(&env, "x"));
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ---------------------------------------------------------------------------
// propose_retirement rejects zero buyback price
// ---------------------------------------------------------------------------

#[test]
fn test_propose_retirement_rejects_zero_price() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "hank");

    let result =
        client.try_propose_retirement(&admin, &creator, &0_i128, &String::from_str(&env, "x"));
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

// ---------------------------------------------------------------------------
// propose_retirement rejects duplicate proposal
// ---------------------------------------------------------------------------

#[test]
fn test_propose_retirement_rejects_duplicate() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "iris");

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(
        &admin,
        &creator,
        &100_i128,
        &String::from_str(&env, "first"),
    );

    let result = client.try_propose_retirement(
        &admin,
        &creator,
        &100_i128,
        &String::from_str(&env, "second"),
    );
    assert_eq!(result, Err(Ok(ContractError::RetirementAlreadyProposed)));
}

// ---------------------------------------------------------------------------
// execute_retirement rejects non-admin
// ---------------------------------------------------------------------------

#[test]
fn test_execute_retirement_rejects_non_admin() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "jade");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder);

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(&admin, &creator, &100_i128, &String::from_str(&env, "x"));
    client.fund_retirement_buyback(&admin, &creator, &100_i128);

    set_timestamp(&env, 1_700_000_000 + NOTICE_WINDOW_SECS + 1);

    let attacker = Address::generate(&env);
    let result = client.try_execute_retirement(&attacker, &creator);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ---------------------------------------------------------------------------
// execute_retirement reverts when no proposal exists
// ---------------------------------------------------------------------------

#[test]
fn test_execute_retirement_without_proposal_reverts() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "kira");

    let result = client.try_execute_retirement(&admin, &creator);
    assert_eq!(result, Err(Ok(ContractError::NoRetirementProposed)));
}

// ---------------------------------------------------------------------------
// fund_retirement_buyback rejects non-admin
// ---------------------------------------------------------------------------

#[test]
fn test_fund_retirement_buyback_rejects_non_admin() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "lena");

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(&admin, &creator, &100_i128, &String::from_str(&env, "x"));

    let attacker = Address::generate(&env);
    let result = client.try_fund_retirement_buyback(&attacker, &creator, &100_i128);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ---------------------------------------------------------------------------
// fund_retirement_buyback rejects zero amount
// ---------------------------------------------------------------------------

#[test]
fn test_fund_retirement_buyback_rejects_zero_amount() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "mike");

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(&admin, &creator, &100_i128, &String::from_str(&env, "x"));

    let result = client.try_fund_retirement_buyback(&admin, &creator, &0_i128);
    assert_eq!(result, Err(Ok(ContractError::InvalidFundAmount)));
}

// ---------------------------------------------------------------------------
// fund_retirement_buyback rejects funding a non-existent proposal
// ---------------------------------------------------------------------------

#[test]
fn test_fund_retirement_buyback_rejects_no_proposal() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "nina");

    let result = client.try_fund_retirement_buyback(&admin, &creator, &500_i128);
    assert_eq!(result, Err(Ok(ContractError::NoRetirementProposed)));
}

// ---------------------------------------------------------------------------
// get_retirement_proposal returns correct data
// ---------------------------------------------------------------------------

#[test]
fn test_get_retirement_proposal_returns_proposal() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "omar");

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(
        &admin,
        &creator,
        &250_i128,
        &String::from_str(&env, "the reason"),
    );

    let proposal = client
        .get_retirement_proposal(&creator)
        .expect("proposal should be present");

    assert_eq!(proposal.key_id, creator);
    assert_eq!(proposal.buyback_price, 250_i128);
    assert_eq!(proposal.reason, String::from_str(&env, "the reason"));
    assert_eq!(proposal.window_ends_at, 1_700_000_000 + NOTICE_WINDOW_SECS);
    assert!(!proposal.executed);
}

// ---------------------------------------------------------------------------
// get_retirement_proposal returns None when no proposal exists
// ---------------------------------------------------------------------------

#[test]
fn test_get_retirement_proposal_returns_none_when_absent() {
    let (env, client, _admin) = setup();
    let creator = register_creator(&env, &client, "pam");

    assert!(client.get_retirement_proposal(&creator).is_none());
}

// ---------------------------------------------------------------------------
// get_retirement_pool_balance accumulates across multiple fund calls
// ---------------------------------------------------------------------------

#[test]
fn test_get_retirement_pool_balance_accumulates() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "quinn");

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(&admin, &creator, &100_i128, &String::from_str(&env, "x"));

    assert_eq!(client.get_retirement_pool_balance(&creator), 0_i128);

    client.fund_retirement_buyback(&admin, &creator, &150_i128);
    assert_eq!(client.get_retirement_pool_balance(&creator), 150_i128);

    client.fund_retirement_buyback(&admin, &creator, &50_i128);
    assert_eq!(client.get_retirement_pool_balance(&creator), 200_i128);
}

// ---------------------------------------------------------------------------
// execute_retirement marks proposal as executed
// ---------------------------------------------------------------------------

#[test]
fn test_execute_retirement_marks_proposal_executed() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "rita");
    let holder = Address::generate(&env);

    buy_one(&client, &creator, &holder);

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(&admin, &creator, &100_i128, &String::from_str(&env, "done"));
    client.fund_retirement_buyback(&admin, &creator, &100_i128);

    set_timestamp(&env, 1_700_000_000 + NOTICE_WINDOW_SECS + 1);
    client.execute_retirement(&admin, &creator);

    // Second call must return RetirementAlreadyExecuted.
    let result = client.try_execute_retirement(&admin, &creator);
    assert_eq!(result, Err(Ok(ContractError::RetirementAlreadyExecuted)));

    // Proposal is still readable but marked executed.
    let proposal = client
        .get_retirement_proposal(&creator)
        .expect("proposal still readable after execution");
    assert!(proposal.executed);
}

// ---------------------------------------------------------------------------
// execute_retirement with zero supply succeeds trivially
// ---------------------------------------------------------------------------

#[test]
fn test_execute_retirement_zero_supply_succeeds() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "sam");

    // No keys purchased — supply is 0, pool need = 0.
    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(
        &admin,
        &creator,
        &100_i128,
        &String::from_str(&env, "empty"),
    );
    // No funding needed when supply is 0.

    set_timestamp(&env, 1_700_000_000 + NOTICE_WINDOW_SECS + 1);
    let result = client.execute_retirement(&admin, &creator);

    assert_eq!(result.total_supply_bought_back, 0);
    assert_eq!(result.total_amount_paid, 0_i128);
}

// ---------------------------------------------------------------------------
// Partial exit: one holder sells during window; remaining bought back
// ---------------------------------------------------------------------------

#[test]
fn test_partial_exit_during_window_then_buyback() {
    let (env, client, admin) = setup();
    let creator = register_creator(&env, &client, "tara");
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    buy_one(&client, &creator, &holder_a); // supply = 1
    buy_one(&client, &creator, &holder_b); // supply = 2

    set_timestamp(&env, 1_700_000_000);
    client.propose_retirement(
        &admin,
        &creator,
        &100_i128,
        &String::from_str(&env, "partial test"),
    );
    // Fund for worst-case 2 keys.
    client.fund_retirement_buyback(&admin, &creator, &200_i128);

    // holder_a exits voluntarily during the window.
    // Advance ledger sequence past the flash-loan guard window before selling.
    set_timestamp(&env, 1_700_000_000 + 2 * SECONDS_PER_DAY);
    {
        let mut ledger = env.ledger().get();
        ledger.sequence_number = 1_000;
        env.ledger().set(ledger);
    }
    client.sell_key(&creator, &holder_a, &None);
    assert_eq!(client.get_creator_supply(&creator), 1);

    // Advance past window — only holder_b remains.
    set_timestamp(&env, 1_700_000_000 + NOTICE_WINDOW_SECS + 1);
    let result = client.execute_retirement(&admin, &creator);

    assert_eq!(result.total_supply_bought_back, 1);
    assert_eq!(result.total_amount_paid, 100_i128);
    assert_eq!(client.get_key_balance(&creator, &holder_b), 0);
    assert_eq!(client.get_creator_supply(&creator), 0);
}
