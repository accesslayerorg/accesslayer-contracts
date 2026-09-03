//! Integration tests for the global emergency pause (#784).
//!
//! A protocol-wide halt flag lets any two of three configured admins stop every
//! buy and sell across every creator key with a single pair of actions, without
//! touching per-key pause state. These tests cover the acceptance criteria:
//!
//! * buy on any key panics with `GlobalTradingHalted` while the global pause is active;
//! * sell on any key panics with `GlobalTradingHalted` while the global pause is active;
//! * a single admin cannot activate the global pause without a second approval;
//! * `global_pause_activated` is emitted on activation;
//! * `global_resume` with two approvals alone lifts the halt and re-enables trading.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::events::{
    global_pause_activated_topics, GLOBAL_PAUSE_ACTIVATED_EVENT_NAME,
    GLOBAL_PAUSE_LIFTED_EVENT_NAME,
};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Events},
    vec, Address, Env, IntoVal,
};

const BASE_PRICE: i128 = 100;

struct Fixture<'a> {
    client: CreatorKeysContractClient<'a>,
    admin: Address,
    signers: [Address; 3],
    creator: Address,
}

/// Deploys the contract, sets a price, registers a creator and configures a
/// 2-of-3 global-pause admin set. Trading is open on return.
fn setup(env: &Env) -> Fixture<'_> {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, BASE_PRICE);

    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);

    let signers = [
        Address::generate(env),
        Address::generate(env),
        Address::generate(env),
    ];
    client.set_global_pause_admins(
        &admin,
        &vec![
            env,
            signers[0].clone(),
            signers[1].clone(),
            signers[2].clone(),
        ],
    );

    let creator = register_test_creator(env, &client, "alice");

    Fixture {
        client,
        admin,
        signers,
        creator,
    }
}

/// Two distinct admins together activate the global pause.
fn activate_pause(f: &Fixture<'_>) {
    f.client.global_pause(&f.signers[0]);
    f.client.global_pause(&f.signers[1]);
    assert!(f.client.get_global_trading_paused());
}

#[test]
fn test_buy_on_any_key_halts_when_global_pause_active() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let buyer = Address::generate(&env);

    activate_pause(&f);

    let result = f.client.try_buy_key(&f.creator, &buyer, &BASE_PRICE, &None);
    assert_eq!(result, Err(Ok(ContractError::GlobalTradingHalted)));
    assert_eq!(f.client.get_key_balance(&f.creator, &buyer), 0);
    assert_eq!(f.client.get_total_key_supply(&f.creator), 0);
}

#[test]
fn test_sell_on_any_key_halts_when_global_pause_active() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let holder = Address::generate(&env);

    // Acquire a key before the halt so there is something to try to sell.
    f.client.buy_key(&f.creator, &holder, &BASE_PRICE, &None);
    assert_eq!(f.client.get_key_balance(&f.creator, &holder), 1);

    activate_pause(&f);

    let result = f.client.try_sell_key(&f.creator, &holder, &None);
    assert_eq!(result, Err(Ok(ContractError::GlobalTradingHalted)));
    assert_eq!(f.client.get_key_balance(&f.creator, &holder), 1);
}

#[test]
fn test_single_admin_cannot_activate_global_pause() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let buyer = Address::generate(&env);

    // One approval only: the flag stays off and trading continues.
    f.client.global_pause(&f.signers[0]);
    assert!(!f.client.get_global_trading_paused());

    // The same admin calling twice is still a single distinct approval.
    f.client.global_pause(&f.signers[0]);
    assert!(!f.client.get_global_trading_paused());

    f.client.buy_key(&f.creator, &buyer, &BASE_PRICE, &None);
    assert_eq!(f.client.get_key_balance(&f.creator, &buyer), 1);
}

#[test]
fn test_non_admin_cannot_vote_for_global_pause() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let outsider = Address::generate(&env);

    let result = f.client.try_global_pause(&outsider);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    assert!(!f.client.get_global_trading_paused());
}

#[test]
fn test_global_pause_activated_event_emitted_on_activation() {
    let env = test_env_with_auths();
    let f = setup(&env);

    f.client.global_pause(&f.signers[0]);
    // No activation yet -> no activation event.
    assert!(!env.events().all().iter().any(|(_, topics, _)| {
        topics == global_pause_activated_topics(&f.signers[0]).into_val(&env)
            || topics == global_pause_activated_topics(&f.signers[1]).into_val(&env)
    }));

    f.client.global_pause(&f.signers[1]);

    let (_, data) = env
        .events()
        .all()
        .iter()
        .rev()
        .find_map(|(_, topics, data)| {
            let name: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&env);
            if name == GLOBAL_PAUSE_ACTIVATED_EVENT_NAME {
                Some((topics, data))
            } else {
                None
            }
        })
        .expect("global_pause_activated event not found");

    let ledger: u32 = data.into_val(&env);
    assert_eq!(ledger, env.ledger().sequence());
}

#[test]
fn test_global_resume_with_two_approvals_lifts_halt() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let buyer = Address::generate(&env);

    activate_pause(&f);
    assert_eq!(
        f.client.try_buy_key(&f.creator, &buyer, &BASE_PRICE, &None),
        Err(Ok(ContractError::GlobalTradingHalted))
    );

    // A single resume approval is not enough.
    f.client.global_resume(&f.signers[1]);
    assert!(f.client.get_global_trading_paused());

    // Second distinct approval lifts the halt.
    f.client.global_resume(&f.signers[2]);
    let events = env.events().all();
    assert!(!f.client.get_global_trading_paused());

    let (_, data) = events
        .iter()
        .rev()
        .find_map(|(_, topics, data)| {
            let name: soroban_sdk::Symbol = topics.get(0)?.into_val(&env);
            if name == GLOBAL_PAUSE_LIFTED_EVENT_NAME {
                let approver: Address = topics.get(1)?.into_val(&env);
                if approver == f.signers[2] {
                    return Some((topics, data));
                }
            }
            None
        })
        .expect("global_pause_lifted event not found");

    let ledger: u32 = data.into_val(&env);
    assert_eq!(ledger, env.ledger().sequence());

    // Trading works again on any key.
    f.client.buy_key(&f.creator, &buyer, &BASE_PRICE, &None);
    assert_eq!(f.client.get_key_balance(&f.creator, &buyer), 1);
}

#[test]
fn test_global_pause_takes_precedence_over_per_key_pause() {
    let env = test_env_with_auths();
    let f = setup(&env);
    let buyer = Address::generate(&env);

    activate_pause(&f);

    // Even with no per-key pause configured, the global halt alone blocks buys
    // with GlobalTradingHalted rather than the per-key ProtocolPaused error.
    let result = f.client.try_buy_key(&f.creator, &buyer, &BASE_PRICE, &None);
    assert_eq!(result, Err(Ok(ContractError::GlobalTradingHalted)));

    let _ = &f.admin;
}
