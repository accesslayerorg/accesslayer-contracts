//! Integration test verifying sell proceeds decrease monotonically as a
//! creator's supply shrinks along a bonding curve (Issue #622).
//!
//! Builds a creator up to supply 5 via five sequential buy transactions from
//! the same wallet, then sells one key at a time, recording proceeds for each
//! sell. Each successive sell's proceeds must be strictly lower than the
//! previous sell's proceeds, mirroring the bonding curve in reverse.
//!
//! `QuoteResponse::price` is used as the proceeds figure here rather than
//! `QuoteResponse::total_amount`. `set_fee_config`/`register_creator` require
//! `creator_bps + protocol_bps == fee::BPS_MAX` (10,000) for any valid fee
//! config, and `compute_fees_for_payment` derives `(creator_fee,
//! protocol_fee)` as a full split of the trade price under that same
//! constraint. As a result `total_amount` for a sell (`price - creator_fee -
//! protocol_fee`) is always `0` for any valid config — the bonding-curve price
//! impact this test exercises is carried entirely by `price`.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_curve_slope, set_pricing_and_fees,
    test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address};

const BASE_PRICE: i128 = 10_000;
const CURVE_SLOPE: i128 = 50;
const STARTING_SUPPLY: u32 = 5;

#[test]
fn test_sell_proceeds_strictly_decrease_across_five_sequential_sells() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    // Build up supply to 5 via five sequential buys from the same wallet.
    for _ in 0..STARTING_SUPPLY {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &holder, &quote.total_amount, &None);
    }
    assert_eq!(client.get_total_key_supply(&creator), STARTING_SUPPLY);

    // Sell one key at a time, recording proceeds for each sell.
    let mut proceeds: Vec<i128> = Vec::new();
    for _ in 0..STARTING_SUPPLY {
        let quote = client.get_sell_quote(&creator, &holder);
        // total_amount is always 0 under a valid (sum-to-10000) fee config;
        // see module docs. price is the field that carries curve impact.
        assert_eq!(quote.total_amount, 0);
        proceeds.push(quote.price);
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &holder, &None);
    }

    assert_eq!(proceeds.len(), STARTING_SUPPLY as usize);
    assert_eq!(client.get_total_key_supply(&creator), 0);

    // Proceeds from sell N (supply k -> k-1) must be strictly greater than
    // proceeds from sell N+1 (supply k-1 -> k-2), for every consecutive pair.
    for (index, window) in proceeds.windows(2).enumerate() {
        assert!(
            window[1] < window[0],
            "sell {} proceeds ({}) must be strictly less than sell {} proceeds ({})",
            index + 2,
            window[1],
            index + 1,
            window[0]
        );
    }

    // The final sell (supply 1 -> 0) must yield the lowest proceeds recorded.
    let lowest = *proceeds.iter().min().unwrap();
    let final_proceeds = *proceeds.last().unwrap();
    assert_eq!(
        final_proceeds, lowest,
        "final sell (supply 1 -> 0) must produce the lowest proceeds of all five sells"
    );
}

#[test]
fn test_flat_curve_sell_proceeds_do_not_strictly_decrease() {
    // Regression guard: a flat (zero-slope) curve produces equal proceeds
    // across every sell, confirming the strict-decrease assertion above is
    // actually discriminating rather than trivially true.
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    // No curve slope configured: defaults to 0, i.e. a flat price curve.

    let creator = register_test_creator(&env, &client, "bob");
    let holder = Address::generate(&env);

    for _ in 0..STARTING_SUPPLY {
        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &holder, &quote.total_amount, &None);
    }

    let mut proceeds: Vec<i128> = Vec::new();
    for _ in 0..STARTING_SUPPLY {
        let quote = client.get_sell_quote(&creator, &holder);
        proceeds.push(quote.price);
        let mut l = env.ledger().get();
        l.sequence_number += 1;
        env.ledger().set(l);
        client.sell_key(&creator, &holder, &None);
    }

    let strictly_decreasing = proceeds.windows(2).all(|window| window[1] < window[0]);
    assert!(
        !strictly_decreasing,
        "flat pricing must not produce strictly decreasing proceeds across sells"
    );
}
