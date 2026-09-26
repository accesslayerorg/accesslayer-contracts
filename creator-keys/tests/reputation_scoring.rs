//! Integration coverage for on-chain creator reputation scoring.
//!
//! 1. `get_reputation` returns a zeroed view for a creator with no history and
//!    rejects an unregistered creator.
//! 2. A successful key launch increments the score.
//! 3. Trading activity and governance participation each increment the score.
//! 4. Key deprecation and governance violations decrement the score.
//! 5. Decrements saturate at the zero floor instead of going negative.
//! 6. `ReputationUpdated` is emitted with the old and new score on every change.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{
    events::{ReputationUpdatedEvent, REPUTATION_UPDATED_EVENT_NAME},
    AllowanceError, ContractError, CreatorKeysContract, CreatorKeysContractClient, CurvePreset,
    KeyMetadata, RegisterCreatorParams, ReputationError, ReputationReason,
    REPUTATION_DEPRECATION_PENALTY, REPUTATION_GOVERNANCE_PARTICIPATION_POINTS,
    REPUTATION_KEY_LAUNCH_POINTS, REPUTATION_MILESTONE_POINTS, REPUTATION_TRADE_POINTS,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    vec, Address, Env, IntoVal, String, Symbol,
};

const KEY_PRICE: i128 = 100;

fn metadata(env: &Env) -> KeyMetadata {
    KeyMetadata {
        name: String::from_str(env, "Alice Key"),
        bio: String::from_str(env, "bio"),
        avatar_uri: String::from_str(env, "ipfs://avatar"),
    }
}

fn options(env: &Env) -> soroban_sdk::Vec<String> {
    vec![env, String::from_str(env, "A"), String::from_str(env, "B")]
}

/// Register a creator with an admin-backed protocol admin address.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);
    (client, admin, Address::generate(env))
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient<'_>, handle: &str) -> Address {
    let creator = Address::generate(env);
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
    creator
}

/// Collect every `ReputationUpdated` payload emitted since the last call.
fn reputation_events(env: &Env) -> soroban_sdk::Vec<ReputationUpdatedEvent> {
    let mut found = soroban_sdk::Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == REPUTATION_UPDATED_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

// ---------------------------------------------------------------------------
// 1. Default and error cases
// ---------------------------------------------------------------------------

#[test]
fn test_get_reputation_returns_zeroed_view_without_history() {
    let env = test_env_with_auths();
    let (client, _admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    let view = client.get_reputation(&creator);
    assert_eq!(view.score, 0);
    assert_eq!(view.breakdown.key_launch_points, 0);
    assert_eq!(view.breakdown.update_count, 0);
    assert_eq!(view.key_launches, 0);
    assert_eq!(view.governance_violations, 0);
    assert!(!view.deprecated);
}

#[test]
fn test_get_reputation_rejects_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _admin, _unused) = setup(&env);
    let stranger = Address::generate(&env);

    let result = client.try_get_reputation(&stranger);
    assert_eq!(result, Err(Ok(ReputationError::NotRegistered)));
}

// ---------------------------------------------------------------------------
// 2. Positive increments
// ---------------------------------------------------------------------------

#[test]
fn test_key_launch_increments_reputation() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);

    client.register_key(
        &admin,
        &creator,
        &String::from_str(&env, "alice"),
        &metadata(&env),
        &CurvePreset::Linear,
        &0,
        &false,
    );

    let view = client.get_reputation(&creator);
    assert_eq!(view.score, REPUTATION_KEY_LAUNCH_POINTS);
    assert_eq!(view.key_launches, 1);
    assert_eq!(
        view.breakdown.key_launch_points,
        REPUTATION_KEY_LAUNCH_POINTS
    );
    assert_eq!(view.breakdown.update_count, 1);
}

