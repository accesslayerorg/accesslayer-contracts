//! Issue #419 — Unit tests for key transfer sender balance decrement.
//!
//! Verifies that `transfer_keys` decrements the sender's key balance by exactly
//! the transferred amount and increments the recipient's balance by the same
//! amount, while leaving total creator supply unchanged. Every test computes an
//! explicit before/after `delta = before - after` (or `after - before` for the
//! recipient) and asserts `delta == amount` so off-by-one regressions in any
//! direction are caught immediately.
//!
//! Acceptance criteria from issue #419:
//!   1. Sender balance decremented by exactly the transfer amount.
//!   2. Recipient balance incremented by exactly the transfer amount.
//!   3. Total supply unchanged after transfer.
//!
//! ## Why relative deltas instead of absolute post-conditions
//!
//! `tests/transfer_keys.rs` already covers absolute post-conditions
//! (`sender_balance == before - 1` etc.). Those tests catch the easy case
//! where the decrement is zero or doubled, but a subtle off-by-one or partial
//! commit (e.g. `sender - amount + 1`) would still pass against hard-coded
//! constants if the test was written against an incorrect expectation. The
//! delta-based assertions in this file pin the contract against the actual
//! pre-state stored in the ledger, so any deviation — even by one key — is
//! flagged. Future contributors: please keep both perspectives.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

/// Fixed key price used by all tests in this file, matching the existing
/// `transfer_keys.rs` helper setup.
const KEY_PRICE: i128 = 100;

/// Helper: register a creator with the standard 90/10 fee split and pricing.
fn setup_pricing(client: &CreatorKeysContractClient<'_>, env: &Env) {
    set_pricing_and_fees(env, client, KEY_PRICE, 9_000, 1_000);
}

/// Helper: buy `count` keys for `buyer` under `creator`.
fn buy_keys(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    count: u32,
) {
    for _ in 0..count {
        client.buy_key(creator, buyer, &KEY_PRICE, &None);
    }
}

/// Asserts the canonical exact-delta invariant for any successful transfer:
/// the sender loses exactly `amount` and the recipient gains exactly `amount`,
/// while `get_total_key_supply` is unchanged.
//
// The eight-argument signature is intentional: capturing the pre-state of both
// parties plus total supply surfaces all four inputs the contract mutates, and
// lets the assertion be a single self-contained call. Slimming any further would
// hide the exact-delta invariant behind an opaque struct.
//
// Read-only helper, so once -7 limits on the lint are still acceptable here.
#[allow(clippy::too_many_arguments)]
fn assert_sender_recipient_exact_delta(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    sender: &Address,
    recipient: &Address,
    amount: u32,
    sender_before: u32,
    recipient_before: u32,
    supply_before: u32,
) {
    let sender_after = client.get_key_balance(creator, sender);
    let recipient_after = client.get_key_balance(creator, recipient);
    let supply_after = client.get_total_key_supply(creator);

    let sender_delta = sender_before - sender_after;
    let recipient_delta = recipient_after - recipient_before;

    assert_eq!(
        sender_delta, amount,
        "sender balance must be decremented by exactly the transfer amount"
    );
    assert_eq!(
        recipient_delta, amount,
        "recipient balance must be incremented by exactly the transfer amount"
    );
    assert_eq!(
        sender_delta, recipient_delta,
        "sender decrement must equal recipient increment (each transfer is a 1:1 move)"
    );
    assert_eq!(
        supply_after, supply_before,
        "total creator supply must be unchanged after a peer-to-peer transfer"
    );
}

