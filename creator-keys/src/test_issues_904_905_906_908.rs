//! Tests for issues #904 (timelocked admin actions), #905 (external price
//! oracle), #906 (event topic and data encoding) and #908 (multi-key staking vault).

use crate::{
    events, ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
    TimelockChangeType,
};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    vec, Address, Bytes, Env, String, Symbol, TryFromVal, TryIntoVal,
};

fn setup_test() -> (Env, CreatorKeysContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_treasury_address(&admin, &treasury);
    client.set_key_price(&admin, &100i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_protocol_fee_recipient(&admin, &treasury);

    (env, client, admin)
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient, creator: &Address) {
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

fn set_timestamp(env: &Env, timestamp: u64) {
    env.ledger().with_mut(|l| l.timestamp = timestamp);
}

/// Asserts every event from the latest invocation uses a Symbol as its first
/// topic and never a raw string in any topic position.
fn assert_horizon_compatible(env: &Env) {
    for (_contract, topics, _data) in env.events().all().iter() {
        let first = topics.get(0).unwrap();
        assert!(Symbol::try_from_val(env, &first).is_ok());
        for topic in topics.iter() {
            assert!(String::try_from_val(env, &topic).is_err());
        }
    }
}

// ─── #905: price oracle ─────────────────────────────────────────────────

#[test]
fn test_oracle_fresh_price_is_not_stale() {
    let (env, client, admin) = setup_test();
    let oracle = Address::generate(&env);
    client.set_oracle_address(&admin, &oracle);
    set_timestamp(&env, 1_000);

    client.set_oracle_price(&oracle, &250i128);

    set_timestamp(&env, 1_000 + 3_600);
    let view = client.get_oracle_price();
    assert_eq!(view.price, 250);
    assert_eq!(view.updated_at, 1_000);
    assert!(!view.is_stale);
}

#[test]
fn test_oracle_stale_price_is_flagged() {
    let (env, client, admin) = setup_test();
    let oracle = Address::generate(&env);
    client.set_oracle_address(&admin, &oracle);
    set_timestamp(&env, 1_000);
    client.set_oracle_price(&oracle, &250i128);

    set_timestamp(&env, 1_000 + 3_601);
    assert!(client.get_oracle_price().is_stale);

    // A larger configured threshold makes the same price fresh again.
    client.set_oracle_staleness_threshold(&admin, &10_000u64);
    let view = client.get_oracle_price();
    assert_eq!(view.price, 250);
    assert!(!view.is_stale);
}

#[test]
fn test_oracle_unauthorised_update_is_rejected() {
    let (env, client, admin) = setup_test();
    let oracle = Address::generate(&env);
    let intruder = Address::generate(&env);

    // No oracle configured yet.
    assert_eq!(
        client.try_set_oracle_price(&intruder, &100i128),
        Err(Ok(ContractError::Unauthorized))
    );

    client.set_oracle_address(&admin, &oracle);
    assert_eq!(
        client.try_set_oracle_price(&intruder, &100i128),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        client.try_get_oracle_price(),
        Err(Ok(ContractError::OraclePriceNotSet))
    );
}

#[test]
fn test_oracle_address_update_is_admin_only() {
    let (env, client, admin) = setup_test();
    let oracle = Address::generate(&env);
    let new_oracle = Address::generate(&env);
    let not_admin = Address::generate(&env);

    assert_eq!(
        client.try_set_oracle_address(&not_admin, &oracle),
        Err(Ok(ContractError::Unauthorized))
    );

    client.set_oracle_address(&admin, &oracle);
    client.set_oracle_address(&admin, &new_oracle);
    assert_eq!(client.get_oracle_address(), Some(new_oracle.clone()));

    // The replaced oracle can no longer publish prices.
    assert_eq!(
        client.try_set_oracle_price(&oracle, &100i128),
        Err(Ok(ContractError::Unauthorized))
    );
    client.set_oracle_price(&new_oracle, &100i128);
}

#[test]
fn test_oracle_price_updated_event_is_emitted() {
    let (env, client, admin) = setup_test();
    let oracle = Address::generate(&env);
    client.set_oracle_address(&admin, &oracle);
    set_timestamp(&env, 42);

    client.set_oracle_price(&oracle, &777i128);

    let all_events = env.events().all();
    assert_eq!(all_events.len(), 1);
    let (_contract, topics, data) = all_events.get(0).unwrap();
    let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_oracle: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    assert_eq!(name, events::ORACLE_PRICE_UPDATED_EVENT_NAME);
    assert_eq!(topic_oracle, oracle);
    let payload: events::OraclePriceUpdatedEvent = data.try_into_val(&env).unwrap();
    assert_eq!(payload.oracle, oracle);
    assert_eq!(payload.price, 777);
    assert_eq!(payload.timestamp, 42);
}

// ─── #904: timelocked admin actions ─────────────────────────────────────

#[test]
fn test_execute_action_before_delay_is_rejected() {
    let (env, client, admin) = setup_test();
    set_timestamp(&env, 1_000);
    let action_id = client.propose_action(&admin, &TimelockChangeType::Fee, &Bytes::new(&env));

    set_timestamp(&env, 1_000 + client.get_timelock_delay() - 1);
    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::TimelockNotElapsed))
    );
    assert!(!client.get_action(&action_id).unwrap().executed);
}

