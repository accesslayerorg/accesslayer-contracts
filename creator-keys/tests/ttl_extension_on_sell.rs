//! Integration tests for persistent-storage TTL extension during sell transactions (#741).
//!
//! A creator's profile entry must survive active trading. `buy_key` extends the
//! creator's persistent TTL and is covered by `ttl_extension_on_buy.rs`; the sell
//! path calls the same `extend_creator_ttl` helper and needs the same guarantees:
//! every successful sell must push the entry back to a full window, repeated sells
//! must *reset* the window rather than stack it, and a reverted sell must not
//! extend anything.
//!
//! These tests assert on the real remaining TTL read back out of the test
//! ledger, so removing the `extend_creator_ttl` call from `sell_key` fails them.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_key_price_for_tests};
use creator_keys::constants::storage;
use creator_keys::{CREATOR_TTL_LEDGERS, TTL_EXTENSION_THRESHOLD};
use soroban_sdk::testutils::storage::Persistent;
use soroban_sdk::testutils::Ledger;
use soroban_sdk::{testutils::Address as _, Address, Env};

const KEY_PRICE: i128 = 100;

/// Remaining TTL, in ledgers, on the creator's persistent profile entry.
fn creator_ttl_remaining(env: &Env, contract_id: &Address, creator: &Address) -> u32 {
    let key = storage::creator(creator);
    env.as_contract(contract_id, || env.storage().persistent().get_ttl(&key))
}

/// Advance the ledger sequence by `ledgers`, draining TTL from live entries.
fn advance_ledgers(env: &Env, ledgers: u32) {
    let mut ledger = env.ledger().get();
    ledger.sequence_number += ledgers;
    env.ledger().set(ledger);
}

fn setup(
    env: &Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    Address,
    Address,
    Address,
) {
    let (client, contract_id) = register_creator_keys(env);
    // The default test env archives the contract instance after ~4095 ledgers.
    // These tests deliberately jump far into the future to drain the creator
    // entry's TTL, so the instance needs the full window to stay invocable.
    env.deployer().extend_ttl(
        contract_id.clone(),
        CREATOR_TTL_LEDGERS,
        CREATOR_TTL_LEDGERS,
    );
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let key_price_key = storage::KEY_PRICE;
    env.as_contract(&contract_id, || {
        env.storage().persistent().extend_ttl(
            &key_price_key,
            CREATOR_TTL_LEDGERS,
            CREATOR_TTL_LEDGERS,
        );
    });
    let creator = register_test_creator(env, &client, "alice");
    let holder = Address::generate(env);
    (client, contract_id, creator, holder)
}

/// After a successful sell the creator entry must be back at (or above) a full
/// extension window — not merely "a bit higher than it was".
#[test]
fn sell_restores_creator_ttl_to_the_full_window() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator, holder) = setup(&env);

    // Two keys so the seller still holds one afterwards and the profile entry
    // is not removed as part of a full exit.
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    // Drain the TTL down to almost nothing.
    let ttl_before_advance = creator_ttl_remaining(&env, &contract_id, &creator);
    advance_ledgers(&env, ttl_before_advance.saturating_sub(1).max(1));

    let ttl_before_sell = creator_ttl_remaining(&env, &contract_id, &creator);
    assert!(
        ttl_before_sell < TTL_EXTENSION_THRESHOLD,
        "precondition: TTL should be drained below the extension threshold, got {ttl_before_sell}"
    );

    let result = client.try_sell_key(&creator, &holder, &None);
    assert_eq!(result, Ok(Ok(1)), "sell should succeed");

    let ttl_after_sell = creator_ttl_remaining(&env, &contract_id, &creator);
    assert!(
        ttl_after_sell >= CREATOR_TTL_LEDGERS,
        "TTL after a sell must reach the full window: \
         before={ttl_before_sell} after={ttl_after_sell} window={CREATOR_TTL_LEDGERS}"
    );
}

