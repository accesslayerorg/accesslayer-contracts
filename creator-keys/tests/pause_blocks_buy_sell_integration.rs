//! Integration test for issue #643.
//!
//! Scope:
//!   - Initialize the contract and pause it via the admin pause function
//!   - Invoke buy and assert the transaction reverts with the paused error code
//!   - Invoke sell and assert it also reverts with the paused error code
//!   - Unpause the contract and assert a subsequent buy succeeds
//!   - Assert no supply or balance changes occur while the contract is paused

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

#[test]
fn test_buy_and_sell_revert_while_paused_then_buy_succeeds_after_unpause() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Baseline: one key purchased before pausing so sell has something to act on.
    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    let supply_before_pause = client.get_total_key_supply(&creator);
    let balance_before_pause = client.get_key_balance(&creator, &buyer);
    assert_eq!(supply_before_pause, 1);
    assert_eq!(balance_before_pause, 1);

    // --- Pause the contract via the admin function ---
    client.pause(&admin);
    assert!(client.get_is_paused());

    // --- buy reverts with the paused error code while paused ---
    let buy_quote_while_paused = client.get_buy_quote(&creator);
    let buy_result = client.try_buy_key(
        &creator,
        &buyer,
        &buy_quote_while_paused.total_amount,
        &None,
    );
    assert_eq!(
        buy_result,
        Err(Ok(ContractError::ProtocolPaused)),
        "buy must revert with ProtocolPaused while contract is paused"
    );

    // --- sell reverts with the paused error code while paused ---
    let sell_result = client.try_sell_key(&creator, &buyer, &None);
    assert_eq!(
        sell_result,
        Err(Ok(ContractError::ProtocolPaused)),
        "sell must revert with ProtocolPaused while contract is paused"
    );

    // --- No supply or balance changes occur while paused ---
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before_pause,
        "supply must be unchanged after failed buy/sell attempts while paused"
    );
    assert_eq!(
        client.get_key_balance(&creator, &buyer),
        balance_before_pause,
        "buyer balance must be unchanged after failed buy/sell attempts while paused"
    );

    // --- Unpause the contract ---
    client.unpause(&admin);
    assert!(!client.get_is_paused());

    // --- A subsequent buy succeeds after unpausing ---
    let quote_after_unpause = client.get_buy_quote(&creator);
    let new_supply = client.buy_key(&creator, &buyer, &quote_after_unpause.total_amount, &None);

    assert_eq!(
        new_supply,
        supply_before_pause + 1,
        "buy after unpause must succeed and increment supply by exactly 1"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before_pause + 1
    );
    assert_eq!(
        client.get_key_balance(&creator, &buyer),
        balance_before_pause + 1
    );
}
