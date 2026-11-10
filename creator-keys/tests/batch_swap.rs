mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{
    batch_trade::{BatchSwapExecuted, BatchSwapItem},
    ContractError, SwapDirection,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, IntoVal, Symbol, Vec,
};

fn item(key: &Address, amount: u32, direction: SwapDirection, tolerance: u32) -> BatchSwapItem {
    BatchSwapItem {
        key_id: key.clone(),
        amount,
        direction,
        max_slippage_bps: tolerance,
    }
}

#[test]
fn quote_matches_execution_and_event_including_mixed_directions() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 500, 500);
    client.set_protocol_fee_recipient(&admin, &Address::generate(&env));
    client.set_curve_slope(&admin, &10);
    let a = register_test_creator(&env, &client, "batch_a");
    let b = register_test_creator(&env, &client, "batch_b");
    let trader = Address::generate(&env);
    client.buy_keys(&b, &trader, &3, &10_000, &None);
    env.ledger().with_mut(|ledger| {
        ledger.sequence_number += 1;
        ledger.timestamp += 100;
    });
    let items = Vec::from_array(
        &env,
        [
            item(&a, 2, SwapDirection::Buy, 500),
            item(&b, 1, SwapDirection::Sell, 500),
        ],
    );
    let quote = client.get_batch_quote(&items);
    assert_eq!(client.get_supply(&a), 0);
    assert_eq!(client.get_supply(&b), 3);
    let receipt = client.batch_swap(&trader, &items);
    let mut found = false;
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&env);
        if name == Symbol::new(&env, "batch_sw") {
            let event: BatchSwapExecuted = data.into_val(&env);
            assert_eq!(event, receipt);
            found = true;
        }
    }
    assert!(found);
    assert_eq!(receipt.items, quote.items);
    assert_eq!(receipt.total_fee, quote.total_fee);
    assert_eq!(client.get_supply(&a), 2);
    assert_eq!(client.get_supply(&b), 2);
    let sum: i128 = receipt.items.iter().map(|output| output.fee).sum();
    assert_eq!(receipt.total_fee, sum);
}

#[test]
fn later_slippage_failure_rolls_back_prior_items() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 500, 500);
    client.set_protocol_fee_recipient(&admin, &Address::generate(&env));
    client.set_curve_slope(&admin, &10);
    let a = register_test_creator(&env, &client, "rollback_a");
    let b = register_test_creator(&env, &client, "rollback_b");
    let trader = Address::generate(&env);
    let items = Vec::from_array(
        &env,
        [
            item(&a, 1, SwapDirection::Buy, 500),
            item(&b, 1, SwapDirection::Buy, 1),
        ],
    );
    assert_eq!(
        client.try_batch_swap(&trader, &items),
        Err(Ok(ContractError::SlippageExceeded))
    );
    for key in [&a, &b] {
        assert_eq!(client.get_supply(key), 0);
        assert_eq!(client.get_key_balance(key, &trader), 0);
    }
}

#[test]
fn rejects_empty_and_sixteen_items_and_zero_amount() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let trader = Address::generate(&env);
    let mut items = Vec::new(&env);
    assert_eq!(
        client.try_batch_swap(&trader, &items),
        Err(Ok(ContractError::BatchSizeExceeded))
    );
    for _ in 0..16 {
        items.push_back(item(&trader, 1, SwapDirection::Buy, 0));
    }
    assert_eq!(
        client.try_get_batch_quote(&items),
        Err(Ok(ContractError::BatchSizeExceeded))
    );
    assert_eq!(
        client.try_batch_swap(&trader, &items),
        Err(Ok(ContractError::BatchSizeExceeded))
    );
    assert_eq!(
        client.try_get_batch_quote(&Vec::from_array(
            &env,
            [item(&trader, 0, SwapDirection::Buy, 0)]
        )),
        Err(Ok(ContractError::NotPositiveAmount))
    );
}

#[test]
fn repeated_key_quotes_walk_virtual_supply() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 500, 500);
    client.set_protocol_fee_recipient(&admin, &Address::generate(&env));
    client.set_curve_slope(&admin, &10);
    let key = register_test_creator(&env, &client, "repeated");
    let trader = Address::generate(&env);
    let items = Vec::from_array(
        &env,
        [
            item(&key, 1, SwapDirection::Buy, 0),
            item(&key, 1, SwapDirection::Buy, 0),
        ],
    );
    let quote = client.get_batch_quote(&items);
    assert_eq!(quote.items.get(0).unwrap().output, 1_000);
    assert_eq!(quote.items.get(1).unwrap().output, 1_010);
    assert_eq!(client.batch_swap(&trader, &items).items, quote.items);
}
