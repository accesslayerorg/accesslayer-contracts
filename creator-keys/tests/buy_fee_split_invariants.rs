//! Fee split sum invariants for buy quote path.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths};

#[test]
fn buy_quote_fees_plus_price_equals_total_nominal() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_000_i128, 9_000, 1_000);

    let creator = register_test_creator(&env, &client, "inv_nominal");
    let quote = client.get_buy_quote(&creator);

    assert_eq!(
        quote.price + quote.creator_fee + quote.protocol_fee,
        quote.total_amount
    );
}

#[test]
fn buy_quote_fees_plus_price_equals_total_boundary_dust() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1_i128, 5_000, 5_000);

    let creator = register_test_creator(&env, &client, "inv_dust");
    let quote = client.get_buy_quote(&creator);

    assert_eq!(quote.protocol_fee, 0);
    assert_eq!(quote.creator_fee, 1);
    assert_eq!(
        quote.price + quote.creator_fee + quote.protocol_fee,
        quote.total_amount
    );
}