/// Single-key transfer: sender has multiple keys, leaves two behind,
/// recipient starts with zero. Delta on both sides must be exactly 1.
#[test]
fn test_transfer_keys_sender_balance_decremented_by_exact_amount_single() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_pricing(&client, &env);

    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Sender starts with 5 keys; recipient starts with 0.
    buy_keys(&client, &creator, &sender, 5);

    let sender_before = client.get_key_balance(&creator, &sender);
    let recipient_before = client.get_key_balance(&creator, &recipient);
    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(sender_before, 5);
    assert_eq!(recipient_before, 0);
    assert_eq!(supply_before, 5);

    let amount: u32 = 1;
    client.transfer_keys(&creator, &sender, &recipient, &amount);

    assert_sender_recipient_exact_delta(
        &client,
        &creator,
        &sender,
        &recipient,
        amount,
        sender_before,
        recipient_before,
        supply_before,
    );

    // Sanity-check the absolute post-conditions as well.
    assert_eq!(client.get_key_balance(&creator, &sender), 4);
    assert_eq!(client.get_key_balance(&creator, &recipient), 1);
    assert_eq!(client.get_total_key_supply(&creator), 5);
}

/// Multi-key transfer: send 5 of 10 to a recipient that already has 3. The
/// sender must lose exactly 5 and the recipient must gain exactly 5; total
/// supply is untouched.
#[test]
fn test_transfer_keys_sender_balance_decremented_by_exact_amount_multi() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_pricing(&client, &env);

    let creator = register_test_creator(&env, &client, "multi");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    buy_keys(&client, &creator, &sender, 10);
    buy_keys(&client, &creator, &recipient, 3);

    let sender_before = client.get_key_balance(&creator, &sender);
    let recipient_before = client.get_key_balance(&creator, &recipient);
    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(sender_before, 10);
    assert_eq!(recipient_before, 3);
    assert_eq!(supply_before, 13);

    let amount: u32 = 5;
    client.transfer_keys(&creator, &sender, &recipient, &amount);

    assert_sender_recipient_exact_delta(
        &client,
        &creator,
        &sender,
        &recipient,
        amount,
        sender_before,
        recipient_before,
        supply_before,
    );

    assert_eq!(client.get_key_balance(&creator, &sender), 5);
    assert_eq!(client.get_key_balance(&creator, &recipient), 8);
    assert_eq!(client.get_total_key_supply(&creator), 13);
}

/// Full-drain transfer: sender transfers every key they own. The sender
/// balance must be zero and the delta on the recipient side equals the
/// (now-zeroed) sender's prior balance.
#[test]
fn test_transfer_keys_sender_zeroed_out_exact_decrement() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_pricing(&client, &env);

    let creator = register_test_creator(&env, &client, "zero");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    buy_keys(&client, &creator, &sender, 4);

    let sender_before = client.get_key_balance(&creator, &sender);
    let recipient_before = client.get_key_balance(&creator, &recipient);
    let supply_before = client.get_total_key_supply(&creator);

    let amount = sender_before; // transfer every key the sender holds
    client.transfer_keys(&creator, &sender, &recipient, &amount);

    assert_eq!(
        client.get_key_balance(&creator, &sender),
        0,
        "sender must be fully decremented to zero"
    );
    assert_eq!(
        client.get_key_balance(&creator, &recipient),
        recipient_before + amount,
        "recipient must receive exactly the transferred amount"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "supply must remain unchanged after a draining transfer"
    );
}

/// Multiple sequential transfers must accumulate on the sender side. The total
/// decrement across three calls (2 + 3 + 1) must equal 6, regardless of
/// intermediate recipient balances.
#[test]
fn test_transfer_keys_sender_decrement_accumulates_across_calls() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_pricing(&client, &env);

    let creator = register_test_creator(&env, &client, "accum");
    let sender = Address::generate(&env);
    let recipient_a = Address::generate(&env);
    let recipient_b = Address::generate(&env);

    buy_keys(&client, &creator, &sender, 10);

    let sender_before = client.get_key_balance(&creator, &sender);
    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(sender_before, 10);

    client.transfer_keys(&creator, &sender, &recipient_a, &2);
    client.transfer_keys(&creator, &sender, &recipient_b, &3);
    client.transfer_keys(&creator, &sender, &recipient_a, &1);

    let sender_after = client.get_key_balance(&creator, &sender);
    let total_transferred: u32 = 2 + 3 + 1;

    assert_eq!(
        sender_before - sender_after,
        total_transferred,
        "sender decrement must equal the sum of transfer amounts across calls"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "supply must remain unchanged across sequential transfer_keys calls"
    );
    assert_eq!(
        client.get_key_balance(&creator, &recipient_a),
        3,
        "recipient A receives exactly 2 + 1 = 3 keys"
    );
    assert_eq!(
        client.get_key_balance(&creator, &recipient_b),
        3,
        "recipient B receives exactly 3 keys"
    );
}

