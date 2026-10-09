//! Tests for issue #1000 (emergency pause for a platform-wide trading halt).

use crate::emergency_pause::{EmergencyPauseError, PLATFORM_RESUME_DELAY_SECS};
use crate::events::{self, PlatformPausedEvent, PlatformResumedEvent};
use crate::{ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    vec, Address, Env, String, Symbol, TryIntoVal, Vec,
};

struct Setup {
    env: Env,
    client: CreatorKeysContractClient<'static>,
    admins: Vec<Address>,
    creator_a: Address,
    creator_b: Address,
    holder: Address,
}

const START_TS: u64 = 1_000_000;

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(START_TS);

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_treasury_address(&admin, &treasury);
    client.set_key_price(&admin, &100i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_protocol_fee_recipient(&admin, &treasury);

    let admins = vec![
        &env,
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    client.set_global_pause_admins(&admin, &admins);

    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);
    register(&env, &client, &creator_a, "alice");
    register(&env, &client, &creator_b, "bobby");

    // Seed a holder on both keys so sells have something to sell.
    let holder = Address::generate(&env);
    for creator in [&creator_a, &creator_b] {
        client.buy_key(creator, &holder, &1000i128, &None);
        client.buy_key(creator, &holder, &1000i128, &None);
    }
    // Step past the flash-loan guard so the holder's sells are otherwise valid.
    next_ledger(&env);

    Setup {
        env,
        client,
        admins,
        creator_a,
        creator_b,
        holder,
    }
}

fn register(env: &Env, client: &CreatorKeysContractClient, creator: &Address, handle: &str) {
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
}

/// Two distinct admins — the minimum multisig quorum.
fn quorum(s: &Setup) -> Vec<Address> {
    vec![&s.env, s.admins.get_unchecked(0), s.admins.get_unchecked(1)]
}

fn assert_buy_halted(s: &Setup, creator: &Address) {
    let buyer = Address::generate(&s.env);
    assert_eq!(
        s.client.try_buy_key(creator, &buyer, &1000i128, &None),
        Err(Ok(ContractError::GlobalTradingHalted))
    );
}

fn assert_sell_halted(s: &Setup, creator: &Address) {
    assert_eq!(
        s.client.try_sell_key(creator, &s.holder, &None),
        Err(Ok(ContractError::GlobalTradingHalted))
    );
}

fn next_ledger(env: &Env) {
    let current = env.ledger().sequence();
    env.ledger().set_sequence_number(current + 1);
}

fn assert_can_trade(s: &Setup, creator: &Address) {
    let buyer = Address::generate(&s.env);
    s.client.buy_key(creator, &buyer, &1000i128, &None);
    next_ledger(&s.env);
    s.client.sell_key(creator, &buyer, &None);
}

// ─── pause_platform halts all keys ───────────────────────────────────────

#[test]
fn pause_platform_halts_buy_and_sell_on_all_keys() {
    let s = setup();
    s.client.pause_platform(&quorum(&s));

    for creator in [&s.creator_a, &s.creator_b] {
        assert_buy_halted(&s, creator);
        assert_sell_halted(&s, creator);
        assert_eq!(
            s.client
                .try_buy_keys(creator, &s.holder, &1u32, &1000i128, &None),
            Err(Ok(ContractError::GlobalTradingHalted))
        );
    }

    let orders = vec![&s.env, (s.creator_a.clone(), 1u32)];
    assert_eq!(
        s.client.try_batch_buy(&s.holder, &orders),
        Err(Ok(ContractError::GlobalTradingHalted))
    );
    assert_eq!(
        s.client.try_batch_sell(&s.holder, &orders),
        Err(Ok(ContractError::GlobalTradingHalted))
    );
    let v2_orders = vec![&s.env, (s.creator_b.clone(), 1u32, None::<i128>)];
    assert_eq!(
        s.client.try_batch_buy_v2(&s.holder, &v2_orders),
        Err(Ok(ContractError::GlobalTradingHalted))
    );
}

