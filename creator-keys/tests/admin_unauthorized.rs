//! Integration tests for admin-only functions reverting when called by a non-admin.
//!
//! Every function gated by `assert_is_admin` must reject a non-admin caller with
//! `ContractError::Unauthorized` and must not mutate any contract state.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_pricing_and_fees, test_env_with_auths};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Register a known admin into contract storage and return it.
fn setup_admin(env: &Env, client: &CreatorKeysContractClient<'_>) -> Address {
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    admin
}

/// Full setup: contract + pricing + fees + admin. Returns (client, admin).
fn full_setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, 100i128, 9000, 1000);
    let admin = setup_admin(env, &client);
    (client, admin)
}

// ── pause ─────────────────────────────────────────────────────────────────────

#[test]
fn test_pause_reverts_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _admin) = full_setup(&env);

    let non_admin = Address::generate(&env);
    let result = client.try_pause(&non_admin);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_pause_no_state_change_on_non_admin_call() {
    let env = test_env_with_auths();
    let (client, _admin) = full_setup(&env);

    let paused_before = client.get_is_paused();

    let non_admin = Address::generate(&env);
    let _ = client.try_pause(&non_admin);

    let paused_after = client.get_is_paused();
    assert_eq!(
        paused_before, paused_after,
        "pause flag must not change when non-admin call is rejected"
    );
}

// ── unpause ───────────────────────────────────────────────────────────────────

#[test]
fn test_unpause_reverts_for_non_admin() {
    let env = test_env_with_auths();
    let (client, admin) = full_setup(&env);

    client.pause(&admin);

    let non_admin = Address::generate(&env);
    let result = client.try_unpause(&non_admin);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_unpause_no_state_change_on_non_admin_call() {
    let env = test_env_with_auths();
    let (client, admin) = full_setup(&env);

    client.pause(&admin);
    let paused_before = client.get_is_paused();

    let non_admin = Address::generate(&env);
    let _ = client.try_unpause(&non_admin);

    let paused_after = client.get_is_paused();
    assert_eq!(
        paused_before, paused_after,
        "pause flag must not change when non-admin unpause is rejected"
    );
}

// ── update_protocol_fee_recipient ─────────────────────────────────────────────

fn setup_fee_recipient(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    admin: &Address,
) -> Address {
    let old_recipient = Address::generate(env);
    client.set_protocol_fee_recipient(admin, &old_recipient);
    old_recipient
}

#[test]
fn test_update_protocol_fee_recipient_reverts_for_non_admin() {
    let env = test_env_with_auths();
    let (client, admin) = full_setup(&env);

    setup_fee_recipient(&env, &client, &admin);
    let new_recipient = Address::generate(&env);

    let non_admin = Address::generate(&env);
    let result = client.try_update_protocol_fee_recipient(&non_admin, &new_recipient);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_update_protocol_fee_recipient_no_state_change_on_non_admin_call() {
    let env = test_env_with_auths();
    let (client, admin) = full_setup(&env);

    let old_recipient = setup_fee_recipient(&env, &client, &admin);

    let non_admin = Address::generate(&env);
    let new_recipient = Address::generate(&env);
    let _ = client.try_update_protocol_fee_recipient(&non_admin, &new_recipient);

    let stored = client.get_protocol_fee_recipient();
    assert_eq!(
        stored,
        Some(old_recipient),
        "fee recipient must not change when non-admin call is rejected"
    );
}

// ── withdraw_treasury ─────────────────────────────────────────────────────────

#[test]
fn test_withdraw_treasury_reverts_for_non_admin() {
    let env = test_env_with_auths();
    let (client, _admin) = full_setup(&env);

    let non_admin = Address::generate(&env);
    let recipient = Address::generate(&env);
    let result = client.try_withdraw_treasury(&non_admin, &1i128, &recipient);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_withdraw_treasury_no_state_change_on_non_admin_call() {
    let env = test_env_with_auths();
    let (client, _admin) = full_setup(&env);

    let balance_before = client.get_treasury_balance();

    let non_admin = Address::generate(&env);
    let recipient = Address::generate(&env);
    let _ = client.try_withdraw_treasury(&non_admin, &1i128, &recipient);

    let balance_after = client.get_treasury_balance();
    assert_eq!(
        balance_before, balance_after,
        "treasury balance must not change when non-admin call is rejected"
    );
}
