//! Regression coverage for per-creator supply-cap enforcement.
//!
//! This test ensures the stored metadata payload matches the current API shape
//! (`name`, `bio`, and `avatar_uri`) and that the creator supply cap rejects
//! any buy that would exceed the configured cap.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_key_price_for_tests, test_env_with_auths};
use creator_keys::{ContractError, CurvePreset, KeyMetadata};
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_supply_cap_enforced_after_key_metadata_initialisation() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_key_price_for_tests(&env, &client, 1_000_i128);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    let creator = Address::generate(&env);
    let metadata = KeyMetadata {
        name: String::from_str(&env, "supply-cap-test-key"),
        bio: String::from_str(&env, "supply cap test key"),
        avatar_uri: String::from_str(&env, "ipfs://avatar"),
    };

    client.register_key(
        &admin,
        &creator,
        &String::from_str(&env, "supplycap"),
        &metadata,
        &CurvePreset::Quadratic,
        &0,
        &false,
    );

    client.set_supply_cap(&creator, &1u32);

    let buyer_one = Address::generate(&env);
    client.buy_key(&creator, &buyer_one, &1_000_i128, &None);

    let buyer_two = Address::generate(&env);
    let result = client.try_buy_key(&creator, &buyer_two, &1_000_i128, &None);
    assert_eq!(
        result,
        Err(Ok(ContractError::SupplyCapExceeded)),
        "second buy must fail when the configured supply cap is reached"
    );

    assert_eq!(
        client.get_total_key_supply(&creator),
        1,
        "supply must remain at cap after the rejected buy"
    );
}
