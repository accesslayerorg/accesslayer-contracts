//! Integration tests for the emergency pause halting all trading (#674).
//!
//! The pause is an emergency brake: while it is active every state-changing
//! trade must be rejected, but the protocol must stay readable so clients can
//! still display prices and balances. These tests walk that contract end to
//! end — pause as admin, prove buy and sell are rejected, prove the price
//! reads still answer, then lift the pause and prove trading resumes on the
//! very next call.
//!
//! The contract exposes pricing through the quote views (`get_buy_quote`,
//! `get_sell_quote`, `get_buyback_quote`) rather than a single `get_price`
//! entrypoint, so those are the read paths asserted here.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// Deploy the contract with pricing, fees, a protocol admin and one creator.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");
    (client, admin, creator)
}

/// Buy a single key for `buyer` at the current quoted price.
fn buy_one(client: &CreatorKeysContractClient<'_>, creator: &Address, buyer: &Address) -> u32 {
    let quote = client.get_buy_quote(creator);
    client.buy_key(creator, buyer, &quote.total_amount, &None)
}

// ---------------------------------------------------------------------------
// Full lifecycle: pause blocks trading, reads stay live, unpause restores both
// ---------------------------------------------------------------------------

#[test]
fn test_pause_blocks_buy_and_sell_then_unpause_restores_trading() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let trader = Address::generate(&env);

    // A holder is needed so the sell attempt fails on the pause, not on balance.
    buy_one(&client, &creator, &trader);
    let supply_before = client.get_total_key_supply(&creator);
    let balance_before = client.get_key_balance(&creator, &trader);
    let quote_before = client.get_buy_quote(&creator);

    // Admin activates the emergency pause.
    client.pause(&admin);
    assert!(client.get_is_paused(), "pause must set the paused flag");

    // Buy is rejected.
    assert_eq!(
        client.try_buy_key(&creator, &trader, &quote_before.total_amount, &None),
        Err(Ok(ContractError::ProtocolPaused)),
        "buy must be rejected with ProtocolPaused while paused"
    );

    // Sell is rejected.
    assert_eq!(
        client.try_sell_key(&creator, &trader, &None),
        Err(Ok(ContractError::ProtocolPaused)),
        "sell must be rejected with ProtocolPaused while paused"
    );

    // Price reads keep working, and report the same numbers as before the pause.
    let quote_while_paused = client.get_buy_quote(&creator);
    assert_eq!(
        quote_while_paused, quote_before,
        "buy quote must be unaffected by the pause"
    );
    let sell_quote_while_paused = client.get_sell_quote(&creator, &trader);
    assert_eq!(sell_quote_while_paused.price, KEY_PRICE);

    // Neither rejected trade may have moved any state.
    assert_eq!(client.get_total_key_supply(&creator), supply_before);
    assert_eq!(client.get_key_balance(&creator, &trader), balance_before);

    // Admin lifts the pause.
    client.unpause(&admin);
    assert!(
        !client.get_is_paused(),
        "unpause must clear the paused flag"
    );

    // The very next buy succeeds — no cooldown, no re-initialisation.
    let supply_after_buy = buy_one(&client, &creator, &trader);
    assert_eq!(
        supply_after_buy,
        supply_before + 1,
        "buy must succeed immediately after the pause is lifted"
    );
    assert_eq!(
        client.get_key_balance(&creator, &trader),
        balance_before + 1
    );

    // Selling works again too, returning the holder to the pre-pause baseline.
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    let supply_after_sell = client.sell_key(&creator, &trader, &None);
    assert_eq!(supply_after_sell, supply_before);
    assert_eq!(client.get_key_balance(&creator, &trader), balance_before);
}

// ---------------------------------------------------------------------------
// Read-only price views are not blocked by the pause
// ---------------------------------------------------------------------------

#[test]
fn test_buy_quote_succeeds_while_paused() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);

    let before = client.get_buy_quote(&creator);
    client.pause(&admin);

    let during = client.get_buy_quote(&creator);
    assert_eq!(
        during, before,
        "get_buy_quote must succeed and return unchanged pricing while paused"
    );
    assert_eq!(during.price, KEY_PRICE);
    assert_eq!(
        during.total_amount,
        during.price + during.creator_fee + during.protocol_fee,
        "the fee breakdown must still add up while paused"
    );
}

#[test]
fn test_sell_quote_succeeds_while_paused() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let holder = Address::generate(&env);
    buy_one(&client, &creator, &holder);

    let before = client.get_sell_quote(&creator, &holder);
    client.pause(&admin);

    let during = client.get_sell_quote(&creator, &holder);
    assert_eq!(
        during, before,
        "get_sell_quote must succeed and return unchanged pricing while paused"
    );
}

#[test]
fn test_buyback_quote_succeeds_while_paused() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let holder = Address::generate(&env);
    buy_one(&client, &creator, &holder);

    let before = client.get_buyback_quote(&creator, &1);
    client.pause(&admin);

    assert_eq!(
        client.get_buyback_quote(&creator, &1),
        before,
        "get_buyback_quote must succeed and return unchanged pricing while paused"
    );
}

#[test]
fn test_quote_views_still_report_errors_normally_while_paused() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let stranger = Address::generate(&env);

    client.pause(&admin);

    // A holder with no keys must still get InsufficientBalance, not ProtocolPaused:
    // the pause must not mask the read's own error reporting.
    assert_eq!(
        client.try_get_sell_quote(&creator, &stranger),
        Err(Ok(ContractError::InsufficientBalance))
    );
}

// ---------------------------------------------------------------------------
// Only the admin may activate or deactivate the pause
// ---------------------------------------------------------------------------

#[test]
fn test_only_admin_can_activate_pause() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let stranger = Address::generate(&env);
    let buyer = Address::generate(&env);

    assert_eq!(
        client.try_pause(&stranger),
        Err(Ok(ContractError::Unauthorized)),
        "a non-admin must not be able to activate the pause"
    );

    // The failed attempt must leave trading open.
    assert!(!client.get_is_paused());
    assert_eq!(buy_one(&client, &creator, &buyer), 1);
}

#[test]
fn test_only_admin_can_deactivate_pause() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);
    let stranger = Address::generate(&env);
    let buyer = Address::generate(&env);

    client.pause(&admin);

    assert_eq!(
        client.try_unpause(&stranger),
        Err(Ok(ContractError::Unauthorized)),
        "a non-admin must not be able to lift the pause"
    );

    // The failed attempt must leave the protocol paused and trading blocked.
    assert!(client.get_is_paused());
    let quote = client.get_buy_quote(&creator);
    assert_eq!(
        client.try_buy_key(&creator, &buyer, &quote.total_amount, &None),
        Err(Ok(ContractError::ProtocolPaused))
    );

    // Only the admin can reopen trading.
    client.unpause(&admin);
    assert_eq!(buy_one(&client, &creator, &buyer), 1);
}
