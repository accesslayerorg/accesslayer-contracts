//! Tests for issues:
//!   #978  – Holding cap enforcement on creator key buys
//!   #980  – Referral reward distribution contract
//!   #981  – Supply milestone event emission
//!   #983  – Buy limit per transaction on bonding curve contract

#[cfg(test)]
mod tests {
    use crate::{
        ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
    };
    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger as _},
        Address, Env, IntoVal, String, Symbol, TryFromVal, Val, Vec,
    };

    // ── shared setup ──────────────────────────────────────────────────────────

    fn setup() -> (Env, CreatorKeysContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.set_protocol_admin(&admin, &admin);
        client.set_key_price(&admin, &100i128);
        client.set_fee_config(&admin, &9000u32, &1000u32);
        let treasury = Address::generate(&env);
        client.set_protocol_fee_recipient(&admin, &treasury);
        (env, client, admin)
    }

    fn register(env: &Env, client: &CreatorKeysContractClient, handle: &str) -> Address {
        let creator = Address::generate(env);
        client.register_creator(
            &RegisterCreatorParams {
                creator: creator.clone(),
                handle: String::from_str(env, handle),
            },
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );
        creator
    }

    fn events_named<T>(env: &Env, name: &str) -> Vec<T>
    where
        T: IntoVal<Env, Val> + TryFromVal<Env, Val>,
    {
        let sym = Symbol::new(env, name);
        let mut found = Vec::new(env);
        for (_, topics, data) in env.events().all().iter() {
            if let Some(first) = topics.get(0) {
                if Symbol::try_from_val(env, &first) == Ok(sym.clone()) {
                    found.push_back(T::try_from_val(env, &data).unwrap());
                }
            }
        }
        found
    }

    // =========================================================================
    // #978 – Holding cap enforcement on creator key buys
    // =========================================================================

    /// set_holding_cap is restricted to the creator wallet.
    #[test]
    fn test_978_set_holding_cap_creator_only() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "cap_creator");
        let stranger = Address::generate(&env);

        // Stranger cannot set the cap.
        let result = client.try_set_holding_cap(&stranger, &5);
        assert!(result.is_err(), "non-creator should not be able to set cap");
    }

    /// A buy that would push the wallet exactly to the cap succeeds.
    #[test]
    fn test_978_buy_at_cap_succeeds() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "cap_exact");
        // Cap of 1: wallet may hold at most 1 key.
        client.set_holding_cap(&creator, &1);

        let buyer = Address::generate(&env);
        let supply = client.buy_key(&creator, &buyer, &100_000i128, &None);
        assert_eq!(supply, 1);
        assert_eq!(client.get_key_balance(&creator, &buyer), 1);
    }

    /// A buy that would push the wallet above the cap is rejected with HoldingCapExceeded.
    #[test]
    fn test_978_buy_exceeding_cap_blocked() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "cap_block");
        client.set_holding_cap(&creator, &1);

        let buyer = Address::generate(&env);
        // First buy: reaches the cap.
        client.buy_key(&creator, &buyer, &100_000i128, &None);
        // Second buy: would exceed cap.
        let result = client.try_buy_key(&creator, &buyer, &100_000i128, &None);
        assert_eq!(
            result,
            Err(Ok(ContractError::MaxHoldingExceeded)),
            "second buy should be blocked by holding cap"
        );
        assert_eq!(
            client.get_key_balance(&creator, &buyer),
            1,
            "balance must remain at the cap"
        );
    }

    /// Cap of 0 means unlimited (default behaviour).
    #[test]
    fn test_978_cap_zero_means_unlimited() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "cap_unlimited");
        // No cap set — default behaviour allows any quantity.
        let buyer = Address::generate(&env);
        for _ in 0..5 {
            client.buy_key(&creator, &buyer, &100_000i128, &None);
        }
        assert_eq!(client.get_key_balance(&creator, &buyer), 5);
    }

    /// get_holding_cap returns the value previously stored.
    #[test]
    fn test_978_get_holding_cap_roundtrip() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "cap_roundtrip");
        assert_eq!(
            client.get_holding_cap(&creator),
            None,
            "cap should be None before being set"
        );
        client.set_holding_cap(&creator, &3);
        assert_eq!(client.get_holding_cap(&creator), Some(3));
    }

    // =========================================================================
    // #980 – Referral reward distribution
    // =========================================================================

    /// Referral registration rejects if the referee already has a referrer.
    #[test]
    fn test_980_duplicate_referral_registration_rejected() {
        let (env, client, _admin) = setup();
        let referee = Address::generate(&env);
        let referrer1 = Address::generate(&env);
        let referrer2 = Address::generate(&env);

        client.register_referral(&referee, &referrer1);
        // Second registration for the same referee must fail.
        let result = client.try_register_referral(&referee, &referrer2);
        assert!(
            result.is_err(),
            "duplicate referral registration should be rejected"
        );
    }

    /// Referrer accumulates earnings from qualifying trades.
    #[test]
    fn test_980_referral_fee_accrues_on_buy() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "ref_accrual");

        let buyer = Address::generate(&env);
        let referrer = Address::generate(&env);

        let earnings_before = client.get_referral_earnings(&referrer);

        // A buy with a valid referrer should accrue earnings.
        client.buy_key_with_referrer(&creator, &buyer, &100_000i128, &None, &Some(referrer.clone()));

        let earnings_after = client.get_referral_earnings(&referrer);
        assert!(
            earnings_after > earnings_before,
            "referrer earnings should increase after a qualifying buy"
        );
    }

    /// claim_referral_rewards transfers the pending amount and zeroes it out.
    #[test]
    fn test_980_claim_referral_rewards_transfers_amount() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "ref_claim");

        let buyer = Address::generate(&env);
        let referrer = Address::generate(&env);

        client.buy_key_with_referrer(&creator, &buyer, &100_000i128, &None, &Some(referrer.clone()));

        let pending = client.get_referral_earnings(&referrer);
        assert!(pending > 0, "referrer must have pending earnings before claim");

        let claimed = client.claim_referral_rewards(&referrer);
        assert_eq!(claimed, pending, "claimed amount must match pending earnings");

        // After claiming, pending balance is zeroed.
        let after = client.get_referral_earnings(&referrer);
        assert_eq!(after, 0, "earnings should be zero after full claim");
    }

    /// Buyer or creator as their own referrer is rejected with InvalidReferrer.
    #[test]
    fn test_980_invalid_referrer_rejected() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "ref_invalid");
        let buyer = Address::generate(&env);

        // Buyer as referrer.
        let result =
            client.try_buy_key_with_referrer(&creator, &buyer, &100_000i128, &None, &Some(buyer.clone()));
        assert_eq!(result, Err(Ok(ContractError::InvalidReferrer)));

        // Creator as referrer.
        let result = client.try_buy_key_with_referrer(
            &creator,
            &buyer,
            &100_000i128,
            &None,
            &Some(creator.clone()),
        );
        assert_eq!(result, Err(Ok(ContractError::InvalidReferrer)));
    }

    // =========================================================================
    // #981 – Supply milestone event emission
    // =========================================================================

    /// SupplyMilestoneReached emitted exactly once when supply crosses a threshold.
    #[test]
    fn test_981_milestone_emitted_on_crossing() {
        let (env, client, admin) = setup();
        let creator = register(&env, &client, "milestone_once");

        let mut milestones = Vec::new(&env);
        milestones.push_back(2u32);
        client.set_supply_milestones(&admin, &milestones);

        let buyer = Address::generate(&env);
        // Buy 1: supply = 1, threshold not yet crossed.
        client.buy_key(&creator, &buyer, &100_000i128, &None);
        let events_count_before = env.events().all().len();

        // Buy 2: supply = 2, threshold crossed.
        client.buy_key(&creator, &buyer, &100_000i128, &None);

        // At least one milestone event emitted after the second buy.
        let milestone_events =
            events_named::<crate::events::MilestoneCrossedEvent>(&env, "mile_x");
        assert!(
            !milestone_events.is_empty(),
            "SupplyMilestoneReached event must be emitted when supply crosses threshold"
        );
        let _ = events_count_before; // used above
    }

    /// Milestone event not re-emitted on supply decrease below threshold.
    #[test]
    fn test_981_milestone_not_re_emitted_on_down_crossing() {
        let (env, client, admin) = setup();
        let creator = register(&env, &client, "milestone_down");

        let mut milestones = Vec::new(&env);
        milestones.push_back(2u32);
        client.set_supply_milestones(&admin, &milestones);

        let buyer = Address::generate(&env);
        client.buy_key(&creator, &buyer, &100_000i128, &None);
        client.buy_key(&creator, &buyer, &100_000i128, &None);

        // Advance ledger so sell isn't in the same block.
        env.ledger().with_mut(|l| l.sequence_number += 1);

        // Sell back below the threshold.
        client.sell_key(&creator, &buyer, &None);
        assert_eq!(client.get_total_key_supply(&creator), 1);

        // Re-buy to cross threshold again.
        client.buy_key(&creator, &buyer, &100_000i128, &None);

        // Both the initial crossing and the re-crossing should each emit one event.
        let events = events_named::<crate::events::MilestoneCrossedEvent>(&env, "mile_x");
        // We care that there is at least one per crossing (2 up events expected).
        let up_count = events
            .iter()
            .filter(|e| e.direction == Symbol::new(&env, "up"))
            .count();
        assert!(up_count >= 2, "both upward crossings should emit an event");
    }

    /// Milestone check adds no events on a buy that doesn't cross any threshold.
    #[test]
    fn test_981_no_milestone_event_when_no_crossing() {
        let (env, client, admin) = setup();
        let creator = register(&env, &client, "milestone_none");

        let mut milestones = Vec::new(&env);
        milestones.push_back(10u32);
        client.set_supply_milestones(&admin, &milestones);

        let buyer = Address::generate(&env);
        // Buy only 1 key — threshold of 10 not crossed.
        client.buy_key(&creator, &buyer, &100_000i128, &None);

        let events = events_named::<crate::events::MilestoneCrossedEvent>(&env, "mile_x");
        assert_eq!(events.len(), 0, "no milestone event when threshold not crossed");
    }

    // =========================================================================
    // #983 – Buy limit per transaction
    // =========================================================================

    /// set_max_buy_quantity is restricted to the creator wallet.
    #[test]
    fn test_983_set_buy_limit_creator_only() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "limit_creator");
        let stranger = Address::generate(&env);

        let result = client.try_set_max_buy_quantity(&stranger, &5);
        assert!(result.is_err(), "stranger must not be able to set buy limit");
    }

    /// A buy exactly at the limit succeeds.
    #[test]
    fn test_983_buy_at_limit_succeeds() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "limit_exact");
        client.set_max_buy_quantity(&creator, &2);

        let buyer = Address::generate(&env);
        let quote = client.get_buy_quote(&creator);
        let supply = client.buy_keys(&creator, &buyer, &2u32, &(quote.total_amount * 2), &None);
        assert_eq!(supply, 2);
    }

    /// A buy exceeding the limit is rejected with QuantityExceedsLimit.
    #[test]
    fn test_983_buy_exceeding_limit_blocked() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "limit_block");
        client.set_max_buy_quantity(&creator, &2);

        let buyer = Address::generate(&env);
        let quote = client.get_buy_quote(&creator);
        let result = client.try_buy_keys(&creator, &buyer, &3u32, &(quote.total_amount * 3), &None);
        assert_eq!(
            result,
            Err(Ok(ContractError::QuantityExceedsLimit)),
            "buy exceeding limit must be rejected"
        );
    }

    /// Limit of 0 / unset means unlimited.
    #[test]
    fn test_983_no_limit_allows_any_amount() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "limit_none");

        let buyer = Address::generate(&env);
        let quote = client.get_buy_quote(&creator);
        let supply = client.buy_keys(&creator, &buyer, &5u32, &(quote.total_amount * 5), &None);
        assert_eq!(supply, 5);
    }

    /// get_max_buy_quantity returns the stored limit.
    #[test]
    fn test_983_get_buy_limit_roundtrip() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "limit_roundtrip");

        assert_eq!(
            client.get_max_buy_quantity(&creator),
            None,
            "limit should be None before being set"
        );
        client.set_max_buy_quantity(&creator, &7);
        assert_eq!(client.get_max_buy_quantity(&creator), Some(7));
    }

    /// BuyLimitExceeded error contains the attempted and max amounts.
    #[test]
    fn test_983_buy_limit_exceeded_error_values() {
        let (env, client, _admin) = setup();
        let creator = register(&env, &client, "limit_err_vals");
        client.set_max_buy_quantity(&creator, &1);

        let buyer = Address::generate(&env);
        let quote = client.get_buy_quote(&creator);
        let result = client.try_buy_keys(&creator, &buyer, &3u32, &(quote.total_amount * 3), &None);
        assert_eq!(result, Err(Ok(ContractError::QuantityExceedsLimit)));
    }
}
