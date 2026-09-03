//! Unit tests for `sell_key` keeping the creator's stored total supply correct (#738).
//!
//! Every successful sell must decrement supply by exactly what was sold, and
//! supply must never be able to go negative — it is a `u32`, so an underflow
//! would wrap to a huge number and corrupt every price quote and dividend split
//! computed from it. A rejected sell must leave supply exactly where it was.
//!
//! # A note on `insufficient_supply`
//!
//! `sell_key` sells one key per call and has no `amount` parameter, so a wallet
//! cannot ask to sell more than the supply through it — the balance guard fires
//! first and returns [`ContractError::InsufficientBalance`]. The contract's
//! `InsufficientSupply` error belongs to the sell-side path that *does* take an
//! amount, `buyback`, which burns N keys and rejects `N > supply`. Both are
//! asserted below so the "cannot sell more than exists" rule is covered on
//! whichever entrypoint enforces it.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

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

/// Buy `count` keys for `buyer`, one call per key.
fn buy_keys(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    count: u32,
) {
    for _ in 0..count {
        let quote = client.get_buy_quote(creator);
        client.buy_key(creator, buyer, &quote.total_amount, &None);
    }
}

// ---------------------------------------------------------------------------
// Supply shrinks by exactly what was sold
// ---------------------------------------------------------------------------

#[test]
fn test_selling_one_key_from_supply_five_sets_supply_to_four() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let seller = Address::generate(&env);

    buy_keys(&client, &creator, &seller, 5);
    assert_eq!(client.get_total_key_supply(&creator), 5);

    let returned = client.sell_key(&creator, &seller, &None);

    assert_eq!(returned, 4, "sell_key must return the post-sell supply");
    assert_eq!(
        client.get_total_key_supply(&creator),
        4,
        "get_total_key_supply must agree with the value sell_key returned"
    );
    assert_eq!(client.get_key_balance(&creator, &seller), 4);
}

#[test]
fn test_selling_all_keys_from_supply_ten_sets_supply_to_zero() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let seller = Address::generate(&env);

    buy_keys(&client, &creator, &seller, 10);
    assert_eq!(client.get_total_key_supply(&creator), 10);

    // Walk the whole position down, checking the view after every sell.
    for expected_supply in (0..10u32).rev() {
        let returned = client.sell_key(&creator, &seller, &None);
        assert_eq!(returned, expected_supply);
        assert_eq!(client.get_total_key_supply(&creator), expected_supply);
    }

    assert_eq!(
        client.get_total_key_supply(&creator),
        0,
        "selling every key must leave supply at zero"
    );
    assert_eq!(client.get_key_balance(&creator, &seller), 0);
    assert_eq!(client.get_creator_holder_count(&creator), 0);
}

#[test]
fn test_supply_tracks_sells_across_two_holders() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let first = Address::generate(&env);
    let second = Address::generate(&env);

    buy_keys(&client, &creator, &first, 4);
    buy_keys(&client, &creator, &second, 6);
    assert_eq!(client.get_total_key_supply(&creator), 10);

    // Supply is the shared total: it drops regardless of which holder sells.
    assert_eq!(client.sell_key(&creator, &first, &None), 9);
    assert_eq!(client.sell_key(&creator, &second, &None), 8);
    assert_eq!(client.sell_key(&creator, &second, &None), 7);

    assert_eq!(client.get_total_key_supply(&creator), 7);
    assert_eq!(client.get_key_balance(&creator, &first), 3);
    assert_eq!(client.get_key_balance(&creator, &second), 4);
}

// ---------------------------------------------------------------------------
// Supply can never go below zero
// ---------------------------------------------------------------------------

