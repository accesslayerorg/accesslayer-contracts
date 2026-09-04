//! Integration test confirming the bonding curve price increases monotonically
//! across ten sequential single-key buys, and decreases after a sell (Issue #697).
//!
//! Covers all four acceptance criteria:
//!   AC-1  Price strictly increases after each sequential buy.
//!   AC-2  Price strictly decreases after a sell.
//!   AC-3  Price at supply 0 equals the configured base price.
//!   AC-4  All ten price snapshots are monotonically increasing.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_curve_slope, set_pricing_and_fees,
    test_env_with_auths,
};
use soroban_sdk::testutils::{Address as _, Ledger};

const BASE_PRICE: i128 = 5_000;
const CURVE_SLOPE: i128 = 100;
const NUM_BUYS: usize = 10;

/// AC-3: `get_buy_quote` at supply 0 returns a price equal to BASE_PRICE.
/// AC-1 + AC-4: all ten buy-quote prices are strictly increasing.
/// AC-2: one sell after ten buys yields a price strictly below the last buy price.
#[test]
fn test_ten_sequential_buys_price_strictly_increases_and_sell_decreases() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    let creator = register_test_creator(&env, &client, "alice");
    let holder = soroban_sdk::Address::generate(&env);

    // AC-3: price at supply 0 must equal BASE_PRICE.
    let price_at_zero = client.get_buy_quote(&creator).price;
    assert_eq!(
        price_at_zero, BASE_PRICE,
        "price at supply 0 must equal the configured base price"
    );

    // AC-1 + AC-4: record the quote price before each buy (price at supply i),
    // execute the buy, then assert each successive quote is strictly greater.
    // Buy 0 goes to `holder` so they hold a key for the sell assertion below.
    let mut buy_prices: Vec<i128> = Vec::new();

    for i in 0..NUM_BUYS {
        let quote = client.get_buy_quote(&creator);

        if let Some(&prev_price) = buy_prices.last() {
            assert!(
                quote.price > prev_price,
                "buy {} price ({}) must be strictly greater than buy {} price ({})",
                i + 1,
                quote.price,
                i,
                prev_price
            );
        }

        buy_prices.push(quote.price);

        let buyer = if i == 0 {
            holder.clone()
        } else {
            soroban_sdk::Address::generate(&env)
        };

        client.buy_key(&creator, &buyer, &quote.total_amount, &None);
    }

    assert_eq!(buy_prices.len(), NUM_BUYS);

    // AC-4: full monotonicity check across the collected snapshots.
    for (index, window) in buy_prices.windows(2).enumerate() {
        assert!(
            window[1] > window[0],
            "price snapshot {} ({}) must be strictly greater than snapshot {} ({})",
            index + 2,
            window[1],
            index + 1,
            window[0]
        );
    }

    // AC-2: price after one sell must be strictly less than the last buy price.
    let price_before_sell = client.get_buy_quote(&creator).price;
    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.sell_key(&creator, &holder, &None);
    let price_after_sell = client.get_buy_quote(&creator).price;

    assert!(
        price_after_sell < price_before_sell,
        "price after sell ({}) must be strictly less than price before sell ({})",
        price_after_sell,
        price_before_sell
    );
}

/// Regression guard: a flat (zero-slope) curve does not produce strictly
/// increasing prices, confirming the main test above is actually discriminating.
#[test]
fn test_flat_curve_prices_do_not_strictly_increase_across_ten_buys() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    // No set_curve_slope call — defaults to 0 (flat).
    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);

    let creator = register_test_creator(&env, &client, "bob");

    let mut prices: Vec<i128> = Vec::new();

    for _ in 0..NUM_BUYS {
        let quote = client.get_buy_quote(&creator);
        prices.push(quote.price);
        let buyer = soroban_sdk::Address::generate(&env);
        client.buy_key(&creator, &buyer, &quote.total_amount, &None);
    }

    let strictly_increasing = prices.windows(2).all(|w| w[1] > w[0]);
    assert!(
        !strictly_increasing,
        "flat pricing must not produce strictly increasing prices across ten buys"
    );
}

/// Validates the ten price snapshots against the bonding curve formula
/// `price = base_price + slope * supply` for every step.
#[test]
fn test_ten_buy_prices_match_bonding_curve_formula() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    set_pricing_and_fees(&env, &client, BASE_PRICE, 9000, 1000);
    set_curve_slope(&env, &client, CURVE_SLOPE);

    let creator = register_test_creator(&env, &client, "carol");

    for i in 0..NUM_BUYS {
        let quote = client.get_buy_quote(&creator);

        let expected = BASE_PRICE + CURVE_SLOPE * i as i128;
        assert_eq!(
            quote.price, expected,
            "price at supply {} must be {} (base_price + slope * supply)",
            i, expected
        );

        let buyer = soroban_sdk::Address::generate(&env);
        client.buy_key(&creator, &buyer, &quote.total_amount, &None);
    }

    assert_eq!(
        client.get_total_key_supply(&creator),
        NUM_BUYS as u32,
        "supply must equal ten after ten buys"
    );
}
