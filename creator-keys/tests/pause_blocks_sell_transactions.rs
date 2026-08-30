//! Unit tests for the contract paused state correctly blocking sell transactions (#727).
//!
//! When the contract is paused, sell transactions should panic with
//! `ProtocolPaused` just like buy transactions. These tests confirm the
//! pause blocks sells and the resume unblocks them, while verifying that
//! holder count and supply remain unchanged after a blocked sell.
//!
//! Acceptance criteria:
//!   1. Sell panics with `ProtocolPaused` when paused.
//!   2. Sell succeeds after resume.
//!   3. State (holder count, supply, key balance) unchanged after blocked sell.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// Shared setup: deploy the contract, configure pricing and fees, register a
/// creator, and return (client, admin, creator).
fn setup(
    env: &soroban_sdk::Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    Address,
    Address,
) {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");
    (client, admin, creator)
}

// ---------------------------------------------------------------------------
// Acceptance criteria #1: Sell panics with ProtocolPaused when paused
// ---------------------------------------------------------------------------

#[test]
fn test_sell_panics_with_protocol_paused_when_contract_is_paused() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let seller = Address::generate(&env);

    // Buy a key before the pause so the seller has something to sell.
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &seller, &quote.total_amount, &None);
    assert_eq!(client.get_total_key_supply(&creator), 1);

    // Pause the contract.
    client.pause(&admin);
    assert!(client.get_is_paused());

    // Attempt to sell — must revert with ProtocolPaused.
    let result = client.try_sell_key(&creator, &seller, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::ProtocolPaused)),
        "sell must panic with ProtocolPaused when the contract is paused"
    );
}

// ---------------------------------------------------------------------------
// Acceptance criteria #2: Sell succeeds after resume
// ---------------------------------------------------------------------------

#[test]
fn test_sell_succeeds_after_resume() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let seller = Address::generate(&env);

    // Buy two keys before the pause so the seller has keys to sell.
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &seller, &quote.total_amount, &None);
    let quote2 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &seller, &quote2.total_amount, &None);
    assert_eq!(client.get_total_key_supply(&creator), 2);
    assert_eq!(client.get_key_balance(&creator, &seller), 2);

    // Pause the contract — sell must fail.
    client.pause(&admin);
    assert!(client.get_is_paused());
    let paused_result = client.try_sell_key(&creator, &seller, &None);
    assert_eq!(paused_result, Err(Ok(ContractError::ProtocolPaused)));

    // Resume the contract.
    client.unpause(&admin);
    assert!(!client.get_is_paused());

    // Sell must succeed immediately after unpause.
    let new_supply = client.sell_key(&creator, &seller, &None);
    assert_eq!(
        new_supply, 1,
        "sell after unpause must succeed and decrement supply by 1"
    );
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &seller), 1);
}

// ---------------------------------------------------------------------------
// Acceptance criteria #3: State unchanged after blocked sell
// ---------------------------------------------------------------------------

#[test]
fn test_holder_count_unchanged_after_blocked_sell() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let seller = Address::generate(&env);

    // Buy a key before the pause.
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &seller, &quote.total_amount, &None);

    let supply_before = client.get_total_key_supply(&creator);
    let holder_count_before = client.get_creator_holder_count(&creator);
    let balance_before = client.get_key_balance(&creator, &seller);

    assert_eq!(supply_before, 1);
    assert_eq!(holder_count_before, 1);
    assert_eq!(balance_before, 1);

    // Pause the contract.
    client.pause(&admin);

    // Attempted sell must fail.
    let result = client.try_sell_key(&creator, &seller, &None);
    assert_eq!(result, Err(Ok(ContractError::ProtocolPaused)));

    // All state must be unchanged after the blocked sell.
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "supply must not change after a blocked sell"
    );
    assert_eq!(
        client.get_creator_holder_count(&creator),
        holder_count_before,
        "holder count must not change after a blocked sell"
    );
    assert_eq!(
        client.get_key_balance(&creator, &seller),
        balance_before,
        "key balance must not change after a blocked sell"
    );
}

