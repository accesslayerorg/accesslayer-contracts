//! #1238: rescue of tokens sent to the contract outside the normal flows.

use creator_keys::{
    token_rescue::{RescueError, RescuedEvent},
    CreatorKeysContractClient,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token, Address, Env, IntoVal, Symbol,
};

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};

const BOND: i128 = 10_000;
const STRAY: i128 = 500;

struct Setup<'a> {
    env: Env,
    client: CreatorKeysContractClient<'a>,
    contract: Address,
    admin: Address,
    token: Address,
    creator: Address,
}

/// Escrows a `BOND` in the reserve token, then sends `STRAY` straight to the contract.
fn setup(env: &Env) -> Setup<'_> {
    let (client, contract) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, 1_000, 9_000, 1_000);
    let token = env
        .register_stellar_asset_contract_v2(Address::generate(env))
        .address();
    client.set_xlm_token_address(&admin, &token);
    client.set_min_performance_bond(&admin, &1_000);

    let creator = register_test_creator(env, &client, "rescuecreator");
    let sac = token::StellarAssetClient::new(env, &token);
    sac.mint(&creator, &BOND);
    client.deposit_performance_bond(&creator, &BOND);
    sac.mint(&contract, &STRAY);

    Setup {
        env: env.clone(),
        client,
        contract,
        admin,
        token,
        creator,
    }
}

fn balance(s: &Setup, who: &Address) -> i128 {
    token::TokenClient::new(&s.env, &s.token).balance(who)
}

#[test]
fn tracked_balance_and_rescuable_split_escrow_from_surplus() {
    let env = test_env_with_auths();
    let s = setup(&env);
    assert_eq!(s.client.tracked_balance(&s.token), BOND);
    assert_eq!(s.client.rescuable(&s.token), STRAY);
    assert_eq!(balance(&s, &s.contract), BOND + STRAY);
}

#[test]
fn admin_rescues_surplus_and_emits_event() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let to = Address::generate(&s.env);
    s.client.rescue(&s.admin, &s.token, &to, &STRAY);

    let mut found = false;
    for (_, topics, data) in s.env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&s.env);
        if name == Symbol::new(&s.env, "Rescued") {
            let event: RescuedEvent = data.into_val(&s.env);
            assert_eq!(
                event,
                RescuedEvent {
                    token: s.token.clone(),
                    to: to.clone(),
                    amount: STRAY,
                }
            );
            found = true;
        }
    }
    assert!(found);

    assert_eq!(balance(&s, &to), STRAY);
    assert_eq!(balance(&s, &s.contract), BOND);
    assert_eq!(s.client.tracked_balance(&s.token), BOND);
    assert_eq!(s.client.rescuable(&s.token), 0);
}

#[test]
fn over_request_reverts_and_leaves_tracked_balance_intact() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let to = Address::generate(&s.env);
    assert_eq!(
        s.client.try_rescue(&s.admin, &s.token, &to, &(STRAY + 1)),
        Err(Ok(RescueError::NotRescuable))
    );
    // Asking for the escrowed bond itself is rejected too.
    assert_eq!(
        s.client
            .try_rescue(&s.admin, &s.token, &to, &(BOND + STRAY)),
        Err(Ok(RescueError::NotRescuable))
    );
    assert_eq!(balance(&s, &to), 0);
    assert_eq!(balance(&s, &s.contract), BOND + STRAY);
    assert_eq!(s.client.tracked_balance(&s.token), BOND);
}

#[test]
fn rescue_never_touches_escrow_even_after_surplus_is_drained() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let to = Address::generate(&s.env);
    s.client.rescue(&s.admin, &s.token, &to, &STRAY);
    assert_eq!(
        s.client.try_rescue(&s.admin, &s.token, &to, &1),
        Err(Ok(RescueError::NotRescuable))
    );
    assert_eq!(
        s.client.get_bond_status(&s.creator).releasable,
        BOND,
        "bond escrow must stay fully backed"
    );
    assert_eq!(balance(&s, &s.contract), BOND);
}

#[test]
fn non_admin_cannot_rescue_or_configure() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let attacker = Address::generate(&s.env);
    assert_eq!(
        s.client.try_rescue(&attacker, &s.token, &attacker, &STRAY),
        Err(Ok(RescueError::Unauthorized))
    );
    assert_eq!(
        s.client.try_set_rescue_threshold(&attacker, &s.token, &1),
        Err(Ok(RescueError::Unauthorized))
    );
    assert_eq!(
        s.client
            .try_queue_rescue(&attacker, &s.token, &attacker, &STRAY),
        Err(Ok(RescueError::Unauthorized))
    );
    assert_eq!(balance(&s, &s.contract), BOND + STRAY);
}

