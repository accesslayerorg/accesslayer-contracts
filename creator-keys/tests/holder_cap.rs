//! Integration tests for the per-creator percentage holding cap (issue #752).
//!
//! Once a creator enables a cap via `set_holder_cap`, `buy_key` rejects any
//! purchase that would push a non-creator wallet above the configured share
//! of the total supply with `MaxHoldingExceeded`. The creator's own wallet is
//! exempt, and caps are restricted to 1%–25%.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::{constants, ContractError};
use soroban_sdk::{testutils::Address as _, Address, Env};

const KEY_PRICE: i128 = 100;

fn setup(env: &Env) -> (creator_keys::CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

/// Seeds `count` keys held by the creator's own wallet (exempt from the cap).
fn seed_creator_supply(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    count: u32,
) {
    for _ in 0..count {
        client.buy_key(creator, creator, &KEY_PRICE, &None);
    }
}

#[test]
fn test_buy_pushing_holder_above_cap_panics() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    // Default-style 10% cap chosen by the creator.
    client.set_holder_cap(&creator, &None);

    // Creator seeds supply to 19 (exempt from the cap).
    seed_creator_supply(&client, &creator, 19);

    let buyer = Address::generate(&env);
    // Buy #1: supply 19 -> 20, cap allows floor(20 * 10%) = 2 keys. OK.
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);

    // Buy #2: supply 20 -> 21, cap still allows 2 keys. OK.
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(client.get_key_balance(&creator, &buyer), 2);

    // Buy #3: supply 21 -> 22, cap stays at 2 keys while the buyer would hold 3.
    let result = client.try_buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::WalletCapExceeded)),
        "a buy past 10% of supply must be rejected"
    );
    assert_eq!(client.get_key_balance(&creator, &buyer), 2);
    assert_eq!(client.get_total_key_supply(&creator), 21);
}

#[test]
fn test_buy_within_cap_succeeds() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    client.set_holder_cap(&creator, &None);
    seed_creator_supply(&client, &creator, 9);

    // Supply 9 -> 10; a single key is exactly 10% of the new supply.
    let buyer = Address::generate(&env);
    let supply = client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    assert_eq!(supply, 10);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
}

#[test]
fn test_creator_wallet_is_exempt_from_cap() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    client.set_holder_cap(&creator, &None);
    seed_creator_supply(&client, &creator, 9);

    // Creator buys 2 keys (20% of new supply 11 > 10% cap).
    client.buy_key(&creator, &creator, &KEY_PRICE, &None);
    client.buy_key(&creator, &creator, &KEY_PRICE, &None);
    assert_eq!(client.get_key_balance(&creator, &creator), 11);
    assert_eq!(client.get_total_key_supply(&creator), 11);
}

#[test]
fn test_creator_can_configure_custom_cap_within_allowed_range() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    assert_eq!(
        client.get_holder_cap(&creator),
        None,
        "no cap is enforced before configuration"
    );

    client.set_holder_cap(&creator, &Some(100)); // 1%, minimum allowed.
    assert_eq!(client.get_holder_cap(&creator), Some(100));

    client.set_holder_cap(&creator, &Some(2500)); // 25%, maximum allowed.
    assert_eq!(client.get_holder_cap(&creator), Some(2500));
}

#[test]
fn test_set_holder_cap_rejects_values_outside_one_and_twenty_five_percent() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);

    let too_small = client.try_set_holder_cap(&creator, &Some(99));
    assert_eq!(too_small, Err(Ok(ContractError::WalletCapExceeded)));

    let too_large = client.try_set_holder_cap(&creator, &Some(2501));
    assert_eq!(too_large, Err(Ok(ContractError::WalletCapExceeded)));

    assert_eq!(client.get_holder_cap(&creator), None);
}

#[test]
fn test_cap_is_stored_in_persistent_storage() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, KEY_PRICE);
    let creator = register_test_creator(&env, &client, "alice");

    client.set_holder_cap(&creator, &Some(1000));

    let stored: Option<u32> = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get(&constants::storage::holder_cap_bps(&creator))
    });
    assert_eq!(
        stored,
        Some(1000),
        "cap must round-trip via persistent storage"
    );
}
