#![cfg(test)]

use crate::{
    EpochRewardDistributed, KeyStakingContract, KeyStakingContractClient, KeyStakingError,
    StakeRewards, EPOCH_DISTRIBUTED_EVENT_NAME, MAX_LOCK_EPOCHS,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, Symbol, TryFromVal, Val, Vec,
};

const EPOCH: u64 = 100;
const T0: u64 = 1_000;

struct Setup {
    env: Env,
    client: KeyStakingContractClient<'static>,
    treasury: Address,
    key_id: Address,
}

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(T0);
    let contract_id = env.register(KeyStakingContract, ());
    let client = KeyStakingContractClient::new(&env, &contract_id);
    let treasury = Address::generate(&env);
    client.initialize(&treasury, &EPOCH);
    let key_id = Address::generate(&env);
    Setup {
        env,
        client,
        treasury,
        key_id,
    }
}

fn advance_to_epoch(env: &Env, epoch: u64) {
    env.ledger().set_timestamp(T0 + epoch * EPOCH);
}

fn get_events<T>(env: &Env, topic_symbol: Symbol) -> Vec<T>
where
    T: IntoVal<Env, Val> + TryFromVal<Env, Val>,
{
    let mut matching = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            if Symbol::try_from_val(env, &first) == Ok(topic_symbol.clone()) {
                if let Ok(event) = T::try_from_val(env, &data) {
                    matching.push_back(event);
                }
            }
        }
    }
    matching
}

#[test]
fn unstake_before_epoch_completion_fails_with_epoch_not_complete() {
    let s = setup();
    let staker = Address::generate(&s.env);
    let id = s.client.stake(&s.key_id, &staker, &1_000, &3);

    // Immediately, mid-lock, and in the final epoch of the lock.
    for epoch in [0, 1, 2] {
        advance_to_epoch(&s.env, epoch);
        assert_eq!(
            s.client.try_unstake(&id),
            Err(Ok(KeyStakingError::EpochNotComplete))
        );
    }
    // The failed attempts left the position open.
    assert!(!s.client.get_stake(&id).is_closed);

    advance_to_epoch(&s.env, 3);
    assert_eq!(s.client.unstake(&id), 1_000);
}

#[test]
fn rewards_are_split_pro_rata_to_stake_weight_within_epoch() {
    let s = setup();
    let a = Address::generate(&s.env);
    let b = Address::generate(&s.env);
    let id_a = s.client.stake(&s.key_id, &a, &300, &1);
    let id_b = s.client.stake(&s.key_id, &b, &100, &1);

    s.client.fund_epoch(&0, &1_000);
    advance_to_epoch(&s.env, 1);

    // 300/400 and 100/400 of the 1_000 pool, on top of principal.
    assert_eq!(s.client.unstake(&id_a), 300 + 750);
    assert_eq!(s.client.unstake(&id_b), 100 + 250);
}

#[test]
fn rewards_accumulate_only_over_the_epochs_a_stake_is_active() {
    let s = setup();
    let long = Address::generate(&s.env);
    let short = Address::generate(&s.env);
    let id_long = s.client.stake(&s.key_id, &long, &100, &2);
    let id_short = s.client.stake(&s.key_id, &short, &100, &1);

    s.client.fund_epoch(&0, &200); // split 100 / 100
    s.client.fund_epoch(&1, &200); // long stake alone

    advance_to_epoch(&s.env, 2);
    assert_eq!(s.client.unstake(&id_long), 100 + 100 + 200);
    assert_eq!(s.client.unstake(&id_short), 100 + 100);
}

