//! Tests for the pre-launch fixed-price auction phase (#787 / #790 / #793).

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{events, AuctionConfig, ContractError, FeatureError};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, Symbol,
};

#[test]
fn test_configure_auction_stores_config_and_emits_event() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");

    client.configure_auction(&creator, &creator, &500i128, &10u32);

    let mut found = false;
    for (contract, topics, data) in env.events().all().iter() {
        if contract != contract_id {
            continue;
        }
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        if event_name == events::AUCTION_CONFIGURED_EVENT_NAME {
            let payload: events::AuctionConfiguredEvent = data.clone().into_val(&env);
            assert_eq!(payload.creator_id, creator);
            assert_eq!(payload.auction_price, 500);
            assert_eq!(payload.auction_supply, 10);
            found = true;
        }
    }
    assert!(found, "expected an AuctionConfigured event");

    assert_eq!(
        client.get_auction_config(&creator),
        Some(AuctionConfig {
            auction_price: 500,
            auction_supply: 10,
            auction_sold: 0,
        })
    );
}

#[test]
fn test_configure_auction_rejects_non_creator_caller() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");
    let attacker = Address::generate(&env);

    let result = client.try_configure_auction(&creator, &attacker, &500i128, &10u32);
    assert_eq!(result, Err(Ok(FeatureError::Unauthorized)));
    assert_eq!(client.get_auction_config(&creator), None);
}

#[test]
fn test_configure_auction_rejects_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = Address::generate(&env);

    let result = client.try_configure_auction(&creator, &creator, &500i128, &10u32);
    assert_eq!(result, Err(Ok(FeatureError::NotRegistered)));
}

#[test]
fn test_configure_auction_rejects_once_supply_is_nonzero() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000i128, &None);

    let result = client.try_configure_auction(&creator, &creator, &500i128, &10u32);
    assert_eq!(result, Err(Ok(FeatureError::AuctionAlreadyStarted)));
}

#[test]
fn test_configure_auction_rejects_non_positive_price() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");

    let result = client.try_configure_auction(&creator, &creator, &0i128, &10u32);
    assert_eq!(result, Err(Ok(FeatureError::NotPositiveAmount)));
}

#[test]
fn test_configure_auction_rejects_invalid_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");

    let zero_supply = client.try_configure_auction(&creator, &creator, &500i128, &0u32);
    assert_eq!(zero_supply, Err(Ok(FeatureError::InvalidAuctionConfig)));

    let too_much_supply = client.try_configure_auction(&creator, &creator, &500i128, &10_001u32);
    assert_eq!(too_much_supply, Err(Ok(FeatureError::InvalidAuctionConfig)));
}

#[test]
fn test_cancel_auction_removes_config_and_emits_event() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");
    client.configure_auction(&creator, &creator, &500i128, &10u32);

    client.cancel_auction(&creator, &creator);

    let mut found = false;
    for (contract, topics, data) in env.events().all().iter() {
        if contract != contract_id {
            continue;
        }
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        if event_name == events::AUCTION_CANCELLED_EVENT_NAME {
            let payload: events::AuctionCancelledEvent = data.clone().into_val(&env);
            assert_eq!(payload.creator_id, creator);
            found = true;
        }
    }
    assert!(found, "expected an AuctionCancelled event");

    assert_eq!(client.get_auction_config(&creator), None);
}

#[test]
fn test_cancel_auction_rejects_non_creator_caller() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");
    client.configure_auction(&creator, &creator, &500i128, &10u32);
    let attacker = Address::generate(&env);

    let result = client.try_cancel_auction(&creator, &attacker);
    assert_eq!(result, Err(Ok(FeatureError::Unauthorized)));
    assert!(client.get_auction_config(&creator).is_some());
}

#[test]
fn test_cancel_auction_fails_when_none_configured() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");

    let result = client.try_cancel_auction(&creator, &creator);
    assert_eq!(result, Err(Ok(FeatureError::NoAuctionConfigured)));
}

#[test]
fn test_cancel_auction_fails_after_a_purchase() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");
    client.configure_auction(&creator, &creator, &500i128, &10u32);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &500i128, &None);

    let result = client.try_cancel_auction(&creator, &creator);
    assert_eq!(result, Err(Ok(FeatureError::AuctionAlreadyStarted)));
}

#[test]
fn test_buy_key_during_auction_settles_at_fixed_price_and_emits_auction_purchase_event() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");
    client.configure_auction(&creator, &creator, &500i128, &2u32);
    let buyer = Address::generate(&env);

    let new_supply = client.buy_key(&creator, &buyer, &500i128, &None);
    assert_eq!(new_supply, 1);

    let mut found = false;
    for (contract, topics, data) in env.events().all().iter() {
        if contract != contract_id {
            continue;
        }
        let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
        if event_name == events::AUCTION_PURCHASE_EVENT_NAME {
            let payload: events::AuctionPurchaseEvent = data.clone().into_val(&env);
            assert_eq!(payload.buyer, buyer);
            assert_eq!(payload.creator_id, creator);
            assert_eq!(payload.price_paid, 500);
            assert_eq!(payload.auction_sold, 1);
            found = true;
        }
    }
    assert!(found, "expected an AuctionPurchase event");

    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
    assert_eq!(
        client.get_auction_config(&creator),
        Some(AuctionConfig {
            auction_price: 500,
            auction_supply: 2,
            auction_sold: 1,
        })
    );

    // Underpaying the fixed auction price still fails, exactly like a bonding-curve buy.
    let underpay = client.try_buy_key(&creator, &buyer, &499i128, &None);
    assert_eq!(underpay, Err(Ok(ContractError::InsufficientPayment)));
}

#[test]
fn test_buy_key_transitions_to_bonding_curve_once_auction_supply_is_exhausted() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1000i128);
    let creator = register_test_creator(&env, &client, "alice");
    client.configure_auction(&creator, &creator, &500i128, &2u32);

    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);
    let buyer_c = Address::generate(&env);

    client.buy_key(&creator, &buyer_a, &500i128, &None);
    client.buy_key(&creator, &buyer_b, &500i128, &None);

    assert_eq!(client.get_auction_config(&creator).unwrap().auction_sold, 2);

    // Auction supply (2) is now exhausted; the next buy settles at the base
    // (bonding-curve) price, not the fixed auction price.
    let new_supply = client.buy_key(&creator, &buyer_c, &1000i128, &None);
    assert_eq!(new_supply, 3);

    // The stored auction config no longer advances past the configured supply.
    assert_eq!(
        client.get_auction_config(&creator),
        Some(AuctionConfig {
            auction_price: 500,
            auction_supply: 2,
            auction_sold: 2,
        })
    );
}

#[test]
fn test_get_buy_quote_reflects_auction_price_then_bonding_curve_price() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 1000i128, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client, "alice");
    client.configure_auction(&creator, &creator, &500i128, &2u32);

    // Before any auction sale: quote must reflect the fixed auction price, not
    // the base (bonding-curve) key price.
    assert_eq!(client.get_buy_quote(&creator).price, 500);

    let buyer_a = Address::generate(&env);
    let buyer_b = Address::generate(&env);
    client.buy_key(&creator, &buyer_a, &500i128, &None);

    // Still one auction slot left: quote must still be the auction price.
    assert_eq!(client.get_buy_quote(&creator).price, 500);

    client.buy_key(&creator, &buyer_b, &500i128, &None);

    // Auction supply exhausted: quote must fall back to the bonding-curve price.
    assert_eq!(client.get_buy_quote(&creator).price, 1000);
}
