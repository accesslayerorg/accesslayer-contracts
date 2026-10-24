#![cfg(test)]

use crate::{
    depth_updated_topics, DepthLevel, DepthUpdated, KeyLiquidityDepthContract,
    KeyLiquidityDepthContractClient, LiquidityDepthError, OrderSide, DEPTH_UPDATED_EVENT_NAME,
};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol,
};

fn setup_test() -> (Env, Address, KeyLiquidityDepthContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KeyLiquidityDepthContract, ());
    let client = KeyLiquidityDepthContractClient::new(&env, &contract_id);
    let key_id = Address::generate(&env);
    (env, key_id, client)
}

// ── 1. Order depth recorded correctly for both buy and sell sides ─────────────

#[test]
fn test_record_order_buy_and_sell_sides() {
    let (_env, key_id, client) = setup_test();

    // Record buy order: price 100, amount 50
    client.record_order(&key_id, &100, &50, &OrderSide::Buy);
    // Record sell order: price 110, amount 30
    client.record_order(&key_id, &110, &30, &OrderSide::Sell);

    let depth = client.get_depth(&key_id, &10);
    assert_eq!(depth.bids.len(), 1);
    assert_eq!(depth.asks.len(), 1);

    let bid = depth.bids.get(0).unwrap();
    assert_eq!(bid.price, 100);
    assert_eq!(bid.volume, 50);
    assert_eq!(bid.cumulative_volume, 50);

    let ask = depth.asks.get(0).unwrap();
    assert_eq!(ask.price, 110);
    assert_eq!(ask.volume, 30);
    assert_eq!(ask.cumulative_volume, 30);

    assert_eq!(
        client.get_price_level_volume(&key_id, &100, &OrderSide::Buy),
        50
    );
    assert_eq!(
        client.get_price_level_volume(&key_id, &110, &OrderSide::Sell),
        30
    );
}

#[test]
fn test_record_multiple_orders_accumulates_at_same_price() {
    let (_env, key_id, client) = setup_test();

    client.record_order(&key_id, &100, &50, &OrderSide::Buy);
    client.record_order(&key_id, &100, &75, &OrderSide::Buy);
    client.record_order(&key_id, &100, &25, &OrderSide::Buy);

    let depth = client.get_depth(&key_id, &10);
    assert_eq!(depth.bids.len(), 1);

    let bid = depth.bids.get(0).unwrap();
    assert_eq!(bid.price, 100);
    assert_eq!(bid.volume, 150);
    assert_eq!(bid.cumulative_volume, 150);
    assert_eq!(
        client.get_price_level_volume(&key_id, &100, &OrderSide::Buy),
        150
    );
}

#[test]
fn test_order_book_sorting_bids_descending_asks_ascending() {
    let (_env, key_id, client) = setup_test();

    // Insert bids out of order: 90, 110, 100, 80, 105
    client.record_order(&key_id, &90, &10, &OrderSide::Buy);
    client.record_order(&key_id, &110, &20, &OrderSide::Buy);
    client.record_order(&key_id, &100, &30, &OrderSide::Buy);
    client.record_order(&key_id, &80, &40, &OrderSide::Buy);
    client.record_order(&key_id, &105, &50, &OrderSide::Buy);

    // Insert asks out of order: 120, 150, 115, 130, 140
    client.record_order(&key_id, &120, &15, &OrderSide::Sell);
    client.record_order(&key_id, &150, &25, &OrderSide::Sell);
    client.record_order(&key_id, &115, &35, &OrderSide::Sell);
    client.record_order(&key_id, &130, &45, &OrderSide::Sell);
    client.record_order(&key_id, &140, &55, &OrderSide::Sell);

    let depth = client.get_depth(&key_id, &10);

    // Bids must be descending: 110, 105, 100, 90, 80
    assert_eq!(depth.bids.len(), 5);
    assert_eq!(depth.bids.get(0).unwrap().price, 110);
    assert_eq!(depth.bids.get(1).unwrap().price, 105);
    assert_eq!(depth.bids.get(2).unwrap().price, 100);
    assert_eq!(depth.bids.get(3).unwrap().price, 90);
    assert_eq!(depth.bids.get(4).unwrap().price, 80);

    // Asks must be ascending: 115, 120, 130, 140, 150
    assert_eq!(depth.asks.len(), 5);
    assert_eq!(depth.asks.get(0).unwrap().price, 115);
    assert_eq!(depth.asks.get(1).unwrap().price, 120);
    assert_eq!(depth.asks.get(2).unwrap().price, 130);
    assert_eq!(depth.asks.get(3).unwrap().price, 140);
    assert_eq!(depth.asks.get(4).unwrap().price, 150);
}