#[test]
fn test_execute_action_after_delay_succeeds_once() {
    let (env, client, admin) = setup_test();
    set_timestamp(&env, 1_000);
    let action_id = client.propose_action(&admin, &TimelockChangeType::Treasury, &Bytes::new(&env));

    let proposed = client.get_action(&action_id).unwrap();
    assert_eq!(proposed.proposed_at, 1_000);
    assert_eq!(
        proposed.execution_not_before,
        1_000 + client.get_timelock_delay()
    );

    set_timestamp(&env, proposed.execution_not_before);
    client.execute_action(&admin, &action_id);
    assert!(client.get_action(&action_id).unwrap().executed);

    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::ActionNotPending))
    );
}

#[test]
fn test_cancelled_action_cannot_be_executed() {
    let (env, client, admin) = setup_test();
    set_timestamp(&env, 1_000);
    let action_id = client.propose_action(&admin, &TimelockChangeType::Fee, &Bytes::new(&env));

    client.cancel_action(&admin, &action_id);
    assert!(client.get_action(&action_id).unwrap().cancelled);

    set_timestamp(&env, 1_000 + client.get_timelock_delay());
    assert_eq!(
        client.try_execute_action(&admin, &action_id),
        Err(Ok(ContractError::ActionNotPending))
    );
    assert_eq!(
        client.try_cancel_action(&admin, &action_id),
        Err(Ok(ContractError::ActionNotPending))
    );
}

#[test]
fn test_action_admin_guards_and_missing_action() {
    let (env, client, admin) = setup_test();
    let not_admin = Address::generate(&env);

    assert_eq!(
        client.try_propose_action(&not_admin, &TimelockChangeType::Fee, &Bytes::new(&env)),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        client.try_execute_action(&admin, &99u32),
        Err(Ok(ContractError::ProposalNotFound))
    );
    assert_eq!(
        client.try_cancel_action(&admin, &99u32),
        Err(Ok(ContractError::ProposalNotFound))
    );
}

#[test]
fn test_timelock_delay_is_configurable_by_admin_only() {
    let (env, client, admin) = setup_test();
    let not_admin = Address::generate(&env);

    assert_eq!(client.get_timelock_delay(), 172_800);
    assert_eq!(
        client.try_set_timelock_delay(&not_admin, &60u64),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        client.try_set_timelock_delay(&admin, &0u64),
        Err(Ok(ContractError::InvalidTimelockDelay))
    );
    assert_eq!(
        client.try_set_timelock_delay(&admin, &2_592_001u64),
        Err(Ok(ContractError::InvalidTimelockDelay))
    );

    set_timestamp(&env, 500);
    client.set_timelock_delay(&admin, &60u64);
    assert_eq!(client.get_timelock_delay(), 60);

    let action_id = client.propose_action(&admin, &TimelockChangeType::Fee, &Bytes::new(&env));
    assert_eq!(
        client.get_action(&action_id).unwrap().execution_not_before,
        560
    );
}

