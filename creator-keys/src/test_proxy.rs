#![cfg(test)]

use crate::proxy::{BondingCurveProxy, BondingCurveProxyClient, ProxyError};
use crate::CreatorKeysContract;
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    Address, Env, IntoVal, Symbol, Vec,
};

#[test]
fn test_proxy_initialization_and_access_control() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let impl_id = env.register(CreatorKeysContract, ());

    let proxy_id = env.register(BondingCurveProxy, ());
    let client = BondingCurveProxyClient::new(&env, &proxy_id);

    client.initialize(&admin, &impl_id);

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_implementation(), impl_id);
    assert!(!client.is_paused());

    // Already initialized rejection
    let res = client.try_initialize(&admin, &impl_id);
    assert_eq!(res, Err(Ok(ProxyError::AlreadyInitialized)));
}

#[test]
fn test_proxy_upgrade_restricted_to_admin_with_interface_validation() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let impl_1 = env.register(CreatorKeysContract, ());
    let impl_2 = env.register(CreatorKeysContract, ());

    let proxy_id = env.register(BondingCurveProxy, ());
    let client = BondingCurveProxyClient::new(&env, &proxy_id);
    client.initialize(&admin, &impl_1);

    // Non-admin upgrade fails with Unauthorized
    let res = client.try_upgrade_implementation(&non_admin, &impl_2);
    assert_eq!(res, Err(Ok(ProxyError::Unauthorized)));

    // Invalid implementation (random address without contract interface) fails with InvalidInterface
    let dummy_addr = Address::generate(&env);
    let res_invalid = client.try_upgrade_implementation(&admin, &dummy_addr);
    assert_eq!(res_invalid, Err(Ok(ProxyError::InvalidInterface)));

    // Admin upgrade to valid implementation succeeds
    client.upgrade_implementation(&admin, &impl_2);
    let events = env.events().all();
    assert!(!events.is_empty());
    assert_eq!(client.get_implementation(), impl_2);
}

#[test]
fn test_proxy_pause_and_unpause() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let impl_id = env.register(CreatorKeysContract, ());

    let proxy_id = env.register(BondingCurveProxy, ());
    let client = BondingCurveProxyClient::new(&env, &proxy_id);
    client.initialize(&admin, &impl_id);

    // Non-admin cannot pause
    let pause_res = client.try_set_paused(&non_admin, &true);
    assert_eq!(pause_res, Err(Ok(ProxyError::Unauthorized)));

    // Admin can pause
    client.set_paused(&admin, &true);
    assert!(client.is_paused());

    // Admin can unpause
    client.set_paused(&admin, &false);
    assert!(!client.is_paused());
}

#[test]
fn test_proxy_delegation_dispatch() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let impl_id = env.register(CreatorKeysContract, ());

    let proxy_id = env.register(BondingCurveProxy, ());
    let client = BondingCurveProxyClient::new(&env, &proxy_id);
    client.initialize(&admin, &impl_id);

    // Call get_timelock_delay via proxy dispatch
    let delay_res = client.dispatch(&Symbol::new(&env, "get_timelock_delay"), &Vec::new(&env));
    let delay_val: u64 = delay_res.into_val(&env);
    assert_eq!(delay_val, 172800u64);

    // When paused, dispatch returns ContractPaused
    client.set_paused(&admin, &true);
    let paused_res = client.try_dispatch(&Symbol::new(&env, "get_timelock_delay"), &Vec::new(&env));
    assert!(matches!(paused_res, Err(Ok(ProxyError::ContractPaused))));
}
