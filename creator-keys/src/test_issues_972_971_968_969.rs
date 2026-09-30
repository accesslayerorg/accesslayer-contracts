//! Unit and integration tests for issues:
//! #972 (ACL whitelist contract for external integration gating)
//! #971 (Dividend pool contract with pro-rata distribution logic)
//! #968 (TWAP oracle storage and update mechanism for bonding curve)
//! #969 (Governance proposal contract with token-weighted voting)

use crate::acl_dividend_twap_gov::{
    AclWhitelistContract, AclWhitelistContractClient, CreateProposalParams, DividendPoolContract,
    DividendPoolContractClient, GovernanceProposalContract, GovernanceProposalContractClient,
    HolderSnapshotRecord, ProposalStatus, TwapOracleContract, TwapOracleContractClient,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    Address, Env, String, Vec,
};

// ═══════════════════════════════════════════════════════════════════════════════
// ISSUE #972: ACL WHITELIST CONTRACT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_acl_add_and_permission_checks() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AclWhitelistContract, ());
    let client = AclWhitelistContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let target_contract = Address::generate(&env);
    client.init(&admin);

    let mut funcs = Vec::new(&env);
    funcs.push_back(symbol_short!("swap"));
    funcs.push_back(symbol_short!("deposit"));

    // Add to ACL
    assert!(client
        .try_add_to_acl(&admin, &target_contract, &funcs)
        .is_ok());

    // Permitted functions
    assert!(client.is_permitted(&target_contract, &symbol_short!("swap")));
    assert!(client.is_permitted(&target_contract, &symbol_short!("deposit")));

    // Non-permitted function
    assert!(!client.is_permitted(&target_contract, &symbol_short!("withdraw")));

    // Non-whitelisted contract
    let other_contract = Address::generate(&env);
    assert!(!client.is_permitted(&other_contract, &symbol_short!("swap")));
}

#[test]
fn test_acl_wildcard_permission() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AclWhitelistContract, ());
    let client = AclWhitelistContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let router_contract = Address::generate(&env);
    client.init(&admin);

    let mut funcs = Vec::new(&env);
    funcs.push_back(symbol_short!("all"));

    assert!(client
        .try_add_to_acl(&admin, &router_contract, &funcs)
        .is_ok());

    assert!(client.is_permitted(&router_contract, &symbol_short!("swap")));
    assert!(client.is_permitted(&router_contract, &symbol_short!("any_fn")));
}

#[test]
fn test_acl_remove_clears_permissions() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AclWhitelistContract, ());
    let client = AclWhitelistContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let target_contract = Address::generate(&env);
    client.init(&admin);

    let mut funcs = Vec::new(&env);
    funcs.push_back(symbol_short!("swap"));

    assert!(client
        .try_add_to_acl(&admin, &target_contract, &funcs)
        .is_ok());
    assert!(client.is_permitted(&target_contract, &symbol_short!("swap")));

    // Remove from ACL
    assert!(client.try_remove_from_acl(&admin, &target_contract).is_ok());

    // Now not permitted
    assert!(!client.is_permitted(&target_contract, &symbol_short!("swap")));

    let acl_list = client.get_acl();
    assert_eq!(acl_list.len(), 0);
}

#[test]
fn test_acl_contract_client_interactions() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AclWhitelistContract, ());
    let client = AclWhitelistContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let caller = Address::generate(&env);
    let target = Address::generate(&env);

    client.init(&admin);

    let mut funcs = Vec::new(&env);
    funcs.push_back(symbol_short!("execute"));

    // Unauthorized caller cannot add
    let res = client.try_add_to_acl(&caller, &target, &funcs);
    assert!(res.is_err());

    // Admin can add
    let res_admin = client.try_add_to_acl(&admin, &target, &funcs);
    assert!(res_admin.is_ok());

    assert!(client.is_permitted(&target, &symbol_short!("execute")));
    assert!(!client.is_permitted(&target, &symbol_short!("unknown")));

    let acl = client.get_acl();
    assert_eq!(acl.len(), 1);
    assert_eq!(acl.get(0).unwrap().contract_address, target);

    // Admin can remove
    let res_remove = client.try_remove_from_acl(&admin, &target);
    assert!(res_remove.is_ok());
    assert!(!client.is_permitted(&target, &symbol_short!("execute")));
}

