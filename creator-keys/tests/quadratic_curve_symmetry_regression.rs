//! Regression test verifying buy and sell quote symmetry for the Quadratic preset.
//!
//! The cost (price/fees) to buy N keys at supply S must equal the proceeds (price/fees)
//! from selling N keys at supply S+N.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, set_curve_slope, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{CurvePreset, RegisterCreatorParams};
use soroban_sdk::{testutils::Address as _, Address, String};

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;
const QUADRATIC_SLOPE: i128 = 10;

fn assert_symmetry_for_params(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    start_supply: u32,
    n: u32,
) {
    // 1. Advance supply to start_supply
    let current_supply = client.get_total_key_supply(creator);
    if current_supply < start_supply {
        for _ in current_supply..start_supply {
            let quote = client.get_buy_quote(creator);
            client.buy_key(creator, buyer, &quote.total_amount, &None);
        }
    }

    assert_eq!(client.get_total_key_supply(creator), start_supply);

    // 2. Accumulate buy quotes for N keys and execute buys
    let mut total_buy_price = 0;
    let mut total_buy_creator_fee = 0;
    let mut total_buy_protocol_fee = 0;

    for _ in 0..n {
        let quote = client.get_buy_quote(creator);
        total_buy_price += quote.price;
        total_buy_creator_fee += quote.creator_fee;
        total_buy_protocol_fee += quote.protocol_fee;
        client.buy_key(creator, buyer, &quote.total_amount, &None);
    }

    assert_eq!(client.get_total_key_supply(creator), start_supply + n);

    // 3. Accumulate sell quotes for N keys and execute sells
    let mut total_sell_price = 0;
    let mut total_sell_creator_fee = 0;
    let mut total_sell_protocol_fee = 0;

    for _ in 0..n {
        let quote = client.get_sell_quote(creator, buyer);
        total_sell_price += quote.price;
        total_sell_creator_fee += quote.creator_fee;
        total_sell_protocol_fee += quote.protocol_fee;
        client.sell_key(creator, buyer, &None);
    }

    assert_eq!(client.get_total_key_supply(creator), start_supply);

    // 4. Assert symmetry
    assert_eq!(
        total_buy_price, total_sell_price,
        "Price asymmetry at supply {} for N {}",
        start_supply, n
    );
    assert_eq!(
        total_buy_creator_fee, total_sell_creator_fee,
        "Creator fee asymmetry at supply {} for N {}",
        start_supply, n
    );
    assert_eq!(
        total_buy_protocol_fee, total_sell_protocol_fee,
        "Protocol fee asymmetry at supply {} for N {}",
        start_supply, n
    );
}

#[test]
fn test_quadratic_curve_symmetry() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    set_curve_slope(&env, &client, QUADRATIC_SLOPE);

    let creator = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "quadcreator"),
        },
        &None,
        &None,
        &None,
        &Some(CurvePreset::Quadratic),
        &None,
        &None,
    );

    let buyer = Address::generate(&env);

    // Cover supply levels: 0, 50, and 500
    // Cover both small and large key amounts
    assert_symmetry_for_params(&client, &creator, &buyer, 0, 1);
    assert_symmetry_for_params(&client, &creator, &buyer, 0, 10);
    assert_symmetry_for_params(&client, &creator, &buyer, 50, 5);
    assert_symmetry_for_params(&client, &creator, &buyer, 500, 50);
}