#[test]
fn test_trade_increments_reputation() {
    let env = test_env_with_auths();
    let (client, _admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    assert_eq!(client.get_reputation(&creator).score, 0);

    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    let view = client.get_reputation(&creator);
    assert_eq!(view.score, REPUTATION_TRADE_POINTS);
    assert_eq!(view.breakdown.trade_points, REPUTATION_TRADE_POINTS);
    assert_eq!(view.breakdown.trades, 1);
}

#[test]
fn test_repeated_trades_accumulate_reputation() {
    let env = test_env_with_auths();
    let (client, _admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);

    for _ in 0..3 {
        client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    }

    let view = client.get_reputation(&creator);
    assert_eq!(view.score, REPUTATION_TRADE_POINTS * 3);
    assert_eq!(view.breakdown.trades, 3);
}

#[test]
fn test_governance_participation_increments_reputation() {
    let env = test_env_with_auths();
    let (client, _admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let voter = Address::generate(&env);

    client.buy_key(&creator, &voter, &KEY_PRICE, &None);
    let before = client.get_reputation(&creator).score;

    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Fund the treasury?"),
        &options(&env),
        &1_000,
    );
    client.cast_vote(&creator, &voter, &poll_id, &0);

    let view = client.get_reputation(&creator);
    assert_eq!(
        view.score,
        before + REPUTATION_GOVERNANCE_PARTICIPATION_POINTS
    );
    assert_eq!(view.governance_participations, 1);
    assert_eq!(
        view.breakdown.governance_points,
        REPUTATION_GOVERNANCE_PARTICIPATION_POINTS
    );
}

#[test]
fn test_milestone_crossing_increments_reputation_only_upward() {
    let env = test_env_with_auths();
    let (client, admin, _unused) = setup(&env);
    // A single-key milestone crossed by the first buy, then crossed downward by a sell.
    client.set_supply_milestones(&admin, &vec![&env, 1_u32]);
    let creator = register_creator(&env, &client, "alice");
    let holder = Address::generate(&env);

    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    let after_buy = client.get_reputation(&creator);
    assert_eq!(after_buy.milestones_reached, 1);
    assert_eq!(
        after_buy.breakdown.milestone_points,
        REPUTATION_MILESTONE_POINTS
    );

    let score_after_buy = after_buy.score;
    // The flash-loan guard rejects a sell in the buy ledger, so advance first.
    env.ledger().set_sequence_number(10);
    client.sell_key(&creator, &holder, &None);

    // Selling back down through the milestone must not credit the creator again.
    let after_sell = client.get_reputation(&creator);
    assert_eq!(after_sell.milestones_reached, 1);
    assert_eq!(
        after_sell.breakdown.milestone_points,
        REPUTATION_MILESTONE_POINTS
    );
    // The sell itself still earns the trade point, so the score only moves by that.
    assert_eq!(after_sell.score, score_after_buy + REPUTATION_TRADE_POINTS);
}

// ---------------------------------------------------------------------------
// 3. Negative decrements
// ---------------------------------------------------------------------------

#[test]
fn test_key_deprecation_decrements_reputation() {
    let env = test_env_with_auths();
    let (client, _admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    // Build up enough score that the 50-point penalty is observable.
    for _ in 0..20 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }

    let before = client.get_reputation(&creator).score;
    assert!(before > REPUTATION_DEPRECATION_PENALTY);
    // Escrow must cover every circulating key at the fixed buyback price.
    let buyback_price = 100_i128;
    let required_escrow = buyback_price * client.get_total_key_supply(&creator) as i128;
    client.deprecate_key(&creator, &creator, &buyback_price, &required_escrow);

    let view = client.get_reputation(&creator);
    assert_eq!(view.score, before - REPUTATION_DEPRECATION_PENALTY);
    assert_eq!(
        view.breakdown.deprecation_penalty,
        -REPUTATION_DEPRECATION_PENALTY
    );
    assert!(view.deprecated);
}

#[test]
fn test_governance_violation_decrements_reputation() {
    let env = test_env_with_auths();
    let (client, admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    for _ in 0..20 {
        client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    }

    let before = client.get_reputation(&creator).score;
    client.apply_governance_violation(&admin, &creator, &40);

    let view = client.get_reputation(&creator);
    assert_eq!(view.score, before - 40);
    assert_eq!(view.breakdown.violation_penalty, -40);
    assert_eq!(view.governance_violations, 1);
}

#[test]
fn test_governance_violation_penalty_is_capped() {
    let env = test_env_with_auths();
    let (client, admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    // A penalty far beyond the ceiling is clamped to MAX_GOVERNANCE_VIOLATION_PENALTY.
    client.apply_governance_violation(&admin, &creator, &1_000_000);

    let view = client.get_reputation(&creator);
    assert_eq!(view.score, 0, "clamped penalty cannot go below the floor");
    assert_eq!(view.breakdown.violation_penalty, -1_000);
}

#[test]
fn test_apply_governance_violation_rejects_non_admin() {
    let env = test_env_with_auths();
    let (client, _admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let impostor = Address::generate(&env);

    let result = client.try_apply_governance_violation(&impostor, &creator, &10);
    assert_eq!(result, Err(Ok(ReputationError::Unauthorized)));
}

#[test]
fn test_apply_governance_violation_rejects_non_positive_penalty() {
    let env = test_env_with_auths();
    let (client, admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    assert_eq!(
        client.try_apply_governance_violation(&admin, &creator, &0),
        Err(Ok(ReputationError::NotPositiveAmount))
    );
    assert_eq!(
        client.try_apply_governance_violation(&admin, &creator, &-5),
        Err(Ok(ReputationError::NotPositiveAmount))
    );
}

#[test]
fn test_apply_governance_violation_rejects_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, admin, _unused) = setup(&env);
    let stranger = Address::generate(&env);

    let result = client.try_apply_governance_violation(&admin, &stranger, &10);
    assert_eq!(result, Err(Ok(ReputationError::NotRegistered)));
}

// ---------------------------------------------------------------------------
// 4. Zero floor
// ---------------------------------------------------------------------------

#[test]
fn test_decrement_saturates_at_zero_floor() {
    let env = test_env_with_auths();
    let (client, admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    // A fresh creator has 0 points; a 500-point violation cannot make it negative.
    client.apply_governance_violation(&admin, &creator, &500);

    let view = client.get_reputation(&creator);
    assert_eq!(view.score, 0, "score must never go negative");
    assert_eq!(
        view.breakdown.violation_penalty, -500,
        "the applied penalty is still recorded in the breakdown"
    );
}

#[test]
fn test_repeated_violations_never_push_score_negative() {
    let env = test_env_with_auths();
    let (client, admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");

    for _ in 0..5 {
        client.apply_governance_violation(&admin, &creator, &500);
    }

    let view = client.get_reputation(&creator);
    assert_eq!(view.score, 0);
    assert_eq!(view.governance_violations, 5);
    assert_eq!(view.breakdown.violation_penalty, -2_500);
}

#[test]
fn test_deprecation_penalty_clamps_to_zero_not_negative() {
    let env = test_env_with_auths();
    let (client, _admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    client.buy_key(&creator, &holder, &KEY_PRICE, &None);

    // A trade only earned 5 points, less than the 50-point deprecation penalty.
    let before = client.get_reputation(&creator).score;
    assert!(before < REPUTATION_DEPRECATION_PENALTY);
    client.deprecate_key(&creator, &creator, &100, &KEY_PRICE);

    assert_eq!(client.get_reputation(&creator).score, 0);
}

// ---------------------------------------------------------------------------
// 5. ReputationUpdated event
// ---------------------------------------------------------------------------

#[test]
fn test_reputation_updated_event_emitted_with_old_and_new_score() {
    let env = test_env_with_auths();
    let (client, admin, creator) = setup(&env);

    client.register_key(
        &admin,
        &creator,
        &String::from_str(&env, "alice"),
        &metadata(&env),
        &CurvePreset::Linear,
        &0,
        &false,
    );

    let events = reputation_events(&env);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.creator, creator);
    assert_eq!(event.old_score, 0);
    assert_eq!(event.new_score, REPUTATION_KEY_LAUNCH_POINTS);
    assert_eq!(event.delta, REPUTATION_KEY_LAUNCH_POINTS);
    assert_eq!(event.reason, ReputationReason::KeyLaunch);
    assert_eq!(event.ledger, env.ledger().sequence());
}

#[test]
fn test_reputation_updated_event_emitted_for_violation() {
    let env = test_env_with_auths();
    let (client, admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    client.apply_governance_violation(&admin, &creator, &5);

    let events = reputation_events(&env);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.old_score, REPUTATION_TRADE_POINTS);
    assert_eq!(event.new_score, 0);
    assert_eq!(event.delta, -5);
    assert_eq!(event.reason, ReputationReason::GovernanceViolation);
}

#[test]
fn test_get_reputation_is_read_only() {
    let env = test_env_with_auths();
    let (client, _admin, _unused) = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    let before = client.get_reputation(&creator);
    // Reading repeatedly must not accrue anything.
    client.get_reputation(&creator);
    client.get_reputation(&creator);
    let after = client.get_reputation(&creator);

    assert_eq!(before, after);
    assert_eq!(after.score, REPUTATION_TRADE_POINTS);
    assert_eq!(after.breakdown.update_count, 1);
}

// ---------------------------------------------------------------------------
// 6. Cross-feature regression guard
// ---------------------------------------------------------------------------

#[test]
fn test_reputation_breakdown_reconciles_with_score() {
    let env = test_env_with_auths();
    let (client, admin, _unused) = setup(&env);
    client.set_supply_milestones(&admin, &vec![&env, 1_u32]);
    let creator = register_creator(&env, &client, "alice");
    let holder = Address::generate(&env);
    let voter = Address::generate(&env);

    client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    client.buy_key(&creator, &voter, &KEY_PRICE, &None);
    let poll_id = client.create_poll(
        &creator,
        &String::from_str(&env, "Ship it?"),
        &options(&env),
        &1_000,
    );
    client.cast_vote(&creator, &voter, &poll_id, &0);
    client.apply_governance_violation(&admin, &creator, &30);

    let view = client.get_reputation(&creator);
    let b = view.breakdown;
    let summed = b.key_launch_points
        + b.milestone_points
        + b.governance_points
        + b.trade_points
        + b.deprecation_penalty
        + b.violation_penalty;
    assert!(
        view.score == summed.max(0),
        "headline score must equal the floor-bounded sum of the breakdown"
    );
    assert_eq!(b.update_count, 5, "one update per scoring event");
}

#[test]
fn test_reputation_unaffected_by_unrelated_creator_trades() {
    let env = test_env_with_auths();
    let (client, _admin, _unused) = setup(&env);
    let alice = register_creator(&env, &client, "alice");
    let bob = register_creator(&env, &client, "bob");
    let buyer = Address::generate(&env);

    client.buy_key(&alice, &buyer, &KEY_PRICE, &None);
    client.buy_key(&bob, &buyer, &KEY_PRICE, &None);

    assert_eq!(client.get_reputation(&alice).score, REPUTATION_TRADE_POINTS);
    assert_eq!(client.get_reputation(&bob).score, REPUTATION_TRADE_POINTS);
}

// Silence unused-import lints for symbols referenced only in doc comments.
#[allow(dead_code)]
fn _type_anchors(_: AllowanceError, _: ContractError) {}

#[test]
fn test_contract_is_registered_for_client_generics() {
    // Guards the `CreatorKeysContract` import used by the client type alias above.
    let env = test_env_with_auths();
    let _ = env.register(CreatorKeysContract, ());
}
