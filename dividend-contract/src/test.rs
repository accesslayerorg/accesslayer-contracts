//! Tests for issue #1073 — snapshot-based dividend distribution for creator
//! revenue sharing.
//!
//! The suite covers every acceptance criterion from the issue:
//! - snapshots record correct holder balances at the time of the call,
//! - pro-rata claims are computed correctly from snapshot balances,
//! - purchases made after the snapshot do not affect snapshot entitlements,
//! - `claim_dividend` reverts on double-claim with `AlreadyClaimed`,
//! - `DividendSnapshotCreated` and `DividendClaimed` events are emitted with
//!   correct snapshot and amount data.

#![cfg(test)]

use crate::{
    DividendDistributionContract, DividendDistributionContractClient, DividendError,
    DividendSnapshotCreatedEvent, DIVIDEND_CLAIMED_EVENT, DIVIDEND_SNAPSHOT_CREATED_EVENT,
    MAX_SNAPSHOT_HOLDERS,
};
use creator_keys::{
    CreatorKeysContract, CreatorKeysContractClient, CurvePreset, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, IntoVal, String, Symbol, TryFromVal, Val, Vec,
};

/// Flat base price used by every test so buy costs stay predictable
/// (`qty` keys cost exactly `qty * KEY_PRICE`).
const KEY_PRICE: i128 = 100;

struct Fixture {
    env: Env,
    keys: CreatorKeysContractClient<'static>,
    dividend: DividendDistributionContractClient<'static>,
    admin: Address,
}

/// Registers a fully configured `creator-keys` contract plus an initialized
/// dividend distribution contract pointing at it.
fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();

    let keys_contract_id = env.register(CreatorKeysContract, ());
    let keys = CreatorKeysContractClient::new(&env, &keys_contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    keys.set_protocol_admin(&admin, &admin);
    keys.set_treasury_address(&admin, &treasury);
    keys.set_key_price(&admin, &KEY_PRICE);
    keys.set_fee_config(&admin, &9000u32, &1000u32);
    keys.set_protocol_fee_recipient(&admin, &treasury);

    let dividend_contract_id = env.register(DividendDistributionContract, ());
    let dividend = DividendDistributionContractClient::new(&env, &dividend_contract_id);
    dividend.initialize(&admin, &keys_contract_id);

    Fixture {
        env,
        keys,
        dividend,
        admin,
    }
}

fn register_creator(f: &Fixture, creator: &Address, handle: &str) {
    f.keys.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&f.env, handle),
        },
        &None,
        &None,
        &None,
        // Flat curve keeps buy costs exactly `qty * KEY_PRICE`.
        &Some(CurvePreset::Flat),
        &None,
        &None,
    );
}

/// Buys `qty` keys for `buyer` on `creator` with a generous payment buffer.
fn buy_keys(f: &Fixture, creator: &Address, buyer: &Address, qty: u32) {
    let payment = KEY_PRICE * qty as i128 + 1_000;
    f.keys.buy_keys(creator, buyer, &qty, &payment, &None);
}

/// Filters all recorded events down to those whose first topic is `symbol`.
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

// ─── Acceptance: snapshot records correct holder balances at time of call ───