#[test]
fn pause_platform_requires_every_signer_to_authorise() {
    let s = setup();
    let signers = quorum(&s);
    s.client.pause_platform(&signers);

    let auths = s.env.auths();
    for signer in signers.iter() {
        assert!(auths.iter().any(|(addr, _)| *addr == signer));
    }
}

#[test]
fn pause_platform_rejects_invalid_signer_sets() {
    let s = setup();
    let single = vec![&s.env, s.admins.get_unchecked(0)];
    assert_eq!(
        s.client.try_pause_platform(&single),
        Err(Ok(EmergencyPauseError::InsufficientSigners))
    );

    let duplicate = vec![&s.env, s.admins.get_unchecked(0), s.admins.get_unchecked(0)];
    assert_eq!(
        s.client.try_pause_platform(&duplicate),
        Err(Ok(EmergencyPauseError::DuplicateSigner))
    );

    let outsider = vec![&s.env, s.admins.get_unchecked(0), Address::generate(&s.env)];
    assert_eq!(
        s.client.try_pause_platform(&outsider),
        Err(Ok(EmergencyPauseError::Unauthorized))
    );

    assert!(!s.client.is_paused());
    assert_can_trade(&s, &s.creator_a);
}

#[test]
fn pause_platform_without_admin_set_is_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let signers = vec![&env, Address::generate(&env), Address::generate(&env)];
    assert_eq!(
        client.try_pause_platform(&signers),
        Err(Ok(EmergencyPauseError::Unauthorized))
    );
}

#[test]
fn pause_platform_twice_is_rejected() {
    let s = setup();
    s.client.pause_platform(&quorum(&s));
    assert_eq!(
        s.client.try_pause_platform(&quorum(&s)),
        Err(Ok(EmergencyPauseError::AlreadyPaused))
    );
}

// ─── PlatformPaused event ────────────────────────────────────────────────

#[test]
fn pause_platform_emits_event_with_timestamp_and_actor() {
    let s = setup();
    let signers = quorum(&s);
    s.client.pause_platform(&signers);

    let events = s.env.events().all();
    let (_contract, topics, data) = events.last().unwrap();
    let name: Symbol = topics.get(0).unwrap().try_into_val(&s.env).unwrap();
    let topic_actor: Address = topics.get(1).unwrap().try_into_val(&s.env).unwrap();
    assert_eq!(name, events::PLATFORM_PAUSED_EVENT_NAME);
    assert_eq!(topic_actor, signers.get_unchecked(0));

    let payload: PlatformPausedEvent = data.try_into_val(&s.env).unwrap();
    assert_eq!(
        payload,
        PlatformPausedEvent {
            actor: signers.get_unchecked(0),
            timestamp: START_TS,
        }
    );
}

// ─── resume_platform 24h timelock ────────────────────────────────────────

#[test]
fn resume_platform_blocked_until_timelock_elapses() {
    let s = setup();
    s.client.pause_platform(&quorum(&s));

    let eta = s.client.queue_platform_resume(&quorum(&s));
    assert_eq!(eta, START_TS + PLATFORM_RESUME_DELAY_SECS);
    assert_eq!(s.client.get_platform_resume_eta(), Some(eta));

    assert_eq!(
        s.client.try_resume_platform(&quorum(&s)),
        Err(Ok(EmergencyPauseError::TimelockNotElapsed))
    );

    s.env.ledger().set_timestamp(eta - 1);
    assert_eq!(
        s.client.try_resume_platform(&quorum(&s)),
        Err(Ok(EmergencyPauseError::TimelockNotElapsed))
    );
    assert!(s.client.is_paused());
    assert_buy_halted(&s, &s.creator_a);

    s.env.ledger().set_timestamp(eta);
    s.client.resume_platform(&quorum(&s));
    assert!(!s.client.is_paused());
    assert_eq!(s.client.get_platform_resume_eta(), None);
    assert_can_trade(&s, &s.creator_a);
    assert_can_trade(&s, &s.creator_b);
}

