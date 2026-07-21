//! Integration test: buy one key then sell it; seller net XLM matches sell-price minus fees (#560).
//!
//! From a fresh creator at supply 0, a buyer purchases one key and immediately sells it.
//! Sell proceeds must equal the sell-step price minus creator and protocol fees.
//! Creator supply returns to 0 after the sell.
//!
//! In this harness, payment amounts are the quote `total_amount` values (gross buy cost and
//! seller net proceeds), which are the XLM deltas tracked by the contract's fee/quote model.

mod contract_test_env;

use contract_test_env::{
    compute_expected_creator_fee, compute_expected_protocol_fee, compute_expected_sell_price,
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

#[test]
fn test_buy_then_sell_round_trip_returns_correct_xlm_to_seller() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = soroban_sdk::Address::generate(&env);

    // --- Fresh creator: supply 0 ---
    assert_eq!(
        client.get_total_key_supply(&creator),
        0,
        "precondition: creator starts at supply 0"
    );
    assert_eq!(
        client.get_key_balance(&creator, &trader),
        0,
        "precondition: trader holds no keys"
    );

    // --- Buy 1 key: record buyer payment (XLM out) ---
    let buy_quote = client.get_buy_quote(&creator);
    assert_eq!(buy_quote.price, KEY_PRICE);
    assert_eq!(
        buy_quote.creator_fee,
        compute_expected_creator_fee(KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS)
    );
    assert_eq!(
        buy_quote.protocol_fee,
        compute_expected_protocol_fee(KEY_PRICE, PROTOCOL_BPS)
    );
    assert_eq!(
        buy_quote.total_amount,
        buy_quote.price + buy_quote.creator_fee + buy_quote.protocol_fee,
        "buyer pays price plus fees"
    );

    let buyer_xlm_out = buy_quote.total_amount;
    let creator_fee_before_buy = client.get_creator_fee_balance(&creator);
    let protocol_fee_before_buy = client.get_protocol_recipient_balance();

    let supply_after_buy = client.buy_key(&creator, &trader, &buy_quote.total_amount, &None);

    assert_eq!(supply_after_buy, 1, "buy should succeed and leave supply at 1");
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &trader), 1);
    assert_eq!(
        client.get_creator_fee_balance(&creator) - creator_fee_before_buy,
        buy_quote.creator_fee,
        "buyer XLM path: creator fee ledger increases by buy quote creator fee"
    );
    assert_eq!(
        client.get_protocol_recipient_balance() - protocol_fee_before_buy,
        buy_quote.protocol_fee,
        "buyer XLM path: protocol fee ledger increases by buy quote protocol fee"
    );

    // --- Sell that 1 key: record seller net proceeds (XLM in) ---
    let sell_quote = client.get_sell_quote(&creator, &trader);
    let expected_sell_price = compute_expected_sell_price(1, KEY_PRICE);
    let expected_creator_fee =
        compute_expected_creator_fee(expected_sell_price, CREATOR_BPS, PROTOCOL_BPS);
    let expected_protocol_fee =
        compute_expected_protocol_fee(expected_sell_price, PROTOCOL_BPS);
    let expected_sell_proceeds =
        expected_sell_price - expected_creator_fee - expected_protocol_fee;

    assert_eq!(
        sell_quote.price, expected_sell_price,
        "sell price at this supply step"
    );
    assert_eq!(sell_quote.creator_fee, expected_creator_fee);
    assert_eq!(sell_quote.protocol_fee, expected_protocol_fee);
    assert_eq!(
        sell_quote.total_amount, expected_sell_proceeds,
        "sell proceeds must equal sell-price minus creator and protocol fees"
    );
    assert_eq!(
        sell_quote.creator_fee + sell_quote.protocol_fee + sell_quote.total_amount,
        sell_quote.price,
        "fee split must conserve sell price"
    );

    // Cross-check with execution-path fee helper used by sell_key.
    let (exec_creator_fee, exec_protocol_fee) =
        client.compute_fees_for_payment(&sell_quote.price);
    assert_eq!(
        sell_quote.total_amount,
        sell_quote.price - exec_creator_fee - exec_protocol_fee,
        "quoted seller net must match execution-path proceeds"
    );

    let seller_xlm_in = sell_quote.total_amount;
    let creator_fee_before_sell = client.get_creator_fee_balance(&creator);
    let protocol_fee_before_sell = client.get_protocol_recipient_balance();

    let supply_after_sell = client.sell_key(&creator, &trader, &None);

    assert_eq!(
        supply_after_sell, 0,
        "sell should succeed and return supply to 0"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        0,
        "creator supply returns to 0 after the sell"
    );
    assert_eq!(
        client.get_key_balance(&creator, &trader),
        0,
        "trader holds no keys after full exit"
    );
    assert_eq!(
        client.get_creator_fee_balance(&creator) - creator_fee_before_sell,
        sell_quote.creator_fee,
        "seller path: creator fee ledger increases by sell quote creator fee"
    );
    assert_eq!(
        client.get_protocol_recipient_balance() - protocol_fee_before_sell,
        sell_quote.protocol_fee,
        "seller path: protocol fee ledger increases by sell quote protocol fee"
    );

    // Explicit XLM delta story for the round-trip (quote-model amounts).
    assert_eq!(
        buyer_xlm_out,
        buy_quote.price + buy_quote.creator_fee + buy_quote.protocol_fee,
        "buyer XLM delta (paid) matches buy quote total"
    );
    assert_eq!(
        seller_xlm_in, expected_sell_proceeds,
        "seller XLM delta (received) matches sell-price minus fees"
    );
    // With 90/10 fees on the same base price, seller net is less than buyer gross outlay.
    assert!(
        seller_xlm_in < buyer_xlm_out,
        "round-trip is not free: seller net ({seller_xlm_in}) < buyer paid ({buyer_xlm_out})"
    );
}