#[test]
fn test_supply_unchanged_after_blocked_sell_with_multiple_holders() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    // Two different holders each buy a key before the pause.
    let quote_a = client.get_buy_quote(&creator);
    client.buy_key(&creator, &holder_a, &quote_a.total_amount, &None);
    let quote_b = client.get_buy_quote(&creator);
    client.buy_key(&creator, &holder_b, &quote_b.total_amount, &None);

    let supply_before = client.get_total_key_supply(&creator);
    let holder_count_before = client.get_creator_holder_count(&creator);
    let balance_a_before = client.get_key_balance(&creator, &holder_a);
    let balance_b_before = client.get_key_balance(&creator, &holder_b);

    assert_eq!(supply_before, 2);
    assert_eq!(holder_count_before, 2);
    assert_eq!(balance_a_before, 1);
    assert_eq!(balance_b_before, 1);

    // Pause the contract.
    client.pause(&admin);

    // Both holders attempt to sell — both must fail.
    let result_a = client.try_sell_key(&creator, &holder_a, &None);
    let result_b = client.try_sell_key(&creator, &holder_b, &None);
    assert_eq!(result_a, Err(Ok(ContractError::ProtocolPaused)));
    assert_eq!(result_b, Err(Ok(ContractError::ProtocolPaused)));

    // All state must be unchanged.
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "supply must not change after blocked sells"
    );
    assert_eq!(
        client.get_creator_holder_count(&creator),
        holder_count_before,
        "holder count must not change after blocked sells"
    );
    assert_eq!(
        client.get_key_balance(&creator, &holder_a),
        balance_a_before,
        "holder A balance must not change after blocked sell"
    );
    assert_eq!(
        client.get_key_balance(&creator, &holder_b),
        balance_b_before,
        "holder B balance must not change after blocked sell"
    );
}

// ---------------------------------------------------------------------------
// Combined lifecycle: pause blocks sells, resume restores, state consistent
// ---------------------------------------------------------------------------

#[test]
fn test_full_pause_sell_lifecycle() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let seller = Address::generate(&env);

    // Buy two keys before the pause.
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &seller, &quote.total_amount, &None);
    let quote2 = client.get_buy_quote(&creator);
    client.buy_key(&creator, &seller, &quote2.total_amount, &None);

    let supply_before_pause = client.get_total_key_supply(&creator);
    let holder_count_before_pause = client.get_creator_holder_count(&creator);
    let balance_before_pause = client.get_key_balance(&creator, &seller);
    assert_eq!(supply_before_pause, 2);
    assert_eq!(holder_count_before_pause, 1);
    assert_eq!(balance_before_pause, 2);

    // --- Phase 1: Pause the contract ---
    client.pause(&admin);
    assert!(client.get_is_paused());

    // Sell must fail while paused.
    let paused_result = client.try_sell_key(&creator, &seller, &None);
    assert_eq!(paused_result, Err(Ok(ContractError::ProtocolPaused)));

    // State must be unchanged after the blocked sell.
    assert_eq!(client.get_total_key_supply(&creator), supply_before_pause);
    assert_eq!(
        client.get_creator_holder_count(&creator),
        holder_count_before_pause
    );
    assert_eq!(
        client.get_key_balance(&creator, &seller),
        balance_before_pause
    );

    // --- Phase 2: Resume the contract ---
    client.unpause(&admin);
    assert!(!client.get_is_paused());

    // Sell must succeed immediately after unpause.
    let supply_after_sell = client.sell_key(&creator, &seller, &None);
    assert_eq!(
        supply_after_sell,
        supply_before_pause - 1,
        "supply must decrease by exactly 1 after a successful sell"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before_pause - 1
    );
    assert_eq!(
        client.get_creator_holder_count(&creator),
        holder_count_before_pause,
        "holder count must remain the same since seller still has keys"
    );
    assert_eq!(
        client.get_key_balance(&creator, &seller),
        balance_before_pause - 1
    );
}