#[test]
fn test_snapshot_records_correct_holder_balances() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");

    let holder_a = Address::generate(&f.env);
    let holder_b = Address::generate(&f.env);
    let holder_c = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder_a, 50);
    buy_keys(&f, &creator, &holder_b, 30);
    buy_keys(&f, &creator, &holder_c, 20);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);
    assert_eq!(snapshot_id, 1);

    let snapshot = f.dividend.get_snapshot(&snapshot_id);
    assert_eq!(snapshot.snapshot_id, snapshot_id);
    assert_eq!(snapshot.key_id, creator);
    assert_eq!(snapshot.creator, creator);
    assert_eq!(snapshot.total_supply, 100);
    assert_eq!(snapshot.holder_count, 3);
    assert_eq!(snapshot.funded_amount, 0);
    assert_eq!(snapshot.total_claimed, 0);

    // Per-holder balances are frozen at their values when the call happened.
    assert_eq!(
        f.dividend
            .get_snapshot_holder_balance(&snapshot_id, &holder_a),
        50
    );
    assert_eq!(
        f.dividend
            .get_snapshot_holder_balance(&snapshot_id, &holder_b),
        30
    );
    assert_eq!(
        f.dividend
            .get_snapshot_holder_balance(&snapshot_id, &holder_c),
        20
    );

    // The holders list records exactly the snapshotted wallets.
    let holders: Vec<Address> = f.dividend.get_snapshot_holders(&snapshot_id);
    assert_eq!(holders.len(), 3);
    assert!(holders.contains(&holder_a));
    assert!(holders.contains(&holder_b));
    assert!(holders.contains(&holder_c));

    assert_eq!(f.dividend.get_latest_snapshot_id(&creator), snapshot_id);
}

#[test]
fn test_snapshot_records_ledger_at_call_time() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 10);

    let mut ledger_info = f.env.ledger().get();
    ledger_info.sequence_number = 500;
    f.env.ledger().set(ledger_info.clone());

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);
    let snapshot = f.dividend.get_snapshot(&snapshot_id);
    assert_eq!(snapshot.snapshot_ledger, 500);
    f.dividend.fund_dividend(&creator, &creator, &1_000);

    // Later ledger activity never rewrites the recorded snapshot ledger.
    let mut ledger_info = f.env.ledger().get();
    ledger_info.sequence_number = 900;
    f.env.ledger().set(ledger_info);
    buy_keys(&f, &creator, &holder, 5);
    f.dividend.claim_dividend(&holder, &snapshot_id);

    let snapshot = f.dividend.get_snapshot(&snapshot_id);
    assert_eq!(snapshot.snapshot_ledger, 500);
}

#[test]
fn test_snapshot_on_key_without_holders_reverts() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");

    assert_eq!(
        f.dividend.try_snapshot_holders(&creator, &creator),
        Err(Ok(DividendError::NoKeyHolders))
    );
}

#[test]
fn test_snapshot_requires_creator_auth() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 10);

    // Clear mocks so require_auth is enforced.
    f.env.mock_auths(&[]);

    let stranger = Address::generate(&f.env);
    let result = f.dividend.try_snapshot_holders(&stranger, &creator);
    assert!(result.is_err());
}

#[test]
fn test_snapshot_requires_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let keys_contract_id = env.register(CreatorKeysContract, ());
    let dividend_contract_id = env.register(DividendDistributionContract, ());
    let dividend = DividendDistributionContractClient::new(&env, &dividend_contract_id);

    let creator = Address::generate(&env);
    assert_eq!(
        dividend.try_snapshot_holders(&creator, &creator),
        Err(Ok(DividendError::NotInitialized))
    );
    let _ = keys_contract_id;
}

#[test]
fn test_initialize_rejects_second_call() {
    let f = setup();
    let other = Address::generate(&f.env);
    assert_eq!(
        f.dividend.try_initialize(&f.admin.clone(), &other),
        Err(Ok(DividendError::AlreadyInitialized))
    );
}

// ─── Acceptance: pro-rata claim computed correctly from snapshot balances ───

#[test]
fn test_claim_pays_pro_rata_share_of_funded_pool() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder_a = Address::generate(&f.env);
    let holder_b = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder_a, 90);
    buy_keys(&f, &creator, &holder_b, 10);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);
    f.dividend.fund_dividend(&creator, &creator, &1_000);

    // 90/10 holdings -> 900/100 of a 1_000 pool.
    let claim_a = f.dividend.claim_dividend(&holder_a, &snapshot_id);
    let claim_b = f.dividend.claim_dividend(&holder_b, &snapshot_id);
    assert_eq!(claim_a, 900);
    assert_eq!(claim_b, 100);
    assert_eq!(claim_a + claim_b, 1_000);

    // The settled amounts and accounting totals match the claims.
    assert_eq!(f.dividend.get_paid_amount(&snapshot_id, &holder_a), 900);
    assert_eq!(f.dividend.get_paid_amount(&snapshot_id, &holder_b), 100);
    let snapshot = f.dividend.get_snapshot(&snapshot_id);
    assert_eq!(snapshot.total_claimed, 1_000);
    assert_eq!(f.dividend.get_total_claimed(), 1_000);
}