#[test]
fn late_staker_does_not_earn_from_epochs_before_it_joined() {
    let s = setup();
    let early = Address::generate(&s.env);
    let late = Address::generate(&s.env);
    let id_early = s.client.stake(&s.key_id, &early, &100, &2);
    s.client.fund_epoch(&0, &500);
    s.client.fund_epoch(&1, &500);

    advance_to_epoch(&s.env, 1);
    let id_late = s.client.stake(&s.key_id, &late, &100, &1);

    advance_to_epoch(&s.env, 2);
    assert_eq!(s.client.unstake(&id_early), 100 + 500 + 250);
    assert_eq!(s.client.unstake(&id_late), 100 + 250);
}

#[test]
fn fund_epoch_requires_the_treasury_contract_auth() {
    let env = Env::default();
    env.ledger().set_timestamp(T0);
    let contract_id = env.register(KeyStakingContract, ());
    let client = KeyStakingContractClient::new(&env, &contract_id);
    let treasury = Address::generate(&env);
    client.initialize(&treasury, &EPOCH);

    // No authorisation at all, and an unrelated address, are both rejected.
    assert!(client.try_fund_epoch(&0, &100).is_err());
    let stranger = Address::generate(&env);
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &stranger,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "fund_epoch",
            args: (0u32, 100i128).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(client.try_fund_epoch(&0, &100).is_err());
    assert_eq!(client.get_epoch(&0).funded, 0);

    // The treasury succeeds.
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &treasury,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "fund_epoch",
            args: (0u32, 100i128).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    client.fund_epoch(&0, &100);
    assert_eq!(client.get_epoch(&0).funded, 100);
}

#[test]
fn fund_epoch_rejects_bad_amount_and_ended_epochs_and_accumulates() {
    let s = setup();
    assert_eq!(
        s.client.try_fund_epoch(&0, &0),
        Err(Ok(KeyStakingError::NotPositiveAmount))
    );
    s.client.fund_epoch(&5, &70);
    s.client.fund_epoch(&5, &30);
    assert_eq!(s.client.get_epoch(&5).funded, 100);

    advance_to_epoch(&s.env, 2);
    assert_eq!(
        s.client.try_fund_epoch(&1, &10),
        Err(Ok(KeyStakingError::EpochEnded))
    );
    // The current epoch is still fundable.
    s.client.fund_epoch(&2, &10);
}

#[test]
fn get_stake_rewards_reports_accrued_and_projected_values() {
    let s = setup();
    let staker = Address::generate(&s.env);
    let id = s.client.stake(&s.key_id, &staker, &100, &2);
    s.client.fund_epoch(&0, &100);
    s.client.fund_epoch(&1, &100);

    let rewards = |accrued, projected| StakeRewards { accrued, projected };
    assert_eq!(s.client.get_stake_rewards(&id), rewards(0, 200));

    advance_to_epoch(&s.env, 1);
    assert_eq!(s.client.get_stake_rewards(&id), rewards(100, 100));

    advance_to_epoch(&s.env, 2);
    assert_eq!(s.client.get_stake_rewards(&id), rewards(200, 0));

    // Projection follows pool and weight changes: a second staker halves epoch 1.
    let other = Address::generate(&s.env);
    let id2 = s.client.stake(&s.key_id, &other, &100, &1);
    assert_eq!(s.client.get_stake_rewards(&id2), rewards(0, 0));

    s.client.unstake(&id);
    assert_eq!(s.client.get_stake_rewards(&id), rewards(0, 0));
}

#[test]
fn projected_rewards_shrink_when_another_staker_joins_the_epoch() {
    let s = setup();
    let a = Address::generate(&s.env);
    let b = Address::generate(&s.env);
    let id_a = s.client.stake(&s.key_id, &a, &100, &1);
    s.client.fund_epoch(&0, &1_000);
    assert_eq!(s.client.get_stake_rewards(&id_a).projected, 1_000);

    s.client.stake(&s.key_id, &b, &100, &1);
    assert_eq!(s.client.get_stake_rewards(&id_a).projected, 500);
}

