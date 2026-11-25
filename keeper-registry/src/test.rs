use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token, Address, Env, IntoVal, Symbol, TryIntoVal,
};

struct Setup<'a> {
    env: Env,
    contract: Address,
    client: KeeperRegistryContractClient<'a>,
    admin: Address,
    governor: Address,
    treasury: Address,
    token: Address,
}

fn setup(exclusive_delay: u32, unbonding_delay: u32) -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.sequence_number = 1);
    let contract = env.register(KeeperRegistryContract, ());
    let client = KeeperRegistryContractClient::new(&env, &contract);
    let admin = Address::generate(&env);
    let governor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.initialize(
        &admin,
        &governor,
        &token,
        &treasury,
        &1_000,
        &unbonding_delay,
        &exclusive_delay,
        &true,
    );
    Setup {
        env,
        contract,
        client,
        admin,
        governor,
        treasury,
        token,
    }
}

fn fund(s: &Setup<'_>, who: &Address, amount: i128) {
    token::StellarAssetClient::new(&s.env, &s.token).mint(who, &amount);
}

#[test]
fn registration_locks_minimum_bond_and_emits_event() {
    let s = setup(10, 5);
    let keeper = Address::generate(&s.env);
    fund(&s, &keeper, 2_000);

    assert!(s.client.try_register_keeper(&keeper, &999).is_err());
    s.client.register_keeper(&keeper, &1_000);

    let registered: Symbol = Symbol::new(&s.env, KEEPER_REGISTERED_EVENT);
    let mut found = false;
    for (_, topics, data) in s.env.events().all().iter() {
        let name: Result<Symbol, _> = topics.get(0).unwrap().try_into_val(&s.env);
        let Ok(name) = name else { continue };
        if name != registered {
            continue;
        }
        let topic_keeper: Result<Address, _> = topics.get(1).unwrap().try_into_val(&s.env);
        let Ok(topic_keeper) = topic_keeper else {
            continue;
        };
        let event: Result<KeeperRegisteredEvent, _> = data.try_into_val(&s.env);
        let Ok(event) = event else { continue };
        if topic_keeper == keeper && event.bond == 1_000 {
            found = true;
        }
    }
    assert!(found);

    let stats = s.client.keeper_stats(&keeper);
    assert_eq!(stats.active_bond, 1_000);
    assert_eq!(stats.successful_actions, 0);
    assert_eq!(stats.failed_submissions, 0);
    assert_eq!(
        token::TokenClient::new(&s.env, &s.token).balance(&s.contract),
        1_000
    );
}

#[test]
fn exclusive_window_blocks_unbonded_callers_and_falls_back_after_delay() {
    let s = setup(10, 5);
    let keeper = Address::generate(&s.env);
    let outsider = Address::generate(&s.env);
    fund(&s, &keeper, 1_000);
    s.client.register_keeper(&keeper, &1_000);

    assert!(s.client.try_record_action(&outsider, &true).is_err());
    s.client.record_action(&keeper, &true);
    s.client.record_action(&keeper, &false);
    assert_eq!(s.client.keeper_stats(&keeper).successful_actions, 1);
    assert_eq!(s.client.keeper_stats(&keeper).failed_submissions, 1);

    s.env.ledger().with_mut(|l| l.sequence_number = 11);
    s.client.record_action(&outsider, &true);
    assert_eq!(s.client.keeper_stats(&outsider).successful_actions, 0);
}

#[test]
fn deregistration_honours_unbonding_delay() {
    let s = setup(0, 5);
    let keeper = Address::generate(&s.env);
    fund(&s, &keeper, 1_000);
    s.client.register_keeper(&keeper, &1_000);
    s.client.deregister_keeper(&keeper);

    assert!(s.client.try_finalize_deregistration(&keeper).is_err());
    s.env.ledger().with_mut(|l| l.sequence_number = 6);
    assert_eq!(s.client.finalize_deregistration(&keeper), 1_000);
    assert_eq!(
        token::TokenClient::new(&s.env, &s.token).balance(&keeper),
        1_000
    );
    assert_eq!(s.client.keeper_stats(&keeper).active_bond, 0);
}

#[test]
fn slashing_never_exceeds_bond_and_removes_underfunded_keepers() {
    let s = setup(10, 5);
    let keeper = Address::generate(&s.env);
    let evidence = BytesN::from_array(&s.env, &[7; 32]);
    fund(&s, &keeper, 1_500);
    s.client.register_keeper(&keeper, &1_500);

    s.client.slash_keeper(&s.governor, &keeper, &300, &evidence);

    let slashed: Symbol = Symbol::new(&s.env, KEEPER_SLASHED_EVENT);
    let mut found = false;
    for (_, topics, _) in s.env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&s.env);
        if name == slashed {
            found = true;
        }
    }
    assert!(found);

    assert_eq!(s.client.keeper_stats(&keeper).slashed_amount, 300);
    assert_eq!(s.client.keeper_stats(&keeper).active_bond, 1_200);

    assert!(s
        .client
        .try_slash_keeper(&s.governor, &keeper, &1_300, &evidence)
        .is_err());
    s.client.slash_keeper(&s.governor, &keeper, &300, &evidence);
    assert_eq!(s.client.keeper_stats(&keeper).active_bond, 900);
    assert!(s.client.try_record_action(&keeper, &true).is_err());

    assert_eq!(s.client.finalize_deregistration(&keeper), 900);
    assert_eq!(
        token::TokenClient::new(&s.env, &s.token).balance(&s.treasury),
        600
    );
}

#[test]
fn rewards_are_proportional_to_successful_actions() {
    let s = setup(0, 5);
    let first = Address::generate(&s.env);
    let second = Address::generate(&s.env);
    fund(&s, &first, 1_000);
    fund(&s, &second, 1_000);
    s.client.register_keeper(&first, &1_000);
    s.client.register_keeper(&second, &1_000);
    fund(&s, &s.admin, 300);

    s.client.record_action(&first, &true);
    s.client.record_action(&first, &true);
    s.client.record_action(&second, &true);
    s.client.fund_rewards(&s.admin, &300);
    assert_eq!(s.client.distribute_rewards(), 300);

    let rewarded: Symbol = Symbol::new(&s.env, KEEPER_REWARDED_EVENT);
    let mut reward_events = 0;
    for (_, topics, _) in s.env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&s.env);
        if name == rewarded {
            reward_events += 1;
        }
    }
    assert_eq!(reward_events, 2);

    let token = token::TokenClient::new(&s.env, &s.token);
    assert_eq!(token.balance(&first), 200);
    assert_eq!(token.balance(&second), 100);
}
