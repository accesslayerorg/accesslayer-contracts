// =============================================================================
// #806 — Integration tests covering the full staking lifecycle from stake
// through early unstake and reward claim.
//
// The staking feature spans `stake_keys_locked`, `stake_extend`,
// `early_unstake` and `claim_stake_reward` across multiple ledgers. These tests
// simulate the full lifecycle, including reward pool accumulation from protocol
// trade fees, and assert every state transition.
//
// Reward pool accrual model used by these tests:
//   - Each `buy_key` at flat price P collects a protocol trade fee of
//     `protocol_fee_bps`% of P.
//   - `REWARDS_SHARE_BPS` (1_000 = 10%) of that trade fee is routed into the
//     creator's staking rewards pool.
//
// With `key_price = 100` and `protocol_fee_bps = 1_000` (10%), every buy
// contributes exactly `1` to the pool, which keeps the pool arithmetic exact
// and greppable in the assertions below.
// =============================================================================

#[cfg(test)]
mod staking_lifecycle_tests {
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{Address, Env, String};

    use crate::{
        CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams, StakingError,
    };

    const KEY_PRICE: i128 = 100;
    // 10% protocol trade fee: each buy prices at 100 -> trade fee of 10.
    const PROTOCOL_FEE_BPS: u32 = 1_000;