#[test]
fn test_get_unclaimed_dividend_reports_claimable_amount() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 25);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);

    // Unfunded snapshot -> nothing claimable yet.
    assert_eq!(f.dividend.get_unclaimed_dividend(&holder, &snapshot_id), 0);

    f.dividend.fund_dividend(&creator, &creator, &1_000);
    assert_eq!(
        f.dividend.get_unclaimed_dividend(&holder, &snapshot_id),
        1_000
    );

    f.dividend.claim_dividend(&holder, &snapshot_id);
    assert_eq!(f.dividend.get_unclaimed_dividend(&holder, &snapshot_id), 0);
}

#[test]
fn test_fund_dividend_accumulates_and_reports_totals() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 10);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);

    let funded = f.dividend.fund_dividend(&creator, &creator, &600);
    assert_eq!(funded, 600);
    let funded = f.dividend.fund_dividend(&creator, &creator, &400);
    assert_eq!(funded, 1_000);

    assert_eq!(f.dividend.get_total_funded(), 1_000);
    let snapshot = f.dividend.get_snapshot(&snapshot_id);
    assert_eq!(snapshot.funded_amount, 1_000);

    // The whole pool is claimable by the sole holder.
    assert_eq!(
        f.dividend.get_unclaimed_dividend(&holder, &snapshot_id),
        1_000
    );
}

#[test]
fn test_fund_dividend_requires_snapshot() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");

    assert_eq!(
        f.dividend.try_fund_dividend(&creator, &creator, &1_000),
        Err(Ok(DividendError::SnapshotNotFound))
    );
}

#[test]
fn test_fund_dividend_rejects_non_creator() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 10);
    f.dividend.snapshot_holders(&creator, &creator);

    let stranger = Address::generate(&f.env);
    assert_eq!(
        f.dividend.try_fund_dividend(&stranger, &creator, &1_000),
        Err(Ok(DividendError::Unauthorized))
    );
}

#[test]
fn test_fund_dividend_rejects_non_positive_amount() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 10);
    f.dividend.snapshot_holders(&creator, &creator);

    assert_eq!(
        f.dividend.try_fund_dividend(&creator, &creator, &0),
        Err(Ok(DividendError::NotPositiveAmount))
    );
    assert_eq!(
        f.dividend.try_fund_dividend(&creator, &creator, &-5),
        Err(Ok(DividendError::NotPositiveAmount))
    );
}

// ─── Acceptance: post-snapshot purchases do not affect entitlements ───

#[test]
fn test_post_snapshot_purchases_do_not_affect_entitlements() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder_a = Address::generate(&f.env);
    let holder_b = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder_a, 90);
    buy_keys(&f, &creator, &holder_b, 10);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);

    // A flash purchaser buys a large position *after* the snapshot…
    let flash_purchaser = Address::generate(&f.env);
    buy_keys(&f, &creator, &flash_purchaser, 90);

    f.dividend.fund_dividend(&creator, &creator, &1_000);

    // …and gets nothing: entitlements come from snapshot balances (90/10),
    // not current holdings (90/10/90).
    assert_eq!(
        f.dividend
            .get_unclaimed_dividend(&flash_purchaser, &snapshot_id),
        0
    );
    assert_eq!(
        f.dividend.get_unclaimed_dividend(&holder_a, &snapshot_id),
        900
    );
    assert_eq!(
        f.dividend.get_unclaimed_dividend(&holder_b, &snapshot_id),
        100
    );

    assert_eq!(
        f.dividend
            .try_claim_dividend(&flash_purchaser, &snapshot_id),
        Err(Ok(DividendError::NothingToClaim))
    );

    assert_eq!(f.dividend.claim_dividend(&holder_a, &snapshot_id), 900);
    assert_eq!(f.dividend.claim_dividend(&holder_b, &snapshot_id), 100);
}

