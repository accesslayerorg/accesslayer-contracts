// =============================================================================
// Tests for issues #487, #489, #491, #492
//
// These tests live alongside the existing test suite in
//   creator-keys/src/test.rs
//
// To integrate:
//   1. Append the contents of this file into creator-keys/src/test.rs
//      (or include it via `mod` from lib.rs if you prefer a separate module).
//   2. Make sure the imports at the top of test.rs already pull in everything
//      listed in the `use` block below; add any that are missing.
// =============================================================================

#[cfg(test)]
mod issue_tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _},
        token, Address, Env, Vec,
    };

    // Re-export the contract client and error type the same way the existing
    // tests do.  Adjust the path if the crate structure differs.
    use crate::{CreatorKeysClient, ContractError};

    // -------------------------------------------------------------------------
    // Shared test helper – shared across all tests in this module
    // -------------------------------------------------------------------------

    /// Register a creator with the given supply cap (pass `None` for no cap).
    /// Returns the creator id used in subsequent calls.
    fn register_creator(env: &Env, client: &CreatorKeysClient, cap: Option<u32>) -> Address {
        let creator = Address::generate(env);
        match cap {
            Some(c) => client.register_creator_with_cap(&creator, &c),
            None => client.register_creator(&creator),
        };
        creator
    }

    /// Fund a wallet with enough XLM so that buy operations can succeed.
    /// This mints native XLM to `wallet` using the Stellar Asset Contract.
    fn fund_wallet(env: &Env, wallet: &Address, amount: i128) {
        let xlm = token::StellarAssetClient::new(env, &env.current_contract_address());
        xlm.mint(wallet, &amount);
    }

    // =========================================================================
    // Issue #492 – Helper: assert total supply equals sum of all holder balances
    // =========================================================================

    /// Assert that a creator's `total_supply` equals the sum of every holder's
    /// individual balance.  Panics with a descriptive message if they differ.
    ///
    /// # Arguments
    /// * `env`        – The test environment.
    /// * `client`     – Contract client.
    /// * `creator_id` – The creator whose supply to verify.
    /// * `holders`    – Every address that holds (or may hold) keys for this creator.
    fn assert_supply_equals_holder_sum(
        env: &Env,
        client: &CreatorKeysClient,
        creator_id: &Address,
        holders: Vec<Address>,
    ) {
        let total_supply: u32 = client.get_total_supply(creator_id);

        let mut computed_sum: u32 = 0u32;
        for holder in holders.iter() {
            let balance: u32 = client.get_balance(creator_id, &holder);
            computed_sum = computed_sum
                .checked_add(balance)
                .expect("holder balance sum overflowed u32");
        }

        assert_eq!(
            total_supply,
            computed_sum,
            "Supply invariant violated for creator {creator_id:?}: \
             total_supply={total_supply} but sum of holder balances={computed_sum}"
        );
    }

    // =========================================================================
    // Issue #487 – distribute_dividend reverts when creator has zero total supply
    // =========================================================================

    /// A creator is registered but nobody buys any keys.
    /// `distribute_dividend` must revert with a descriptive error, and the
    /// caller's XLM balance must be unchanged after the failed call.
    #[test]
    fn test_distribute_dividend_reverts_on_zero_supply() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, crate::CreatorKeys);
        let client = CreatorKeysClient::new(&env, &contract_id);

        // Register a creator but do NOT buy any keys → total supply stays 0.
        let creator = register_creator(&env, &client, None);

        // Prepare a caller with an XLM balance.
        let caller = Address::generate(&env);
        let initial_xlm: i128 = 10_000_000; // 1 XLM in stroops
        fund_wallet(&env, &caller, initial_xlm);

        // Attempting to distribute a dividend must revert.
        let dividend_amount: i128 = 5_000_000; // 0.5 XLM
        let result = client.try_distribute_dividend(&creator, &caller, &dividend_amount);

        assert!(
            result.is_err(),
            "distribute_dividend should revert when total supply is zero, but it succeeded"
        );

        // The error variant must be something descriptive and distinct – not a
        // generic panic.  We check that it maps to a known ContractError.
        let err = result.unwrap_err().unwrap();
        assert!(
            matches!(err, ContractError::ZeroTotalSupply),
            "Expected ContractError::ZeroTotalSupply, got {err:?}"
        );

        // Caller's XLM balance must be unchanged (no XLM was transferred).
        let xlm_client = token::Client::new(
            &env,
            &env.invoke_contract(
                &contract_id,
                &soroban_sdk::symbol_short!("xlm_addr"),
                soroban_sdk::vec![&env],
            ),
        );
        let balance_after = xlm_client.balance(&caller);
        assert_eq!(
            balance_after, initial_xlm,
            "Caller XLM balance should be unchanged after a reverted distribute_dividend, \
             expected {initial_xlm} but got {balance_after}"
        );
    }

    // =========================================================================
    // Issue #489 – Multi-holder dividend split with one majority holder
    // =========================================================================

    /// Wallet A holds 90 keys, wallet B holds 10 keys (total supply = 100).
    /// Distributing 1 000 stroops (after any protocol fee subtraction has
    /// already been applied by the contract) must give:
    ///   • Wallet A → 900 stroops  (90 %)
    ///   • Wallet B → 100 stroops  (10 %)
    ///
    /// The shares must sum to the net distributed amount and neither holder
    /// may receive more than their proportional entitlement.
    #[test]
    fn test_multi_holder_dividend_majority_holder_receives_larger_share() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, crate::CreatorKeys);
        let client = CreatorKeysClient::new(&env, &contract_id);

        let creator = register_creator(&env, &client, None);

        let wallet_a = Address::generate(&env);
        let wallet_b = Address::generate(&env);

        // Mint enough XLM for both wallets to buy keys.
        fund_wallet(&env, &wallet_a, 100_000_000);
        fund_wallet(&env, &wallet_b, 100_000_000);

        // Wallet A buys 90 keys; wallet B buys 10 keys.
        client.buy_keys(&creator, &wallet_a, &90u32);
        client.buy_keys(&creator, &wallet_b, &10u32);

        // Verify total supply before distributing.
        assert_eq!(client.get_total_supply(&creator), 100u32);

        // Use the #492 helper to confirm supply invariant holds after the buys.
        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, wallet_a.clone(), wallet_b.clone()],
        );

        // Fund a distributor and call distribute_dividend with 1 000 stroops.
        let distributor = Address::generate(&env);
        let gross_amount: i128 = 1_000;
        fund_wallet(&env, &distributor, gross_amount + 1_000_000 /* gas */);
        client.distribute_dividend(&creator, &distributor, &gross_amount);

        // Query the claimable amounts for each holder.
        let claimable_a: i128 = client.get_claimable_dividend(&creator, &wallet_a);
        let claimable_b: i128 = client.get_claimable_dividend(&creator, &wallet_b);

        // --- Acceptance criteria ---

        // 1. Majority holder receives proportionally larger share.
        assert!(
            claimable_a > claimable_b,
            "Wallet A (90 keys) should receive more than wallet B (10 keys), \
             but got claimable_a={claimable_a}, claimable_b={claimable_b}"
        );

        // 2. Exact proportional amounts.
        assert_eq!(
            claimable_a, 900,
            "Wallet A should receive 900 stroops (90% of 1000), got {claimable_a}"
        );
        assert_eq!(
            claimable_b, 100,
            "Wallet B should receive 100 stroops (10% of 1000), got {claimable_b}"
        );

        // 3. Shares sum to the net distributed amount (1 000 stroops).
        let total_claimable = claimable_a + claimable_b;
        assert_eq!(
            total_claimable, gross_amount,
            "Claimable amounts ({total_claimable}) should sum to distributed amount ({gross_amount})"
        );

        // 4. Neither holder receives more than their proportion.
        assert!(
            claimable_a <= gross_amount * 90 / 100 + 1, // +1 for rounding tolerance
            "Wallet A received more than its 90% proportion"
        );
        assert!(
            claimable_b <= gross_amount * 10 / 100 + 1,
            "Wallet B received more than its 10% proportion"
        );
    }

    // =========================================================================
    // Issue #491 – Supply cap blocks buy that would partially exceed the cap
    // =========================================================================

    /// With a cap of 10 and current supply at 8, a buy of 3 must revert
    /// even though 2 of those keys are within the cap.  A buy of exactly 2
    /// (filling to the cap) must succeed.
    #[test]
    fn test_supply_cap_rejects_partial_exceed() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, crate::CreatorKeys);
        let client = CreatorKeysClient::new(&env, &contract_id);

        // Register creator with supply cap = 10.
        let creator = register_creator(&env, &client, Some(10u32));

        let buyer = Address::generate(&env);
        fund_wallet(&env, &buyer, 100_000_000);

        // Buy 8 keys to bring supply to 8.
        client.buy_keys(&creator, &buyer, &8u32);
        assert_eq!(
            client.get_total_supply(&creator),
            8u32,
            "Total supply should be 8 after buying 8 keys"
        );

        // Use the #492 helper.
        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone()],
        );

        // Attempt to buy 3 more keys (would bring supply to 11 > cap 10).
        let result = client.try_buy_keys(&creator, &buyer, &3u32);
        assert!(
            result.is_err(),
            "Buying 3 keys when only 2 slots remain should revert, but it succeeded"
        );

        // Error must be SupplyCapExceeded.
        let err = result.unwrap_err().unwrap();
        assert!(
            matches!(err, ContractError::SupplyCapExceeded),
            "Expected ContractError::SupplyCapExceeded, got {err:?}"
        );

        // Total supply must still be 8 after the failed buy.
        assert_eq!(
            client.get_total_supply(&creator),
            8u32,
            "Total supply should remain at 8 after a reverted buy"
        );

        // Supply invariant must still hold.
        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone()],
        );

        // A buy of exactly 2 (filling to the cap) must succeed.
        let buyer2 = Address::generate(&env);
        fund_wallet(&env, &buyer2, 100_000_000);
        client.buy_keys(&creator, &buyer2, &2u32);
        assert_eq!(
            client.get_total_supply(&creator),
            10u32,
            "Total supply should be 10 after filling to the cap"
        );

        // Final supply invariant check with both holders.
        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone(), buyer2.clone()],
        );
    }

    // =========================================================================
    // Issue #492 – assert_supply_equals_holder_sum used in existing buy/sell/
    //              transfer flows (at least three existing-style test cases)
    // =========================================================================

    /// After buying keys the supply invariant must hold.
    #[test]
    fn test_invariant_after_buy() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, crate::CreatorKeys);
        let client = CreatorKeysClient::new(&env, &contract_id);

        let creator = register_creator(&env, &client, None);
        let buyer = Address::generate(&env);
        fund_wallet(&env, &buyer, 100_000_000);

        client.buy_keys(&creator, &buyer, &5u32);

        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone()],
        );
    }

    /// After selling keys the supply invariant must hold.
    #[test]
    fn test_invariant_after_sell() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, crate::CreatorKeys);
        let client = CreatorKeysClient::new(&env, &contract_id);

        let creator = register_creator(&env, &client, None);
        let buyer = Address::generate(&env);
        fund_wallet(&env, &buyer, 100_000_000);

        client.buy_keys(&creator, &buyer, &10u32);

        // Verify invariant before sell.
        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone()],
        );

        client.sell_keys(&creator, &buyer, &4u32);

        // Verify invariant after sell.
        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, buyer.clone()],
        );

        assert_eq!(
            client.get_total_supply(&creator),
            6u32,
            "Total supply should be 6 after selling 4 of 10 keys"
        );
    }

    /// After transferring keys the supply invariant must hold.
    #[test]
    fn test_invariant_after_transfer() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, crate::CreatorKeys);
        let client = CreatorKeysClient::new(&env, &contract_id);

        let creator = register_creator(&env, &client, None);
        let sender = Address::generate(&env);
        let receiver = Address::generate(&env);
        fund_wallet(&env, &sender, 100_000_000);

        client.buy_keys(&creator, &sender, &8u32);

        // Verify invariant before transfer.
        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, sender.clone(), receiver.clone()],
        );

        client.transfer_keys(&creator, &sender, &receiver, &3u32);

        // Verify invariant after transfer – total supply must not change,
        // but balances must have shifted.
        assert_supply_equals_holder_sum(
            &env,
            &client,
            &creator,
            soroban_sdk::vec![&env, sender.clone(), receiver.clone()],
        );

        assert_eq!(client.get_balance(&creator, &sender), 5u32);
        assert_eq!(client.get_balance(&creator, &receiver), 3u32);
        assert_eq!(client.get_total_supply(&creator), 8u32);
    }
}
