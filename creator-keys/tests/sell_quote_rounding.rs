//! Deterministic tests for [`CreatorKeysContract::get_sell_quote`] fee and payout math.
//!
//! Sell quotes use the same fee floor as other payment paths: protocol share is
//! `floor(price * protocol_bps / 10_000)`; the remainder goes to the creator;
//! `total_amount` is `price - creator_fee - protocol_fee` (seller net).

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::fee;
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn register_holder_with_one_key(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
) -> Address {
    let holder = Address::generate(env);
    let price = client.get_buy_quote(creator).price;
    client.buy_key(creator, &holder, &price);
    holder
}

fn assert_sell_quote(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    holder: &Address,
    key_price: i128,
    creator_bps: u32,
    protocol_bps: u32,
) {
    let (exp_creator, exp_protocol) =
        fee::checked_compute_fee_split(key_price, creator_bps, protocol_bps)
            .expect("fee split in range");
    let exp_total = key_price
        .checked_sub(exp_creator)
        .and_then(|x| x.checked_sub(exp_protocol))
        .expect("payout sub");

    let q = client.get_sell_quote(creator, holder);
    assert_eq!(q.price, key_price, "price");
    assert_eq!(q.creator_fee, exp_creator, "creator_fee");
    assert_eq!(q.protocol_fee, exp_protocol, "protocol_fee");
    assert_eq!(q.total_amount, exp_total, "total_amount (net to seller)");
    assert_eq!(
        exp_creator + exp_protocol,
        key_price
            .checked_sub(exp_total)
            .expect("fees from price - payout"),
        "fees + payout = price"
    );
}

#[test]
fn sell_quote_90_10_remainder_favors_creator_on_indivisible_price() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 999_i128;
    set_pricing_and_fees(&env, &client, key_price, 9000, 1000);
    let creator = register_test_creator(&env, &client, "cr1");
    let holder = register_holder_with_one_key(&env, &client, &creator);
    // 999 * 1000 / 10000 = 99 protocol; creator gets 900; net = 0
    assert_sell_quote(&client, &creator, &holder, key_price, 9000, 1000);
}

#[test]
fn sell_quote_90_10_dust_price_one_all_creator_no_protocol_rounding() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 1_i128;
    set_pricing_and_fees(&env, &client, key_price, 9000, 1000);
    let creator = register_test_creator(&env, &client, "cr2");
    let holder = register_holder_with_one_key(&env, &client, &creator);
    assert_sell_quote(&client, &creator, &holder, key_price, 9000, 1000);
    let q = client.get_sell_quote(&creator, &holder);
    assert_eq!(q.protocol_fee, 0);
    assert_eq!(q.creator_fee, 1);
    assert_eq!(q.total_amount, 0);
}

#[test]
fn sell_quote_50_50_small_price_protocol_takes_first_floor_unit() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 3_i128;
    set_pricing_and_fees(&env, &client, key_price, 5000, 5000);
    let creator = register_test_creator(&env, &client, "cr3");
    let holder = register_holder_with_one_key(&env, &client, &creator);
    // floor(3 * 5000 / 10000) = 1 protocol; creator = 2; net = 0
    assert_sell_quote(&client, &creator, &holder, key_price, 5000, 5000);
}

#[test]
fn sell_quote_50_50_price_ten_equal_split_zero_net() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 10_i128;
    set_pricing_and_fees(&env, &client, key_price, 5000, 5000);
    let creator = register_test_creator(&env, &client, "cr4");
    let holder = register_holder_with_one_key(&env, &client, &creator);
    assert_sell_quote(&client, &creator, &holder, key_price, 5000, 5000);
    let q = client.get_sell_quote(&creator, &holder);
    assert_eq!((q.creator_fee, q.protocol_fee, q.total_amount), (5, 5, 0));
}

#[test]
fn sell_quote_100_percent_creator_seller_net_is_zero_fees_absorb_full_price() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let key_price = 100_i128;
    set_pricing_and_fees(&env, &client, key_price, 10000, 0);
    let creator = register_test_creator(&env, &client, "cr5");
    let holder = register_holder_with_one_key(&env, &client, &creator);
    let q = client.get_sell_quote(&creator, &holder);
    assert_eq!((q.creator_fee, q.protocol_fee, q.total_amount), (100, 0, 0));
}

#[test]
fn sell_quote_max_allowed_protocol_bps_50_50_dust_price_floors_protocol_share_to_zero() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    // 50% / 50% is the maximum protocol share allowed (`PROTOCOL_BPS_MAX` = 5000).
    let key_price = 1_i128;
    set_pricing_and_fees(&env, &client, key_price, 5000, 5000);
    let creator = register_test_creator(&env, &client, "cr6");
    let holder = register_holder_with_one_key(&env, &client, &creator);
    // floor(1 * 5000 / 10000) = 0 protocol; creator 1; net 0
    assert_sell_quote(&client, &creator, &holder, key_price, 5000, 5000);
    let q = client.get_sell_quote(&creator, &holder);
    assert_eq!((q.creator_fee, q.protocol_fee, q.total_amount), (1, 0, 0));
}
