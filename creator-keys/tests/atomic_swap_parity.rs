//! Integration tests for `atomic_swap`: the parity guard and the atomic
//! two-leg swap it wraps.

mod contract_test_env;

use contract_test_env::{
    assert_event, register_creator_keys, register_test_creator, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{events, ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

const KEY_PRICE: i128 = 100;

struct Fixture<'a> {
    client: CreatorKeysContractClient<'a>,
    contract_id: Address,
    key_a: Address,
    key_b: Address,
    wallet_a: Address,
    wallet_b: Address,
}

fn fixture(env: &Env) -> Fixture<'_> {
    let (client, contract_id) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, KEY_PRICE, 9000, 1000);
    Fixture {
        key_a: register_test_creator(env, &client, "alice"),
        key_b: register_test_creator(env, &client, "bob"),
        wallet_a: Address::generate(env),
        wallet_b: Address::generate(env),
        client,
        contract_id,
    }
}

fn buy(f: &Fixture, key: &Address, wallet: &Address, qty: u32) {
    for _ in 0..qty {
        f.client.buy_key(key, wallet, &KEY_PRICE, &None);
    }
}

#[test]
fn atomic_swap_moves_both_legs_when_values_match() {
    let env = test_env_with_auths();
    let f = fixture(&env);
    buy(&f, &f.key_a, &f.wallet_a, 2);
    buy(&f, &f.key_b, &f.wallet_b, 2);

    f.client
        .atomic_swap(&f.key_a, &2, &f.wallet_a, &f.key_b, &2, &f.wallet_b, &0);

    // Checked before the balance reads below: the test host clears its event
    // buffer at the start of every invocation, so the swap's event is only
    // observable while it is still the most recent call.
    assert_event(
        &env,
        &f.contract_id,
        events::ATOMIC_SWAP_EVENT_NAME,
        &events::AtomicSwapEvent {
            key_a: f.key_a.clone(),
            qty_a: 2,
            price_a: KEY_PRICE,
            key_b: f.key_b.clone(),
            qty_b: 2,
            price_b: KEY_PRICE,
            ledger: env.ledger().sequence(),
        },
    );

    assert_eq!(f.client.get_key_balance(&f.key_a, &f.wallet_a), 0);
    assert_eq!(f.client.get_key_balance(&f.key_b, &f.wallet_a), 2);
    assert_eq!(f.client.get_key_balance(&f.key_b, &f.wallet_b), 0);
    assert_eq!(f.client.get_key_balance(&f.key_a, &f.wallet_b), 2);
}

#[test]
fn atomic_swap_zero_tolerance_requires_exact_value_match() {
    let env = test_env_with_auths();
    let f = fixture(&env);
    buy(&f, &f.key_a, &f.wallet_a, 2);
    buy(&f, &f.key_b, &f.wallet_b, 1);

    // 200 vs 100 is a 5000 bps deviation, so a zero tolerance must reject it.
    let result = f
        .client
        .try_atomic_swap(&f.key_a, &2, &f.wallet_a, &f.key_b, &1, &f.wallet_b, &0);
    assert_eq!(result, Err(Ok(ContractError::ParityDeviationExceeded)));

    assert_eq!(f.client.get_key_balance(&f.key_a, &f.wallet_a), 2);
    assert_eq!(f.client.get_key_balance(&f.key_b, &f.wallet_b), 1);
}

#[test]
fn atomic_swap_rejects_deviation_above_tolerance() {
    let env = test_env_with_auths();
    let f = fixture(&env);
    buy(&f, &f.key_a, &f.wallet_a, 3);
    buy(&f, &f.key_b, &f.wallet_b, 1);

    // 300 vs 100 is ~6666 bps, above a 5000 bps ceiling.
    let result =
        f.client
            .try_atomic_swap(&f.key_a, &3, &f.wallet_a, &f.key_b, &1, &f.wallet_b, &5000);
    assert_eq!(result, Err(Ok(ContractError::ParityDeviationExceeded)));
}

#[test]
fn atomic_swap_accepts_deviation_at_tolerance_boundary() {
    let env = test_env_with_auths();
    let f = fixture(&env);
    buy(&f, &f.key_a, &f.wallet_a, 2);
    buy(&f, &f.key_b, &f.wallet_b, 1);

    // Exactly 5000 bps is within a 5000 bps ceiling.
    f.client
        .atomic_swap(&f.key_a, &2, &f.wallet_a, &f.key_b, &1, &f.wallet_b, &5000);

    assert_eq!(f.client.get_key_balance(&f.key_a, &f.wallet_a), 0);
    assert_eq!(f.client.get_key_balance(&f.key_b, &f.wallet_a), 1);
    assert_eq!(f.client.get_key_balance(&f.key_a, &f.wallet_b), 2);
}

#[test]
fn atomic_swap_reverts_both_legs_when_one_side_is_short() {
    let env = test_env_with_auths();
    let f = fixture(&env);
    buy(&f, &f.key_a, &f.wallet_a, 1);

    // Parity passes (both sides are one key at the same price), but wallet_b
    // holds nothing to send, so the second leg fails and the swap reverts whole.
    let result = f
        .client
        .try_atomic_swap(&f.key_a, &1, &f.wallet_a, &f.key_b, &1, &f.wallet_b, &0);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));

    assert_eq!(f.client.get_key_balance(&f.key_a, &f.wallet_a), 1);
    assert_eq!(f.client.get_key_balance(&f.key_a, &f.wallet_b), 0);
}
