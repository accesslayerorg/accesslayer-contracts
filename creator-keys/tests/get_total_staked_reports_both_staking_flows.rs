//! Regression test: `get_total_staked` must report keys staked through the
//! simple `stake_keys` flow, not only the locked `stake_keys_locked` flow.
//!
//! The two flows maintain separate aggregates:
//! - `stake_keys_locked` / `early_unstake` / `claim_stake_reward` maintain
//!   `StakingRewardsState::total_staked` (alongside the rewards pool).
//! - `stake_keys` / `unstake_keys` maintain `DataKey::TotalStaked`.
//!
//! The view previously read only the former, so it reported `0` for keys
//! staked through the simple flow.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use soroban_sdk::{testutils::Address as _, Address};

const KEY_PRICE: i128 = 1_000;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;
const LOCK_LEDGERS: u32 = 100;

/// Buys `keys` keys for `holder`, one buy at a time.
fn buy_keys(
    client: &creator_keys::CreatorKeysContractClient<'_>,
    creator: &Address,
    holder: &Address,
    keys: u32,
) {
    for _ in 0..keys {
        client.buy_key(creator, holder, &KEY_PRICE, &None);
    }
}

#[test]
fn simple_stake_flow_is_visible_to_get_total_staked() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    assert_eq!(client.get_total_staked(&creator), 0);

    buy_keys(&client, &creator, &holder, 1);
    client.stake_keys(&creator, &holder, &1u32);

    assert_eq!(
        client.get_staked_balance(&creator, &holder),
        1,
        "the simple flow records the per-wallet staked balance",
    );
    assert_eq!(
        client.get_total_staked(&creator),
        1,
        "get_total_staked must reflect keys staked via stake_keys",
    );

    // Unstaking the last key must bring the aggregate back to zero.
    client.unstake_keys(&creator, &holder, &1u32);
    assert_eq!(client.get_total_staked(&creator), 0);
}

#[test]
fn locked_stake_flow_remains_visible_to_get_total_staked() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "bob");
    let holder = Address::generate(&env);

    buy_keys(&client, &creator, &holder, 4);
    client.stake_keys_locked(&creator, &holder, &3u32, &LOCK_LEDGERS);

    assert_eq!(
        client.get_total_staked(&creator),
        3,
        "get_total_staked must still reflect keys staked via stake_keys_locked",
    );
}

#[test]
fn both_staking_flows_are_summed() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "carol");
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    // holder_a stakes 2 via the simple flow.
    buy_keys(&client, &creator, &holder_a, 2);
    client.stake_keys(&creator, &holder_a, &2u32);

    // holder_b stakes 3 via the locked flow.
    buy_keys(&client, &creator, &holder_b, 3);
    client.stake_keys_locked(&creator, &holder_b, &3u32, &LOCK_LEDGERS);

    assert_eq!(client.get_staked_balance(&creator, &holder_a), 2);
    assert_eq!(client.get_staked_balance(&creator, &holder_b), 3);
    assert_eq!(
        client.get_total_staked(&creator),
        5,
        "both flows must contribute to the same aggregate",
    );
}
