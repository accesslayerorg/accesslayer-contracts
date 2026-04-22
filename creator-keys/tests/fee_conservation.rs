//! Tests specifically verifying that fee accounting conserves value
//! across protocol and creator fee paths.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Env, String};

fn setup_test(
    env: &Env,
) -> (
    CreatorKeysContractClient<'_>,
    soroban_sdk::Address,
    soroban_sdk::Address,
) {
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &contract_id);

    let admin = soroban_sdk::Address::generate(env);
    let creator = soroban_sdk::Address::generate(env);
    let handle = String::from_str(env, "alice");
    client.register_creator(&creator, &handle);

    (client, admin, creator)
}

#[test]
fn test_fee_conservation_across_price_range() {
    let env = Env::default();
    let (client, admin, _creator) = setup_test(&env);

    // Test across various price points
    let prices = [1, 7, 99, 100, 1000, 12345, 100000, 1234567];
    // Test across various fee configurations (summing to 10000 BPS)
    let fee_configs = [
        (9000, 1000), // 90% creator, 10% protocol
        (5000, 5000), // 50/50
        (8000, 2000), // 80/20
        (9900, 100),  // 99/1
    ];

    for price in prices {
        for (creator_bps, protocol_bps) in fee_configs {
            client.set_key_price(&admin, &price);
            client.set_fee_config(&admin, &creator_bps, &protocol_bps);

            let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&price);

            // ASSERTION: creator_fee + protocol_fee must EXACTLY equal the price
            // because compute_fees_for_payment takes 'total' and splits it.
            // In the contract, quotes use 'price' as the 'total' for fee calculation.
            assert_eq!(
                creator_fee + protocol_fee,
                price,
                "Value lost/created in split for price={}, bps=({}/{})",
                price,
                creator_bps,
                protocol_bps
            );
        }
    }
}

#[test]
fn test_quote_total_conservation() {
    let env = Env::default();
    let (client, admin, creator) = setup_test(&env);

    let price = 1000;
    client.set_key_price(&admin, &price);
    client.set_fee_config(&admin, &9000, &1000);

    // Buy Quote
    let buy_quote = client.get_buy_quote(&creator);
    assert_eq!(
        buy_quote.total_amount,
        buy_quote.price + buy_quote.creator_fee + buy_quote.protocol_fee,
        "Buy quote total amount does not conserve value"
    );

    // Sell Quote
    let holder = soroban_sdk::Address::generate(&env);
    client.buy_key(&creator, &holder, &2000); // 1000 price + 1000 fees

    let sell_quote = client.get_sell_quote(&creator, &holder);
    assert_eq!(
        sell_quote.total_amount,
        sell_quote.price - (sell_quote.creator_fee + sell_quote.protocol_fee),
        "Sell quote total amount does not conserve value"
    );
}

#[test]
fn test_low_value_trade_dust_handling() {
    let env = Env::default();
    let (client, admin, _creator) = setup_test(&env);

    // Price of 1 is the absolute minimum positive price
    let price = 1;
    client.set_key_price(&admin, &price);
    client.set_fee_config(&admin, &9000, &1000);

    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&price);

    // 1 * 1000 / 10000 = 0.1 -> 0 protocol fee
    // Creator gets remainder: 1 - 0 = 1
    assert_eq!(protocol_fee, 0);
    assert_eq!(creator_fee, 1);
    assert_eq!(creator_fee + protocol_fee, price);

    // Price where protocol fee is exactly 1
    let price = 10; // 10 * 1000 / 10000 = 1
    client.set_key_price(&admin, &price);
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&price);
    assert_eq!(protocol_fee, 1);
    assert_eq!(creator_fee, 9);
    assert_eq!(creator_fee + protocol_fee, price);
}

#[test]
fn test_fee_split_bps_edge_cases() {
    let env = Env::default();
    let (client, admin, _creator) = setup_test(&env);

    let price = 1000;
    client.set_key_price(&admin, &price);

    // 100% Creator
    client.set_fee_config(&admin, &10000, &0);
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&price);
    assert_eq!(creator_fee, 1000);
    assert_eq!(protocol_fee, 0);
    assert_eq!(creator_fee + protocol_fee, price);

    // 50% Protocol (Max allowed by contract)
    client.set_fee_config(&admin, &5000, &5000);
    let (creator_fee, protocol_fee) = client.compute_fees_for_payment(&price);
    assert_eq!(creator_fee, 500);
    assert_eq!(protocol_fee, 500);
    assert_eq!(creator_fee + protocol_fee, price);
}