#[test]
fn test_selling_after_snapshot_keeps_entitlement() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    let buyer = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 20);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);

    // The holder dumps their whole position after the snapshot. The snapshot
    // already froze their balance, so their claim is unaffected. Sells move to
    // the next ledger first: `creator-keys` rejects same-ledger buys-and-sells
    // as a flash-loan guard.
    f.env.ledger().with_mut(|l| l.sequence_number += 1);
    for _ in 0..20 {
        f.keys.sell_key(&creator, &holder, &None);
    }
    assert_eq!(f.keys.get_key_balance(&creator, &holder), 0);

    f.dividend.fund_dividend(&creator, &creator, &500);
    assert_eq!(
        f.dividend.get_unclaimed_dividend(&holder, &snapshot_id),
        500
    );
    assert_eq!(f.dividend.claim_dividend(&holder, &snapshot_id), 500);

    // A wallet that bought after the snapshot owns keys now but has no claim.
    buy_keys(&f, &creator, &buyer, 20);
    assert_eq!(
        f.dividend.try_claim_dividend(&buyer, &snapshot_id),
        Err(Ok(DividendError::NothingToClaim))
    );
}

#[test]
fn test_multiple_snapshots_are_independent() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder_a = Address::generate(&f.env);
    let holder_b = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder_a, 90);
    buy_keys(&f, &creator, &holder_b, 10);

    let first = f.dividend.snapshot_holders(&creator, &creator);

    let holder_c = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder_c, 90);
    let second = f.dividend.snapshot_holders(&creator, &creator);
    assert_eq!(second, 2);

    // The second snapshot reflects the new holder and the grown supply.
    let snapshot = f.dividend.get_snapshot(&second);
    assert_eq!(snapshot.total_supply, 190);
    assert_eq!(snapshot.holder_count, 3);
    assert_eq!(
        f.dividend.get_snapshot_holder_balance(&second, &holder_c),
        90
    );

    // Funding targets the latest snapshot for the key.
    f.dividend.fund_dividend(&creator, &creator, &1_900);
    assert_eq!(f.dividend.get_snapshot(&second).funded_amount, 1_900);
    assert_eq!(f.dividend.get_snapshot(&first).funded_amount, 0);

    // Entitlements in the first snapshot are untouched by the second round.
    assert_eq!(f.dividend.get_unclaimed_dividend(&holder_a, &first), 0);
    assert_eq!(f.dividend.get_snapshot_holder_balance(&first, &holder_c), 0);

    // Pro-rata in the second snapshot: 90/10/90 of 1_900.
    assert_eq!(f.dividend.claim_dividend(&holder_a, &second), 900);
    assert_eq!(f.dividend.claim_dividend(&holder_b, &second), 100);
    assert_eq!(f.dividend.claim_dividend(&holder_c, &second), 900);
}

// ─── Acceptance: double-claim reverts with AlreadyClaimed ───

#[test]
fn test_double_claim_reverts_with_already_claimed() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 10);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);
    f.dividend.fund_dividend(&creator, &creator, &1_000);

    assert_eq!(f.dividend.claim_dividend(&holder, &snapshot_id), 1_000);
    assert!(f.dividend.has_claimed(&snapshot_id, &holder));

    assert_eq!(
        f.dividend.try_claim_dividend(&holder, &snapshot_id),
        Err(Ok(DividendError::AlreadyClaimed))
    );

    // The failed second claim paid nothing: totals still reflect one claim.
    assert_eq!(f.dividend.get_paid_amount(&snapshot_id, &holder), 1_000);
    assert_eq!(f.dividend.get_total_claimed(), 1_000);
    assert_eq!(f.dividend.get_snapshot(&snapshot_id).total_claimed, 1_000);
}

