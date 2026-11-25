//! Integration test for a creator's state after a `RESTORING` lifecycle
//! transition (#709).
//!
//! While the archive is `RESTORING`, read calls keep serving the current
//! values and buys are held off until `complete_restore` finishes.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use creator_keys::{ContractError, CreatorKeysContractClient, CreatorLifecycle};
use soroban_sdk::{testutils::Address as _, Address, Env};

const KEY_PRICE: i128 = 100;

/// Contract with a registered creator holding two keys in one wallet, plus an
/// approved caller for the `get_price` oracle view.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);

    let oracle = Address::generate(env);
    client.add_approved_caller(&admin, &oracle);

    let creator = register_test_creator(env, &client, "alice");
    let holder = Address::generate(env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    (client, oracle, creator, holder)
}

#[test]
fn test_restoring_serves_reads_gates_buys_and_restores_state() {
    let env = test_env_with_auths();
    let (client, oracle, creator, holder) = setup(&env);

    // Pre-archive state: two keys held by a single wallet.
    let supply_before = client.get_total_key_supply(&creator);
    let holders_before = client.get_creator_holder_count(&creator);
    let balance_before = client.get_balance(&creator, &holder);
    let price_before = client.get_price(&creator, &oracle);
    assert_eq!((supply_before, holders_before, balance_before), (2, 1, 2));

    // Archive the creator, then transition the manifest into RESTORING.
    client.archive_creator(&creator);
    client.begin_restore(&creator);
    assert_eq!(
        client.get_creator_lifecycle(&creator),
        CreatorLifecycle::Restoring
    );

    // Reads keep returning the current values during the window.
    assert_eq!(client.get_balance(&creator, &holder), balance_before);
    assert_eq!(client.get_price(&creator, &oracle), price_before);

    // Buys are gated while the state is being restored.
    assert_eq!(
        client.try_buy_key(&creator, &holder, &KEY_PRICE, &None),
        Err(Ok(ContractError::StateRestoring))
    );

    // Restoration finishes and the creator is active again.
    client.complete_restore(&creator);
    assert_eq!(
        client.get_creator_lifecycle(&creator),
        CreatorLifecycle::Active
    );

    // The restored state matches the pre-archive values.
    assert_eq!(client.get_total_key_supply(&creator), supply_before);
    assert_eq!(client.get_creator_holder_count(&creator), holders_before);
    assert_eq!(client.get_balance(&creator, &holder), balance_before);

    // ...and a buy succeeds immediately.
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    assert_eq!(client.get_balance(&creator, &holder), balance_before + 1);
}
