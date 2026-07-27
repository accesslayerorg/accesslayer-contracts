// =============================================================================
// Tests for issues #487, #489, #491, #492
// =============================================================================

#[cfg(test)]
mod issue_tests {
    extern crate std;
    use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

    use crate::{
        compute_bonding_curve_price, constants, ContractError, CreatorKeysContract,
        CreatorKeysContractClient, CurvePreset,
    };

    const KEY_PRICE: i128 = 100;

    /// Register a creator with the given supply cap (pass `None` for no cap).
    /// Returns the creator id used in subsequent calls.
    fn register_creator(
        env: &Env,
        client: &CreatorKeysContractClient,
        cap: Option<u32>,
    ) -> Address {
        let creator = Address::generate(env);
        let handle = String::from_str(env, "alice");
        match cap {
            Some(c) => {
                client.register_creator(
                    &crate::RegisterCreatorParams {
                        creator: creator.clone(),
                        handle: handle.clone(),
                    },
                    &None,
                    &Some(c),
                    &None,
                    &None,
                    &None,
                    &None,
                );
            }
            None => {
                client.register_creator(
                    &crate::RegisterCreatorParams {
                        creator: creator.clone(),
                        handle: handle.clone(),
                    },
                    &None,
                    &None,
                    &None,
                    &None,
                    &None,
                    &None,
                );
            }
        }
        creator
    }

    /// Assert that a creator's `total_supply` equals the sum of every holder's
    /// individual balance.
    fn assert_supply_equals_holder_sum(
        _env: &Env,
        client: &CreatorKeysContractClient,
        creator_id: &Address,
        holders: Vec<Address>,
    ) {
        let total_supply: u32 = client.get_total_key_supply(creator_id);

        let mut computed_sum: u32 = 0u32;
        for holder in holders.iter() {
            let balance: u32 = client.get_key_balance(creator_id, &holder);
            computed_sum = computed_sum
                .checked_add(balance)
                .expect("holder balance sum overflowed u32");
        }

        assert_eq!(
            total_supply, computed_sum,
            "Supply invariant violated for creator {creator_id:?}: \
             total_supply={total_supply} but sum of holder balances={computed_sum}"
        );
    }

    #[test]
    fn test_distribute_dividend_reverts_on_zero_supply() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let creator = register_creator(&env, &client, None);
        let caller = Address::generate(&env);

        let result = client.try_distribute_dividend(&creator, &caller, &5_000_000);

        assert!(
            result.is_err(),
            "distribute_dividend should revert when total supply is zero, but it succeeded"
        );