#[test]
fn zero_amount_is_rejected() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let to = Address::generate(&s.env);
    assert_eq!(
        s.client.try_rescue(&s.admin, &s.token, &to, &0),
        Err(Ok(RescueError::NotPositiveAmount))
    );
}

#[test]
fn large_rescue_requires_elapsed_timelock() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let to = Address::generate(&s.env);
    s.client.set_rescue_threshold(&s.admin, &s.token, &100);
    assert_eq!(s.client.get_rescue_threshold(&s.token), Some(100));

    // Below the threshold: immediate.
    s.client.rescue(&s.admin, &s.token, &to, &100);

    // Above the threshold: must be queued first.
    let amount = STRAY - 100;
    assert_eq!(
        s.client.try_rescue(&s.admin, &s.token, &to, &amount),
        Err(Ok(RescueError::TimelockRequired))
    );

    let now = s.env.ledger().timestamp();
    let executable_at = s.client.queue_rescue(&s.admin, &s.token, &to, &amount);
    assert_eq!(executable_at, now + s.client.get_timelock_delay());
    assert_eq!(
        s.client.try_queue_rescue(&s.admin, &s.token, &to, &amount),
        Err(Ok(RescueError::AlreadyQueued))
    );

    s.env.ledger().with_mut(|l| l.timestamp = executable_at - 1);
    assert_eq!(
        s.client.try_rescue(&s.admin, &s.token, &to, &amount),
        Err(Ok(RescueError::TimelockNotElapsed))
    );

    s.env.ledger().with_mut(|l| l.timestamp = executable_at);
    s.client.rescue(&s.admin, &s.token, &to, &amount);
    assert_eq!(balance(&s, &to), STRAY);
    assert_eq!(balance(&s, &s.contract), BOND);

    // The queued entry is consumed.
    assert_eq!(
        s.client.try_rescue(&s.admin, &s.token, &to, &amount),
        Err(Ok(RescueError::TimelockRequired))
    );
}

#[test]
fn matured_queue_still_cannot_exceed_surplus() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let to = Address::generate(&s.env);
    s.client.set_rescue_threshold(&s.admin, &s.token, &100);
    let amount = BOND + STRAY;
    let executable_at = s.client.queue_rescue(&s.admin, &s.token, &to, &amount);
    s.env.ledger().with_mut(|l| l.timestamp = executable_at);
    assert_eq!(
        s.client.try_rescue(&s.admin, &s.token, &to, &amount),
        Err(Ok(RescueError::NotRescuable))
    );
    assert_eq!(balance(&s, &s.contract), BOND + STRAY);
}

#[test]
fn cancelled_rescue_cannot_execute() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let to = Address::generate(&s.env);
    s.client.set_rescue_threshold(&s.admin, &s.token, &100);
    let executable_at = s.client.queue_rescue(&s.admin, &s.token, &to, &STRAY);
    s.client.cancel_rescue(&s.admin, &s.token, &to, &STRAY);
    assert_eq!(
        s.client.try_cancel_rescue(&s.admin, &s.token, &to, &STRAY),
        Err(Ok(RescueError::NotQueued))
    );
    s.env.ledger().with_mut(|l| l.timestamp = executable_at);
    assert_eq!(
        s.client.try_rescue(&s.admin, &s.token, &to, &STRAY),
        Err(Ok(RescueError::TimelockRequired))
    );
}

#[test]
fn untracked_token_is_fully_rescuable() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let other = s
        .env
        .register_stellar_asset_contract_v2(Address::generate(&s.env))
        .address();
    token::StellarAssetClient::new(&s.env, &other).mint(&s.contract, &777);
    assert_eq!(s.client.tracked_balance(&other), 0);
    assert_eq!(s.client.rescuable(&other), 777);

    let to = Address::generate(&s.env);
    s.client.rescue(&s.admin, &other, &to, &777);
    assert_eq!(token::TokenClient::new(&s.env, &other).balance(&to), 777);
}

#[test]
fn bond_release_reduces_tracked_balance() {
    let env = test_env_with_auths();
    let s = setup(&env);
    s.env.as_contract(&s.contract, || {
        s.env.storage().persistent().set(
            &creator_keys::constants::storage::deprecated_key(&s.creator),
            &0i128,
        );
    });
    assert_eq!(s.client.finalize_deprecation(&s.creator), BOND);
    assert_eq!(s.client.tracked_balance(&s.token), 0);
    assert_eq!(s.client.rescuable(&s.token), STRAY);
}
