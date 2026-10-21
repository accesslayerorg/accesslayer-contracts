mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{
    constants,
    protocol_governance::{
        GovernanceKey, ProtocolParameter, ProtocolProposal, EXECUTION_DELAY_SECONDS, VOTING_SECONDS,
    },
    ContractError, CreatorProfile, LeaderboardConfig, LeaderboardKey,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    xdr::ToXdr,
    Address, Env, IntoVal, String, Symbol, Vec,
};

#[test]
fn storage_keys_do_not_alias_existing_governance_or_leaderboard_keys() {
    let env = Env::default();
    assert_ne!(
        GovernanceKey::ProtocolGovConfig.to_xdr(&env),
        LeaderboardKey::Config.to_xdr(&env)
    );
    assert_ne!(
        GovernanceKey::ProtocolGovProposal(0).to_xdr(&env),
        creator_keys::acl_dividend_twap_gov::GovDataKey::Proposal(0).to_xdr(&env)
    );
}

#[test]
fn threshold_snapshot_quorum_timelock_and_execution() {
    let env = test_env_with_auths();
    let (client, contract) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 9_000, 1_000);
    let creator = register_test_creator(&env, &client, "electorate");
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let whale = Address::generate(&env);
    env.as_contract(&contract, || {
        env.storage().persistent().set(
            &LeaderboardKey::Config,
            &LeaderboardConfig {
                top_n: 5,
                retention_ledgers: 100,
            },
        );
        let key = constants::storage::creator(&creator);
        let mut profile: CreatorProfile = env.storage().persistent().get(&key).unwrap();
        profile.supply = 100;
        env.storage().persistent().set(&key, &profile);
        env.storage().persistent().set(
            &constants::storage::holder_registry(&creator),
            &Vec::from_array(&env, [alice.clone(), bob.clone(), whale.clone()]),
        );
        for (holder, balance) in [(&alice, 4u32), (&bob, 6), (&whale, 90)] {
            env.storage().persistent().set(
                &constants::storage::holder_balance_key(&creator, holder),
                &balance,
            );
        }
    });
    client.configure_protocol_governance(&admin, &creator, &5);
    assert_eq!(
        client.try_create_proposal(
            &alice,
            &ProtocolParameter::KeyPrice,
            &2_000,
            &String::from_str(&env, "raise price")
        ),
        Err(Ok(ContractError::InsufficientBalance))
    );
    let id = client.create_proposal(
        &bob,
        &ProtocolParameter::KeyPrice,
        &2_000,
        &String::from_str(&env, "raise price"),
    );
    let (_, topics, data) = env.events().all().last().unwrap();
    let name: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(name, Symbol::new(&env, "gov_new"));
    let created: ProtocolProposal = data.into_val(&env);
    assert_eq!(created.id, id);
    assert_eq!(created.snapshot_supply, 100);
    // A later transfer cannot multiply voting power: the snapshot remains 4/6/90.
    env.as_contract(&contract, || {
        env.storage().persistent().set(
            &constants::storage::holder_balance_key(&creator, &alice),
            &94u32,
        );
    });
    client.cast_proposal_vote(&alice, &id, &true);
    let (_, topics, data) = env.events().all().last().unwrap();
    let name: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(name, Symbol::new(&env, "gov_vote"));
    let voted: (bool, u32) = data.into_val(&env);
    assert_eq!(voted, (true, 4));
    assert_eq!(client.get_protocol_proposal(&id).votes_for, 4);
    assert_eq!(
        client.try_cast_proposal_vote(&alice, &id, &true),
        Err(Ok(ContractError::AlreadyApproved))
    );
    // Below 10% turnout must fail even after both waiting periods.
    env.ledger()
        .with_mut(|ledger| ledger.timestamp = VOTING_SECONDS + EXECUTION_DELAY_SECONDS);
    assert_eq!(
        client.try_execute_proposal(&id),
        Err(Ok(ContractError::Unauthorized))
    );

    // A new proposal meets the exact quorum with snapshot weights 4 + 6.
    env.as_contract(&contract, || {
        env.storage().persistent().set(
            &constants::storage::holder_balance_key(&creator, &alice),
            &4u32,
        );
    });
    let id = client.create_proposal(
        &bob,
        &ProtocolParameter::KeyPrice,
        &2_000,
        &String::from_str(&env, "raise price"),
    );
    client.cast_proposal_vote(&alice, &id, &true);
    client.cast_proposal_vote(&bob, &id, &true);
    let proposal = client.get_protocol_proposal(&id);
    assert_eq!(proposal.votes_for, 10);
    env.ledger()
        .with_mut(|ledger| ledger.timestamp = proposal.executable_at - 1);
    assert_eq!(
        client.try_execute_proposal(&id),
        Err(Ok(ContractError::TimelockNotElapsed))
    );
    assert_eq!(
        client.try_cast_proposal_vote(&whale, &id, &false),
        Err(Ok(ContractError::DeadlinePassed))
    );
    env.ledger()
        .with_mut(|ledger| ledger.timestamp = proposal.executable_at);
    client.execute_proposal(&id);
    let (_, topics, data) = env.events().all().last().unwrap();
    let name: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(name, Symbol::new(&env, "gov_exec"));
    let executed: ProtocolProposal = data.into_val(&env);
    assert!(executed.executed);
    assert_eq!(executed.id, id);
    assert!(client.get_protocol_proposal(&id).executed);
    assert_eq!(
        client.try_execute_proposal(&id),
        Err(Ok(ContractError::ActionNotPending))
    );
    env.as_contract(&contract, || {
        assert_eq!(
            env.storage()
                .persistent()
                .get::<_, i128>(&constants::storage::KEY_PRICE),
            Some(2_000)
        );
        let leaderboard: LeaderboardConfig = env
            .storage()
            .persistent()
            .get(&LeaderboardKey::Config)
            .unwrap();
        assert_eq!(leaderboard.top_n, 5);
        assert_eq!(leaderboard.retention_ledgers, 100);
    });
}
