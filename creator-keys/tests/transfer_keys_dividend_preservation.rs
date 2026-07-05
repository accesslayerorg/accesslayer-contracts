//! Integration test verifying that key transfer operations preserve
//! the claimable dividend balances of the sender and recipient wallets.

mod contract_test_env;

use contract_test_env::{
    assert_claimable, register_creator_keys, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::CurvePreset;
use soroban_sdk::{testutils::Address as _, Address, String};

const KEY_PRICE: i128 = 1000;
const CREATOR_BPS: u32 = 9000;
const PROTOCOL_BPS: u32 = 1000;

#[test]
fn test_transfer_keys_preserves_claimable_dividends() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let creator = Address::generate(&env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &Some(CurvePreset::Flat),
        &None,
        &None,
    );

    let wallet_a = Address::generate(&env);
    let wallet_b = Address::generate(&env);

    // 1. Distribute keys to Wallet A (2 keys) and Wallet B (1 key)
    let buy_quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &wallet_a, &buy_quote.total_amount, &None);
    client.buy_key(&creator, &wallet_a, &buy_quote.total_amount, &None);
    client.buy_key(&creator, &wallet_b, &buy_quote.total_amount, &None);

    assert_eq!(client.get_key_balance(&creator, &wallet_a), 2);
    assert_eq!(client.get_key_balance(&creator, &wallet_b), 1);
    assert_eq!(client.get_total_key_supply(&creator), 3);

    // 2. Distribute a dividend
    // protocol fee of 10% is deducted first.
    // distributed amount: 300
    // protocol fee: 30
    // net distributed: 270
    // per-key distribution: 270 / 3 = 90
    let distributor = Address::generate(&env);
    client.distribute_dividend(&creator, &distributor, &300);

    // 3. Record claimable balances before transfer
    // Wallet A has 2 keys: 2 * 90 = 180
    // Wallet B has 1 key: 1 * 90 = 90
    assert_claimable(&client, &creator, &wallet_a, 180);
    assert_claimable(&client, &creator, &wallet_b, 90);

    // 4. Transfer 1 key from A to B
    client.transfer_keys(&creator, &wallet_a, &wallet_b, &1);

    // Assert balances changed
    assert_eq!(client.get_key_balance(&creator, &wallet_a), 1);
    assert_eq!(client.get_key_balance(&creator, &wallet_b), 2);

    // 5. Assert claimable balances are unchanged after transfer
    assert_claimable(&client, &creator, &wallet_a, 180);
    assert_claimable(&client, &creator, &wallet_b, 90);

    // 6. Assert both wallets can claim their pre-transfer dividend amounts
    let claimed_a = client.claim_dividend(&creator, &wallet_a);
    let claimed_b = client.claim_dividend(&creator, &wallet_b);

    assert_eq!(claimed_a, 180);
    assert_eq!(claimed_b, 90);

    // Verify claimable balances become 0 after claiming
    assert_claimable(&client, &creator, &wallet_a, 0);
    assert_claimable(&client, &creator, &wallet_b, 0);
}