// ═══════════════════════════════════════════════════════════════════════════════
// ISSUE #971: DIVIDEND POOL CONTRACT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_dividend_pro_rata_distribution_and_claiming() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DividendPoolContract, ());
    let client = DividendPoolContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let key_id = Address::generate(&env);
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);

    client.init(&admin);

    // Snapshot: total supply = 1000, holder A has 300 (30%), holder B has 700 (70%)
    let mut records = Vec::new(&env);
    records.push_back(HolderSnapshotRecord {
        holder: holder_a.clone(),
        balance: 300,
    });
    records.push_back(HolderSnapshotRecord {
        holder: holder_b.clone(),
        balance: 700,
    });

    let epoch = client.snapshot_holdings(&creator, &key_id, &1000, &records);
    assert_eq!(epoch, 1);

    // Deposit 10_000 dividend
    assert!(client
        .try_deposit_dividends(&creator, &key_id, &10_000)
        .is_ok());

    // Check pending dividends before claim
    let pending_a = client.get_pending_dividends(&key_id, &holder_a);
    let pending_b = client.get_pending_dividends(&key_id, &holder_b);

    assert_eq!(pending_a, 3_000); // 30% of 10,000
    assert_eq!(pending_b, 7_000); // 70% of 10,000

    // Holder A claims
    let claimed_a = client.claim_dividends(&key_id, &holder_a);
    assert_eq!(claimed_a, 3_000);

    // Check pending after claim
    assert_eq!(client.get_pending_dividends(&key_id, &holder_a), 0);
    assert_eq!(client.get_pending_dividends(&key_id, &holder_b), 7_000);

    // Double claim prevented
    let double_claim_res = client.try_claim_dividends(&key_id, &holder_a);
    assert!(double_claim_res.is_err());

    // Holder B claims
    let claimed_b = client.claim_dividends(&key_id, &holder_b);
    assert_eq!(claimed_b, 7_000);
    assert_eq!(client.get_pending_dividends(&key_id, &holder_b), 0);

    // Verify distribution history
    let history = client.get_distribution_history(&key_id);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap().total_amount, 10_000);
}

#[test]
fn test_dividend_contract_client() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DividendPoolContract, ());
    let client = DividendPoolContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let key_id = Address::generate(&env);
    let holder = Address::generate(&env);

    client.init(&admin);

    let mut records = Vec::new(&env);
    records.push_back(HolderSnapshotRecord {
        holder: holder.clone(),
        balance: 500,
    });

    let epoch = client.snapshot_holdings(&creator, &key_id, &1000, &records);
    assert_eq!(epoch, 1);

    client.deposit_dividends(&creator, &key_id, &20_000);

    let pending = client.get_pending_dividends(&key_id, &holder);
    assert_eq!(pending, 10_000); // 50% of 20,000

    let claimed = client.claim_dividends(&key_id, &holder);
    assert_eq!(claimed, 10_000);

    assert_eq!(client.get_pending_dividends(&key_id, &holder), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ISSUE #968: TWAP ORACLE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_twap_oracle_accumulation_and_windows() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TwapOracleContract, ());
    let client = TwapOracleContractClient::new(&env, &contract_id);

    let key_id = Address::generate(&env);

    // Initial trade at t = 10_000, price = 100
    env.ledger().with_mut(|l| l.timestamp = 10_000);
    assert!(client.try_record_trade(&key_id, &100).is_ok());

    // Min observation guard should fail when only 1 observation exists
    assert!(client.try_get_twap(&key_id, &3600).is_err());

    // Trade 2 at t = 13_600 (1h later), price = 200
    // Cumulative added: 100 * 3600 = 360,000
    env.ledger().with_mut(|l| l.timestamp = 13_600);
    assert!(client.try_record_trade(&key_id, &200).is_ok());

    // Trade 3 at t = 24_400 (3h later, total 4h from t=10_000), price = 300
    // Cumulative added: 200 * 10800 = 2,160,000; total = 2,520,000
    env.ledger().with_mut(|l| l.timestamp = 24_400);
    assert!(client.try_record_trade(&key_id, &300).is_ok());

    // Check TWAP over 4h (14400s) window: 2,520,000 / 14400 = 175
    let twap_4h = client.get_twap(&key_id, &14400);
    assert_eq!(twap_4h, 175);

    // Check TWAP over 1h (3600s) window from t=24_400 (from 20_800 to 24_400, price was 200)
    let twap_1h = client.get_twap(&key_id, &3600);
    assert_eq!(twap_1h, 200);
}

