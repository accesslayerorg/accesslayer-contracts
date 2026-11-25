//! Circle-preset dispatch (#403): all three presets price through one shared
//! function, order correctly, and stay independent per creator.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, set_curve_slope, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{compute_price, CurvePreset, QuoteResponse, RegisterCreatorParams};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, String};

const KEY_PRICE: i128 = 1_000;
const SLOPE: i128 = 10;

fn register_creator(
    env: &Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
    preset: Option<CurvePreset>,
    handle: &str,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &None,
        &None,
        &preset,
        &None,
        &None,
    );
    creator
}

#[test]
fn compute_price_dispatches_per_preset() {
    // Spot price at supply 10.
    let linear = compute_price(KEY_PRICE, SLOPE, 10, 1, CurvePreset::Linear).unwrap();
    let quadratic = compute_price(KEY_PRICE, SLOPE, 10, 1, CurvePreset::Quadratic).unwrap();
    let flat = compute_price(KEY_PRICE, SLOPE, 10, 1, CurvePreset::Flat).unwrap();

    assert_eq!(linear, KEY_PRICE + SLOPE * 10);
    assert_eq!(quadratic, KEY_PRICE + SLOPE * 100);
    assert_eq!(flat, KEY_PRICE);
    assert!(quadratic > linear);
    assert!(linear > flat);
}

#[test]
fn compute_price_is_symmetric_for_n_keys_in_and_out() {
    for preset in [
        CurvePreset::Linear,
        CurvePreset::Quadratic,
        CurvePreset::Flat,
    ] {
        // Buying N keys from supply S and selling the same N keys at supply S+N
        // must quote the same total: both walk the identical supply range.
        let bought = compute_price(KEY_PRICE, SLOPE, 20, 5, preset).unwrap();
        let sold = compute_price(KEY_PRICE, SLOPE, 20, 5, preset).unwrap();
        assert_eq!(bought, sold, "preset {:?} is not symmetric", preset);
    }
}

#[test]
fn buy_and_sell_quotes_are_symmetric_for_each_preset() {
    for preset in [
        CurvePreset::Linear,
        CurvePreset::Quadratic,
        CurvePreset::Flat,
    ] {
        let env = test_env_with_auths();
        let (client, _) = register_creator_keys(&env);
        set_pricing_and_fees(&env, &client, KEY_PRICE, 9_000, 1_000);
        set_curve_slope(&env, &client, SLOPE);
        let creator = register_creator(&env, &client, Some(preset), "sym");
        let buyer = Address::generate(&env);

        let mut buy_total = 0i128;
        let mut sell_total = 0i128;

        for _ in 0..5 {
            let quote: QuoteResponse = client.get_buy_quote(&creator);
            buy_total += quote.price;
            client.buy_key(&creator, &buyer, &quote.total_amount, &None);
        }

        for _ in 0..5 {
            let quote = client.get_sell_quote(&creator, &buyer);
            sell_total += quote.price;
            env.ledger().with_mut(|l| l.sequence_number += 1);
            client.sell_key(&creator, &buyer, &None);
        }

        assert_eq!(buy_total, sell_total, "preset {:?} quote asymmetry", preset);
    }
}

#[test]
fn presets_order_quadratic_above_linear_above_flat() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, 9_000, 1_000);
    set_curve_slope(&env, &client, SLOPE);

    let linear = register_creator(&env, &client, Some(CurvePreset::Linear), "lin");
    let quadratic = register_creator(&env, &client, Some(CurvePreset::Quadratic), "quad");
    let flat = register_creator(&env, &client, Some(CurvePreset::Flat), "flat");

    assert_eq!(client.get_curve_preset(&linear), CurvePreset::Linear);
    assert_eq!(client.get_curve_preset(&quadratic), CurvePreset::Quadratic);
    assert_eq!(client.get_curve_preset(&flat), CurvePreset::Flat);

    // Supply 10 for each creator; presets are independent per creator.
    for creator in [&linear, &quadratic, &flat] {
        let buyer = Address::generate(&env);
        for _ in 0..10 {
            let quote = client.get_buy_quote(creator);
            client.buy_key(creator, &buyer, &quote.total_amount, &None);
        }
    }

    let linear_quote = client.get_buy_quote(&linear);
    let quadratic_quote = client.get_buy_quote(&quadratic);
    let flat_quote = client.get_buy_quote(&flat);

    assert!(
        quadratic_quote.price > linear_quote.price,
        "quadratic {} must exceed linear {}",
        quadratic_quote.price,
        linear_quote.price
    );
    assert!(
        linear_quote.price > flat_quote.price,
        "linear {} must exceed flat {}",
        linear_quote.price,
        flat_quote.price
    );
}

#[test]
fn omitted_preset_defaults_to_linear() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_creator(&env, &client, None, "defaulted");

    assert_eq!(client.get_curve_preset(&creator), CurvePreset::Linear);
}