// ── 2. cancel_order_depth correctly removes cancelled volumes ─────────────────

#[test]
fn test_cancel_order_depth_partial_and_full() {
    let (_env, key_id, client) = setup_test();

    client.record_order(&key_id, &100, &100, &OrderSide::Buy);

    // Partial cancel: 40 out of 100
    client.cancel_order_depth(&key_id, &100, &40, &OrderSide::Buy);

    let depth = client.get_depth(&key_id, &10);
    assert_eq!(depth.bids.len(), 1);
    let bid = depth.bids.get(0).unwrap();
    assert_eq!(bid.price, 100);
    assert_eq!(bid.volume, 60);
    assert_eq!(bid.cumulative_volume, 60);
    assert_eq!(
        client.get_price_level_volume(&key_id, &100, &OrderSide::Buy),
        60
    );

    // Remaining cancel: 60 out of 60 (full level exhaustion)
    client.cancel_order_depth(&key_id, &100, &60, &OrderSide::Buy);

    let depth_after = client.get_depth(&key_id, &10);
    assert_eq!(depth_after.bids.len(), 0);
    assert_eq!(
        client.get_price_level_volume(&key_id, &100, &OrderSide::Buy),
        0
    );
}

#[test]
fn test_cancel_order_depth_re_adding_after_exhaustion() {
    let (_env, key_id, client) = setup_test();

    // Add and exhaust level
    client.record_order(&key_id, &100, &50, &OrderSide::Buy);
    client.cancel_order_depth(&key_id, &100, &50, &OrderSide::Buy);
    assert_eq!(client.get_depth(&key_id, &10).bids.len(), 0);

    // Re-add at same level
    client.record_order(&key_id, &100, &80, &OrderSide::Buy);
    let depth = client.get_depth(&key_id, &10);
    assert_eq!(depth.bids.len(), 1);
    assert_eq!(depth.bids.get(0).unwrap().volume, 80);
}

#[test]
fn test_cancel_non_existent_price_level_fails() {
    let (_env, key_id, client) = setup_test();

    let res = client.try_cancel_order_depth(&key_id, &100, &10, &OrderSide::Buy);
    assert_eq!(res, Err(Ok(LiquidityDepthError::PriceLevelNotFound)));
}

#[test]
fn test_cancel_amount_exceeding_depth_fails() {
    let (_env, key_id, client) = setup_test();

    client.record_order(&key_id, &100, &50, &OrderSide::Sell);

    let res = client.try_cancel_order_depth(&key_id, &100, &51, &OrderSide::Sell);
    assert_eq!(res, Err(Ok(LiquidityDepthError::InsufficientDepth)));

    // State remains untouched
    assert_eq!(
        client.get_price_level_volume(&key_id, &100, &OrderSide::Sell),
        50
    );
}

// ── 3. get_depth returns accurate cumulative volumes at each level ───────────

#[test]
fn test_get_depth_cumulative_volumes_bids_and_asks() {
    let (_env, key_id, client) = setup_test();

    // Bids: 105 (vol 20), 100 (vol 30), 95 (vol 50)
    client.record_order(&key_id, &100, &30, &OrderSide::Buy);
    client.record_order(&key_id, &105, &20, &OrderSide::Buy);
    client.record_order(&key_id, &95, &50, &OrderSide::Buy);

    // Asks: 110 (vol 40), 115 (vol 60), 120 (vol 100)
    client.record_order(&key_id, &120, &100, &OrderSide::Sell);
    client.record_order(&key_id, &110, &40, &OrderSide::Sell);
    client.record_order(&key_id, &115, &60, &OrderSide::Sell);

    let depth = client.get_depth(&key_id, &10);

    // Check bids cumulative volume
    assert_eq!(depth.bids.len(), 3);
    assert_eq!(
        depth.bids.get(0).unwrap(),
        DepthLevel {
            price: 105,
            volume: 20,
            cumulative_volume: 20
        }
    );
    assert_eq!(
        depth.bids.get(1).unwrap(),
        DepthLevel {
            price: 100,
            volume: 30,
            cumulative_volume: 50
        }
    );
    assert_eq!(
        depth.bids.get(2).unwrap(),
        DepthLevel {
            price: 95,
            volume: 50,
            cumulative_volume: 100
        }
    );

    // Check asks cumulative volume
    assert_eq!(depth.asks.len(), 3);
    assert_eq!(
        depth.asks.get(0).unwrap(),
        DepthLevel {
            price: 110,
            volume: 40,
            cumulative_volume: 40
        }
    );
    assert_eq!(
        depth.asks.get(1).unwrap(),
        DepthLevel {
            price: 115,
            volume: 60,
            cumulative_volume: 100
        }
    );
    assert_eq!(
        depth.asks.get(2).unwrap(),
        DepthLevel {
            price: 120,
            volume: 100,
            cumulative_volume: 200
        }
    );
}

