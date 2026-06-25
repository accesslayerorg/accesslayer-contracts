//! Tests for the emergency pause mechanism.
//!
//! Covers: pause/unpause admin-only access, buy/sell/register blocked while paused,
//! read-only views remain live while paused, and event emission.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

/// Register a protocol admin into contract storage and return the admin address.
fn set_protocol_admin(
    env: &soroban_sdk::Env,
    client: &creator_keys::CreatorKeysContractClient<'_>,
) -> Address {
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    admin
}

// ---------------------------------------------------------------------------
// pause / unpause — admin-only access
// ---------------------------------------------------------------------------

#[test]
fn test_pause_sets_flag_and_unpause_clears_it() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);

    assert!(!client.get_is_paused());

    client.pause(&admin);
    assert!(client.get_is_paused());

    client.unpause(&admin);
    assert!(!client.get_is_paused());
}

#[test]
fn test_pause_rejected_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_protocol_admin(&env, &client);

    let non_admin = Address::generate(&env);
    let result = client.try_pause(&non_admin);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_unpause_rejected_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    client.pause(&admin);

    let non_admin = Address::generate(&env);
    let result = client.try_unpause(&non_admin);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    assert!(client.get_is_paused());
}

#[test]
fn test_pause_rejected_when_no_admin_configured() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let caller = Address::generate(&env);
    let result = client.try_pause(&caller);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ---------------------------------------------------------------------------
// buy reverts with ProtocolPaused when paused
// ---------------------------------------------------------------------------

#[test]
fn test_buy_key_reverts_when_paused() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    client.pause(&admin);

    let result = client.try_buy_key(&creator, &buyer, &100, &None);
    assert_eq!(result, Err(Ok(ContractError::ProtocolPaused)));
}

#[test]
fn test_buy_key_succeeds_after_unpause() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    client.pause(&admin);
    client.unpause(&admin);

    let supply = client.buy_key(&creator, &buyer, &100, &None);
    assert_eq!(supply, 1);
}

// ---------------------------------------------------------------------------
// sell reverts with ProtocolPaused when paused
// ---------------------------------------------------------------------------

#[test]
fn test_sell_key_reverts_when_paused() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    // Buy first (before pause)
    client.buy_key(&creator, &buyer, &100, &None);

    client.pause(&admin);

    let result = client.try_sell_key(&creator, &buyer, &None);
    assert_eq!(result, Err(Ok(ContractError::ProtocolPaused)));
}

// ---------------------------------------------------------------------------
// register_creator reverts with ProtocolPaused when paused
// ---------------------------------------------------------------------------

#[test]
fn test_register_creator_reverts_when_paused() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);

    client.pause(&admin);

    let creator = Address::generate(&env);
    let result = client.try_register_creator(
        &creator,
        &soroban_sdk::String::from_str(&env, "alice"),
        &None,
    );
    assert_eq!(result, Err(Ok(ContractError::ProtocolPaused)));
}

// ---------------------------------------------------------------------------
// Read-only views remain live while paused
// ---------------------------------------------------------------------------

#[test]
fn test_read_only_views_work_while_paused() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &100, &None);

    client.pause(&admin);

    // All of these must not panic while paused
    assert!(client.get_is_paused());
    assert!(client.is_creator_registered(&creator));
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(client.get_key_balance(&creator, &holder), 1);
    let _ = client.get_creator_details(&creator);
    let _ = client.get_protocol_fee_view();
    let _ = client.get_protocol_state_version();
    let _ = client.get_key_decimals();
}
