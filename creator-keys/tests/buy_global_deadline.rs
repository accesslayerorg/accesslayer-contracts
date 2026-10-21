//! Integration tests for the global deadline ledger gating buys (#737).
//!
//! The protocol admin can pin a ledger sequence after which no further buys are
//! accepted, so stale orders cannot execute once a sale window has closed. The
//! boundary is exclusive: the deadline ledger itself is already too late.
//!
//! These tests drive that rule end to end — set the deadline to the current
//! ledger and prove the buy is rejected with `DeadlinePassed` and leaves no
//! trace in supply, holder count or balances; then move the deadline ten
//! ledgers out and prove the very next buy succeeds.

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, register_creator_keys, register_test_creator, set_ledger_sequence,
    set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// Ledger sequence the tests start from, so deadlines can be set on either
/// side of "now" without underflowing.
const START_LEDGER: u32 = 1_000;

/// Deploy the contract with pricing, fees, a protocol admin and one creator.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");
    set_ledger_sequence(env, START_LEDGER);
    (client, admin, creator)
}

/// Buy a single key for `buyer` at the current quoted price.
fn buy_one(client: &CreatorKeysContractClient<'_>, creator: &Address, buyer: &Address) -> u32 {
    let quote = client.get_buy_quote(creator);
    client.buy_key(creator, buyer, &quote.total_amount, &None)
}

// ---------------------------------------------------------------------------
// Full lifecycle: deadline blocks the buy, extending it lets the buy through
// ---------------------------------------------------------------------------

#[test]
fn test_buy_rejected_at_deadline_then_accepted_once_deadline_moves_out() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    // Deadline lands exactly on the current ledger, so the window is closed.
    client.set_global_deadline(&admin, &Some(START_LEDGER));
    assert_eq!(client.get_global_deadline(), Some(START_LEDGER));

    let before = capture_snapshot(&client, &creator, &buyer);
    let quote = client.get_buy_quote(&creator);

    assert_eq!(
        client.try_buy_key(&creator, &buyer, &quote.total_amount, &None),
        Err(Ok(ContractError::DeadlinePassed)),
        "buy must be rejected once the current ledger has reached the deadline"
    );

    // The rejected buy must not have moved supply, holder count or balance.
    before.assert_unchanged(&capture_snapshot(&client, &creator, &buyer));

    // Admin pushes the deadline ten ledgers into the future.
    client.set_global_deadline(&admin, &Some(START_LEDGER + 10));
    assert_eq!(client.get_global_deadline(), Some(START_LEDGER + 10));

    // The very next buy succeeds — no cooldown, no re-initialisation.
    assert_eq!(
        buy_one(&client, &creator, &buyer),
        before.supply + 1,
        "buy must succeed while the current ledger is below the deadline"
    );
    assert_eq!(
        client.get_key_balance(&creator, &buyer),
        before.key_balance + 1
    );
}

// ---------------------------------------------------------------------------
// The boundary itself
// ---------------------------------------------------------------------------

#[test]
fn test_buy_accepted_on_the_last_ledger_before_the_deadline() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    // Deadline is one ledger away: this is the last ledger that still trades.
    client.set_global_deadline(&admin, &Some(START_LEDGER + 1));

    assert_eq!(buy_one(&client, &creator, &buyer), 1);
}

#[test]
fn test_buy_rejected_after_the_ledger_advances_past_the_deadline() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    client.set_global_deadline(&admin, &Some(START_LEDGER + 1));

    // One buy lands inside the window.
    assert_eq!(buy_one(&client, &creator, &buyer), 1);

    // Time passes and the window closes behind it.
    set_ledger_sequence(&env, START_LEDGER + 5);
    let before = capture_snapshot(&client, &creator, &buyer);
    let quote = client.get_buy_quote(&creator);

    assert_eq!(
        client.try_buy_key(&creator, &buyer, &quote.total_amount, &None),
        Err(Ok(ContractError::DeadlinePassed)),
        "buy must be rejected once the ledger has advanced past the deadline"
    );
    before.assert_unchanged(&capture_snapshot(&client, &creator, &buyer));
}

// ---------------------------------------------------------------------------
// Absent and cleared deadlines never gate a buy
// ---------------------------------------------------------------------------

#[test]
fn test_buy_unaffected_when_no_deadline_is_configured() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    assert_eq!(
        client.get_global_deadline(),
        None,
        "no deadline may be configured by default"
    );

    assert_eq!(buy_one(&client, &creator, &buyer), 1);
}

#[test]
fn test_clearing_the_deadline_reopens_buying() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    client.set_global_deadline(&admin, &Some(START_LEDGER));
    let quote = client.get_buy_quote(&creator);
    assert_eq!(
        client.try_buy_key(&creator, &buyer, &quote.total_amount, &None),
        Err(Ok(ContractError::DeadlinePassed))
    );

    client.set_global_deadline(&admin, &None);
    assert_eq!(client.get_global_deadline(), None);

    assert_eq!(buy_one(&client, &creator, &buyer), 1);
}

// ---------------------------------------------------------------------------
// Only the admin may move the deadline
// ---------------------------------------------------------------------------

#[test]
fn test_only_admin_can_set_the_global_deadline() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let stranger = Address::generate(&env);
    let buyer = Address::generate(&env);

    assert_eq!(
        client.try_set_global_deadline(&stranger, &Some(START_LEDGER)),
        Err(Ok(ContractError::Unauthorized)),
        "a non-admin must not be able to set the global deadline"
    );

    // The failed attempt must leave buying open.
    assert_eq!(client.get_global_deadline(), None);
    assert_eq!(buy_one(&client, &creator, &buyer), 1);
}