#[test]
fn resume_platform_emits_event_with_timestamp_and_actor() {
    let s = setup();
    s.client.pause_platform(&quorum(&s));
    let eta = s.client.queue_platform_resume(&quorum(&s));
    s.env.ledger().set_timestamp(eta);

    let signers = vec![&s.env, s.admins.get_unchecked(2), s.admins.get_unchecked(1)];
    s.client.resume_platform(&signers);

    let events = s.env.events().all();
    let (_contract, topics, data) = events.last().unwrap();
    let name: Symbol = topics.get(0).unwrap().try_into_val(&s.env).unwrap();
    assert_eq!(name, events::PLATFORM_RESUMED_EVENT_NAME);
    let payload: PlatformResumedEvent = data.try_into_val(&s.env).unwrap();
    assert_eq!(
        payload,
        PlatformResumedEvent {
            actor: s.admins.get_unchecked(2),
            timestamp: eta,
        }
    );
}

#[test]
fn resume_platform_requires_queue_and_multisig() {
    let s = setup();
    assert_eq!(
        s.client.try_queue_platform_resume(&quorum(&s)),
        Err(Ok(EmergencyPauseError::NotPaused))
    );

    s.client.pause_platform(&quorum(&s));
    assert_eq!(
        s.client.try_resume_platform(&quorum(&s)),
        Err(Ok(EmergencyPauseError::ResumeNotQueued))
    );

    let eta = s.client.queue_platform_resume(&quorum(&s));
    assert_eq!(
        s.client.try_queue_platform_resume(&quorum(&s)),
        Err(Ok(EmergencyPauseError::ResumeAlreadyQueued))
    );

    s.env.ledger().set_timestamp(eta);
    let single = vec![&s.env, s.admins.get_unchecked(0)];
    assert_eq!(
        s.client.try_resume_platform(&single),
        Err(Ok(EmergencyPauseError::InsufficientSigners))
    );
    assert!(s.client.is_paused());
}

// ─── per-key pause override ──────────────────────────────────────────────

#[test]
fn key_override_halts_only_that_key_while_platform_live() {
    let s = setup();
    s.client
        .set_key_pause_override(&quorum(&s), &s.creator_a, &true);

    assert!(!s.client.is_paused());
    assert!(s.client.is_key_paused(&s.creator_a));
    assert!(!s.client.is_key_paused(&s.creator_b));
    assert_buy_halted(&s, &s.creator_a);
    assert_sell_halted(&s, &s.creator_a);
    assert_can_trade(&s, &s.creator_b);

    s.client
        .set_key_pause_override(&quorum(&s), &s.creator_a, &false);
    assert!(!s.client.is_key_paused(&s.creator_a));
    assert_can_trade(&s, &s.creator_a);
}

#[test]
fn key_override_survives_platform_resume() {
    let s = setup();
    s.client
        .set_key_pause_override(&quorum(&s), &s.creator_a, &true);
    s.client.pause_platform(&quorum(&s));
    let eta = s.client.queue_platform_resume(&quorum(&s));
    s.env.ledger().set_timestamp(eta);
    s.client.resume_platform(&quorum(&s));

    assert!(s.client.is_key_paused(&s.creator_a));
    assert_buy_halted(&s, &s.creator_a);
    assert_can_trade(&s, &s.creator_b);
}

#[test]
fn clearing_key_override_does_not_bypass_platform_pause() {
    let s = setup();
    s.client
        .set_key_pause_override(&quorum(&s), &s.creator_a, &true);
    s.client.pause_platform(&quorum(&s));
    s.client
        .set_key_pause_override(&quorum(&s), &s.creator_a, &false);

    assert!(!s.client.is_key_paused(&s.creator_a));
    assert!(s.client.is_paused());
    assert_buy_halted(&s, &s.creator_a);
    assert_sell_halted(&s, &s.creator_a);
}

#[test]
fn key_override_requires_multisig() {
    let s = setup();
    let single = vec![&s.env, s.admins.get_unchecked(0)];
    assert_eq!(
        s.client
            .try_set_key_pause_override(&single, &s.creator_a, &true),
        Err(Ok(EmergencyPauseError::InsufficientSigners))
    );
    assert!(!s.client.is_key_paused(&s.creator_a));
}