#[test]
fn test_claim_unknown_snapshot_reverts() {
    let f = setup();
    let holder = Address::generate(&f.env);
    assert_eq!(
        f.dividend.try_claim_dividend(&holder, &42),
        Err(Ok(DividendError::SnapshotNotFound))
    );
}

#[test]
fn test_claim_reverts_for_non_holder() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 10);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);
    f.dividend.fund_dividend(&creator, &creator, &1_000);

    let outsider = Address::generate(&f.env);
    assert_eq!(
        f.dividend.try_claim_dividend(&outsider, &snapshot_id),
        Err(Ok(DividendError::NothingToClaim))
    );
}

#[test]
fn test_claim_reverts_on_unfunded_snapshot() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 10);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);

    assert_eq!(
        f.dividend.try_claim_dividend(&holder, &snapshot_id),
        Err(Ok(DividendError::NothingToClaim))
    );
}

// ─── Acceptance: both events emitted with correct snapshot and amount data ───

#[test]
fn test_dividend_snapshot_created_event_emitted_with_correct_data() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder_a = Address::generate(&f.env);
    let holder_b = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder_a, 70);
    buy_keys(&f, &creator, &holder_b, 30);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);
    let events: Vec<DividendSnapshotCreatedEvent> =
        get_events(&f.env, DIVIDEND_SNAPSHOT_CREATED_EVENT);

    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.snapshot_id, snapshot_id);
    assert_eq!(event.key_id, creator);
    assert_eq!(event.creator, creator);
    assert_eq!(event.total_supply, 100);
    assert_eq!(event.holder_count, 2);
    assert_eq!(event.snapshot_ledger, f.env.ledger().get().sequence_number);
}

#[test]
fn test_dividend_claimed_event_emitted_with_correct_amount() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");
    let holder = Address::generate(&f.env);
    buy_keys(&f, &creator, &holder, 25);

    let snapshot_id = f.dividend.snapshot_holders(&creator, &creator);
    f.dividend.fund_dividend(&creator, &creator, &800);

    f.dividend.claim_dividend(&holder, &snapshot_id);
    let events: Vec<crate::DividendClaimedEvent> = get_events(&f.env, DIVIDEND_CLAIMED_EVENT);

    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.snapshot_id, snapshot_id);
    assert_eq!(event.holder, holder);
    assert_eq!(event.amount, 800);
}

// ─── Additional guards ───

#[test]
fn test_snapshot_exceeding_holder_limit_reverts() {
    let f = setup();
    let creator = Address::generate(&f.env);
    register_creator(&f, &creator, "alice");

    // Register more holders than the snapshot cap allows.
    for _ in 0..=MAX_SNAPSHOT_HOLDERS {
        let holder = Address::generate(&f.env);
        buy_keys(&f, &creator, &holder, 1);
    }

    assert_eq!(
        f.dividend.try_snapshot_holders(&creator, &creator),
        Err(Ok(DividendError::HolderLimitExceeded))
    );
}

#[test]
fn test_views_return_defaults_for_unknown_snapshot() {
    let f = setup();
    let wallet = Address::generate(&f.env);

    assert_eq!(f.dividend.get_unclaimed_dividend(&wallet, &7), 0);
    assert_eq!(f.dividend.get_snapshot_holder_balance(&7, &wallet), 0);
    assert!(!f.dividend.has_claimed(&7, &wallet));
    assert_eq!(f.dividend.get_paid_amount(&7, &wallet), 0);
    assert_eq!(
        f.dividend.try_get_snapshot(&7),
        Err(Ok(DividendError::SnapshotNotFound))
    );
    assert_eq!(
        f.dividend.try_get_snapshot_holders(&7),
        Err(Ok(DividendError::SnapshotNotFound))
    );
}
