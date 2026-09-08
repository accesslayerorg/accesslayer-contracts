//! Unit tests for `buy_key` keeping the creator's stored total supply correct (#739).
//!
//! Supply is the number every price quote, cap check and dividend split is
//! computed from, so the field written on a buy has to equal the pre-buy supply
//! plus what was bought — and the `get_total_key_supply` view has to report the
//! same number the buy returned. A buy that fails must leave it alone entirely.
//!
//! `buy_key` buys one key per call, so "buying 5 keys" below is five calls.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{ContractError, CreatorKeysContractClient, RegisterCreatorParams};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

/// Deploy the contract with pricing, fees, a protocol admin and one creator.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(env, &client, "alice");
    (client, admin, creator)
}

/// Register a creator carrying a hard supply cap.
///
/// The cap can only be set at registration — there is no setter for it — so a
/// capped creator has to be registered separately from the fixture one.
fn register_capped_creator(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    handle: &str,
    max_supply: u32,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &Some(max_supply),
        &None,
        &None,
        &None,
        &None,
    );
    creator
}

/// Buy one key at the current quote, returning the supply the call reports.
fn buy_one(client: &CreatorKeysContractClient<'_>, creator: &Address, buyer: &Address) -> u32 {
    let quote = client.get_buy_quote(creator);
    client.buy_key(creator, buyer, &quote.total_amount, &None)
}

/// Buy `count` keys for `buyer`, asserting the view agrees with the return
/// value after every single call.
fn buy_keys_checking_supply(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    count: u32,
) -> u32 {
    let mut last_supply = client.get_total_key_supply(creator);
    for _ in 0..count {
        let before = last_supply;
        last_supply = buy_one(client, creator, buyer);
        assert_eq!(
            last_supply,
            before + 1,
            "each buy must increment supply by exactly one"
        );
        assert_eq!(
            client.get_total_key_supply(creator),
            last_supply,
            "get_total_key_supply must agree with the value buy_key returned"
        );
    }
    last_supply
}

// ---------------------------------------------------------------------------
// Supply grows by exactly what was bought
// ---------------------------------------------------------------------------

#[test]
fn test_buying_one_key_from_zero_supply_sets_supply_to_one() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    assert_eq!(
        client.get_total_key_supply(&creator),
        0,
        "a freshly registered creator starts at zero supply"
    );

    assert_eq!(buy_one(&client, &creator, &buyer), 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);
}

#[test]
fn test_buying_five_keys_from_supply_ten_sets_supply_to_fifteen() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let early_buyer = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Establish the starting supply of 10.
    buy_keys_checking_supply(&client, &creator, &early_buyer, 10);
    assert_eq!(client.get_total_key_supply(&creator), 10);

    assert_eq!(
        buy_keys_checking_supply(&client, &creator, &buyer, 5),
        15,
        "five more buys on a supply of ten must land on fifteen"
    );
    assert_eq!(client.get_total_key_supply(&creator), 15);
}

#[test]
fn test_sequential_buys_from_different_wallets_each_increment_supply() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let first = Address::generate(&env);
    let second = Address::generate(&env);

    assert_eq!(buy_one(&client, &creator, &first), 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);

    assert_eq!(
        buy_one(&client, &creator, &second),
        2,
        "a second wallet's buy must continue the same supply counter"
    );
    assert_eq!(client.get_total_key_supply(&creator), 2);

    // Supply counts keys in circulation, not distinct holders.
    assert_eq!(client.get_key_balance(&creator, &first), 1);
    assert_eq!(client.get_key_balance(&creator, &second), 1);
}

#[test]
fn test_interleaved_buys_across_wallets_accumulate_supply_in_order() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let first = Address::generate(&env);
    let second = Address::generate(&env);

    let wallets = [&first, &second, &first, &second, &first];
    for (index, wallet) in wallets.iter().enumerate() {
        let expected = index as u32 + 1;
        assert_eq!(buy_one(&client, &creator, wallet), expected);
        assert_eq!(client.get_total_key_supply(&creator), expected);
    }

    assert_eq!(client.get_key_balance(&creator, &first), 3);
    assert_eq!(client.get_key_balance(&creator, &second), 2);
}

// ---------------------------------------------------------------------------
// A rejected buy leaves supply exactly where it was
// ---------------------------------------------------------------------------

#[test]
fn test_supply_unchanged_after_a_buy_rejected_on_underpayment() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    buy_keys_checking_supply(&client, &creator, &buyer, 3);
    let supply_before = client.get_total_key_supply(&creator);
    let balance_before = client.get_key_balance(&creator, &buyer);

    // One stroop short of the quoted price.
    let quote = client.get_buy_quote(&creator);
    assert_eq!(
        client.try_buy_key(&creator, &buyer, &(quote.price - 1), &None),
        Err(Ok(ContractError::InsufficientPayment))
    );

    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "a rejected buy must not move supply"
    );
    assert_eq!(client.get_key_balance(&creator, &buyer), balance_before);
}

#[test]
fn test_supply_unchanged_after_a_buy_rejected_on_non_positive_payment() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    buy_keys_checking_supply(&client, &creator, &buyer, 2);
    let supply_before = client.get_total_key_supply(&creator);

    assert_eq!(
        client.try_buy_key(&creator, &buyer, &0, &None),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    assert_eq!(client.get_total_key_supply(&creator), supply_before);

    assert_eq!(
        client.try_buy_key(&creator, &buyer, &-1, &None),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    assert_eq!(client.get_total_key_supply(&creator), supply_before);
}

#[test]
fn test_supply_unchanged_after_a_buy_rejected_on_the_supply_cap() {
    let env = test_env_with_auths();
    let (client, _admin, _creator) = setup(&env);
    let capped = register_capped_creator(&env, &client, "capped", 3);
    let buyer = Address::generate(&env);

    // Fill the creator right up to their cap.
    buy_keys_checking_supply(&client, &capped, &buyer, 3);
    let supply_before = client.get_total_key_supply(&capped);
    assert_eq!(supply_before, 3);

    let quote = client.get_buy_quote(&capped);
    assert_eq!(
        client.try_buy_key(&capped, &buyer, &quote.total_amount, &None),
        Err(Ok(ContractError::SupplyCapExceeded))
    );

    assert_eq!(
        client.get_total_key_supply(&capped),
        supply_before,
        "a buy rejected on the supply cap must not move supply"
    );
    assert_eq!(client.get_key_balance(&capped, &buyer), 3);
}

#[test]
fn test_supply_unchanged_after_a_buy_for_an_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);
    let unregistered = Address::generate(&env);

    buy_keys_checking_supply(&client, &creator, &buyer, 2);
    let supply_before = client.get_total_key_supply(&creator);

    assert_eq!(
        client.try_buy_key(&unregistered, &buyer, &(KEY_PRICE * 2), &None),
        Err(Ok(ContractError::NotRegistered))
    );

    assert_eq!(client.get_total_key_supply(&creator), supply_before);
    assert_eq!(
        client.get_total_key_supply(&unregistered),
        0,
        "an unregistered creator must report zero supply"
    );
}
