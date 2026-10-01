use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, String, Vec};

use crate::{
    ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
    StakingError, StakingMultiplierTier,
};

fn setup() -> (Env, CreatorKeysContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &10_000);
    client.set_fee_config(&admin, &10_000, &0);
    client.set_protocol_fee(&admin, &Some(1_000), &treasury);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "multiplier"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    (env, client, admin, creator)
}

#[test]
fn default_tiers_weight_and_distribute_rewards() {
    let (env, client, _admin, creator) = setup();
    let holders = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    for (holder, period) in holders.iter().zip([1u32, 100, 200]) {
        for _ in 0..3 {
            client.buy_key(&creator, holder, &10_000, &None);
        }
        client.stake_keys_locked(&creator, holder, &2, &period);
    }
    assert_eq!(client.get_staking_rewards_pool(&creator), 900);
    assert_eq!(client.get_total_staked(&creator), 6);
    for (holder, weight) in holders.iter().zip([20_000i128, 30_000, 40_000]) {
        assert_eq!(client.get_staking_weight(&creator, holder, &0), weight);
    }
    let mut ledger = env.ledger().get();
    ledger.sequence_number += 200;
    env.ledger().set(ledger);
    for (holder, reward) in holders.iter().zip([200i128, 300, 400]) {
        assert_eq!(
            client.claim_stake_reward(&creator, holder, &0).reward,
            reward
        );
    }
    assert_eq!(client.get_staking_rewards_pool(&creator), 0);
    assert_eq!(
        client.try_get_staking_weight(&creator, &holders[0], &0),
        Err(Ok(StakingError::PositionNotFound))
    );
}

#[test]
fn tier_update_requires_admin_and_applies_to_new_positions() {
    let (env, client, admin, creator) = setup();
    let holder = Address::generate(&env);
    let non_admin = Address::generate(&env);
    for _ in 0..4 {
        client.buy_key(&creator, &holder, &10_000, &None);
    }
    let first = client.stake_keys_locked(&creator, &holder, &2, &100);
    let mut tiers = Vec::new(&env);
    tiers.push_back(StakingMultiplierTier {
        min_lock_ledgers: 1,
        multiplier_bps: 10_000,
    });
    tiers.push_back(StakingMultiplierTier {
        min_lock_ledgers: 100,
        multiplier_bps: 20_000,
    });
    assert_eq!(
        client.try_set_staking_multiplier_tiers(&non_admin, &tiers),
        Err(Ok(ContractError::Unauthorized))
    );
    client.set_staking_multiplier_tiers(&admin, &tiers);
    assert_eq!(client.get_staking_multiplier_tiers(), tiers);
    let second = client.stake_keys_locked(&creator, &holder, &2, &100);
    assert_eq!(client.get_staking_weight(&creator, &holder, &first), 30_000);
    assert_eq!(
        client.get_staking_weight(&creator, &holder, &second),
        40_000
    );
}