        let err = result.unwrap_err().unwrap();
        assert!(
            matches!(err, ContractError::NoKeyHolders),
            "Expected ContractError::NoKeyHolders, got {err:?}"
        );
    }

    #[test]
    fn test_multi_holder_dividend_majority_holder_receives_larger_share() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.set_key_price(&admin, &KEY_PRICE);
        client.set_fee_config(&admin, &10_000, &0);

        let creator = register_creator(&env, &client, None);

        let wallet_a = Address::generate(&env);
        let wallet_b = Address::generate(&env);

        for _ in 0..90 {
            client.buy_key(&creator, &wallet_a, &KEY_PRICE, &None);
        }
        for _ in 0..10 {
            client.buy_key(&creator, &wallet_b, &KEY_PRICE, &None);
        }

        assert_eq!(client.get_total_key_supply(&creator), 100u32);

        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, wallet_a.clone(), wallet_b.clone()],
        );

        let distributor = Address::generate(&env);
        let gross_amount: i128 = 1_000;
        client.distribute_dividend(&creator, &distributor, &gross_amount);

        let claimable_a: i128 = client.get_claimable_dividend(&creator, &wallet_a);
        let claimable_b: i128 = client.get_claimable_dividend(&creator, &wallet_b);

        assert!(
            claimable_a > claimable_b,
            "Wallet A (90 keys) should receive more than wallet B (10 keys), \
             but got claimable_a={claimable_a}, claimable_b={claimable_b}"
        );
        assert_eq!(
            claimable_a, 900,
            "Wallet A should receive 900 stroops (90% of 1000), got {claimable_a}"
        );
        assert_eq!(
            claimable_b, 100,
            "Wallet B should receive 100 stroops (10% of 1000), got {claimable_b}"
        );

        let total_claimable = claimable_a + claimable_b;
        assert_eq!(
            total_claimable, gross_amount,
            "Claimable amounts ({total_claimable}) should sum to distributed amount ({gross_amount})"
        );
        assert!(
            claimable_a <= gross_amount * 90 / 100 + 1,
            "Wallet A received more than its 90% proportion"
        );
        assert!(
            claimable_b <= gross_amount * 10 / 100 + 1,
            "Wallet B received more than its 10% proportion"
        );
    }

    #[test]
    fn test_supply_cap_rejects_partial_exceed() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.set_key_price(&admin, &KEY_PRICE);

        let creator = register_creator(&env, &client, Some(10u32));
        let buyer = Address::generate(&env);

        for _ in 0..8 {
            client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
        }
        assert_eq!(
            client.get_total_key_supply(&creator),
            8u32,
            "Total supply should be 8 after buying 8 keys"
        );

        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone()],
        );

        for _ in 0..2 {
            client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
        }
        assert_eq!(
            client.get_total_key_supply(&creator),
            10u32,
            "Total supply should be 10 after filling to the cap"
        );

        let result = client.try_buy_key(&creator, &buyer, &KEY_PRICE, &None);
        assert!(
            result.is_err(),
            "Buying at the cap should revert, but it succeeded"
        );

        let err = result.unwrap_err().unwrap();
        assert!(
            matches!(err, ContractError::SupplyCapExceeded),
            "Expected ContractError::SupplyCapExceeded, got {err:?}"
        );

        assert_eq!(
            client.get_total_key_supply(&creator),
            10u32,
            "Total supply should remain at 10 after a reverted buy"
        );

        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone()],
        );
    }

    #[test]
    fn test_invariant_after_buy() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.set_key_price(&admin, &KEY_PRICE);

        let creator = register_creator(&env, &client, None);
        let buyer = Address::generate(&env);

        client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone()],
        );
    }

    #[test]
    fn test_invariant_after_sell() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.set_key_price(&admin, &KEY_PRICE);

        let creator = register_creator(&env, &client, None);
        let buyer = Address::generate(&env);

        for _ in 0..10 {
            client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
        }

        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone()],
        );

        for _ in 0..4 {
            client.sell_key(&creator, &buyer, &None);
        }

        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone()],
        );

        assert_eq!(
            client.get_total_key_supply(&creator),
            6u32,
            "Total supply should be 6 after selling 4 of 10 keys"
        );
    }

    #[test]
    fn test_invariant_after_transfer() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.set_key_price(&admin, &KEY_PRICE);

        let creator = register_creator(&env, &client, None);
        let sender = Address::generate(&env);
        let receiver = Address::generate(&env);

        for _ in 0..8 {
            client.buy_key(&creator, &sender, &KEY_PRICE, &None);
        }

        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, sender.clone(), receiver.clone()],
        );

        client.transfer_keys(&creator, &sender, &receiver, &3u32);

        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, sender.clone(), receiver.clone()],
        );

        assert_eq!(client.get_key_balance(&creator, &sender), 5u32);
        assert_eq!(client.get_key_balance(&creator, &receiver), 3u32);
        assert_eq!(client.get_total_key_supply(&creator), 8u32);
    }

    #[test]
    fn test_holder_balance_key_generation() {
        let env = Env::default();
        let creator_1 = Address::generate(&env);
        let holder_1 = Address::generate(&env);
        let creator_2 = Address::generate(&env);
        let holder_2 = Address::generate(&env);

        let key_1 = crate::constants::storage::holder_balance_key(&creator_1, &holder_1);
        let key_2 = crate::constants::storage::holder_balance_key(&creator_2, &holder_2);

        assert_eq!(key_1, crate::DataKey::KeyBalance(creator_1, holder_1));
        assert_eq!(key_2, crate::DataKey::KeyBalance(creator_2, holder_2));
    }

    fn test_bonding_curve_step_helper(
        supply: u32,
        expected_flat: i128,
        expected_linear: i128,
        expected_quadratic: i128,
    ) {
        let env = Env::default();
        let contract_id = env.register(CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let base_price = 100i128;
        let slope = 10i128;

        env.as_contract(&contract_id, || {
            // Test Flat curve
            env.storage().persistent().set(
                &constants::storage::curve_preset(&creator),
                &CurvePreset::Flat,
            );
            let price_flat =
                compute_bonding_curve_price(&env, &creator, base_price, supply).unwrap();
            assert_eq!(
                price_flat, expected_flat,
                "Flat price mismatch at supply {}",
                supply
            );

            // Test Linear curve
            env.storage().persistent().set(
                &constants::storage::curve_preset(&creator),
                &CurvePreset::Linear,
            );
            env.storage()
                .persistent()
                .set(&constants::storage::CURVE_SLOPE, &slope);
            let price_linear =
                compute_bonding_curve_price(&env, &creator, base_price, supply).unwrap();
            assert_eq!(
                price_linear, expected_linear,
                "Linear price mismatch at supply {}",
                supply
            );

            // Test Quadratic curve
            env.storage().persistent().set(
                &constants::storage::curve_preset(&creator),
                &CurvePreset::Quadratic,
            );
            env.storage()
                .persistent()
                .set(&constants::storage::CURVE_SLOPE, &slope);
            let price_quadratic =
                compute_bonding_curve_price(&env, &creator, base_price, supply).unwrap();
            assert_eq!(
                price_quadratic, expected_quadratic,
                "Quadratic price mismatch at supply {}",
                supply
            );
        });
    }

    #[test]
    fn test_bonding_curve_step_0() {
        test_bonding_curve_step_helper(0, 100, 100, 100);
    }

    #[test]
    fn test_bonding_curve_step_1() {
        test_bonding_curve_step_helper(1, 100, 110, 110);
    }

    #[test]
    fn test_bonding_curve_step_2() {
        test_bonding_curve_step_helper(2, 100, 120, 140);
    }

    #[test]
    fn test_bonding_curve_step_3() {
        test_bonding_curve_step_helper(3, 100, 130, 190);
    }

    #[test]
    fn test_bonding_curve_step_4() {
        test_bonding_curve_step_helper(4, 100, 140, 260);
    }

    #[test]
    fn test_bonding_curve_step_5() {
        test_bonding_curve_step_helper(5, 100, 150, 350);
    }

    // =========================================================================
    // Storage key collision tests (#597)
    //
    // Confirm that two distinct inputs never produce the same storage key.
    // =========================================================================

    #[test]
    fn test_holder_balance_key_different_holders_differ_for_same_creator() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder_a = Address::generate(&env);
        let holder_b = Address::generate(&env);

        let key_a = constants::storage::holder_balance_key(&creator, &holder_a);
        let key_b = constants::storage::holder_balance_key(&creator, &holder_b);

        assert_ne!(
            key_a, key_b,
            "different holders must produce different keys"
        );
    }

    #[test]
    fn test_holder_balance_key_argument_order_matters() {
        let env = Env::default();
        let addr_a = Address::generate(&env);
        let addr_b = Address::generate(&env);

        let key_ab = constants::storage::holder_balance_key(&addr_a, &addr_b);
        let key_ba = constants::storage::holder_balance_key(&addr_b, &addr_a);

        assert_ne!(
            key_ab, key_ba,
            "swapping creator and holder must produce different keys"
        );
    }

    #[test]
    fn test_holder_balance_key_never_equals_creator_key() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder = Address::generate(&env);

        let balance_key = constants::storage::holder_balance_key(&creator, &holder);
        let creator_key = constants::storage::creator(&creator);

        assert_ne!(
            balance_key, creator_key,
            "holder balance key must not collide with creator profile key"
        );
    }

    #[test]
    fn test_holder_balance_key_never_equals_dividend_accumulator_key() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder = Address::generate(&env);

        let balance_key = constants::storage::holder_balance_key(&creator, &holder);
        let dividend_key = constants::storage::dividend_accumulator(&creator);

        assert_ne!(
            balance_key, dividend_key,
            "holder balance key must not collide with dividend accumulator key"
        );
    }

    #[test]
    fn test_holder_dividend_checkpoint_different_holders_differ() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder_a = Address::generate(&env);
        let holder_b = Address::generate(&env);

        let key_a = constants::storage::holder_dividend_checkpoint(&creator, &holder_a);
        let key_b = constants::storage::holder_dividend_checkpoint(&creator, &holder_b);

        assert_ne!(
            key_a, key_b,
            "different holders must produce different dividend checkpoint keys"
        );
    }

    #[test]
    fn test_holder_dividend_pending_different_holders_differ() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder_a = Address::generate(&env);
        let holder_b = Address::generate(&env);

        let key_a = constants::storage::holder_dividend_pending(&creator, &holder_a);
        let key_b = constants::storage::holder_dividend_pending(&creator, &holder_b);

        assert_ne!(
            key_a, key_b,
            "different holders must produce different dividend pending keys"
        );
    }

    #[test]
    fn test_co_creator_fee_balance_different_pairs_differ() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let co_a = Address::generate(&env);
        let co_b = Address::generate(&env);

        let key_a = constants::storage::co_creator_fee_balance(&creator, &co_a);
        let key_b = constants::storage::co_creator_fee_balance(&creator, &co_b);

        assert_ne!(
            key_a, key_b,
            "different co-creator addresses must produce different fee balance keys"
        );
    }

    #[test]
    fn test_co_creator_fee_balance_different_creators_differ() {
        let env = Env::default();
        let creator_a = Address::generate(&env);
        let creator_b = Address::generate(&env);
        let co_creator = Address::generate(&env);

        let key_a = constants::storage::co_creator_fee_balance(&creator_a, &co_creator);
        let key_b = constants::storage::co_creator_fee_balance(&creator_b, &co_creator);

        assert_ne!(
            key_a, key_b,
            "different creators with same co-creator must produce different keys"
        );
    }

    #[test]
    fn test_creator_fee_balance_different_creators_differ() {
        let env = Env::default();
        let creator_a = Address::generate(&env);
        let creator_b = Address::generate(&env);

        let key_a = constants::storage::creator_fee_balance(&creator_a);
        let key_b = constants::storage::creator_fee_balance(&creator_b);

        assert_ne!(
            key_a, key_b,
            "different creators must produce different fee balance keys"
        );
    }

    #[test]
    fn test_holder_balance_key_never_equals_creator_fee_balance_key() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder = Address::generate(&env);

        let balance_key = constants::storage::holder_balance_key(&creator, &holder);
        let fee_key = constants::storage::creator_fee_balance(&creator);

        assert_ne!(
            balance_key, fee_key,
            "holder balance key must not collide with creator fee balance key"
        );
    }

    #[test]
    fn test_holder_dividend_checkpoint_never_equals_holder_balance_key() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder = Address::generate(&env);

        let checkpoint_key = constants::storage::holder_dividend_checkpoint(&creator, &holder);
        let balance_key = constants::storage::holder_balance_key(&creator, &holder);

        assert_ne!(
            checkpoint_key, balance_key,
            "dividend checkpoint key must not collide with holder balance key"
        );
    }

    #[test]
    fn test_holder_dividend_pending_never_equals_holder_balance_key() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder = Address::generate(&env);

        let pending_key = constants::storage::holder_dividend_pending(&creator, &holder);
        let balance_key = constants::storage::holder_balance_key(&creator, &holder);

        assert_ne!(
            pending_key, balance_key,
            "dividend pending key must not collide with holder balance key"
        );
    }

    #[test]
    fn test_curve_preset_different_creators_differ() {
        let env = Env::default();
        let creator_a = Address::generate(&env);
        let creator_b = Address::generate(&env);

        let key_a = constants::storage::curve_preset(&creator_a);
        let key_b = constants::storage::curve_preset(&creator_b);

        assert_ne!(
            key_a, key_b,
            "different creators must produce different curve preset keys"
        );
    }

    #[test]
    fn test_max_supply_different_creators_differ() {
        let env = Env::default();
        let creator_a = Address::generate(&env);
        let creator_b = Address::generate(&env);

        let key_a = constants::storage::max_supply(&creator_a);
        let key_b = constants::storage::max_supply(&creator_b);

        assert_ne!(
            key_a, key_b,
            "different creators must produce different max supply keys"
        );
    }

    #[test]
    fn test_max_keys_per_wallet_different_creators_differ() {
        let env = Env::default();
        let creator_a = Address::generate(&env);
        let creator_b = Address::generate(&env);

        let key_a = constants::storage::max_keys_per_wallet(&creator_a);
        let key_b = constants::storage::max_keys_per_wallet(&creator_b);

        assert_ne!(
            key_a, key_b,
            "different creators must produce different max keys per wallet keys"
        );
    }

    #[test]
    fn test_locked_allocation_different_creators_differ() {
        let env = Env::default();
        let creator_a = Address::generate(&env);
        let creator_b = Address::generate(&env);

        let key_a = constants::storage::locked_allocation(&creator_a);
        let key_b = constants::storage::locked_allocation(&creator_b);

        assert_ne!(
            key_a, key_b,
            "different creators must produce different locked allocation keys"
        );
    }

    // --- holder_balance_key helper unit tests (#619) ---

    #[test]
    fn test_holder_balance_key_non_empty_and_valid_variant() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder = Address::generate(&env);

        let key = constants::storage::holder_balance_key(&creator, &holder);
        assert_eq!(
            key,
            crate::DataKey::KeyBalance(creator.clone(), holder.clone()),
            "holder_balance_key must produce a valid DataKey::KeyBalance variant"
        );
    }

    #[test]
    fn test_holder_balance_key_deterministic() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder = Address::generate(&env);

        let key1 = constants::storage::holder_balance_key(&creator, &holder);
        let key2 = constants::storage::holder_balance_key(&creator, &holder);

        assert_eq!(key1, key2, "same inputs must always produce equal keys");
    }

    #[test]
    fn test_holder_balance_key_different_holders_produce_equal_length_keys() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder_a = Address::generate(&env);
        let holder_b = Address::generate(&env);

        let key_a = constants::storage::holder_balance_key(&creator, &holder_a);
        let key_b = constants::storage::holder_balance_key(&creator, &holder_b);

        // Both keys are DataKey::KeyBalance(Address, Address) variants with 32-byte address payloads
        assert_ne!(key_a, key_b, "different holders must produce distinct keys");

        // Debug representation length check for structural equality
        let str_a = soroban_sdk::String::from_str(&env, &std::format!("{:?}", key_a));
        let str_b = soroban_sdk::String::from_str(&env, &std::format!("{:?}", key_b));
        assert_eq!(
            str_a.len(),
            str_b.len(),
            "keys for different holders must have equal bounds/length"
        );
    }

    #[test]
    fn test_holder_balance_key_distinguishable_from_other_storage_keys() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let holder = Address::generate(&env);

        let balance_key = constants::storage::holder_balance_key(&creator, &holder);

        let creator_profile_key = constants::storage::creator(&creator);
        let dividend_acc_key = constants::storage::dividend_accumulator(&creator);
        let fee_balance_key = constants::storage::creator_fee_balance(&creator);

        assert_ne!(
            balance_key, creator_profile_key,
            "balance key must differ from creator profile key"
        );
        assert_ne!(
            balance_key, dividend_acc_key,
            "balance key must differ from dividend accumulator key"
        );
        assert_ne!(
            balance_key, fee_balance_key,
            "balance key must differ from creator fee balance key"
        );
    }
}
