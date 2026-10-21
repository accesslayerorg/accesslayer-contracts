//! Integration tests covering the full auction-to-bonding-curve transition
//! for a creator key (#788): configuring the auction, buying all auction
//! keys at the fixed price, and verifying the next buy uses the bonding
//! curve price.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use creator_keys::events;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, Symbol,
};

const BASE_PRICE: i128 = 1000;
const CURVE_SLOPE: i128 = 50;
const AUCTION_PRICE: i128 = 10;
const AUCTION_SUPPLY: u32 = 5;

fn auction_purchase_price_paid(env: &soroban_sdk::Env, contract_id: &Address) -> Option<i128> {
    for (contract, topics, data) in env.events().all().iter() {
        if contract != *contract_id {
            continue;
        }
        let event_name: Symbol = topics.get(0).unwrap().into_val(env);
        if event_name == events::AUCTION_PURCHASE_EVENT_NAME {
            let payload: events::AuctionPurchaseEvent = data.clone().into_val(env);
            return Some(payload.price_paid);
        }
    }
    None
}

fn standard_buy_event(
    env: &soroban_sdk::Env,
    contract_id: &Address,
) -> Option<events::KeysBoughtEvent> {
    for (contract, topics, data) in env.events().all().iter() {
        if contract != *contract_id {
            continue;
        }
        let event_name: Symbol = topics.get(0).unwrap().into_val(env);
        if event_name == events::BUY_EVENT_NAME {
            return Some(data.clone().into_val(env));
        }
    }
    None
}

#[test]
fn test_full_auction_to_bonding_curve_transition() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &BASE_PRICE);
    client.set_curve_slope(&admin, &CURVE_SLOPE);
    client.set_protocol_admin(&admin, &admin);
    client.set_fee_config(&admin, &9000u32, &1000u32);

    let creator = register_test_creator(&env, &client, "alice");
    client.configure_auction(&creator, &creator, &AUCTION_PRICE, &AUCTION_SUPPLY);

    let buyer = Address::generate(&env);

    // Buy all 5 auction keys; each must settle at the fixed auction_price,
    // each must emit an auction_purchase event, and auction_sold must track
    // the running count.
    for expected_supply in 1..=AUCTION_SUPPLY {
        let new_supply = client.buy_key(&creator, &buyer, &AUCTION_PRICE, &None);
        assert_eq!(new_supply, expected_supply);

        let price_paid = auction_purchase_price_paid(&env, &contract_id);
        assert_eq!(
            price_paid,
            Some(AUCTION_PRICE),
            "buy #{expected_supply} should emit an auction_purchase event priced at auction_price"
        );
    }

    // total_supply is 5 and auction_sold equals auction_supply after the last auction buy.
    assert_eq!(client.get_total_key_supply(&creator), AUCTION_SUPPLY);
    let auction_config = client.get_auction_config(&creator).unwrap();
    assert_eq!(auction_config.auction_sold, AUCTION_SUPPLY);
    assert_eq!(auction_config.auction_supply, AUCTION_SUPPLY);

    // The 6th buy is priced at the bonding curve formula for supply level 5,
    // not the auction price, and emits a standard buy event (not another
    // auction_purchase event).
    let expected_curve_price = client.query_price(&creator, &(AUCTION_SUPPLY as u64));
    assert_eq!(expected_curve_price, BASE_PRICE + CURVE_SLOPE * 5);
    assert_ne!(expected_curve_price, AUCTION_PRICE);

    let new_supply = client.buy_key(&creator, &buyer, &expected_curve_price, &None);
    assert_eq!(new_supply, AUCTION_SUPPLY + 1);

    assert_eq!(
        auction_purchase_price_paid(&env, &contract_id),
        None,
        "the post-auction buy must not emit another auction_purchase event"
    );
    let buy_event = standard_buy_event(&env, &contract_id)
        .expect("the post-auction buy should emit a standard KeysBought event");
    assert_eq!(buy_event.price_paid, expected_curve_price);
    assert_eq!(buy_event.new_supply, AUCTION_SUPPLY + 1);

    // The stored auction config no longer advances past auction_supply.
    assert_eq!(
        client.get_auction_config(&creator).unwrap().auction_sold,
        AUCTION_SUPPLY
    );
}
