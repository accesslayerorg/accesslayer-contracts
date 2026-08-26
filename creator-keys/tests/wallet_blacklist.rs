//! Tests for the admin-managed wallet blacklist.
//!
//! Covers: blacklisted wallets rejected on buy/sell/creator registration,
//! restored access after removal from the blacklist, and admin-only
//! access to the blacklist mutators.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::{ContractError, RegisterCreatorParams};
use soroban_sdk::{testutils::Address as _, Address, String};

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
// buy reverts with WalletBlacklisted when the buyer is blacklisted
// ---------------------------------------------------------------------------

#[test]
fn test_buy_key_reverts_for_blacklisted_buyer() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    client.blacklist_wallet(&admin, &buyer);

    let result = client.try_buy_key(&creator, &buyer, &100, &None);
    assert_eq!(result, Err(Ok(ContractError::WalletBlacklisted)));
}

#[test]
fn test_buy_key_no_state_change_for_blacklisted_buyer() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    client.blacklist_wallet(&admin, &buyer);

    let supply_before = client.get_total_key_supply(&creator);
    let _ = client.try_buy_key(&creator, &buyer, &100, &None);
    let supply_after = client.get_total_key_supply(&creator);

    assert_eq!(
        supply_before, supply_after,
        "supply must not change when a blacklisted buyer's purchase is rejected"
    );
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);
}

// ---------------------------------------------------------------------------
// sell reverts with WalletBlacklisted when the seller is blacklisted
// ---------------------------------------------------------------------------

#[test]
fn test_sell_key_reverts_for_blacklisted_seller() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let seller = Address::generate(&env);

    // Buy first, while not yet blacklisted.
    client.buy_key(&creator, &seller, &100, &None);

    client.blacklist_wallet(&admin, &seller);

    let result = client.try_sell_key(&creator, &seller, &None);
    assert_eq!(result, Err(Ok(ContractError::WalletBlacklisted)));
}

#[test]
fn test_sell_key_no_state_change_for_blacklisted_seller() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let seller = Address::generate(&env);

    client.buy_key(&creator, &seller, &100, &None);
    client.blacklist_wallet(&admin, &seller);

    let balance_before = client.get_key_balance(&creator, &seller);
    let _ = client.try_sell_key(&creator, &seller, &None);
    let balance_after = client.get_key_balance(&creator, &seller);

    assert_eq!(
        balance_before, balance_after,
        "seller balance must not change when a blacklisted seller's sale is rejected"
    );
}

// ---------------------------------------------------------------------------
// register_creator reverts with WalletBlacklisted when the creator is blacklisted
// ---------------------------------------------------------------------------

#[test]
fn test_register_creator_reverts_for_blacklisted_wallet() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);

    let creator = Address::generate(&env);
    client.blacklist_wallet(&admin, &creator);

    let result = client.try_register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    assert_eq!(result, Err(Ok(ContractError::WalletBlacklisted)));
}

#[test]
fn test_register_creator_no_state_change_for_blacklisted_wallet() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);

    let creator = Address::generate(&env);
    client.blacklist_wallet(&admin, &creator);

    let _ = client.try_register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    assert!(!client.is_creator_registered(&creator));
}

// ---------------------------------------------------------------------------
// removing a wallet from the blacklist restores access
// ---------------------------------------------------------------------------

#[test]
fn test_buy_key_succeeds_after_removal_from_blacklist() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    client.blacklist_wallet(&admin, &buyer);
    assert!(client.is_wallet_blacklisted(&buyer));

    let blocked = client.try_buy_key(&creator, &buyer, &100, &None);
    assert_eq!(blocked, Err(Ok(ContractError::WalletBlacklisted)));

    client.remove_from_blacklist(&admin, &buyer);
    assert!(!client.is_wallet_blacklisted(&buyer));

    let supply = client.buy_key(&creator, &buyer, &100, &None);
    assert_eq!(supply, 1);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
}

#[test]
fn test_sell_key_succeeds_after_removal_from_blacklist() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");
    let seller = Address::generate(&env);

    client.buy_key(&creator, &seller, &100, &None);
    client.blacklist_wallet(&admin, &seller);
    client.remove_from_blacklist(&admin, &seller);

    let supply = client.sell_key(&creator, &seller, &None);
    assert_eq!(supply, 0);
    assert_eq!(client.get_key_balance(&creator, &seller), 0);
}

#[test]
fn test_register_creator_succeeds_after_removal_from_blacklist() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);

    let creator = Address::generate(&env);
    client.blacklist_wallet(&admin, &creator);
    client.remove_from_blacklist(&admin, &creator);

    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    assert!(client.is_creator_registered(&creator));
}

// ---------------------------------------------------------------------------
// only the admin can add or remove blacklist entries
// ---------------------------------------------------------------------------

#[test]
fn test_blacklist_wallet_reverts_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_protocol_admin(&env, &client);

    let non_admin = Address::generate(&env);
    let target = Address::generate(&env);

    let result = client.try_blacklist_wallet(&non_admin, &target);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    assert!(!client.is_wallet_blacklisted(&target));
}

#[test]
fn test_remove_from_blacklist_reverts_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);

    let non_admin = Address::generate(&env);
    let target = Address::generate(&env);
    client.blacklist_wallet(&admin, &target);

    let result = client.try_remove_from_blacklist(&non_admin, &target);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    assert!(client.is_wallet_blacklisted(&target));
}

#[test]
fn test_blacklist_wallet_rejected_when_no_admin_configured() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let caller = Address::generate(&env);
    let target = Address::generate(&env);
    let result = client.try_blacklist_wallet(&caller, &target);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ---------------------------------------------------------------------------
// blacklist is scoped per-wallet and does not affect unrelated wallets
// ---------------------------------------------------------------------------

#[test]
fn test_blacklist_does_not_affect_other_wallets() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);
    set_key_price_for_tests(&env, &client, 100);
    let creator = register_test_creator(&env, &client, "alice");

    let blocked_buyer = Address::generate(&env);
    let allowed_buyer = Address::generate(&env);

    client.blacklist_wallet(&admin, &blocked_buyer);

    let result = client.try_buy_key(&creator, &blocked_buyer, &100, &None);
    assert_eq!(result, Err(Ok(ContractError::WalletBlacklisted)));

    let supply = client.buy_key(&creator, &allowed_buyer, &100, &None);
    assert_eq!(supply, 1);
    assert_eq!(client.get_key_balance(&creator, &allowed_buyer), 1);
}

// ---------------------------------------------------------------------------
// is_wallet_blacklisted view returns correct status
// ---------------------------------------------------------------------------

#[test]
fn test_is_wallet_blacklisted_returns_false_for_non_blacklisted_wallet() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_protocol_admin(&env, &client);

    let wallet = Address::generate(&env);
    assert!(!client.is_wallet_blacklisted(&wallet));
}

#[test]
fn test_is_wallet_blacklisted_returns_true_after_blacklist() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);

    let wallet = Address::generate(&env);
    client.blacklist_wallet(&admin, &wallet);
    assert!(client.is_wallet_blacklisted(&wallet));
}

#[test]
fn test_is_wallet_blacklisted_returns_false_after_removal() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_admin(&env, &client);

    let wallet = Address::generate(&env);
    client.blacklist_wallet(&admin, &wallet);
    assert!(client.is_wallet_blacklisted(&wallet));

    client.remove_from_blacklist(&admin, &wallet);
    assert!(!client.is_wallet_blacklisted(&wallet));
}
