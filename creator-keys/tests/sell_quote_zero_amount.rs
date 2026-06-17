//! Unit tests for zero-amount sell quote returning the expected result.
//!
//! When the stored key price is zero, `get_sell_quote` must return a zero-valued
//! QuoteResponse without error. The expected result is the same zero quote returned
//! by buy paths under the same condition — price, creator_fee, protocol_fee, and
//! total_amount are all zero.
//!
//! The balance check is skipped for zero-price inputs because the price normalisation
//! step returns early (`None` from `normalize_quote_amount`), so even a holder with
//! no keys receives a zero quote instead of `InsufficientBalance`.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_protocol_fee_bps, set_stored_key_price,
    test_env_with_auths,
};
use soroban_sdk::testutils::Address as _;

/// Zero sell amount (key price = 0) returns a zero QuoteResponse without error.
///
/// Expected result: price=0, creator_fee=0, protocol_fee=0, total_amount=0.
#[test]
fn test_sell_quote_zero_amount_returns_zero_quote() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let admin = soroban_sdk::Address::generate(&env);
    let holder = soroban_sdk::Address::generate(&env);

    client.set_key_price(&admin, &100);
    set_protocol_fee_bps(&env, &client, 9000, 1000);
    let creator = register_test_creator(&env, &client, "alice");
    client.buy_key(&creator, &holder, &100);

    // Zero out the key price — produces a zero sell amount input
    set_stored_key_price(&env, &contract_id, 0);

    let quote = client.get_sell_quote(&creator, &holder);

    assert_eq!(quote.price, 0, "zero sell amount returns zero price");
    assert_eq!(
        quote.creator_fee, 0,
        "zero sell amount returns zero creator fee"
    );
    assert_eq!(
        quote.protocol_fee, 0,
        "zero sell amount returns zero protocol fee"
    );
    assert_eq!(
        quote.total_amount, 0,
        "zero sell amount returns zero total amount"
    );
}

/// Zero sell amount with a holder who has no keys still returns a zero quote,
/// not InsufficientBalance.
///
/// The balance check is only reached after price normalisation. When the price is
/// zero, `normalize_quote_amount` returns `None` and the function returns the zero
/// quote early — the holder balance is never consulted.
#[test]
fn test_sell_quote_zero_amount_holder_no_keys_returns_zero_quote() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let admin = soroban_sdk::Address::generate(&env);

    client.set_key_price(&admin, &100);
    set_protocol_fee_bps(&env, &client, 9000, 1000);
    let creator = register_test_creator(&env, &client, "bob");

    // Holder has no keys — would normally trigger InsufficientBalance for non-zero price
    let holder_no_keys = soroban_sdk::Address::generate(&env);

    // Zero out the key price
    set_stored_key_price(&env, &contract_id, 0);

    // Must return the zero quote, not Err(InsufficientBalance)
    let quote = client.get_sell_quote(&creator, &holder_no_keys);

    assert_eq!(quote.price, 0);
    assert_eq!(quote.creator_fee, 0);
    assert_eq!(quote.protocol_fee, 0);
    assert_eq!(quote.total_amount, 0);
}

/// Zero sell amount does not modify contract state.
///
/// `get_sell_quote` is read-only; calling it with zero amount must leave
/// creator supply and holder key balance unchanged across repeated calls.
#[test]
fn test_sell_quote_zero_amount_no_state_modification() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let admin = soroban_sdk::Address::generate(&env);
    let holder = soroban_sdk::Address::generate(&env);

    client.set_key_price(&admin, &100);
    set_protocol_fee_bps(&env, &client, 9000, 1000);
    let creator = register_test_creator(&env, &client, "charlie");
    client.buy_key(&creator, &holder, &100);

    // Zero the price and capture state before the read-only calls.
    set_stored_key_price(&env, &contract_id, 0);
    let before = contract_test_env::capture_snapshot(&client, &creator, &holder);

    for _ in 0..5 {
        let quote = client.get_sell_quote(&creator, &holder);
        assert_eq!(quote.price, 0);
        assert_eq!(quote.total_amount, 0);
    }

    let after = contract_test_env::capture_snapshot(&client, &creator, &holder);
    before.assert_unchanged(&after);
}
