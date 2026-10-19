mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{
    timed_pause::{TimedPlatformPause, TimedResumeEvent, MAX_PAUSE_SECONDS},
    ContractError, SwapDirection,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, IntoVal, String, Symbol,
};

#[test]
fn admin_pause_blocks_both_directions_and_expires_at_boundary() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 500, 500);
    client.set_protocol_fee_recipient(&admin, &Address::generate(&env));
    let key = register_test_creator(&env, &client, "timed_pause");
    let trader = Address::generate(&env);
    let other = Address::generate(&env);
    assert_eq!(
        client.try_pause_all(&other, &String::from_str(&env, "incident"), &60),
        Err(Ok(ContractError::Unauthorized))
    );
    client.pause_all(&admin, &String::from_str(&env, "incident"), &60);
    let (_, topics, data) = env.events().all().last().unwrap();
    let name: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(name, Symbol::new(&env, "all_pause"));
    let event: TimedPlatformPause = data.into_val(&env);
    assert_eq!(event.reason, String::from_str(&env, "incident"));
    assert_eq!(event.paused_at, 0);
    assert_eq!(event.resume_at, 60);
    assert_eq!(client.get_platform_pause().unwrap().resume_at, 60);
    for direction in [SwapDirection::Buy, SwapDirection::Sell] {
        assert_eq!(
            client.try_swap(&key, &trader, &1, &direction, &0),
            Err(Ok(ContractError::PlatformPaused))
        );
    }
    assert_eq!(
        client.try_buy_key(&key, &trader, &1_000, &None),
        Err(Ok(ContractError::PlatformPaused))
    );
    assert_eq!(
        client.try_sell_key(&key, &trader, &None),
        Err(Ok(ContractError::PlatformPaused))
    );
    assert_eq!(
        client.try_pause_all(&admin, &String::from_str(&env, "extend"), &120),
        Err(Ok(ContractError::PlatformPaused))
    );
    env.ledger().with_mut(|ledger| ledger.timestamp = 59);
    assert!(client.get_platform_pause().unwrap().active);
    env.ledger().with_mut(|ledger| ledger.timestamp = 60);
    assert!(!client.get_platform_pause().unwrap().active);
    client.swap(&key, &trader, &1, &SwapDirection::Buy, &0);
    let mut resumed = false;
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&env);
        if name == Symbol::new(&env, "all_resum") {
            let event: TimedResumeEvent = data.into_val(&env);
            assert!(event.automatic);
            assert_eq!(event.timestamp, 60);
            resumed = true;
        }
    }
    assert!(resumed);
}

#[test]
fn admin_can_resume_early_without_clearing_an_independent_pause() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 500, 500);
    let key = register_test_creator(&env, &client, "early_resume");
    client.pause(&admin);
    client.pause_all(&admin, &String::from_str(&env, "incident"), &120);
    assert_eq!(
        client.try_resume_all(&Address::generate(&env)),
        Err(Ok(ContractError::Unauthorized))
    );
    env.ledger().with_mut(|ledger| ledger.timestamp = 30);
    client.resume_all(&admin);
    let (_, topics, data) = env.events().all().last().unwrap();
    let name: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(name, Symbol::new(&env, "all_resum"));
    let event: TimedResumeEvent = data.into_val(&env);
    assert!(!event.automatic);
    assert_eq!(event.reason, String::from_str(&env, "incident"));
    assert_eq!(event.timestamp, 30);
    assert!(!client.get_platform_pause().unwrap().active);
    assert_eq!(
        client.try_resume_all(&admin),
        Err(Ok(ContractError::ActionNotPending))
    );
    assert_eq!(
        client.try_swap(&key, &admin, &1, &SwapDirection::Buy, &0),
        Err(Ok(ContractError::ProtocolPaused))
    );
}

#[test]
fn pause_duration_and_reason_are_bounded() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 500, 500);
    for duration in [0, MAX_PAUSE_SECONDS + 1] {
        assert_eq!(
            client.try_pause_all(&admin, &String::from_str(&env, "incident"), &duration),
            Err(Ok(ContractError::NotPositiveAmount))
        );
    }
    assert_eq!(
        client.try_pause_all(&admin, &String::from_str(&env, ""), &60),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    env.ledger()
        .with_mut(|ledger| ledger.timestamp = u64::MAX - 10);
    assert_eq!(
        client.try_pause_all(&admin, &String::from_str(&env, "incident"), &60),
        Err(Ok(ContractError::Overflow))
    );
    assert!(client.get_platform_pause().is_none());
}