#[test]
fn test_action_events_carry_id_and_timestamps() {
    let (env, client, admin) = setup_test();
    client.set_timelock_delay(&admin, &60u64);
    set_timestamp(&env, 500);

    let action_id = client.propose_action(&admin, &TimelockChangeType::Fee, &Bytes::new(&env));
    let all_events = env.events().all();
    assert_eq!(all_events.len(), 1);
    let (_contract, topics, data) = all_events.get(0).unwrap();
    let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_id: u32 = topics.get(1).unwrap().try_into_val(&env).unwrap();
    assert_eq!(name, events::ACTION_PROPOSED_EVENT_NAME);
    assert_eq!(topic_id, action_id);
    let proposed: events::ActionProposedEvent = data.try_into_val(&env).unwrap();
    assert_eq!(proposed.action_id, action_id);
    assert_eq!(proposed.proposed_at, 500);
    assert_eq!(proposed.execution_not_before, 560);

    set_timestamp(&env, 560);
    client.execute_action(&admin, &action_id);
    let all_events = env.events().all();
    let (_contract, topics, data) = all_events.get(0).unwrap();
    let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    assert_eq!(name, events::ACTION_EXECUTED_EVENT_NAME);
    let executed: events::ActionExecutedEvent = data.try_into_val(&env).unwrap();
    assert_eq!(executed.action_id, action_id);
    assert_eq!(executed.executed_at, 560);
}

// ─── #908: multi-key staking vault ──────────────────────────────────────

#[test]
fn test_vault_deposit_records_share_and_locks_keys() {
    let (env, client, _admin) = setup_test();
    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);
    register_creator(&env, &client, &creator_a);
    register_creator(&env, &client, &creator_b);
    let holder = Address::generate(&env);
    client.buy_key(&creator_a, &holder, &1000i128, &None);
    client.buy_key(&creator_a, &holder, &1000i128, &None);
    client.buy_key(&creator_b, &holder, &1000i128, &None);

    client.vault_deposit(
        &holder,
        &vec![&env, creator_a.clone(), creator_b.clone()],
        &vec![&env, 2u32, 1u32],
    );

    assert_eq!(client.get_vault_share(&creator_a, &holder), 2);
    assert_eq!(client.get_vault_share(&creator_b, &holder), 1);
    assert_eq!(client.get_vault_total_shares(&creator_a), 2);
    assert_eq!(client.get_vault_total_shares(&creator_b), 1);
    assert_eq!(client.get_liquid_balance(&creator_a, &holder), 0);
    assert_eq!(client.get_staked_balance(&creator_a, &holder), 2);
}

#[test]
fn test_vault_deposit_rejects_invalid_input() {
    let (env, client, _admin) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &1000i128, &None);

    assert_eq!(
        client.try_vault_deposit(
            &holder,
            &vec![&env, creator.clone()],
            &vec![&env, 1u32, 1u32]
        ),
        Err(Ok(ContractError::InvalidVaultInput))
    );
    assert_eq!(
        client.try_vault_deposit(&holder, &vec![&env, creator.clone()], &vec![&env, 0u32]),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    assert_eq!(
        client.try_vault_deposit(&holder, &vec![&env, creator.clone()], &vec![&env, 2u32]),
        Err(Ok(ContractError::InsufficientBalance))
    );
}

#[test]
fn test_vault_partial_and_full_withdraw() {
    let (env, client, _admin) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &1000i128, &None);
    client.buy_key(&creator, &holder, &1000i128, &None);
    client.buy_key(&creator, &holder, &1000i128, &None);
    client.vault_deposit(&holder, &vec![&env, creator.clone()], &vec![&env, 3u32]);

    client.vault_withdraw(&holder, &vec![&env, creator.clone()], &vec![&env, 1u32]);
    assert_eq!(client.get_vault_share(&creator, &holder), 2);
    assert_eq!(client.get_vault_total_shares(&creator), 2);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 1);

    assert_eq!(
        client.try_vault_withdraw(&holder, &vec![&env, creator.clone()], &vec![&env, 3u32]),
        Err(Ok(ContractError::InsufficientBalance))
    );

    client.vault_withdraw(&holder, &vec![&env, creator.clone()], &vec![&env, 2u32]);
    assert_eq!(client.get_vault_share(&creator, &holder), 0);
    assert_eq!(client.get_vault_total_shares(&creator), 0);
    assert_eq!(client.get_liquid_balance(&creator, &holder), 3);
    assert_eq!(client.get_staked_balance(&creator, &holder), 0);
}

