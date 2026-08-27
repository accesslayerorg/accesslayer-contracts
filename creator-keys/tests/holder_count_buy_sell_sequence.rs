//! Unit tests for the holder-count view across a full buy/sell sequence (#743).
//!
//! `get_creator_holder_count` must report the number of *unique wallets* currently
//! holding at least one key — not the number of keys, and not the number of wallets
//! that have ever traded. `holder_count_multiple_buyers.rs` covers three wallets
//! holding one key each; the case that distinguishes a unique-wallet counter from a
//! key counter is a wallet holding *several* keys, where only the sale of the last
//! key may decrement the count.
//!
//! Every step also asserts the view against `get_creator_supply` and the per-wallet
//! balance, so the count cannot drift away from the state it is derived from.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, set_key_price_for_tests};
use soroban_sdk::{testutils::Address as _, Address, Env};

const KEY_PRICE: i128 = 100;

/// Assert the holder count, total supply and both wallets' balances in one step.
fn assert_state(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    expected_holders: u32,
    expected_supply: u32,
    wallets: &[(&Address, u32)],
    step: &str,
) {
    assert_eq!(
        client.get_creator_holder_count(creator),
        expected_holders,
        "holder count mismatch at step: {step}"
    );
    assert_eq!(
        client.get_creator_supply(creator),
        expected_supply,
        "supply mismatch at step: {step}"
    );
    for (wallet, expected_balance) in wallets {
        assert_eq!(
            client.get_key_balance(creator, wallet),
            *expected_balance,
            "wallet balance mismatch at step: {step}"
        );
    }
}

fn setup(
    env: &Env,
) -> (
    creator_keys::CreatorKeysContractClient<'_>,
    Address,
    Address,
    Address,
) {
    let (client, _contract_id) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    (
        client,
        creator,
        Address::generate(env),
        Address::generate(env),
    )
}

#[test]
fn holder_count_is_zero_before_any_buys() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator, wallet_a, _wallet_b) = setup(&env);

    assert_state(
        &client,
        &creator,
        0,
        0,
        &[(&wallet_a, 0)],
        "freshly registered creator",
    );
}

#[test]
fn holder_count_tracks_two_wallets_through_buys_and_full_exits() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator, wallet_a, wallet_b) = setup(&env);

    assert_state(
        &client,
        &creator,
        0,
        0,
        &[(&wallet_a, 0), (&wallet_b, 0)],
        "before any buys",
    );

    // Two distinct wallets buy; each first buy is a new holder.
    client.buy_key(&creator, &wallet_a, &KEY_PRICE, &None);
    assert_state(
        &client,
        &creator,
        1,
        1,
        &[(&wallet_a, 1), (&wallet_b, 0)],
        "wallet A's first buy",
    );

    client.buy_key(&creator, &wallet_b, &KEY_PRICE, &None);
    assert_state(
        &client,
        &creator,
        2,
        2,
        &[(&wallet_a, 1), (&wallet_b, 1)],
        "wallet B's first buy",
    );

    // Wallet A sells its only key: a full exit, so the count drops to 1.
    client.sell_key(&creator, &wallet_a, &None);
    assert_state(
        &client,
        &creator,
        1,
        1,
        &[(&wallet_a, 0), (&wallet_b, 1)],
        "wallet A sells all of its keys",
    );

    // Wallet B follows: no holders left.
    client.sell_key(&creator, &wallet_b, &None);
    assert_state(
        &client,
        &creator,
        0,
        0,
        &[(&wallet_a, 0), (&wallet_b, 0)],
        "wallet B sells all of its keys",
    );
}

/// The discriminating case: a wallet with several keys stays a holder until its
/// balance actually reaches zero.
#[test]
fn partial_sells_do_not_decrement_the_holder_count() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator, wallet_a, wallet_b) = setup(&env);

    // Wallet A takes three keys, wallet B takes two.
    for _ in 0..3 {
        client.buy_key(&creator, &wallet_a, &KEY_PRICE, &None);
    }
    for _ in 0..2 {
        client.buy_key(&creator, &wallet_b, &KEY_PRICE, &None);
    }
    assert_state(
        &client,
        &creator,
        2,
        5,
        &[(&wallet_a, 3), (&wallet_b, 2)],
        "two wallets holding multiple keys",
    );

    // Selling down wallet A one key at a time leaves the count at 2 until the
    // last key goes. If the count tracked keys instead of wallets, the first of
    // these assertions would fail.
    client.sell_key(&creator, &wallet_a, &None);
    assert_state(
        &client,
        &creator,
        2,
        4,
        &[(&wallet_a, 2), (&wallet_b, 2)],
        "wallet A partial sell (2 keys left)",
    );

    client.sell_key(&creator, &wallet_a, &None);
    assert_state(
        &client,
        &creator,
        2,
        3,
        &[(&wallet_a, 1), (&wallet_b, 2)],
        "wallet A partial sell (1 key left)",
    );

    client.sell_key(&creator, &wallet_a, &None);
    assert_state(
        &client,
        &creator,
        1,
        2,
        &[(&wallet_a, 0), (&wallet_b, 2)],
        "wallet A's final key sold",
    );

    // Wallet B exits the same way.
    client.sell_key(&creator, &wallet_b, &None);
    assert_state(
        &client,
        &creator,
        1,
        1,
        &[(&wallet_a, 0), (&wallet_b, 1)],
        "wallet B partial sell",
    );

    client.sell_key(&creator, &wallet_b, &None);
    assert_state(
        &client,
        &creator,
        0,
        0,
        &[(&wallet_a, 0), (&wallet_b, 0)],
        "all holders exited",
    );
}

/// A wallet that buys again after a full exit is counted once more, and repeat
/// buys by an existing holder never double-count.
#[test]
fn repeat_buys_and_re_entry_are_counted_once_per_wallet() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let (client, creator, wallet_a, wallet_b) = setup(&env);

    client.buy_key(&creator, &wallet_a, &KEY_PRICE, &None);
    client.buy_key(&creator, &wallet_a, &KEY_PRICE, &None);
    assert_state(
        &client,
        &creator,
        1,
        2,
        &[(&wallet_a, 2)],
        "a second buy by the same wallet is not a second holder",
    );

    client.sell_key(&creator, &wallet_a, &None);
    client.sell_key(&creator, &wallet_a, &None);
    assert_state(
        &client,
        &creator,
        0,
        0,
        &[(&wallet_a, 0)],
        "wallet A exited",
    );

    // Re-entry counts again.
    client.buy_key(&creator, &wallet_a, &KEY_PRICE, &None);
    client.buy_key(&creator, &wallet_b, &KEY_PRICE, &None);
    assert_state(
        &client,
        &creator,
        2,
        2,
        &[(&wallet_a, 1), (&wallet_b, 1)],
        "wallet A re-entered alongside wallet B",
    );
}
