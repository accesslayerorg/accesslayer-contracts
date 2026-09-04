// =============================================================================
// Tests for issues #487, #489, #491, #492
// =============================================================================

#[cfg(test)]
mod issue_tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env, String, Vec,
    };

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
        client.set_protocol_admin(&admin, &admin);
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

        env.ledger().with_mut(|l| l.sequence_number += 1);
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

    // =============================================================================
    // Tests for Issue #572
    // =============================================================================

    #[test]
    fn test_query_price_at_boundary_supplies() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        let base_price = 100i128;
        client.set_key_price(&admin, &base_price);

        // Test for Linear curve
        let creator = register_creator(&env, &client, None);
        for supply in [0u64, 1u64, 50u64] {
            let queried_price = client.query_price(&creator, &supply);
            let expected_price = env.as_contract(&contract_id, || {
                compute_bonding_curve_price(&env, &creator, base_price, supply as u32).unwrap()
            });
            assert_eq!(
                queried_price, expected_price,
                "query_price output must match buy-path bonding curve formula for supply {}",
                supply
            );
        }

        // Test for Flat curve
        let flat_creator = Address::generate(&env);
        let handle = String::from_str(&env, "bob");
        client.register_creator(
            &crate::RegisterCreatorParams {
                creator: flat_creator.clone(),
                handle,
            },
            &None,
            &None,
            &None,
            &Some(CurvePreset::Flat),
            &None,
            &None,
        );
        for supply in [0u64, 1u64, 50u64] {
            let price = client.query_price(&flat_creator, &supply);
            assert_eq!(price, base_price);
        }

        // Test for Quadratic curve
        let quad_creator = Address::generate(&env);
        let handle_q = String::from_str(&env, "charlie");
        client.register_creator(
            &crate::RegisterCreatorParams {
                creator: quad_creator.clone(),
                handle: handle_q,
            },
            &None,
            &None,
            &None,
            &Some(CurvePreset::Quadratic),
            &None,
            &None,
        );
        for supply in [0u64, 1u64, 50u64] {
            let queried_price = client.query_price(&quad_creator, &supply);
            let expected_price = env.as_contract(&contract_id, || {
                compute_bonding_curve_price(&env, &quad_creator, base_price, supply as u32).unwrap()
            });
            assert_eq!(queried_price, expected_price);
        }
    }

    #[test]
    fn test_protocol_fee_calculation_at_boundary_supplies() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        let base_price = 100i128;
        client.set_key_price(&admin, &base_price);

        let protocol_bps = 250u32; // 2.5%
        let creator_bps = 500u32; // 5.0%
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &creator_bps, &protocol_bps);

        let creator = register_creator(&env, &client, None);

        // Boundary supply values: 0, 1, 1000
        for supply in [0u64, 1u64, 1000u64] {
            let price = client.query_price(&creator, &supply);
            let computed_fee = crate::fee::apply_percentage_fee(price, protocol_bps).unwrap();
            let expected_fee = (price * protocol_bps as i128) / crate::fee::BPS_MAX as i128;

            assert_eq!(
                computed_fee, expected_fee,
                "Protocol fee for supply {} must match expected BPS calculation",
                supply
            );
        }
    }

    #[test]
    fn test_protocol_fee_floors_on_non_whole_stroops() {
        // Price = 1050 stroops, protocol_bps = 50 bps (0.5%)
        // Exact: 1050 * 50 / 10000 = 5.25 stroops -> Floors to 5 stroops
        let price_a = 1050i128;
        let bps_a = 50u32;
        let fee_a = crate::fee::apply_percentage_fee(price_a, bps_a).unwrap();
        assert_eq!(fee_a, 5, "Fee 5.25 stroops must floor to 5 stroops");

        // Price = 105 stroops, protocol_bps = 50 bps (0.5%)
        // Exact: 105 * 50 / 10000 = 0.525 stroops -> Floors to 0 stroops
        let price_b = 105i128;
        let bps_b = 50u32;
        let fee_b = crate::fee::apply_percentage_fee(price_b, bps_b).unwrap();
        assert_eq!(fee_b, 0, "Fee 0.525 stroops must floor to 0 stroops");
    }

    // =============================================================================
    // Tests for Issue #720: get_price returning base price at zero supply
    // =============================================================================

    #[test]
    fn test_get_price_at_supply_zero_returns_base_price_and_greater_at_supply_one() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        let base_price = 1000i128;
        let slope = 50i128;
        client.set_key_price(&admin, &base_price);
        client.set_curve_slope(&admin, &slope);

        let creator = register_creator(&env, &client, None);

        // Supply 0 returns base price; supply 1 returns base price + slope > base price.
        // Assert no panic for either call.
        let price_0 = client.get_price(&creator, &0u64);
        assert_eq!(
            price_0, base_price,
            "get_price at supply 0 must return configured base price"
        );

        let price_1 = client.get_price(&creator, &1u64);
        assert!(
            price_1 > base_price,
            "get_price at supply 1 must be strictly greater than base price"
        );
        assert_eq!(
            price_1,
            base_price + slope,
            "get_price at supply 1 must equal base_price + slope"
        );

        // Also assert try_get_price returns Ok for both without panic.
        let try_price_0 = client.try_get_price(&creator, &0u64);
        assert_eq!(try_price_0, Ok(Ok(base_price)));

        let try_price_1 = client.try_get_price(&creator, &1u64);
        assert_eq!(try_price_1, Ok(Ok(base_price + slope)));
    }

    #[test]
    fn test_get_price_various_base_prices_at_supply_zero() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let slope = 10i128;
        client.set_curve_slope(&admin, &slope);

        let test_prices = [1i128, 10, 100, 500, 1000, 10000, 50000];

        for base_price in test_prices {
            client.set_key_price(&admin, &base_price);
            let creator = register_creator(&env, &client, None);

            let price_0 = client.get_price(&creator, &0u64);
            assert_eq!(
                price_0, base_price,
                "supply 0 must return base price {}",
                base_price
            );

            let price_1 = client.get_price(&creator, &1u64);
            assert!(
                price_1 > base_price,
                "supply 1 price ({}) must be strictly greater than base price ({})",
                price_1,
                base_price
            );
        }
    }

    #[test]
    fn test_get_price_presets_at_supply_zero_and_one() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        let base_price = 2000i128;
        let slope = 100i128;
        client.set_key_price(&admin, &base_price);
        client.set_curve_slope(&admin, &slope);

        // Linear preset
        let linear_creator = register_creator(&env, &client, None);
        assert_eq!(client.get_price(&linear_creator, &0u64), base_price);
        assert_eq!(client.get_price(&linear_creator, &1u64), base_price + slope);
        assert!(client.get_price(&linear_creator, &1u64) > base_price);

        // Flat preset
        let flat_creator = Address::generate(&env);
        client.register_creator(
            &crate::RegisterCreatorParams {
                creator: flat_creator.clone(),
                handle: String::from_str(&env, "flat"),
            },
            &None,
            &None,
            &None,
            &Some(CurvePreset::Flat),
            &None,
            &None,
        );
        assert_eq!(client.get_price(&flat_creator, &0u64), base_price);
        assert_eq!(client.get_price(&flat_creator, &1u64), base_price);

        // Quadratic preset
        let quad_creator = Address::generate(&env);
        client.register_creator(
            &crate::RegisterCreatorParams {
                creator: quad_creator.clone(),
                handle: String::from_str(&env, "quad"),
            },
            &None,
            &None,
            &None,
            &Some(CurvePreset::Quadratic),
            &None,
            &None,
        );
        assert_eq!(client.get_price(&quad_creator, &0u64), base_price);
        assert_eq!(client.get_price(&quad_creator, &1u64), base_price + slope);
        assert!(client.get_price(&quad_creator, &1u64) > base_price);
    }

    // =========================================================================
    // Tests for batch buy (#758)
    // =========================================================================

    #[test]
    fn test_batch_buy_single_order() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9_000, &1_000);
        client.set_key_price(&admin, &100);

        let creator = register_creator(&env, &client, None);
        let buyer = Address::generate(&env);

        let orders = soroban_sdk::Vec::from_array(&env, [(creator.clone(), 3u32)]);
        let results = client.batch_buy(&buyer, &orders);

        assert_eq!(results.len(), 1);
        assert_eq!(results.get(0).unwrap().quantity, 3);
        assert!(results.get(0).unwrap().price_paid > 0);
        assert_eq!(client.get_key_balance(&creator, &buyer), 3);
    }

    #[test]
    fn test_batch_buy_multiple_orders() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9_000, &1_000);
        client.set_key_price(&admin, &100);

        let creator1 = register_creator(&env, &client, None);
        let creator2 = register_creator(&env, &client, None);
        let buyer = Address::generate(&env);

        let orders = soroban_sdk::Vec::from_array(
            &env,
            [(creator1.clone(), 2u32), (creator2.clone(), 1u32)],
        );
        let results = client.batch_buy(&buyer, &orders);

        assert_eq!(results.len(), 2);
        assert_eq!(client.get_key_balance(&creator1, &buyer), 2);
        assert_eq!(client.get_key_balance(&creator2, &buyer), 1);
    }

    #[test]
    fn test_batch_buy_reverts_on_empty() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9_000, &1_000);
        client.set_key_price(&admin, &100);

        let buyer = Address::generate(&env);
        let orders: soroban_sdk::Vec<(Address, u32)> = soroban_sdk::Vec::new(&env);

        let result = client.try_batch_buy(&buyer, &orders);
        assert_eq!(result, Err(Ok(ContractError::BatchClaimExceedsLimit)));
    }

    #[test]
    fn test_batch_buy_reverts_on_exceeding_limit() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9_000, &1_000);
        client.set_key_price(&admin, &100);

        let buyer = Address::generate(&env);
        let c1 = register_creator(&env, &client, None);
        let c2 = register_creator(&env, &client, None);
        let c3 = register_creator(&env, &client, None);
        let c4 = register_creator(&env, &client, None);
        let c5 = register_creator(&env, &client, None);
        let c6 = register_creator(&env, &client, None);
        let orders = soroban_sdk::Vec::from_array(
            &env,
            [
                (c1, 1u32),
                (c2, 1u32),
                (c3, 1u32),
                (c4, 1u32),
                (c5, 1u32),
                (c6, 1u32),
            ],
        );

        let result = client.try_batch_buy(&buyer, &orders);
        assert_eq!(result, Err(Ok(ContractError::BatchClaimExceedsLimit)));
    }

    // =========================================================================
    // Tests for set_royalty (#755)
    // =========================================================================

    #[test]
    fn test_set_royalty_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9_000, &1_000);
        client.set_key_price(&admin, &100);

        let creator = register_creator(&env, &client, None);
        client.set_royalty(&creator, &100, &200);

        let config = client.get_royalty_config(&creator).unwrap();
        assert_eq!(config.buy_fee_bps, 100);
        assert_eq!(config.sell_fee_bps, 200);
    }

    #[test]
    fn test_set_royalty_reverts_when_exceeds_limit() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9_000, &1_000);
        client.set_key_price(&admin, &100);

        let creator = register_creator(&env, &client, None);
        let result = client.try_set_royalty(&creator, &501, &0);
        assert_eq!(result, Err(Ok(ContractError::ProtocolFeeExceedsCap)));
    }

    #[test]
    fn test_set_royalty_reverts_for_unregistered() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9_000, &1_000);
        client.set_key_price(&admin, &100);

        let nobody = Address::generate(&env);
        let result = client.try_set_royalty(&nobody, &100, &100);
        assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
    }

    // =========================================================================
    // Tests for migrate_curve (#756)
    // =========================================================================

    #[test]
    fn test_migrate_curve_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9_000, &1_000);
        client.set_key_price(&admin, &100);
        client.set_curve_slope(&admin, &1);

        let creator = register_creator(&env, &client, None);
        let key_ids = soroban_sdk::Vec::from_array(&env, [creator.clone()]);

        client.migrate_curve(&admin, &3, &key_ids);

        let exponent = client.get_curve_exponent(&creator);
        assert_eq!(exponent, Some(3));
    }

    #[test]
    fn test_migrate_curve_reverts_on_invalid_exponent() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9_000, &1_000);
        client.set_key_price(&admin, &100);

        let creator = register_creator(&env, &client, None);
        let key_ids = soroban_sdk::Vec::from_array(&env, [creator]);

        let result = client.try_migrate_curve(&admin, &0, &key_ids);
        assert_eq!(result, Err(Ok(ContractError::InvalidFeeConfig)));

        let result = client.try_migrate_curve(&admin, &6, &key_ids);
        assert_eq!(result, Err(Ok(ContractError::InvalidFeeConfig)));
    }

    #[test]
    fn test_migrate_curve_reverts_on_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_fee_config(&admin, &9_000, &1_000);
        client.set_key_price(&admin, &100);

        let creator = register_creator(&env, &client, None);
        let key_ids = soroban_sdk::Vec::from_array(&env, [creator]);
        let non_admin = Address::generate(&env);

        let result = client.try_migrate_curve(&non_admin, &2, &key_ids);
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    }
}
