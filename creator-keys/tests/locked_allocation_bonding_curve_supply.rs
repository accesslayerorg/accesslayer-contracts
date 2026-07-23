//! Regression tests for locked allocation moving bonding-curve supply at registration.
//!
//! When a creator registers with a locked allocation of N keys the contract immediately
//! counts those keys in `total_supply`. Because the bonding curve prices the next buy
//! against the current supply, the buy quote must reflect N, not zero.
//! A creator registered without a locked allocation must still start at supply zero.

mod contract_test_env;

use contract_test_env::{
    compute_expected_bonding_curve_price, register_creator_keys, set_curve_slope,
    set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::LockedAllocation;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

const KEY_PRICE: i128 = 100;
const CURVE_SLOPE: i128 = 10;
const ALLOCATION_AMOUNT: u32 = 5;
const UNLOCK_LEDGER: u32 = 100;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

fn setup(
    env: &Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    Address,
    Address,
) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    set_curve_slope(env, &client, CURVE_SLOPE);

    // unlock_ledger must be strictly greater than current sequence.
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 1;
    env.ledger().set(ledger_info);

    let creator_with_alloc = Address::generate(env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator_with_alloc.clone(),
            handle: String::from_str(env, "alice"),
        },
        &Some(LockedAllocation {
            amount: ALLOCATION_AMOUNT,
            unlock_ledger: UNLOCK_LEDGER,
            claimed: false,
        }),
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let creator_no_alloc = Address::generate(env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator_no_alloc.clone(),
            handle: String::from_str(env, "bob"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    (client, creator_with_alloc, creator_no_alloc)
}

#[test]
fn test_locked_allocation_sets_total_supply_immediately() {
    let env = test_env_with_auths();
    let (client, creator_with_alloc, _) = setup(&env);

    assert_eq!(
        client.get_total_key_supply(&creator_with_alloc),
        ALLOCATION_AMOUNT,
        "total supply must equal the locked allocation amount immediately after registration"
    );
}

#[test]
fn test_buy_quote_reflects_locked_supply() {
    let env = test_env_with_auths();
    let (client, creator_with_alloc, _) = setup(&env);

    let quote = client.get_buy_quote(&creator_with_alloc);
    let expected_price =
        compute_expected_bonding_curve_price(CURVE_SLOPE, KEY_PRICE, ALLOCATION_AMOUNT);

    assert_eq!(
        quote.price, expected_price,
        "buy quote must be priced at supply {} (not zero) to reflect the locked allocation",
        ALLOCATION_AMOUNT
    );
}

#[test]
fn test_creator_without_allocation_starts_at_zero_supply() {
    let env = test_env_with_auths();
    let (client, _, creator_no_alloc) = setup(&env);

    assert_eq!(
        client.get_total_key_supply(&creator_no_alloc),
        0,
        "creator without locked allocation must start with supply zero"
    );

    let quote = client.get_buy_quote(&creator_no_alloc);
    let expected_price = compute_expected_bonding_curve_price(CURVE_SLOPE, KEY_PRICE, 0);

    assert_eq!(
        quote.price, expected_price,
        "buy quote for creator without allocation must be priced at supply zero"
    );
}
