//! Tests for #1072 (atomic swap), #1186 (key dividends), #1250 (payout
//! streams) and #1251 (oracle staleness guard).

use crate::p2p_swap_dividend_stream_oracle::*;
use crate::ContractError;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Events, Ledger},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, Symbol, TryFromVal, Val,
};

fn setup() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

fn new_token(env: &Env) -> (Address, StellarAssetClient<'_>, TokenClient<'_>) {
    let sac = env.register_stellar_asset_contract_v2(Address::generate(env));
    let addr = sac.address();
    (
        addr.clone(),
        StellarAssetClient::new(env, &addr),
        TokenClient::new(env, &addr),
    )
}

fn has_event(env: &Env, name: &str) -> bool {
    let sym = Symbol::new(env, name);
    env.events().all().iter().any(|(_, topics, _)| {
        topics
            .get(0)
            .and_then(|t| Symbol::try_from_val(env, &t).ok())
            .is_some_and(|s| s == sym)
    })
}

fn event_data<T: TryFromVal<Env, Val>>(env: &Env, name: &str) -> T {
    let sym = Symbol::new(env, name);
    env.events()
        .all()
        .iter()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|t| Symbol::try_from_val(env, &t).ok())
                .is_some_and(|s| s == sym)
        })
        .map(|(_, _, data)| T::try_from_val(env, &data).ok().unwrap())
        .unwrap()
}

// ── #1072 swap ───────────────────────────────────────────────────────────────

struct SwapFixture<'a> {
    env: &'a Env,
    swap: KeySwapContractClient<'a>,
    alice: Address,
    bob: Address,
    key_a: TokenClient<'a>,
    key_b: TokenClient<'a>,
}

fn swap_fixture(env: &Env) -> SwapFixture<'_> {
    let swap = KeySwapContractClient::new(env, &env.register(KeySwapContract, ()));
    let (_, a_admin, key_a) = new_token(env);
    let (_, b_admin, key_b) = new_token(env);
    let alice = Address::generate(env);
    let bob = Address::generate(env);
    a_admin.mint(&alice, &100);
    b_admin.mint(&bob, &50);
    SwapFixture {
        env,
        swap,
        alice,
        bob,
        key_a,
        key_b,
    }
}

fn propose(f: &SwapFixture) -> u32 {
    f.swap.propose_swap(
        &f.alice,
        &f.key_a.address,
        &40,
        &f.key_b.address,
        &20,
        &f.bob,
    )
}

#[test]
fn swap_accept_transfers_both_sides_atomically() {
    let env = setup();
    let f = swap_fixture(&env);
    let id = propose(&f);
    assert!(has_event(f.env, "SwapProposed"));
    assert_eq!(f.key_a.balance(&f.alice), 60);
    assert_eq!(f.key_a.balance(&f.swap.address), 40);

    f.swap.accept_swap(&f.bob, &id);
    let data: (Address, Address, i128, i128) = event_data(f.env, "SwapExecuted");
    assert_eq!(data, (f.alice.clone(), f.bob.clone(), 40, 20));
    assert_eq!(f.key_a.balance(&f.bob), 40);
    assert_eq!(f.key_b.balance(&f.alice), 20);
    assert_eq!(f.key_b.balance(&f.bob), 30);
    assert_eq!(f.key_a.balance(&f.swap.address), 0);
    assert_eq!(
        f.swap.try_get_swap(&id),
        Err(Ok(ContractError::ProposalNotFound))
    );
}