#[test]
fn test_twap_wash_trade_same_block_protection() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TwapOracleContract, ());
    let client = TwapOracleContractClient::new(&env, &contract_id);

    let key_id = Address::generate(&env);

    env.ledger().with_mut(|l| l.timestamp = 1_000);
    assert!(client.try_record_trade(&key_id, &100).is_ok());

    // Same block trades update price without cumulative inflation
    assert!(client.try_record_trade(&key_id, &150).is_ok());
    assert!(client.try_record_trade(&key_id, &120).is_ok());

    // Advance time to t = 2_000 (1000s later)
    env.ledger().with_mut(|l| l.timestamp = 2_000);
    assert!(client.try_record_trade(&key_id, &180).is_ok());

    // TWAP over 1000s window should be exactly 120
    let twap = client.get_twap(&key_id, &1000);
    assert_eq!(twap, 120);
}

#[test]
fn test_twap_oracle_contract_client() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TwapOracleContract, ());
    let client = TwapOracleContractClient::new(&env, &contract_id);

    let key_id = Address::generate(&env);

    env.ledger().with_mut(|l| l.timestamp = 1_000);
    client.record_trade(&key_id, &500);

    env.ledger().with_mut(|l| l.timestamp = 4_600); // +3600s
    client.record_trade(&key_id, &700);

    let twap = client.get_twap(&key_id, &3600);
    assert_eq!(twap, 500);

    let observations = client.get_observations(&key_id);
    assert_eq!(observations.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ISSUE #969: GOVERNANCE PROPOSAL CONTRACT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_governance_proposal_lifecycle_pass_and_execute() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(GovernanceProposalContract, ());
    let client = GovernanceProposalContractClient::new(&env, &contract_id);

    let proposer = Address::generate(&env);
    let key_id = Address::generate(&env);
    let voter_a = Address::generate(&env);
    let voter_b = Address::generate(&env);

    let discussion = String::from_str(&env, "https://gov.forum/prop-1");

    // Proposer with insufficient balance rejected
    let invalid_params = CreateProposalParams {
        key_id: key_id.clone(),
        discussion_link: discussion.clone(),
        proposer_balance: 50, // < min holding
        min_holding_threshold: 100,
        voting_duration_ledgers: 1000,
        min_quorum_weight: 500,
        pass_threshold_bps: 5000,
    };
    let fail_res = client.try_create_proposal(&proposer, &invalid_params);
    assert!(fail_res.is_err());

    // Proposer with sufficient balance succeeds
    let valid_params = CreateProposalParams {
        key_id: key_id.clone(),
        discussion_link: discussion,
        proposer_balance: 150, // >= min holding
        min_holding_threshold: 100,
        voting_duration_ledgers: 1000,
        min_quorum_weight: 500,
        pass_threshold_bps: 5000,
    };
    let proposal_id = client.create_proposal(&proposer, &valid_params);

    let prop = client.get_proposal(&proposal_id);
    assert_eq!(prop.status, ProposalStatus::Active);

    // Vote approval: voter A (weight 400) votes yes
    assert!(client.try_vote(&voter_a, &proposal_id, &true, &400).is_ok());

    // Double voting rejected
    let double_vote_res = client.try_vote(&voter_a, &proposal_id, &true, &400);
    assert!(double_vote_res.is_err());

    // Voter B (weight 200) votes yes
    assert!(client.try_vote(&voter_b, &proposal_id, &true, &200).is_ok());

    // Execution before voting period ends fails
    let early_exec = client.try_execute_proposal(&proposer, &proposal_id);
    assert!(early_exec.is_err());

    // Advance ledger past voting end
    env.ledger().with_mut(|l| l.sequence_number += 1001);

    // Proposal status becomes Passed
    let prop_passed = client.get_proposal(&proposal_id);
    assert_eq!(prop_passed.status, ProposalStatus::Passed);

    // Execute proposal
    let exec_res = client.execute_proposal(&proposer, &proposal_id);
    assert_eq!(exec_res, ProposalStatus::Executed);

    let prop_executed = client.get_proposal(&proposal_id);
    assert_eq!(prop_executed.status, ProposalStatus::Executed);
}