    /// Registers a creator at a flat price of `KEY_PRICE`. The protocol trade
    /// fee is intentionally NOT configured here so tests that do not care about
    /// reward-pool accumulation start from an empty pool; tests that need fee
    /// accrual call `set_protocol_fee` explicitly.
    /// Returns `(env, client, admin, creator, treasury)`.
    fn setup() -> (
        Env,
        CreatorKeysContractClient<'static>,
        Address,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let treasury = Address::generate(&env);

        client.set_protocol_admin(&admin, &admin);
        client.set_key_price(&admin, &KEY_PRICE);
        // 100% to the creator so every paid amount minus the trade fee is easy
        // to reason about; the trade fee itself is configured separately below.
        client.set_fee_config(&admin, &10_000, &0);

        let creator = Address::generate(&env);
        client.register_creator(
            &RegisterCreatorParams {
                creator: creator.clone(),
                handle: String::from_str(&env, "alice"),
            },
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        (env, client, admin, creator, treasury)
    }

    /// Enables the 10% protocol trade fee so every buy accrues the staking
    /// rewards pool (`REWARD_SHARE_PER_BUY` per buy).
    fn enable_fee_accrual(client: &CreatorKeysContractClient, admin: &Address, treasury: &Address) {
        client.set_protocol_fee(admin, &Some(PROTOCOL_FEE_BPS), treasury);
    }

    /// Buys `count` keys from `creator` with `buyer`.
    fn buy_keys(
        client: &CreatorKeysContractClient,
        creator: &Address,
        buyer: &Address,
        count: u32,
    ) {
        for _ in 0..count {
            client.buy_key(creator, buyer, &KEY_PRICE, &None);
        }
    }

    /// Advances the ledger sequence to `target` to simulate the passage of time
    /// across staking lock periods.
    fn jump_ledger(env: &Env, target: u32) {
        let mut ledger_info = env.ledger().get();
        ledger_info.sequence_number = target;
        env.ledger().set(ledger_info);
    }

    #[test]
    fn test_stake_records_position_and_books_keys_as_staked() {
        let (env, client, _admin, creator, _treasury) = setup();
        let holder = Address::generate(&env);

        // Buy 10 keys so the holder has a liquid balance to stake.
        buy_keys(&client, &creator, &holder, 10);
        assert_eq!(client.get_key_balance(&creator, &holder), 10);

        let start_seq = env.ledger().sequence();
        let lock_ledgers = 100u32;

        let stake_id = client.stake_keys_locked(&creator, &holder, &10u32, &lock_ledgers);
        assert_eq!(stake_id, 0);

        // Pool records the cross-holder staked total.
        assert_eq!(client.get_total_staked(&creator), 10);
        // Liquid balance is fully staked, so nothing remains sellable.
        assert_eq!(client.get_liquid_balance(&creator, &holder), 0);
        assert_eq!(client.get_staked_balance(&creator, &holder), 10);

        // Verify the stored staking position.
        let position = client
            .get_staking_position(&creator, &holder, &stake_id)
            .unwrap();
        assert_eq!(position.stake_id, 0);
        assert_eq!(position.amount, 10);
        assert_eq!(position.unlock_ledger, start_seq + lock_ledgers);
    }

    #[test]
    fn test_stake_rejects_zero_or_overdraft_and_unregistered_creator() {
        let (env, client, _admin, creator, _treasury) = setup();
        let holder = Address::generate(&env);
        buy_keys(&client, &creator, &holder, 5);

        // Zero amount.
        assert_eq!(
            client.try_stake_keys_locked(&creator, &holder, &0u32, &100u32),
            Err(Ok(StakingError::NotPositiveAmount))
        );
        // Zero lock period.
        assert_eq!(
            client.try_stake_keys_locked(&creator, &holder, &5u32, &0u32),
            Err(Ok(StakingError::NotPositiveAmount))
        );
        // More keys than the holder has liquid.
        assert_eq!(
            client.try_stake_keys_locked(&creator, &holder, &6u32, &100u32),
            Err(Ok(StakingError::InsufficientBalance))
        );

        // Unregistered creator is rejected.
        let ghost = Address::generate(&env);
        assert_eq!(
            client.try_stake_keys_locked(&ghost, &holder, &1u32, &100u32),
            Err(Ok(StakingError::NotRegistered))
        );
    }

    #[test]
    fn test_full_lifecycle_stake_extend_early_unstake_and_claim() {
        let (env, client, admin, creator, treasury) = setup();

        // Enable the known protocol fee rate so trade fees accrue the staking
        // rewards pool.
        enable_fee_accrual(&client, &admin, &treasury);
        assert_eq!(
            client.get_protocol_trade_fee(),
            (PROTOCOL_FEE_BPS, Some(treasury.clone()))
        );

        let holder = Address::generate(&env);
        let trader = Address::generate(&env);

        // Holder buys 12 keys, then stakes 2 (position 0) and 10 (position 1).
        // Each of the holder's own buys already accrues 1 to the pool.
        buy_keys(&client, &creator, &holder, 12);
        let start_seq = env.ledger().sequence();
        let lock_ledgers = 100u32;

        let pos0 = client.stake_keys_locked(&creator, &holder, &2u32, &lock_ledgers);
        let pos1 = client.stake_keys_locked(&creator, &holder, &10u32, &lock_ledgers);
        assert_eq!(pos0, 0);
        assert_eq!(pos1, 1);
        assert_eq!(client.get_total_staked(&creator), 12);
        // Staking itself does not change the pool; it is at 12 from the 12 buys.
        assert_eq!(client.get_staking_rewards_pool(&creator), 12);

        // Sanity: both positions share the same maturity (same start ledger).
        let p0 = client
            .get_staking_position(&creator, &holder, &pos0)
            .unwrap();
        let p1 = client
            .get_staking_position(&creator, &holder, &pos1)
            .unwrap();
        let base_unlock = start_seq + lock_ledgers;
        assert_eq!(p0.unlock_ledger, base_unlock);
        assert_eq!(p1.unlock_ledger, base_unlock);

        // -----------------------------------------------------------------
        // Pool accumulation from protocol fees over several trade events.
        // Each buy by an unrelated trader contributes exactly 1 to the pool,
        // on top of the 12 already accrued by the holder's buys.
        // -----------------------------------------------------------------
        buy_keys(&client, &creator, &trader, 1);
        assert_eq!(client.get_staking_rewards_pool(&creator), 13);
        buy_keys(&client, &creator, &trader, 2);
        assert_eq!(client.get_staking_rewards_pool(&creator), 15);
        buy_keys(&client, &creator, &trader, 57);
        assert_eq!(client.get_staking_rewards_pool(&creator), 72);

        // -----------------------------------------------------------------
        // stake_extend pushes position 1's maturity forward by 50 ledgers.
        // -----------------------------------------------------------------
        let additional = 50u32;
        let extended_unlock = client.stake_extend(&creator, &holder, &pos1, &additional);
        assert_eq!(extended_unlock, base_unlock + additional);
        let p1_ext = client
            .get_staking_position(&creator, &holder, &pos1)
            .unwrap();
        assert_eq!(p1_ext.unlock_ledger, base_unlock + additional);

        // Position 0 still matures at the original ledger.
        let p0_after = client
            .get_staking_position(&creator, &holder, &pos0)
            .unwrap();
        assert_eq!(p0_after.unlock_ledger, base_unlock);

        // -----------------------------------------------------------------
        // early_unstake position 0 (2 keys) while it is still locked.
        //
        // reward_share     = amount * pool / total_staked = 2 * 72 / 12 = 12
        // penalty          = 20% of reward_share          = 2
        // pool'            = pool - reward_share + penalty = 72 - 12 + 2 = 62
        // total_staked'    = 12 - 2 = 10
        // -----------------------------------------------------------------
        assert!(env.ledger().sequence() < p0_after.unlock_ledger);
        let exit = client.early_unstake(&creator, &holder, &pos0);
        assert_eq!(exit.stake_id, pos0);
        assert_eq!(exit.amount, 2);
        assert_eq!(exit.forgone_reward, 12);
        assert_eq!(exit.penalty, 2);

        // The penalty is retained in the pool (reward entitlement removed, then
        // penalty added back).
        assert_eq!(client.get_staking_rewards_pool(&creator), 62);
        assert_eq!(client.get_total_staked(&creator), 10);
        // Position is closed and keys return to the liquid balance.
        assert!(client
            .get_staking_position(&creator, &holder, &pos0)
            .is_none());
        assert_eq!(client.get_staked_balance(&creator, &holder), 10);
        assert_eq!(client.get_liquid_balance(&creator, &holder), 2);

        // early_unstake on an already-closed position reverts.
        assert_eq!(
            client.try_early_unstake(&creator, &holder, &pos0),
            Err(Ok(StakingError::PositionNotFound))
        );

        // -----------------------------------------------------------------
        // Advance past position 1's maturity, then claim its reward.
        //
        // reward'         = amount * pool / total_staked = 10 * 62 / 10 = 62
        // pool''          = 62 - 62 = 0
        // total_staked''  = 10 - 10 = 0
        // -----------------------------------------------------------------
        jump_ledger(&env, extended_unlock + 10);

        // While matured, early_unstake is rejected in favour of claim.
        assert_eq!(
            client.try_early_unstake(&creator, &holder, &pos1),
            Err(Ok(StakingError::PositionNotLocked))
        );

        let claim = client.claim_stake_reward(&creator, &holder, &pos1);
        assert_eq!(claim.stake_id, pos1);
        assert_eq!(claim.amount, 10);
        assert_eq!(claim.reward, 62);

        // Pool is fully drained and no positions remain.
        assert_eq!(client.get_staking_rewards_pool(&creator), 0);
        assert_eq!(client.get_total_staked(&creator), 0);
        assert!(client
            .get_staking_position(&creator, &holder, &pos1)
            .is_none());
        // All 12 of the holder's keys are back in liquid balance.
        assert_eq!(client.get_staked_balance(&creator, &holder), 0);
        assert_eq!(client.get_liquid_balance(&creator, &holder), 12);
    }

    #[test]
    fn test_stake_extend_requires_locked_position_and_positive_period() {
        let (env, client, _admin, creator, _treasury) = setup();
        let holder = Address::generate(&env);
        buy_keys(&client, &creator, &holder, 3);

        let lock_ledgers = 100u32;
        let pos = client.stake_keys_locked(&creator, &holder, &3u32, &lock_ledgers);

        // Zero extension period reverts.
        assert_eq!(
            client.try_stake_extend(&creator, &holder, &pos, &0u32),
            Err(Ok(StakingError::NotPositiveAmount))
        );
        // Unknown position reverts.
        assert_eq!(
            client.try_stake_extend(&creator, &holder, &99u32, &10u32),
            Err(Ok(StakingError::PositionNotFound))
        );
        // A positive extension succeeds.
        let new_unlock = client.stake_extend(&creator, &holder, &pos, &25u32);
        let stored = client
            .get_staking_position(&creator, &holder, &pos)
            .unwrap();
        assert_eq!(stored.unlock_ledger, new_unlock);
    }

    #[test]
    fn test_claim_requires_maturity_and_early_unstake_requires_lock() {
        let (env, client, _admin, creator, _treasury) = setup();
        let holder = Address::generate(&env);
        buy_keys(&client, &creator, &holder, 1);
        let lock_ledgers = 100u32;

        let pos = client.stake_keys_locked(&creator, &holder, &1u32, &lock_ledgers);
        let unlock = client
            .get_staking_position(&creator, &holder, &pos)
            .unwrap()
            .unlock_ledger;

        // Claim while still locked reverts.
        assert!(env.ledger().sequence() < unlock);
        assert_eq!(
            client.try_claim_stake_reward(&creator, &holder, &pos),
            Err(Ok(StakingError::PositionLocked))
        );
    }

    #[test]
    fn test_claim_after_maturity_returns_reward_and_defaults_to_zero_pool() {
        let (env, client, _admin, creator, _treasury) = setup();
        let holder = Address::generate(&env);
        buy_keys(&client, &creator, &holder, 4);

        let lock_ledgers = 100u32;
        let pos = client.stake_keys_locked(&creator, &holder, &4u32, &lock_ledgers);
        let unlock = client
            .get_staking_position(&creator, &holder, &pos)
            .unwrap()
            .unlock_ledger;

        // Advance past maturity without any fees accruing: reward is 0 since the
        // pool never accrued, but keys are still returned.
        jump_ledger(&env, unlock + 5);
        let claim = client.claim_stake_reward(&creator, &holder, &pos);
        assert_eq!(claim.amount, 4);
        assert_eq!(claim.reward, 0);
        assert_eq!(client.get_staking_rewards_pool(&creator), 0);
        assert_eq!(client.get_liquid_balance(&creator, &holder), 4);
    }
}
