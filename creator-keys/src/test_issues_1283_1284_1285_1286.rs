//! Comprehensive tests for issues #1283, #1284, #1285, and #1286.
//!
//! These tests verify the implementation of:
//! - Issue #1283: Creator allocation with vesting
//! - Issue #1284: English auction for secondary key sales
//! - Issue #1285: Holder enumeration views
//! - Issue #1286: Badge-gated access control

#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _},
        Address, Env, Vec,
    };

    use crate::{badge_access_control, creator_allocation, english_auction, holder_enumeration};

    // ============================================================================
    // Issue #1283: Creator allocation with vesting
    // ============================================================================

    #[test]
    fn test_creator_allocation_basic() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let key_id = Address::generate(&env);

        let amount = 1000;
        let vesting_ledgers = 1000;
        let cliff_ledgers = 100;

        let result = env.as_contract(&contract_id, || {
            creator_allocation::set_creator_allocation(
                &env,
                creator.clone(),
                key_id.clone(),
                amount,
                vesting_ledgers,
                cliff_ledgers,
            )
        });

        assert!(result.is_ok());

        // Check vesting status
        let status = env
            .as_contract(&contract_id, || {
                creator_allocation::vesting_status(&env, key_id)
            })
            .unwrap();
        assert_eq!(status.total_amount, amount);
        assert_eq!(status.vested_amount, 0); // Before cliff
        assert_eq!(status.claimed_amount, 0);
        assert_eq!(status.claimable_amount, 0);
    }

    #[test]
    fn test_creator_allocation_cliff() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_sequence_number(0);
        let contract_id = env.register(crate::CreatorKeysContract, ());

        let creator = Address::generate(&env);
        let key_id = Address::generate(&env);

        let amount = 1000;
        let vesting_ledgers = 1000;
        let cliff_ledgers = 100;

        env.as_contract(&contract_id, || {
            creator_allocation::set_creator_allocation(
                &env,
                creator.clone(),
                key_id.clone(),
                amount,
                vesting_ledgers,
                cliff_ledgers,
            )
            .unwrap();
        });

        // Before cliff - nothing vested
        let status = env
            .as_contract(&contract_id, || {
                creator_allocation::vesting_status(&env, key_id.clone())
            })
            .unwrap();
        assert_eq!(status.vested_amount, 0);

        // After cliff but before full vesting - partial vesting
        env.ledger().set_sequence_number(600);
        let status = env
            .as_contract(&contract_id, || {
                creator_allocation::vesting_status(&env, key_id.clone())
            })
            .unwrap();
        assert!(status.vested_amount > 0);
        assert!(status.vested_amount < amount);

        // After full vesting - everything vested
        env.ledger().set_sequence_number(1000);
        let status = env
            .as_contract(&contract_id, || {
                creator_allocation::vesting_status(&env, key_id)
            })
            .unwrap();
        assert_eq!(status.vested_amount, amount);
    }

    #[test]
    fn test_creator_allocation_claim() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_sequence_number(0);
        let contract_id = env.register(crate::CreatorKeysContract, ());

        let creator = Address::generate(&env);
        let key_id = Address::generate(&env);

        let amount = 1000;
        let vesting_ledgers = 1000;
        let cliff_ledgers = 100;

        env.as_contract(&contract_id, || {
            creator_allocation::set_creator_allocation(
                &env,
                creator.clone(),
                key_id.clone(),
                amount,
                vesting_ledgers,
                cliff_ledgers,
            )
            .unwrap();
        });

        // Try to claim before cliff - should fail
        let result = env.as_contract(&contract_id, || {
            creator_allocation::claim_vested(&env, creator.clone(), key_id.clone())
        });
        assert!(result.is_err());

        // After cliff, claim partial
        env.ledger().set_sequence_number(550);
        let claimed = env
            .as_contract(&contract_id, || {
                creator_allocation::claim_vested(&env, creator.clone(), key_id.clone())
            })
            .unwrap();
        assert!(claimed > 0);
        assert!(claimed < amount);

        // Check status after claim
        let status = env
            .as_contract(&contract_id, || {
                creator_allocation::vesting_status(&env, key_id.clone())
            })
            .unwrap();
        assert_eq!(status.claimed_amount, claimed);
    }

    #[test]
    fn test_creator_allocation_extend_vesting() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let admin = Address::generate(&env);
        let key_id = Address::generate(&env);

        let amount = 1000;
        let vesting_ledgers = 1000;
        let cliff_ledgers = 100;

        env.as_contract(&contract_id, || {
            creator_allocation::set_creator_allocation(
                &env,
                creator.clone(),
                key_id.clone(),
                amount,
                vesting_ledgers,
                cliff_ledgers,
            )
            .unwrap();
        });

        // Extend vesting
        let additional_ledgers = 500;
        let result = env.as_contract(&contract_id, || {
            creator_allocation::extend_vesting(&env, admin, key_id.clone(), additional_ledgers)
        });
        assert!(result.is_ok());

        // Verify extension
        let status = env
            .as_contract(&contract_id, || {
                creator_allocation::vesting_status(&env, key_id)
            })
            .unwrap();
        assert_eq!(status.vesting_ledgers, vesting_ledgers + additional_ledgers);
    }

    // ============================================================================
    // Issue #1284: English auction
    // ============================================================================

    #[test]
    fn test_english_auction_start() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let seller = Address::generate(&env);
        let key_id = Address::generate(&env);

        let quantity = 100;
        let reserve_price = 1000;
        let duration = 1000;
        let min_increment_bps = 500; // 5%

        let auction_id = env
            .as_contract(&contract_id, || {
                english_auction::start_english_auction(
                    &env,
                    seller.clone(),
                    key_id.clone(),
                    quantity,
                    reserve_price,
                    duration,
                    min_increment_bps,
                )
            })
            .unwrap();

        assert_eq!(auction_id, 1);

        let auction = env
            .as_contract(&contract_id, || {
                english_auction::get_auction(&env, auction_id)
            })
            .unwrap();
        assert_eq!(auction.seller, seller);
        assert_eq!(auction.quantity, quantity);
        assert_eq!(auction.reserve_price, reserve_price);
        assert!(!auction.finalized);
    }

    #[test]
    fn test_english_auction_bidding() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_sequence_number(0);
        let contract_id = env.register(crate::CreatorKeysContract, ());

        let seller = Address::generate(&env);
        let key_id = Address::generate(&env);
        let bidder1 = Address::generate(&env);
        let bidder2 = Address::generate(&env);

        let reserve_price = 1000;
        let min_increment_bps = 500; // 5%

        let auction_id = env
            .as_contract(&contract_id, || {
                english_auction::start_english_auction(
                    &env,
                    seller.clone(),
                    key_id.clone(),
                    100,
                    reserve_price,
                    1000,
                    min_increment_bps,
                )
            })
            .unwrap();

        // First bid at reserve price
        let result = env.as_contract(&contract_id, || {
            english_auction::place_bid(&env, bidder1.clone(), auction_id, reserve_price)
        });
        assert!(result.is_ok());

        // Second bid below minimum increment - should fail
        let result = env.as_contract(&contract_id, || {
            english_auction::place_bid(&env, bidder2.clone(), auction_id, reserve_price + 10)
        });
        assert!(result.is_err());

        // Second bid at proper increment
        let min_bid = reserve_price + (reserve_price * 500 / 10000);
        let result = env.as_contract(&contract_id, || {
            english_auction::place_bid(&env, bidder2.clone(), auction_id, min_bid)
        });
        assert!(result.is_ok());

        let auction = env
            .as_contract(&contract_id, || {
                english_auction::get_auction(&env, auction_id)
            })
            .unwrap();
        assert_eq!(auction.highest_bid, min_bid);
        assert_eq!(auction.highest_bidder.unwrap(), bidder2);
    }

    #[test]
    fn test_english_auction_extension() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_sequence_number(0);
        let contract_id = env.register(crate::CreatorKeysContract, ());

        let seller = Address::generate(&env);
        let key_id = Address::generate(&env);
        let bidder = Address::generate(&env);

        let duration = 200;
        let auction_id = env
            .as_contract(&contract_id, || {
                english_auction::start_english_auction(
                    &env, seller, key_id, 100, 1000, duration, 500,
                )
            })
            .unwrap();

        // Bid near the end (within extension window)
        env.ledger().set_sequence_number(150);
        env.as_contract(&contract_id, || {
            english_auction::place_bid(&env, bidder, auction_id, 1000).unwrap();
        });

        let auction = env
            .as_contract(&contract_id, || {
                english_auction::get_auction(&env, auction_id)
            })
            .unwrap();
        assert!(auction.current_end_ledger > auction.original_end_ledger);
        assert_eq!(auction.total_extensions, 50);
    }

    #[test]
    fn test_english_auction_finalize_success() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_sequence_number(0);
        let contract_id = env.register(crate::CreatorKeysContract, ());

        let seller = Address::generate(&env);
        let key_id = Address::generate(&env);
        let bidder = Address::generate(&env);

        let auction_id = env
            .as_contract(&contract_id, || {
                english_auction::start_english_auction(&env, seller, key_id, 100, 1000, 100, 500)
            })
            .unwrap();

        // Place bid
        env.as_contract(&contract_id, || {
            english_auction::place_bid(&env, bidder, auction_id, 1000).unwrap();
        });

        // Try to finalize before end - should fail
        let result = env.as_contract(&contract_id, || {
            english_auction::finalize_auction(&env, auction_id)
        });
        assert!(result.is_err());

        // Finalize after end (bid at ledger 0 triggers anti-snipe extension, so end > 100)
        env.ledger().set_sequence_number(200);
        let result = env.as_contract(&contract_id, || {
            english_auction::finalize_auction(&env, auction_id)
        });
        assert!(result.is_ok());

        let auction = env
            .as_contract(&contract_id, || {
                english_auction::get_auction(&env, auction_id)
            })
            .unwrap();
        assert!(auction.finalized);
    }

    #[test]
    fn test_english_auction_cancel() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let seller = Address::generate(&env);
        let key_id = Address::generate(&env);

        let auction_id = env
            .as_contract(&contract_id, || {
                english_auction::start_english_auction(
                    &env,
                    seller.clone(),
                    key_id,
                    100,
                    1000,
                    1000,
                    500,
                )
            })
            .unwrap();

        // Cancel before any bids
        let result = env.as_contract(&contract_id, || {
            english_auction::cancel_auction(&env, seller, auction_id)
        });
        assert!(result.is_ok());

        // Auction should be removed
        let auction = env.as_contract(&contract_id, || {
            english_auction::get_auction(&env, auction_id)
        });
        assert!(auction.is_none());
    }

    // ============================================================================
    // Issue #1285: Holder enumeration
    // ============================================================================

    #[test]
    fn test_holder_enumeration_add_remove() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let key_id = Address::generate(&env);
        let holder1 = Address::generate(&env);
        let holder2 = Address::generate(&env);

        // Initially no holders
        let count = env.as_contract(&contract_id, || {
            holder_enumeration::holder_count(&env, key_id.clone())
        });
        assert_eq!(count, 0);

        // Add first holder
        env.as_contract(&contract_id, || {
            holder_enumeration::add_holder(&env, key_id.clone(), holder1.clone()).unwrap();
        });
        let count = env.as_contract(&contract_id, || {
            holder_enumeration::holder_count(&env, key_id.clone())
        });
        assert_eq!(count, 1);

        // Add second holder
        env.as_contract(&contract_id, || {
            holder_enumeration::add_holder(&env, key_id.clone(), holder2.clone()).unwrap();
        });
        let count = env.as_contract(&contract_id, || {
            holder_enumeration::holder_count(&env, key_id.clone())
        });
        assert_eq!(count, 2);

        // Remove first holder
        env.as_contract(&contract_id, || {
            holder_enumeration::remove_holder(&env, key_id.clone(), holder1).unwrap();
        });
        let count = env.as_contract(&contract_id, || {
            holder_enumeration::holder_count(&env, key_id.clone())
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn test_holder_enumeration_pagination() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let key_id = Address::generate(&env);

        // Add 10 holders
        let mut holders = Vec::new(&env);
        for _ in 0..10 {
            let holder = Address::generate(&env);
            holders.push_back(holder.clone());
            env.as_contract(&contract_id, || {
                holder_enumeration::add_holder(&env, key_id.clone(), holder).unwrap();
            });
        }

        // Get first page
        let page1 = env
            .as_contract(&contract_id, || {
                holder_enumeration::holders_page(&env, key_id.clone(), 0, 5)
            })
            .unwrap();
        assert_eq!(page1.len(), 5);

        // Get second page
        let page2 = env
            .as_contract(&contract_id, || {
                holder_enumeration::holders_page(&env, key_id.clone(), 5, 5)
            })
            .unwrap();
        assert_eq!(page2.len(), 5);

        // Try to exceed max limit - should fail
        let result = env.as_contract(&contract_id, || {
            holder_enumeration::holders_page(&env, key_id, 0, 101)
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_holder_enumeration_top_holders() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let key_id = Address::generate(&env);
        let holder1 = Address::generate(&env);
        let holder2 = Address::generate(&env);
        let holder3 = Address::generate(&env);

        // Add holders and update their balances
        env.as_contract(&contract_id, || {
            holder_enumeration::add_holder(&env, key_id.clone(), holder1.clone()).unwrap();
            holder_enumeration::update_top_holders(&env, key_id.clone(), holder1.clone(), 1000)
                .unwrap();
        });

        env.as_contract(&contract_id, || {
            holder_enumeration::add_holder(&env, key_id.clone(), holder2.clone()).unwrap();
            holder_enumeration::update_top_holders(&env, key_id.clone(), holder2.clone(), 2000)
                .unwrap();
        });

        env.as_contract(&contract_id, || {
            holder_enumeration::add_holder(&env, key_id.clone(), holder3.clone()).unwrap();
            holder_enumeration::update_top_holders(&env, key_id.clone(), holder3.clone(), 500)
                .unwrap();
        });

        // Get top 2 holders
        let top = env
            .as_contract(&contract_id, || {
                holder_enumeration::top_holders(&env, key_id, 2)
            })
            .unwrap();
        assert_eq!(top.len(), 2);

        // Should be sorted by balance (highest first)
        let top0 = top.get(0).unwrap();
        let top1 = top.get(1).unwrap();
        assert_eq!(top0.address, holder2);
        assert_eq!(top0.balance, 2000);
        assert_eq!(top1.address, holder1);
        assert_eq!(top1.balance, 1000);
    }

    #[test]
    fn test_holder_enumeration_top_holders_limit() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let key_id = Address::generate(&env);

        // Try to get 0 holders - should fail
        let result = env.as_contract(&contract_id, || {
            holder_enumeration::top_holders(&env, key_id.clone(), 0)
        });
        assert!(result.is_err());

        // Try to exceed max - should fail
        let result = env.as_contract(&contract_id, || {
            holder_enumeration::top_holders(&env, key_id, 51)
        });
        assert!(result.is_err());
    }

    // ============================================================================
    // Issue #1286: Badge-gated access control
    // ============================================================================

    #[test]
    fn test_badge_requirement_set_and_get() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let key_id = Address::generate(&env);
        let badge_contract = Address::generate(&env);

        // Set badge requirement for buy action
        let result = env.as_contract(&contract_id, || {
            badge_access_control::set_badge_requirement(
                &env,
                creator.clone(),
                key_id.clone(),
                badge_access_control::BadgeGatedAction::Buy,
                badge_contract.clone(),
            )
        });
        assert!(result.is_ok());

        // Get requirements
        let requirements = env.as_contract(&contract_id, || {
            badge_access_control::badge_requirement_of(
                &env,
                key_id.clone(),
                badge_access_control::BadgeGatedAction::Buy,
            )
        });
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements.get(0).unwrap(), badge_contract);

        // Check has requirement
        let has_req = env.as_contract(&contract_id, || {
            badge_access_control::has_badge_requirement(
                &env,
                key_id,
                badge_access_control::BadgeGatedAction::Buy,
            )
        });
        assert!(has_req);
    }

    #[test]
    fn test_badge_requirement_multiple_badges() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let key_id = Address::generate(&env);

        let mut badge_contracts = Vec::new(&env);
        badge_contracts.push_back(Address::generate(&env));
        badge_contracts.push_back(Address::generate(&env));
        badge_contracts.push_back(Address::generate(&env));

        // Add multiple badge contracts
        let result = env.as_contract(&contract_id, || {
            badge_access_control::add_badge_contracts(
                &env,
                creator,
                key_id.clone(),
                badge_access_control::BadgeGatedAction::Vote,
                badge_contracts.clone(),
            )
        });
        assert!(result.is_ok());

        // Get requirements
        let requirements = env.as_contract(&contract_id, || {
            badge_access_control::badge_requirement_of(
                &env,
                key_id,
                badge_access_control::BadgeGatedAction::Vote,
            )
        });
        assert_eq!(requirements.len(), 3);
    }

    #[test]
    fn test_badge_requirement_clear() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let key_id = Address::generate(&env);
        let badge_contract = Address::generate(&env);

        // Set requirement
        env.as_contract(&contract_id, || {
            badge_access_control::set_badge_requirement(
                &env,
                creator.clone(),
                key_id.clone(),
                badge_access_control::BadgeGatedAction::Claim,
                badge_contract,
            )
            .unwrap();
        });

        // Clear requirement
        let result = env.as_contract(&contract_id, || {
            badge_access_control::clear_badge_requirement(
                &env,
                creator,
                key_id.clone(),
                badge_access_control::BadgeGatedAction::Claim,
            )
        });
        assert!(result.is_ok());

        // Should have no requirements
        let requirements = env.as_contract(&contract_id, || {
            badge_access_control::badge_requirement_of(
                &env,
                key_id.clone(),
                badge_access_control::BadgeGatedAction::Claim,
            )
        });
        assert_eq!(requirements.len(), 0);

        let has_req = env.as_contract(&contract_id, || {
            badge_access_control::has_badge_requirement(
                &env,
                key_id,
                badge_access_control::BadgeGatedAction::Claim,
            )
        });
        assert!(!has_req);
    }

    #[test]
    fn test_badge_requirement_remove_specific() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::CreatorKeysContract, ());
        let creator = Address::generate(&env);
        let key_id = Address::generate(&env);
        let badge1 = Address::generate(&env);
        let badge2 = Address::generate(&env);

        // Add two badges (separate as_contract calls to avoid duplicate auth frame)
        env.as_contract(&contract_id, || {
            badge_access_control::set_badge_requirement(
                &env,
                creator.clone(),
                key_id.clone(),
                badge_access_control::BadgeGatedAction::Buy,
                badge1.clone(),
            )
            .unwrap();
        });
        env.as_contract(&contract_id, || {
            badge_access_control::set_badge_requirement(
                &env,
                creator.clone(),
                key_id.clone(),
                badge_access_control::BadgeGatedAction::Buy,
                badge2.clone(),
            )
            .unwrap();
        });

        // Remove one badge
        env.as_contract(&contract_id, || {
            badge_access_control::remove_badge_contract(
                &env,
                creator,
                key_id.clone(),
                badge_access_control::BadgeGatedAction::Buy,
                badge1,
            )
            .unwrap();
        });

        // Should have only one badge remaining
        let requirements = env.as_contract(&contract_id, || {
            badge_access_control::badge_requirement_of(
                &env,
                key_id,
                badge_access_control::BadgeGatedAction::Buy,
            )
        });
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements.get(0).unwrap(), badge2);
    }
}
