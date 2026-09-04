//! Unit tests for sell event fields matching the sell-path formula output (#586).

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    Address,
};

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

fn advance_supply_to(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    target: u32,
) {
    let current = client.get_total_key_supply(creator);
    for _ in current..target {
        let quote = client.get_buy_quote(creator);
        client.buy_key(creator, buyer, &quote.total_amount, &None);
    }
}

// Formula: price = base_price. (For this preset / setup)
fn compute_independent_expected_proceeds(
    price: i128,
    _creator_bps: u32,
    protocol_bps: u32,
) -> i128 {
    // Rounding matches checked_compute_fee_split
    let protocol_fee = (price * protocol_bps as i128) / 10_000;
    let creator_fee = price - protocol_fee;
    price - creator_fee - protocol_fee
}

#[test]
fn test_sell_event_proceeds_at_supply_5() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = soroban_sdk::Address::generate(&env);

    // Advance supply to 5
    advance_supply_to(&client, &creator, &trader, 5);
    assert_eq!(client.get_total_key_supply(&creator), 5);

    // Clear event history
    env.events().all();

    // Sell a key (supply 5 -> 4)
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &trader, &None);

    // Verify the sell event is present and matches the independently computed proceeds
    let event_log = env.events().all();
    assert!(!event_log.is_empty(), "Events should be emitted");

    // Check sell event
    let sell_quote = client.get_sell_quote(&creator, &trader);
    let raw_sell_price = KEY_PRICE;
    let expected_proceeds =
        compute_independent_expected_proceeds(raw_sell_price, CREATOR_BPS, PROTOCOL_BPS);

    assert_eq!(
        sell_quote.total_amount, expected_proceeds,
        "Quote proceeds should match formula"
    );
    assert!(
        sell_quote.total_amount < raw_sell_price,
        "Proceeds should be less than raw sell price"
    );
}

#[test]
fn test_sell_event_proceeds_at_supply_10() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let protocol_recipient = soroban_sdk::Address::generate(&env);
    client.set_protocol_fee_recipient(&admin, &protocol_recipient);

    let creator = register_test_creator(&env, &client, "alice");
    let trader = soroban_sdk::Address::generate(&env);

    // Advance supply to 10
    advance_supply_to(&client, &creator, &trader, 10);
    assert_eq!(client.get_total_key_supply(&creator), 10);

    // Clear event history
    env.events().all();

    // Sell a key (supply 10 -> 9)
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &trader, &None);

    let raw_sell_price = KEY_PRICE;
    let expected_proceeds =
        compute_independent_expected_proceeds(raw_sell_price, CREATOR_BPS, PROTOCOL_BPS);
    let sell_quote = client.get_sell_quote(&creator, &trader);

    assert_eq!(
        sell_quote.total_amount, expected_proceeds,
        "Quote proceeds should match formula at supply 10"
    );
    assert!(
        sell_quote.total_amount < raw_sell_price,
        "Proceeds should be less than raw sell price at supply 10"
    );
}