#[test]
fn test_governance_proposal_lifecycle_failure_below_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(GovernanceProposalContract, ());
    let client = GovernanceProposalContractClient::new(&env, &contract_id);

    let proposer = Address::generate(&env);
    let key_id = Address::generate(&env);
    let voter_no = Address::generate(&env);

    let discussion = String::from_str(&env, "https://gov.forum/prop-2");
    let params = CreateProposalParams {
        key_id: key_id.clone(),
        discussion_link: discussion,
        proposer_balance: 200,
        min_holding_threshold: 100,
        voting_duration_ledgers: 500,
        min_quorum_weight: 100,
        pass_threshold_bps: 5000, // 50% pass threshold
    };
    let proposal_id = client.create_proposal(&proposer, &params);

    // Vote reject: voter votes false
    assert!(client
        .try_vote(&voter_no, &proposal_id, &false, &300)
        .is_ok());

    // Advance ledger past voting end
    env.ledger().with_mut(|l| l.sequence_number += 501);

    let prop = client.get_proposal(&proposal_id);
    assert_eq!(prop.status, ProposalStatus::Failed);

    // Execution fails for rejected proposal
    let exec_res = client.try_execute_proposal(&proposer, &proposal_id);
    assert!(exec_res.is_err());
}

#[test]
fn test_governance_proposal_contract_client() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(GovernanceProposalContract, ());
    let client = GovernanceProposalContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let key_id = Address::generate(&env);
    let voter = Address::generate(&env);

    client.init(&admin, &50);

    let discussion = String::from_str(&env, "https://forum.accesslayer.org");
    let params = CreateProposalParams {
        key_id: key_id.clone(),
        discussion_link: discussion,
        proposer_balance: 100,
        min_holding_threshold: 50,
        voting_duration_ledgers: 300,
        min_quorum_weight: 100,
        pass_threshold_bps: 5000,
    };
    let prop_id = client.create_proposal(&proposer, &params);

    client.vote(&voter, &prop_id, &true, &200);

    env.ledger().with_mut(|l| l.sequence_number += 301);

    let prop = client.get_proposal(&prop_id);
    assert_eq!(prop.status, ProposalStatus::Passed);

    let final_status = client.execute_proposal(&admin, &prop_id);
    assert_eq!(final_status, ProposalStatus::Executed);
}

// ═══════════════════════════════════════════════════════════════════════════════
// KEY RATING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_key_rating_flow_and_rejections() {
    use crate::{
        CreatorKeysContract, CreatorKeysContractClient, RatingError, RegisterCreatorParams,
    };

    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let rater1 = Address::generate(&env);
    let rater2 = Address::generate(&env);
    let non_holder = Address::generate(&env);

    // Register creator
    let handle = String::from_str(&env, "alice");
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle,
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Non-holder attempts to rate -> fails with NotAHolder
    let res = client.try_rate_key(&creator, &non_holder, &5);
    assert_eq!(res, Err(Ok(RatingError::NotAHolder)));

    // Setup protocol admin, fee config, and key price
    let admin = Address::generate(&env);
    let fee_recipient = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_fee_config(&admin, &9000, &1000);
    client.set_protocol_fee_recipient(&admin, &fee_recipient);
    client.set_key_price(&admin, &10_000_000);

    client.buy_key(&creator, &rater1, &100_000_000, &None);
    client.buy_key(&creator, &rater2, &100_000_000, &None);

    // Invalid score (< 1 or > 5)
    let res = client.try_rate_key(&creator, &rater1, &0);
    assert_eq!(res, Err(Ok(RatingError::InvalidScore)));
    let res = client.try_rate_key(&creator, &rater1, &6);
    assert_eq!(res, Err(Ok(RatingError::InvalidScore)));

    // First rating from rater1 (score 4)
    let agg1 = client.rate_key(&creator, &rater1, &4);
    assert_eq!(agg1.count, 1);
    assert_eq!(agg1.total_score, 4);
    assert_eq!(agg1.average_score_scaled, 400); // 4.00 stars

    // Rating from rater2 (score 5)
    let agg2 = client.rate_key(&creator, &rater2, &5);
    assert_eq!(agg2.count, 2);
    assert_eq!(agg2.total_score, 9);
    assert_eq!(agg2.average_score_scaled, 450); // 4.50 stars

    // Re-rating from rater1 (update score 4 -> 2)
    let agg3 = client.rate_key(&creator, &rater1, &2);
    assert_eq!(agg3.count, 2); // count remains 2
    assert_eq!(agg3.total_score, 7); // (9 - 4 + 2)
    assert_eq!(agg3.average_score_scaled, 350); // 3.50 stars

    // Verify getter view
    let current_agg = client.get_key_rating(&creator);
    assert_eq!(current_agg, agg3);
}