/// The extension is `current_ledger + CREATOR_TTL_LEDGERS`, an absolute target.
/// A second sell must therefore land on the same window measured from the new
/// ledger — the remaining TTL must not be two windows deep.
#[test]
fn repeated_sells_reset_the_ttl_window_rather_than_accumulate() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator, holder) = setup(&env);

    for _ in 0..3 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }

    let ttl_initial = creator_ttl_remaining(&env, &contract_id, &creator);
    advance_ledgers(&env, ttl_initial.saturating_sub(1).max(1));

    client.sell_key(&creator, &holder, &None);
    let ttl_after_first = creator_ttl_remaining(&env, &contract_id, &creator);

    // Keep contract instance alive for the second advance
    env.deployer().extend_ttl(
        contract_id.clone(),
        CREATOR_TTL_LEDGERS,
        CREATOR_TTL_LEDGERS,
    );
    let key_price_key = storage::KEY_PRICE;
    env.as_contract(&contract_id, || {
        env.storage().persistent().extend_ttl(
            &key_price_key,
            CREATOR_TTL_LEDGERS,
            CREATOR_TTL_LEDGERS,
        );
    });

    // Burn a chunk of the freshly granted window, then sell again.
    let elapsed = CREATOR_TTL_LEDGERS / 4;
    advance_ledgers(&env, elapsed);
    let ttl_after_elapsing = creator_ttl_remaining(&env, &contract_id, &creator);
    assert!(
        ttl_after_elapsing < ttl_after_first,
        "precondition: advancing the ledger should have consumed TTL"
    );

    client.sell_key(&creator, &holder, &None);
    let ttl_after_second = creator_ttl_remaining(&env, &contract_id, &creator);

    assert!(
        ttl_after_second >= CREATOR_TTL_LEDGERS,
        "the second sell should restore a full window: {ttl_after_second}"
    );
    assert!(
        ttl_after_second <= CREATOR_TTL_LEDGERS + elapsed,
        "the window must be reset from the current ledger, not stacked on the \
         remaining TTL: after_second={ttl_after_second} window={CREATOR_TTL_LEDGERS}"
    );
}

/// A sell that reverts must leave the TTL exactly where it was — a failed
/// transaction rolls back its storage effects, extension included.
#[test]
fn failed_sell_does_not_extend_ttl() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator, holder) = setup(&env);

    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    let ttl_initial = creator_ttl_remaining(&env, &contract_id, &creator);
    advance_ledgers(&env, ttl_initial.saturating_sub(1).max(1));

    let ttl_before_failed_sell = creator_ttl_remaining(&env, &contract_id, &creator);

    // `min_proceeds` above the achievable payout trips the slippage guard, so
    // the sell reverts after the entrypoint has been entered.
    let result = client.try_sell_key(&creator, &holder, &Some(KEY_PRICE * 100));
    assert!(
        result.is_err() || matches!(result, Ok(Err(_))),
        "sell should revert on slippage"
    );

    assert_eq!(
        creator_ttl_remaining(&env, &contract_id, &creator),
        ttl_before_failed_sell,
        "a reverted sell must not extend the creator TTL"
    );
}

/// A sell by a wallet holding no keys reverts before reaching the extension
/// call at the end of `sell_key`.
#[test]
fn sell_without_balance_does_not_extend_ttl() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, contract_id, creator, _holder) = setup(&env);

    let ttl_initial = creator_ttl_remaining(&env, &contract_id, &creator);
    advance_ledgers(&env, ttl_initial.saturating_sub(1).max(1));

    let ttl_before = creator_ttl_remaining(&env, &contract_id, &creator);

    let stranger = Address::generate(&env);
    let result = client.try_sell_key(&creator, &stranger, &None);
    assert!(
        result.is_err() || matches!(result, Ok(Err(_))),
        "selling with no balance should revert"
    );

    assert_eq!(
        creator_ttl_remaining(&env, &contract_id, &creator),
        ttl_before,
        "a sell that never reaches the extension call must not extend the TTL"
    );
}
