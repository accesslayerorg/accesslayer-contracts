// =============================================================================
// Tests for issues #487, #489, #491, #492
// =============================================================================

#[cfg(test)]
mod issue_tests {
    use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

    use crate::{ContractError, CreatorKeysContract, CreatorKeysContractClient};

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
                client.register_creator(&creator, &handle, &None, &Some(c), &None, &None);
            }
            None => {
                client.register_creator(&creator, &handle, &None, &None, &None, &None);
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
            let quote = client.get_buy_quote(&creator);
            client.buy_key(&creator, &wallet_a, &quote.total_amount, &None);
        }
        for _ in 0..10 {
            let quote = client.get_buy_quote(&creator);
            client.buy_key(&creator, &wallet_b, &quote.total_amount, &None);
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
            let quote = client.get_buy_quote(&creator);
            client.buy_key(&creator, &buyer, &quote.total_amount, &None);
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
            let quote = client.get_buy_quote(&creator);
            client.buy_key(&creator, &buyer, &quote.total_amount, &None);
        }
        assert_eq!(
            client.get_total_key_supply(&creator),
            10u32,
            "Total supply should be 10 after filling to the cap"
        );

        let quote = client.get_buy_quote(&creator);
        let result = client.try_buy_key(&creator, &buyer, &quote.total_amount, &None);
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

        let quote = client.get_buy_quote(&creator);
        client.buy_key(&creator, &buyer, &quote.total_amount, &None);

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
            let quote = client.get_buy_quote(&creator);
            client.buy_key(&creator, &buyer, &quote.total_amount, &None);
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
            let quote = client.get_buy_quote(&creator);
            client.buy_key(&creator, &sender, &quote.total_amount, &None);
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
}