/// Sender and recipient balances must be isolated across creators. A transfer
/// under creator A must not influence sender/recipient balances under creator B.
#[test]
fn test_transfer_keys_sender_recipient_deltas_isolated_per_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_pricing(&client, &env);

    let creator_a = register_test_creator(&env, &client, "alpha");
    let creator_b = register_test_creator(&env, &client, "beta");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    buy_keys(&client, &creator_a, &sender, 7);
    buy_keys(&client, &creator_b, &sender, 6);

    let sender_a_before = client.get_key_balance(&creator_a, &sender);
    let sender_b_before = client.get_key_balance(&creator_b, &sender);
    let recipient_a_before = client.get_key_balance(&creator_a, &recipient);
    let recipient_b_before = client.get_key_balance(&creator_b, &recipient);
    let supply_a_before = client.get_total_key_supply(&creator_a);
    let supply_b_before = client.get_total_key_supply(&creator_b);

    let amount: u32 = 4;
    client.transfer_keys(&creator_a, &sender, &recipient, &amount);

    let sender_a_after = client.get_key_balance(&creator_a, &sender);
    let sender_b_after = client.get_key_balance(&creator_b, &sender);
    let recipient_a_after = client.get_key_balance(&creator_a, &recipient);
    let recipient_b_after = client.get_key_balance(&creator_b, &recipient);

    assert_eq!(
        sender_a_before - sender_a_after,
        amount,
        "creator A sender must lose exactly the transferred amount"
    );
    assert_eq!(
        sender_b_before, sender_b_after,
        "creator B sender balance must be untouched"
    );
    assert_eq!(
        recipient_a_after - recipient_a_before,
        amount,
        "creator A recipient must gain exactly the transferred amount"
    );
    assert_eq!(
        recipient_b_before, recipient_b_after,
        "creator B recipient balance must be untouched"
    );
    assert_eq!(
        client.get_total_key_supply(&creator_a),
        supply_a_before,
        "creator A total supply must be unchanged"
    );
    assert_eq!(
        client.get_total_key_supply(&creator_b),
        supply_b_before,
        "creator B total supply must be unchanged"
    );
}

/// A transferKeys call that reverts (insufficient balance) must not change any
/// sender or recipient balance, and must leave total supply unchanged.
#[test]
fn test_transfer_keys_reverted_call_does_not_decrement_sender() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    setup_pricing(&client, &env);

    let creator = register_test_creator(&env, &client, "rejected");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    buy_keys(&client, &creator, &sender, 3);

    let sender_before = client.get_key_balance(&creator, &sender);
    let recipient_before = client.get_key_balance(&creator, &recipient);
    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(sender_before, 3);
    assert_eq!(recipient_before, 0);

    // Attempting to transfer 10 keys when sender only holds 3 must revert.
    let result = client.try_transfer_keys(&creator, &sender, &recipient, &10u32);
    assert!(
        result.is_err(),
        "transfer with insufficient balance must revert"
    );

    assert_eq!(
        client.get_key_balance(&creator, &sender),
        sender_before,
        "sender balance must be unchanged on a rejected transfer"
    );
    assert_eq!(
        client.get_key_balance(&creator, &recipient),
        recipient_before,
        "recipient balance must be unchanged on a rejected transfer"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "total supply must remain unchanged after a reverted transfer"
    );
}