#[test]
fn test_vault_rewards_are_distributed_pro_rata() {
    let (env, client, _admin) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);
    client.buy_key(&creator, &holder_a, &1000i128, &None);
    for _ in 0..3 {
        client.buy_key(&creator, &holder_b, &1000i128, &None);
    }
    client.vault_deposit(&holder_a, &vec![&env, creator.clone()], &vec![&env, 1u32]);
    client.vault_deposit(&holder_b, &vec![&env, creator.clone()], &vec![&env, 3u32]);

    let distributor = Address::generate(&env);
    client.distribute_vault_rewards(&distributor, &creator, &4_000i128);

    assert_eq!(client.get_vault_pending_rewards(&creator, &holder_a), 1_000);
    assert_eq!(client.get_vault_pending_rewards(&creator, &holder_b), 3_000);

    // Withdrawing keeps earned rewards claimable and stops further accrual.
    client.vault_withdraw(&holder_b, &vec![&env, creator.clone()], &vec![&env, 3u32]);
    client.distribute_vault_rewards(&distributor, &creator, &1_000i128);
    assert_eq!(client.get_vault_pending_rewards(&creator, &holder_a), 2_000);
    assert_eq!(client.claim_vault_rewards(&creator, &holder_b), 3_000);
    assert_eq!(client.claim_vault_rewards(&creator, &holder_a), 2_000);
    assert_eq!(
        client.try_claim_vault_rewards(&creator, &holder_a),
        Err(Ok(ContractError::NoDividendClaimable))
    );
}

#[test]
fn test_vault_rewards_require_deposits() {
    let (env, client, _admin) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let distributor = Address::generate(&env);

    assert_eq!(
        client.try_distribute_vault_rewards(&distributor, &creator, &100i128),
        Err(Ok(ContractError::NoKeyHolders))
    );
    assert_eq!(
        client.try_distribute_vault_rewards(&distributor, &creator, &0i128),
        Err(Ok(ContractError::ZeroDistributionAmount))
    );
}

#[test]
fn test_vault_events_are_emitted_with_correct_fields() {
    let (env, client, _admin) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &1000i128, &None);
    client.buy_key(&creator, &holder, &1000i128, &None);

    client.vault_deposit(&holder, &vec![&env, creator.clone()], &vec![&env, 2u32]);
    let all_events = env.events().all();
    assert_eq!(all_events.len(), 1);
    let (_contract, topics, data) = all_events.get(0).unwrap();
    let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    let topic_creator: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    let topic_holder: Address = topics.get(2).unwrap().try_into_val(&env).unwrap();
    assert_eq!(name, events::VAULT_DEPOSIT_EVENT_NAME);
    assert_eq!(topic_creator, creator);
    assert_eq!(topic_holder, holder);
    let deposit: events::VaultDepositEvent = data.try_into_val(&env).unwrap();
    assert_eq!(deposit.amount, 2);
    assert_eq!(deposit.holder_shares, 2);
    assert_eq!(deposit.total_shares, 2);

    client.vault_withdraw(&holder, &vec![&env, creator.clone()], &vec![&env, 1u32]);
    let all_events = env.events().all();
    assert_eq!(all_events.len(), 1);
    let (_contract, topics, data) = all_events.get(0).unwrap();
    let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    assert_eq!(name, events::VAULT_WITHDRAW_EVENT_NAME);
    let withdraw: events::VaultWithdrawEvent = data.try_into_val(&env).unwrap();
    assert_eq!(withdraw.creator_id, creator);
    assert_eq!(withdraw.holder, holder);
    assert_eq!(withdraw.amount, 1);
    assert_eq!(withdraw.holder_shares, 1);
    assert_eq!(withdraw.total_shares, 1);
}

// ─── #906: event topic and data encoding ────────────────────────────────

#[test]
fn test_events_use_symbol_first_topic_and_no_raw_string_topics() {
    let (env, client, admin) = setup_test();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    assert_horizon_compatible(&env);

    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &1000i128, &None);
    assert_horizon_compatible(&env);
    assert!(!env.events().all().is_empty());

    client.vault_deposit(&holder, &vec![&env, creator.clone()], &vec![&env, 1u32]);
    assert_horizon_compatible(&env);
    client.vault_withdraw(&holder, &vec![&env, creator.clone()], &vec![&env, 1u32]);
    assert_horizon_compatible(&env);

    let oracle = Address::generate(&env);
    client.set_oracle_address(&admin, &oracle);
    client.set_oracle_price(&oracle, &100i128);
    assert_horizon_compatible(&env);
    assert!(!env.events().all().is_empty());

    let action_id = client.propose_action(&admin, &TimelockChangeType::Fee, &Bytes::new(&env));
    assert_horizon_compatible(&env);
    client.cancel_action(&admin, &action_id);
    assert_horizon_compatible(&env);
    assert!(!env.events().all().is_empty());
}