#[test]
fn test_selling_past_zero_supply_is_rejected_and_never_underflows() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let seller = Address::generate(&env);

    buy_keys(&client, &creator, &seller, 2);
    client.sell_key(&creator, &seller, &None);
    client.sell_key(&creator, &seller, &None);
    assert_eq!(client.get_total_key_supply(&creator), 0);

    // The wallet now holds nothing, so the next sell is refused rather than
    // wrapping the u32 supply around.
    assert_eq!(
        client.try_sell_key(&creator, &seller, &None),
        Err(Ok(ContractError::InsufficientBalance))
    );

    assert_eq!(
        client.get_total_key_supply(&creator),
        0,
        "supply must stay at zero, not wrap to u32::MAX"
    );
}

#[test]
fn test_selling_more_than_the_wallet_holds_is_rejected_on_balance() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let seller = Address::generate(&env);

    buy_keys(&client, &creator, &seller, 3);

    // Three sells drain the position exactly.
    for _ in 0..3 {
        client.sell_key(&creator, &seller, &None);
    }

    // `sell_key` takes no amount, so "more than I hold" surfaces as a fourth
    // call against an empty balance.
    assert_eq!(
        client.try_sell_key(&creator, &seller, &None),
        Err(Ok(ContractError::InsufficientBalance))
    );
    assert_eq!(client.get_total_key_supply(&creator), 0);
}

#[test]
fn test_burning_more_keys_than_supply_is_rejected_with_insufficient_supply() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);

    // `buyback` is the sell-side path that takes an amount, so it is the one
    // that can be asked to remove more keys than exist.
    buy_keys(&client, &creator, &creator, 5);
    let supply_before = client.get_total_key_supply(&creator);
    assert_eq!(supply_before, 5);

    let over_supply = supply_before + 1;
    assert_eq!(
        client.try_get_buyback_quote(&creator, &over_supply),
        Err(Ok(ContractError::InsufficientSupply)),
        "quoting a burn larger than supply must be rejected"
    );
    assert_eq!(
        client.try_buyback(&creator, &creator, &over_supply, &(KEY_PRICE * 100), &None),
        Err(Ok(ContractError::InsufficientSupply)),
        "burning more keys than exist must be rejected"
    );

    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "a rejected burn must not move supply"
    );
}

// ---------------------------------------------------------------------------
// A rejected sell leaves supply exactly where it was
// ---------------------------------------------------------------------------

#[test]
fn test_supply_unchanged_after_a_sell_from_a_wallet_holding_nothing() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let holder = Address::generate(&env);
    let stranger = Address::generate(&env);

    buy_keys(&client, &creator, &holder, 6);
    let supply_before = client.get_total_key_supply(&creator);

    assert_eq!(
        client.try_sell_key(&creator, &stranger, &None),
        Err(Ok(ContractError::InsufficientBalance))
    );

    assert_eq!(client.get_total_key_supply(&creator), supply_before);
    assert_eq!(client.get_key_balance(&creator, &holder), 6);
}

#[test]
fn test_supply_unchanged_after_a_sell_rejected_on_slippage() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let seller = Address::generate(&env);

    buy_keys(&client, &creator, &seller, 4);
    let supply_before = client.get_total_key_supply(&creator);

    // Demand more proceeds than the sale can return.
    assert_eq!(
        client.try_sell_key(&creator, &seller, &Some(i128::MAX)),
        Err(Ok(ContractError::SlippageExceeded))
    );

    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "a sell rejected on slippage must not move supply"
    );
    assert_eq!(client.get_key_balance(&creator, &seller), 4);

    // The position is still sellable on the very next call.
    assert_eq!(client.sell_key(&creator, &seller, &None), supply_before - 1);
}

#[test]
fn test_supply_unchanged_after_a_sell_for_an_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let seller = Address::generate(&env);
    let unregistered = Address::generate(&env);

    buy_keys(&client, &creator, &seller, 3);
    let supply_before = client.get_total_key_supply(&creator);

    assert_eq!(
        client.try_sell_key(&unregistered, &seller, &None),
        Err(Ok(ContractError::NotRegistered))
    );

    assert_eq!(client.get_total_key_supply(&creator), supply_before);
    assert_eq!(client.get_total_key_supply(&unregistered), 0);
}