#[test]
fn swap_accept_restricted_to_counterparty() {
    let env = setup();
    let f = swap_fixture(&env);
    let id = propose(&f);
    let mallory = Address::generate(f.env);
    assert_eq!(
        f.swap.try_accept_swap(&mallory, &id),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn swap_cancel_releases_lock_to_proposer() {
    let env = setup();
    let f = swap_fixture(&env);
    let id = propose(&f);
    assert_eq!(
        f.swap.try_cancel_swap(&f.bob, &id),
        Err(Ok(ContractError::Unauthorized))
    );
    f.swap.cancel_swap(&f.alice, &id);
    let data: (Address, i128) = event_data(f.env, "SwapCancelled");
    assert_eq!(data, (f.alice.clone(), 40));
    assert_eq!(f.key_a.balance(&f.alice), 100);
    assert_eq!(
        f.swap.try_accept_swap(&f.bob, &id),
        Err(Ok(ContractError::ProposalNotFound))
    );
}

#[test]
fn swap_expires_after_one_hour_and_unlocks() {
    let env = setup();
    let f = swap_fixture(&env);
    let id = propose(&f);
    f.env.ledger().with_mut(|l| l.timestamp += SWAP_EXPIRY_SECS);
    assert_eq!(
        f.swap.try_accept_swap(&f.bob, &id),
        Err(Ok(ContractError::DeadlinePassed))
    );
    // Once expired, anyone can trigger the unlock; funds go to the proposer.
    let keeper = Address::generate(f.env);
    f.swap.cancel_swap(&keeper, &id);
    assert_eq!(f.key_a.balance(&f.alice), 100);
}

// ── #1186 dividends ──────────────────────────────────────────────────────────

#[test]
fn dividend_pro_rata_claim_expiry_and_reclaim() {
    let env = setup();
    let div = KeyDividendContractClient::new(&env, &env.register(KeyDividendContract, ()));
    let (key_id, key_admin, key) = new_token(&env);
    let (pay_id, pay_admin, pay) = new_token(&env);
    let creator = Address::generate(&env);
    let (h1, h2, h3, nobody) = (
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    );
    key_admin.mint(&h1, &10);
    key_admin.mint(&h2, &30);
    key_admin.mint(&h3, &60);
    pay_admin.mint(&creator, &1_000);

    let expiry = env.ledger().sequence() + 100;
    let id = div.declare_dividend(
        &creator,
        &key_id,
        &pay_id,
        &1_000,
        &vec![&env, h1.clone(), h2.clone(), h3.clone(), h1.clone()],
        &expiry,
    );
    assert_eq!(div.get_dividend(&key_id, &id).total_supply, 100);

    // Post-snapshot transfers do not change entitlements.
    key.transfer(&h3, &h1, &60);
    assert_eq!(div.get_dividend_claimable(&key_id, &id, &h1), 100);
    assert_eq!(div.get_dividend_claimable(&key_id, &id, &h3), 600);
    assert_eq!(div.get_dividend_claimable(&key_id, &id, &nobody), 0);

    assert_eq!(div.claim_dividend(&h2, &key_id, &id), 300);
    assert_eq!(pay.balance(&h2), 300);
    assert_eq!(
        div.try_claim_dividend(&h2, &key_id, &id),
        Err(Ok(ContractError::AlreadyClaimed))
    );
    assert_eq!(
        div.try_claim_dividend(&nobody, &key_id, &id),
        Err(Ok(ContractError::NoDividendClaimable))
    );
    assert_eq!(
        div.try_reclaim_expired_dividend(&key_id, &id),
        Err(Ok(ContractError::TimelockNotElapsed))
    );

    env.ledger().with_mut(|l| l.sequence_number = expiry + 1);
    assert_eq!(div.get_dividend_claimable(&key_id, &id, &h1), 0);
    assert_eq!(
        div.try_claim_dividend(&h1, &key_id, &id),
        Err(Ok(ContractError::DeadlinePassed))
    );
    assert_eq!(div.reclaim_expired_dividend(&key_id, &id), 700);
    assert_eq!(pay.balance(&creator), 700);
    assert_eq!(pay.balance(&div.address), 0);
    assert_eq!(
        div.try_reclaim_expired_dividend(&key_id, &id),
        Err(Ok(ContractError::AlreadyClaimed))
    );
}

// ── #1250 streams ────────────────────────────────────────────────────────────

#[test]
fn stream_accrual_cap_pause_and_auth() {
    let env = setup();
    let s = PayoutStreamContractClient::new(&env, &env.register(PayoutStreamContract, ()));
    let (pay_id, pay_admin, pay) = new_token(&env);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let payer = Address::generate(&env);
    pay_admin.mint(&payer, &10_000);
    s.init_stream(&admin, &pay_id);

    let id = s.create_stream(&creator, &Address::generate(&env), &10, &100);
    assert!(has_event(&env, "stream_created"));
    s.fund_stream(&payer, &id, &100);
    assert!(has_event(&env, "stream_funded"));

    let advance = |n: u32| env.ledger().with_mut(|l| l.sequence_number += n);

    // Accrual: rate * elapsed.
    advance(5);
    assert_eq!(s.claimable(&id), 50);

    // Capped by funded balance; extra funding lifts the cap without
    // resetting elapsed time.
    advance(10);
    assert_eq!(s.claimable(&id), 100);
    s.fund_stream(&payer, &id, &100);
    assert_eq!(s.stream_info(&id).cap, 200);
    assert_eq!(s.claimable(&id), 150);

    // Only the creator withdraws, and never above claimable.
    assert_eq!(
        s.try_withdraw(&payer, &id, &10),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        s.try_withdraw(&creator, &id, &151),
        Err(Ok(ContractError::InsufficientBalance))
    );
    s.withdraw(&creator, &id, &150);
    assert!(has_event(&env, "stream_withdrawn"));
    assert_eq!(pay.balance(&creator), 150);
    assert_eq!(s.claimable(&id), 0);

    // Pause freezes accrual; resume continues from the preserved balance.
    advance(2);
    assert_eq!(s.claimable(&id), 20);
    assert_eq!(
        s.try_pause_stream(&creator, &id),
        Err(Ok(ContractError::Unauthorized))
    );
    s.pause_stream(&admin, &id);
    advance(50);
    assert_eq!(s.claimable(&id), 20);
    s.resume_stream(&admin, &id);
    advance(1);
    assert_eq!(s.claimable(&id), 30);

    let info = s.stream_info(&id);
    assert_eq!(
        (info.funded, info.withdrawn, info.rate_per_ledger),
        (200, 150, 10)
    );
}

// ── #1251 oracle guard ───────────────────────────────────────────────────────

#[contract]
struct MockFeed;

#[contractimpl]
impl MockFeed {
    pub fn set(env: Env, asset: Address, price: i128, ledger: u32) {
        env.storage()
            .instance()
            .set(&asset, &PriceData { price, ledger });
    }

    pub fn price(env: Env, asset: Address) -> PriceData {
        env.storage().instance().get(&asset).unwrap()
    }
}

struct OracleFixture<'a> {
    env: &'a Env,
    guard: OracleGuardContractClient<'a>,
    primary: MockFeedClient<'a>,
    fallback: MockFeedClient<'a>,
    asset: Address,
}

fn oracle_fixture(env: &Env) -> OracleFixture<'_> {
    env.ledger().with_mut(|l| l.sequence_number = 1_000);
    let guard = OracleGuardContractClient::new(env, &env.register(OracleGuardContract, ()));
    let primary = MockFeedClient::new(env, &env.register(MockFeed, ()));
    let fallback = MockFeedClient::new(env, &env.register(MockFeed, ()));
    let admin = Address::generate(env);
    guard.init_oracle(&admin);
    // max age 10 ledgers, max deviation 5%.
    guard.set_oracle_config(
        &admin,
        &primary.address,
        &Some(fallback.address.clone()),
        &10,
        &500,
    );
    OracleFixture {
        asset: Address::generate(env),
        env,
        guard,
        primary,
        fallback,
    }
}