#[test]
fn test_get_depth_level_capping() {
    let (_env, key_id, client) = setup_test();

    client.record_order(&key_id, &100, &10, &OrderSide::Buy);
    client.record_order(&key_id, &95, &20, &OrderSide::Buy);
    client.record_order(&key_id, &90, &30, &OrderSide::Buy);

    // Level 0 returns empty
    let depth0 = client.get_depth(&key_id, &0);
    assert_eq!(depth0.bids.len(), 0);
    assert_eq!(depth0.asks.len(), 0);

    // Level 2 returns top 2
    let depth2 = client.get_depth(&key_id, &2);
    assert_eq!(depth2.bids.len(), 2);
    assert_eq!(depth2.bids.get(0).unwrap().price, 100);
    assert_eq!(depth2.bids.get(1).unwrap().price, 95);

    // Level 10 returns all 3
    let depth10 = client.get_depth(&key_id, &10);
    assert_eq!(depth10.bids.len(), 3);
}

#[test]
fn test_get_depth_unrecorded_key_returns_empty() {
    let (env, _key_id, client) = setup_test();
    let unused_key = Address::generate(&env);

    let depth = client.get_depth(&unused_key, &5);
    assert_eq!(depth.bids.len(), 0);
    assert_eq!(depth.asks.len(), 0);
}

#[test]
fn test_multiple_keys_isolated_depth() {
    let (env, key1, client) = setup_test();
    let key2 = Address::generate(&env);

    client.record_order(&key1, &100, &50, &OrderSide::Buy);
    client.record_order(&key2, &200, &80, &OrderSide::Buy);

    let depth1 = client.get_depth(&key1, &10);
    let depth2 = client.get_depth(&key2, &10);

    assert_eq!(depth1.bids.len(), 1);
    assert_eq!(depth1.bids.get(0).unwrap().price, 100);
    assert_eq!(depth1.bids.get(0).unwrap().volume, 50);

    assert_eq!(depth2.bids.len(), 1);
    assert_eq!(depth2.bids.get(0).unwrap().price, 200);
    assert_eq!(depth2.bids.get(0).unwrap().volume, 80);
}

// ── 4. Depth updates atomically with no partial state ─────────────────────────

#[test]
fn test_invalid_parameters_reject_without_state_change() {
    let (_env, key_id, client) = setup_test();

    // Invalid price <= 0
    let res1 = client.try_record_order(&key_id, &0, &100, &OrderSide::Buy);
    assert_eq!(res1, Err(Ok(LiquidityDepthError::InvalidPrice)));

    let res2 = client.try_record_order(&key_id, &-5, &100, &OrderSide::Buy);
    assert_eq!(res2, Err(Ok(LiquidityDepthError::InvalidPrice)));

    // Invalid amount <= 0
    let res3 = client.try_record_order(&key_id, &100, &0, &OrderSide::Buy);
    assert_eq!(res3, Err(Ok(LiquidityDepthError::InvalidAmount)));

    let res4 = client.try_record_order(&key_id, &100, &-10, &OrderSide::Buy);
    assert_eq!(res4, Err(Ok(LiquidityDepthError::InvalidAmount)));

    // Cancel invalid price/amount
    let res5 = client.try_cancel_order_depth(&key_id, &0, &50, &OrderSide::Buy);
    assert_eq!(res5, Err(Ok(LiquidityDepthError::InvalidPrice)));

    let res6 = client.try_cancel_order_depth(&key_id, &100, &0, &OrderSide::Buy);
    assert_eq!(res6, Err(Ok(LiquidityDepthError::InvalidAmount)));

    // Verify completely empty state
    let depth = client.get_depth(&key_id, &10);
    assert_eq!(depth.bids.len(), 0);
    assert_eq!(depth.asks.len(), 0);
}