#[test]
fn distribute_epoch_emits_event_with_epoch_and_total_distributed() {
    let s = setup();
    let a = Address::generate(&s.env);
    let b = Address::generate(&s.env);
    s.client.stake(&s.key_id, &a, &300, &1);
    s.client.stake(&s.key_id, &b, &100, &1);
    s.client.fund_epoch(&0, &600);
    s.client.fund_epoch(&0, &400);

    assert_eq!(
        s.client.try_distribute_epoch(&0),
        Err(Ok(KeyStakingError::EpochNotComplete))
    );
    assert_eq!(
        get_events::<EpochRewardDistributed>(&s.env, EPOCH_DISTRIBUTED_EVENT_NAME).len(),
        0
    );

    advance_to_epoch(&s.env, 1);
    assert_eq!(s.client.distribute_epoch(&0), 1_000);
    let events = get_events::<EpochRewardDistributed>(&s.env, EPOCH_DISTRIBUTED_EVENT_NAME);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events.get(0).unwrap(),
        EpochRewardDistributed {
            epoch_id: 0,
            total_distributed: 1_000
        }
    );

    assert_eq!(
        s.client.try_distribute_epoch(&0),
        Err(Ok(KeyStakingError::EpochAlreadyDistributed))
    );
}

#[test]
fn distribute_epoch_without_stakers_distributes_nothing() {
    let s = setup();
    s.client.fund_epoch(&0, &500);
    advance_to_epoch(&s.env, 1);
    assert_eq!(s.client.distribute_epoch(&0), 0);
    let events = get_events::<EpochRewardDistributed>(&s.env, EPOCH_DISTRIBUTED_EVENT_NAME);
    assert_eq!(events.get(0).unwrap().total_distributed, 0);
}

#[test]
fn stake_validates_amount_and_epoch_count() {
    let s = setup();
    let staker = Address::generate(&s.env);
    assert_eq!(
        s.client.try_stake(&s.key_id, &staker, &0, &1),
        Err(Ok(KeyStakingError::NotPositiveAmount))
    );
    assert_eq!(
        s.client.try_stake(&s.key_id, &staker, &10, &0),
        Err(Ok(KeyStakingError::InvalidEpochs))
    );
    assert_eq!(
        s.client
            .try_stake(&s.key_id, &staker, &10, &(MAX_LOCK_EPOCHS + 1)),
        Err(Ok(KeyStakingError::InvalidEpochs))
    );
    assert_eq!(s.client.stake(&s.key_id, &staker, &10, &MAX_LOCK_EPOCHS), 1);
}

#[test]
fn unstake_twice_and_unknown_stake_are_rejected() {
    let s = setup();
    let staker = Address::generate(&s.env);
    let id = s.client.stake(&s.key_id, &staker, &10, &1);
    advance_to_epoch(&s.env, 1);
    s.client.unstake(&id);
    assert_eq!(
        s.client.try_unstake(&id),
        Err(Ok(KeyStakingError::StakeAlreadyClosed))
    );
    assert_eq!(
        s.client.try_unstake(&99),
        Err(Ok(KeyStakingError::StakeNotFound))
    );
    assert_eq!(
        s.client.try_get_stake_rewards(&99),
        Err(Ok(KeyStakingError::StakeNotFound))
    );
}

#[test]
fn initialize_is_one_time_and_contract_requires_it() {
    let s = setup();
    assert_eq!(
        s.client.try_initialize(&s.treasury, &EPOCH),
        Err(Ok(KeyStakingError::AlreadyInitialized))
    );

    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(KeyStakingContract, ());
    let client = KeyStakingContractClient::new(&env, &id);
    assert_eq!(
        client.try_initialize(&Address::generate(&env), &0),
        Err(Ok(KeyStakingError::InvalidEpochDuration))
    );
    let staker = Address::generate(&env);
    assert_eq!(
        client.try_stake(&Address::generate(&env), &staker, &1, &1),
        Err(Ok(KeyStakingError::NotInitialized))
    );
}