#[test]
fn oracle_fresh_primary_returned_unchanged() {
    let env = setup();
    let f = oracle_fixture(&env);
    f.primary.set(&f.asset, &1_000, &995);
    f.fallback.set(&f.asset, &1_040, &998);
    assert_eq!(f.guard.get_price(&f.asset), 1_000);
    let st = f.guard.oracle_status(&f.asset);
    assert_eq!(
        (st.primary_age, st.fallback_age, st.source),
        (5, Some(2), PriceSource::Primary)
    );
}

#[test]
fn oracle_stale_primary_uses_fresh_fallback() {
    let env = setup();
    let f = oracle_fixture(&env);
    f.primary.set(&f.asset, &1_000, &980);
    f.fallback.set(&f.asset, &2_000, &999);
    // Deviation is ignored when one feed is stale.
    assert_eq!(f.guard.get_price(&f.asset), 2_000);
    assert!(has_event(f.env, "oracle_fallback_used"));
    assert_eq!(
        f.guard.oracle_status(&f.asset).source,
        PriceSource::Fallback
    );
}

#[test]
fn oracle_both_stale_reverts() {
    let env = setup();
    let f = oracle_fixture(&env);
    f.primary.set(&f.asset, &1_000, &980);
    f.fallback.set(&f.asset, &1_000, &985);
    assert_eq!(
        f.guard.try_get_price(&f.asset),
        Err(Ok(ContractError::StalePrice))
    );
    assert_eq!(
        f.guard.oracle_status(&f.asset).source,
        PriceSource::Unavailable
    );
}

#[test]
fn oracle_stale_primary_without_fallback_reverts() {
    let env = setup();
    let f = oracle_fixture(&env);
    let admin = Address::generate(f.env);
    let g = OracleGuardContractClient::new(f.env, &f.env.register(OracleGuardContract, ()));
    g.init_oracle(&admin);
    g.set_oracle_config(&admin, &f.primary.address, &None, &10, &500);
    f.primary.set(&f.asset, &1_000, &980);
    assert_eq!(
        g.try_get_price(&f.asset),
        Err(Ok(ContractError::StalePrice))
    );
}

#[test]
fn oracle_large_deviation_between_fresh_feeds_reverts() {
    let env = setup();
    let f = oracle_fixture(&env);
    f.primary.set(&f.asset, &1_000, &999);
    f.fallback.set(&f.asset, &1_100, &999);
    assert_eq!(
        f.guard.try_get_price(&f.asset),
        Err(Ok(ContractError::PriceDeviation))
    );
}

#[test]
fn oracle_config_admin_only() {
    let env = setup();
    let f = oracle_fixture(&env);
    let mallory = Address::generate(f.env);
    assert_eq!(
        f.guard
            .try_set_oracle_config(&mallory, &f.primary.address, &None, &1, &1),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        f.guard.try_init_oracle(&mallory),
        Err(Ok(ContractError::AlreadyInitialized))
    );
}