// ── 5. DepthUpdated emitted with correct key and price level data ──────────────

#[test]
fn test_depth_updated_event_emitted_on_record_and_cancel() {
    let (env, key_id, client) = setup_test();

    // 1. Record order
    client.record_order(&key_id, &100, &50, &OrderSide::Buy);

    let events = env.events().all();
    assert_eq!(events.len(), 1);

    let (contract_id, topics, data) = events.get(0).unwrap();
    assert_eq!(contract_id, client.address);

    let (expected_sym, expected_key) = depth_updated_topics(&key_id);
    let sym: Symbol = topics.get(0).unwrap().into_val(&env);
    let addr: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(sym, expected_sym);
    assert_eq!(addr, expected_key);

    let payload: DepthUpdated = data.into_val(&env);
    assert_eq!(payload.key_id, key_id);
    assert_eq!(payload.price, 100);
    assert_eq!(payload.amount, 50);
    assert_eq!(payload.side, OrderSide::Buy);
    assert_eq!(payload.volume, 50);

    // 2. Cancel partial
    client.cancel_order_depth(&key_id, &100, &20, &OrderSide::Buy);

    let events_after = env.events().all();
    assert_eq!(events_after.len(), 1);

    let (_, cancel_topics, cancel_data) = events_after.get(0).unwrap();
    let cancel_sym: Symbol = cancel_topics.get(0).unwrap().into_val(&env);
    assert_eq!(cancel_sym, DEPTH_UPDATED_EVENT_NAME);

    let cancel_payload: DepthUpdated = cancel_data.into_val(&env);
    assert_eq!(cancel_payload.key_id, key_id);
    assert_eq!(cancel_payload.price, 100);
    assert_eq!(cancel_payload.amount, 20);
    assert_eq!(cancel_payload.side, OrderSide::Buy);
    assert_eq!(cancel_payload.volume, 30); // 50 - 20 = 30
}

// ── 6. Cross-contract call from mock swap and limit order contract ─────────────

#[contract]
struct MockSwapContract;

#[contractimpl]
impl MockSwapContract {
    pub fn place_limit_order(
        env: Env,
        depth_tracker: Address,
        key_id: Address,
        price: i128,
        amount: i128,
        side: OrderSide,
    ) {
        let client = KeyLiquidityDepthContractClient::new(&env, &depth_tracker);
        client.record_order(&key_id, &price, &amount, &side);
    }

    pub fn cancel_limit_order(
        env: Env,
        depth_tracker: Address,
        key_id: Address,
        price: i128,
        amount: i128,
        side: OrderSide,
    ) {
        let client = KeyLiquidityDepthContractClient::new(&env, &depth_tracker);
        client.cancel_order_depth(&key_id, &price, &amount, &side);
    }
}

#[test]
fn test_cross_contract_integration() {
    let env = Env::default();
    env.mock_all_auths();

    let depth_tracker_id = env.register(KeyLiquidityDepthContract, ());
    let depth_client = KeyLiquidityDepthContractClient::new(&env, &depth_tracker_id);

    let mock_swap_id = env.register(MockSwapContract, ());
    let mock_client = MockSwapContractClient::new(&env, &mock_swap_id);

    let key_id = Address::generate(&env);

    // Call through mock swap / limit order contract
    mock_client.place_limit_order(&depth_tracker_id, &key_id, &150, &60, &OrderSide::Sell);
    mock_client.place_limit_order(&depth_tracker_id, &key_id, &140, &40, &OrderSide::Sell);

    let depth = depth_client.get_depth(&key_id, &5);
    assert_eq!(depth.asks.len(), 2);
    assert_eq!(depth.asks.get(0).unwrap().price, 140);
    assert_eq!(depth.asks.get(0).unwrap().volume, 40);
    assert_eq!(depth.asks.get(1).unwrap().price, 150);
    assert_eq!(depth.asks.get(1).unwrap().volume, 60);

    // Cancel through mock contract
    mock_client.cancel_limit_order(&depth_tracker_id, &key_id, &140, &40, &OrderSide::Sell);

    let depth_after = depth_client.get_depth(&key_id, &5);
    assert_eq!(depth_after.asks.len(), 1);
    assert_eq!(depth_after.asks.get(0).unwrap().price, 150);
}
