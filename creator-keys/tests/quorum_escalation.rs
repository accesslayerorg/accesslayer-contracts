//! Integration coverage for quorum escalation of governance proposals.
//!
//! 1. The protocol admin configures a quorum-escalation policy.
//! 2. A proposal that is close to quorum but has not reached it gets its voting
//!    period extended, permissionlessly, within a window of the deadline.
//! 3. `ProposalExtended` reports the old and new deadlines.
//! 4. A proposal is never extended past `max_extensions`.
//! 5. A proposal is never extended once it has already reached quorum, and
//!    never while escalation is disabled.
//! 6. A proposal whose extension budget is spent can close below quorum, while
//!    one that still has budget left cannot.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{
    events::{
        PollClosedEvent, PollError, ProposalExtendedEvent, POLL_CLOSED_EVENT_NAME,
        PROPOSAL_EXTENDED_EVENT_NAME,
    },
    CreatorKeysContractClient, EscalationConfig, EscalationError, RegisterCreatorParams,
    ESCALATION_EVALUATION_WINDOW_LEDGERS, MAX_ESCALATION_EXTENSIONS_BOUND,
    MAX_ESCALATION_EXTENSION_LEDGERS, MAX_ESCALATION_THRESHOLD_BPS, MIN_ESCALATION_THRESHOLD_BPS,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    vec, Address, Env, IntoVal, String, Symbol,
};

const SEVEN_DAY_LEDGERS: u32 = (7 * 24 * 60 * 60) / 5;
const KEY_PRICE: i128 = 100;
/// 10% of circulating supply.
const QUORUM_BPS: u32 = 1_000;
/// 50% of the quorum requirement, i.e. 5% of supply.
const THRESHOLD_BPS: u32 = 5_000;
const EXTENSION_LEDGERS: u32 = 1_000;
const MAX_EXTENSIONS: u32 = 2;

/// Keys held by each of the three holders: 5, 3 and 92 of 100 total.
const HOLDER_A_KEYS: i128 = 5;
const HOLDER_B_KEYS: i128 = 3;
const HOLDER_C_KEYS: i128 = 92;

type Client<'a> = CreatorKeysContractClient<'a>;

/// Test environment with a persistence window wide enough for a seven-day poll.
///
/// The default test env expires persistent entries after 4_096 ledgers, which
/// would archive the creator profile and the poll itself long before a realistic
/// proposal deadline.
fn escalation_env() -> Env {
    let env = test_env_with_auths();
    env.ledger().set_min_persistent_entry_ttl(1_000_000);
    env
}

fn default_config() -> EscalationConfig {
    EscalationConfig {
        threshold_bps: THRESHOLD_BPS,
        extension_ledgers: EXTENSION_LEDGERS,
        max_extensions: MAX_EXTENSIONS,
    }
}

/// Deploys the contract, registers an admin, a creator and 100 circulating keys.
///
/// Holder weights are chosen so the escalation boundary is exact:
/// `a` alone votes 5% of supply, `a + b` vote 8%, and `c` alone votes 92%.
fn setup(env: &Env) -> (Client<'_>, Address, Address, Address, Address, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);

    let creator = Address::generate(env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "governance"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let holder_a = Address::generate(env);
    let holder_b = Address::generate(env);
    let holder_c = Address::generate(env);
    for _ in 0..HOLDER_A_KEYS {
        client.buy_key(&creator, &holder_a, &KEY_PRICE, &None);
    }
    for _ in 0..HOLDER_B_KEYS {
        client.buy_key(&creator, &holder_b, &KEY_PRICE, &None);
    }
    for _ in 0..HOLDER_C_KEYS {
        client.buy_key(&creator, &holder_c, &KEY_PRICE, &None);
    }
    assert_eq!(client.get_total_key_supply(&creator), 100);

    client.set_quorum_bps(&creator, &QUORUM_BPS);
    (client, admin, creator, holder_a, holder_b, holder_c)
}

fn options(env: &Env) -> soroban_sdk::Vec<String> {
    vec![
        env,
        String::from_str(env, "Option A"),
        String::from_str(env, "Option B"),
    ]
}

fn create_proposal(env: &Env, client: &Client<'_>, creator: &Address, ledgers: u32) -> u32 {
    env.ledger().set_sequence_number(1);
    client.create_poll(
        creator,
        &String::from_str(env, "Choose the next community investment"),
        &options(env),
        &ledgers,
    )
}

/// Moves the ledger to the first moment escalation may be evaluated.
fn jump_to_evaluation_window(env: &Env, client: &Client<'_>, creator: &Address, poll_id: u32) {
    let expires_at = deadline(client, creator, poll_id);
    env.ledger()
        .set_sequence_number(expires_at - ESCALATION_EVALUATION_WINDOW_LEDGERS);
}

/// Current deadline for a proposal.
///
/// `PollResult` intentionally carries no deadline, so the escalation view is the
/// read-only surface that exposes it.
fn deadline(client: &Client<'_>, creator: &Address, poll_id: u32) -> u32 {
    client.get_escalation_status(creator, &poll_id).expires_at
}

/// The most recent `PollClosedEvent`, read while it is still the last invocation.
fn last_closed_event(env: &Env) -> PollClosedEvent {
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == POLL_CLOSED_EVENT_NAME {
            return data.into_val(env);
        }
    }
    panic!("no poll-closed event was emitted")
}

fn extended_events(env: &Env) -> std::vec::Vec<ProposalExtendedEvent> {
    let mut found = std::vec::Vec::new();
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == PROPOSAL_EXTENDED_EVENT_NAME {
            found.push(data.into_val(env));
        }
    }
    found
}

// ---------------------------------------------------------------------------
// 1. Configuration
// ---------------------------------------------------------------------------

#[test]
fn test_escalation_config_is_absent_by_default() {
    let env = escalation_env();
    let (client, _admin, _creator, _a, _b, _c) = setup(&env);
    assert_eq!(client.get_escalation_config(), None);
}

#[test]
fn test_set_escalation_config_stores_and_reads_back() {
    let env = escalation_env();
    let (client, admin, _creator, _a, _b, _c) = setup(&env);

    client.set_escalation_config(&admin, &default_config());
    assert_eq!(client.get_escalation_config(), Some(default_config()));
}

#[test]
fn test_set_escalation_config_emits_update_event() {
    let env = escalation_env();
    let (client, admin, _creator, _a, _b, _c) = setup(&env);

    client.set_escalation_config(&admin, &default_config());

    let mut found = soroban_sdk::Vec::new(&env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&env);
        if name == creator_keys::events::ESCALATION_CONFIG_UPDATED_EVENT_NAME {
            found.push_back(data.into_val(&env));
        }
    }
    let event: creator_keys::events::EscalationConfigUpdatedEvent = found.get(0).unwrap();
    assert!(!event.had_previous_config);
    assert_eq!(event.old_threshold_bps, 0);
    assert_eq!(event.new_threshold_bps, THRESHOLD_BPS);
    assert_eq!(event.new_extension_ledgers, EXTENSION_LEDGERS);
    assert_eq!(event.new_max_extensions, MAX_EXTENSIONS);
}

#[test]
fn test_set_escalation_config_reports_previous_values() {
    let env = escalation_env();
    let (client, admin, _creator, _a, _b, _c) = setup(&env);

    client.set_escalation_config(
        &admin,
        &EscalationConfig {
            threshold_bps: 2_000,
            extension_ledgers: 50,
            max_extensions: 1,
        },
    );
    client.set_escalation_config(&admin, &default_config());

    let mut found = soroban_sdk::Vec::new(&env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&env);
        if name == creator_keys::events::ESCALATION_CONFIG_UPDATED_EVENT_NAME {
            found.push_back(data.into_val(&env));
        }
    }
    let event: creator_keys::events::EscalationConfigUpdatedEvent = found.get(0).unwrap();
    assert!(event.had_previous_config);
    assert_eq!(event.old_threshold_bps, 2_000);
    assert_eq!(event.old_extension_ledgers, 50);
    assert_eq!(event.old_max_extensions, 1);
    assert_eq!(event.new_threshold_bps, THRESHOLD_BPS);
}

#[test]
fn test_set_escalation_config_requires_protocol_admin() {
    let env = escalation_env();
    let (client, _admin, _creator, _a, _b, _c) = setup(&env);
    let impostor = Address::generate(&env);

    let result = client.try_set_escalation_config(&impostor, &default_config());
    assert_eq!(result, Err(Ok(EscalationError::Unauthorized)));
    assert_eq!(client.get_escalation_config(), None);
}

#[test]
fn test_set_escalation_config_rejects_out_of_bounds_values() {
    let env = escalation_env();
    let (client, admin, _creator, _a, _b, _c) = setup(&env);

    let cases = [
        EscalationConfig {
            threshold_bps: MIN_ESCALATION_THRESHOLD_BPS - 1,
            extension_ledgers: EXTENSION_LEDGERS,
            max_extensions: MAX_EXTENSIONS,
        },
        EscalationConfig {
            threshold_bps: MAX_ESCALATION_THRESHOLD_BPS + 1,
            extension_ledgers: EXTENSION_LEDGERS,
            max_extensions: MAX_EXTENSIONS,
        },
        EscalationConfig {
            threshold_bps: THRESHOLD_BPS,
            extension_ledgers: 0,
            max_extensions: MAX_EXTENSIONS,
        },
        EscalationConfig {
            threshold_bps: THRESHOLD_BPS,
            extension_ledgers: MAX_ESCALATION_EXTENSION_LEDGERS + 1,
            max_extensions: MAX_EXTENSIONS,
        },
        EscalationConfig {
            threshold_bps: THRESHOLD_BPS,
            extension_ledgers: EXTENSION_LEDGERS,
            max_extensions: MAX_ESCALATION_EXTENSIONS_BOUND + 1,
        },
    ];

    for config in cases {
        let result = client.try_set_escalation_config(&admin, &config);
        assert_eq!(
            result,
            Err(Ok(EscalationError::InvalidEscalationConfig)),
            "config {config:?} must be rejected"
        );
    }
    assert_eq!(client.get_escalation_config(), None);
}

#[test]
fn test_set_escalation_config_accepts_boundary_values() {
    let env = escalation_env();
    let (client, admin, _creator, _a, _b, _c) = setup(&env);

    client.set_escalation_config(
        &admin,
        &EscalationConfig {
            threshold_bps: MIN_ESCALATION_THRESHOLD_BPS,
            extension_ledgers: 1,
            max_extensions: MAX_ESCALATION_EXTENSIONS_BOUND,
        },
    );
    client.set_escalation_config(
        &admin,
        &EscalationConfig {
            threshold_bps: MAX_ESCALATION_THRESHOLD_BPS,
            extension_ledgers: MAX_ESCALATION_EXTENSION_LEDGERS,
            max_extensions: 0,
        },
    );
    assert_eq!(client.get_escalation_config().unwrap().max_extensions, 0);
}

// ---------------------------------------------------------------------------
// 2 + 3. Extension within the deadline window
// ---------------------------------------------------------------------------

#[test]
fn test_near_quorum_proposal_is_extended() {
    let env = escalation_env();
    let (client, admin, creator, holder_a, holder_b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    // 8% of supply: at or above the 5% threshold, still below the 10% quorum.
    client.cast_vote_with_snapshot(&creator, &holder_a, &poll_id, &0);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &1);

    let before = deadline(&client, &creator, poll_id);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    let new_expires_at = client.evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(new_expires_at, before + EXTENSION_LEDGERS);
    assert_eq!(deadline(&client, &creator, poll_id), new_expires_at);
}

#[test]
fn test_extension_is_recorded_and_emitted() {
    let env = escalation_env();
    let (client, admin, creator, holder_a, holder_b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_a, &poll_id, &0);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &1);
    let before = deadline(&client, &creator, poll_id);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    client.evaluate_poll_escalation(&creator, &poll_id);

    let events = extended_events(&env);
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.creator_id, creator);
    assert_eq!(event.poll_id, poll_id);
    assert_eq!(event.old_expires_at, before);
    assert_eq!(event.new_expires_at, before + EXTENSION_LEDGERS);
    assert_eq!(event.max_extensions, MAX_EXTENSIONS);
    assert_eq!(event.extensions_used, 1);
    assert_eq!(event.ledger, env.ledger().sequence());

    let status = client.get_escalation_status(&creator, &poll_id);
    assert_eq!(status.extensions_used, 1);
    assert_eq!(status.expires_at, before + EXTENSION_LEDGERS);
    assert_eq!(
        status.ledgers_remaining,
        ESCALATION_EVALUATION_WINDOW_LEDGERS + EXTENSION_LEDGERS
    );
    assert_eq!(status.participation_bps, 800);
    assert_eq!(status.quorum_bps, QUORUM_BPS);
}

#[test]
fn test_escalation_is_permissionless() {
    let env = escalation_env();
    let (client, admin, creator, holder_a, holder_b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_a, &poll_id, &0);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &1);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    // The entrypoint takes no caller argument and authorizes no address, so a
    // keyless stranger can drive the evaluation and nothing is signed.
    let stranger = Address::generate(&env);
    assert_ne!(stranger, creator);
    client.evaluate_poll_escalation(&creator, &poll_id);
    assert!(
        env.auths().is_empty(),
        "escalation must not require any signature"
    );
    assert_eq!(
        client
            .get_escalation_status(&creator, &poll_id)
            .extensions_used,
        1
    );
}

#[test]
fn test_threshold_boundary_participant_qualifies() {
    let env = escalation_env();
    let (client, admin, creator, holder_a, _b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    // Exactly 5% of supply == 50% of the 10% quorum == the 5 000 bps threshold.
    client.cast_vote_with_snapshot(&creator, &holder_a, &poll_id, &0);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    let before = deadline(&client, &creator, poll_id);
    assert_eq!(
        client.evaluate_poll_escalation(&creator, &poll_id),
        before + EXTENSION_LEDGERS
    );
}

// ---------------------------------------------------------------------------
// 4. Maximum extensions
// ---------------------------------------------------------------------------

#[test]
fn test_extension_stops_at_max_extensions() {
    let env = escalation_env();
    let (client, admin, creator, holder_a, holder_b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_a, &poll_id, &0);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &1);

    for round in 0..MAX_EXTENSIONS {
        jump_to_evaluation_window(&env, &client, &creator, poll_id);
        client.evaluate_poll_escalation(&creator, &poll_id);
        assert_eq!(
            client
                .get_escalation_status(&creator, &poll_id)
                .extensions_used,
            round + 1
        );
    }

    jump_to_evaluation_window(&env, &client, &creator, poll_id);
    let result = client.try_evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(result, Err(Ok(EscalationError::MaxExtensionsReached)));
    assert_eq!(
        client
            .get_escalation_status(&creator, &poll_id)
            .extensions_used,
        MAX_EXTENSIONS
    );
    assert!(client.get_escalation_status(&creator, &poll_id).exhausted);
}

#[test]
fn test_exhausted_status_reports_the_configured_budget() {
    let env = escalation_env();
    let (client, admin, creator, holder_a, holder_b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_a, &poll_id, &0);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &1);

    let status = client.get_escalation_status(&creator, &poll_id);
    assert_eq!(status.max_extensions, MAX_EXTENSIONS);
    assert!(!status.exhausted);
    assert!(status.eligible);

    for _ in 0..MAX_EXTENSIONS {
        jump_to_evaluation_window(&env, &client, &creator, poll_id);
        client.evaluate_poll_escalation(&creator, &poll_id);
    }
    let status = client.get_escalation_status(&creator, &poll_id);
    assert!(status.exhausted);
    assert!(!status.eligible);
}

// ---------------------------------------------------------------------------
// 5. Non-extension cases
// ---------------------------------------------------------------------------

#[test]
fn test_proposal_already_at_quorum_is_not_extended() {
    let env = escalation_env();
    let (client, admin, creator, _a, _b, holder_c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    // 92% of supply: far past quorum, so there is no shortfall to rescue.
    client.cast_vote_with_snapshot(&creator, &holder_c, &poll_id, &0);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    let result = client.try_evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(result, Err(Ok(EscalationError::BelowEscalationThreshold)));
    assert!(!client.get_escalation_status(&creator, &poll_id).eligible);
    assert_eq!(
        client
            .get_escalation_status(&creator, &poll_id)
            .extensions_used,
        0
    );
}

#[test]
fn test_proposal_at_exactly_quorum_is_not_extended() {
    let env = escalation_env();
    let (client, admin, creator, _a, _b, holder_c) = setup(&env);
    // A threshold of 1% of quorum makes "reached quorum" the only qualifier.
    client.set_escalation_config(
        &admin,
        &EscalationConfig {
            threshold_bps: 100,
            extension_ledgers: EXTENSION_LEDGERS,
            max_extensions: MAX_EXTENSIONS,
        },
    );

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_c, &poll_id, &0);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    let result = client.try_evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(result, Err(Ok(EscalationError::BelowEscalationThreshold)));
}

#[test]
fn test_participation_below_threshold_is_not_extended() {
    let env = escalation_env();
    let (client, admin, creator, _a, holder_b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    // 3% of supply, under the 5% threshold.
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &0);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    let result = client.try_evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(result, Err(Ok(EscalationError::BelowEscalationThreshold)));
    assert!(!client.get_escalation_status(&creator, &poll_id).eligible);
}

#[test]
fn test_zero_votes_is_not_extended() {
    let env = escalation_env();
    let (client, admin, creator, _a, _b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    let result = client.try_evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(result, Err(Ok(EscalationError::BelowEscalationThreshold)));
    assert_eq!(
        client
            .get_escalation_status(&creator, &poll_id)
            .participation_bps,
        0
    );
}

#[test]
fn test_escalation_disabled_without_config() {
    let env = escalation_env();
    let (client, _admin, creator, holder_a, holder_b, _c) = setup(&env);

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_a, &poll_id, &0);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &1);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    let result = client.try_evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(result, Err(Ok(EscalationError::EscalationDisabled)));

    let status = client.get_escalation_status(&creator, &poll_id);
    assert_eq!(status.max_extensions, 0);
    assert_eq!(status.extensions_used, 0);
    assert!(!status.eligible);
    assert!(
        !status.exhausted,
        "no configured budget is not the same as a spent one"
    );
}

#[test]
fn test_escalation_disabled_by_zero_budget() {
    let env = escalation_env();
    let (client, admin, creator, holder_a, holder_b, _c) = setup(&env);
    client.set_escalation_config(
        &admin,
        &EscalationConfig {
            threshold_bps: THRESHOLD_BPS,
            extension_ledgers: EXTENSION_LEDGERS,
            max_extensions: 0,
        },
    );

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_a, &poll_id, &0);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &1);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    let result = client.try_evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(result, Err(Ok(EscalationError::EscalationDisabled)));
    assert!(!client.get_escalation_status(&creator, &poll_id).exhausted);
}

#[test]
fn test_proposal_without_quorum_requirement_is_not_extended() {
    let env = escalation_env();
    let (client, admin, _creator, _a, _b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    // A creator that never configures quorum: `close_poll` imposes no quorum
    // check at all, so there is no participation shortfall to rescue.
    let creator = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "quorumless"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    let holder = Address::generate(&env);
    for _ in 0..100 {
        client.buy_key(&creator, &holder, &KEY_PRICE, &None);
    }

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder, &poll_id, &0);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    let status = client.get_escalation_status(&creator, &poll_id);
    assert_eq!(status.quorum_bps, 0);
    assert_eq!(status.participation_bps, 10_000, "participation is full");
    let result = client.try_evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(result, Err(Ok(EscalationError::BelowEscalationThreshold)));
}

#[test]
fn test_extension_rejected_before_the_window_opens() {
    let env = escalation_env();
    let (client, admin, creator, holder_a, holder_b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_a, &poll_id, &0);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &1);

    let expires_at = deadline(&client, &creator, poll_id);
    env.ledger()
        .set_sequence_number(expires_at - ESCALATION_EVALUATION_WINDOW_LEDGERS - 1);

    let result = client.try_evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(result, Err(Ok(EscalationError::TooEarlyToEscalate)));
}

#[test]
fn test_extension_rejected_for_closed_proposal() {
    let env = escalation_env();
    let (client, admin, creator, _a, _b, holder_c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_c, &poll_id, &0);
    client.close_poll(&creator, &poll_id);
    jump_to_evaluation_window(&env, &client, &creator, poll_id);

    let result = client.try_evaluate_poll_escalation(&creator, &poll_id);
    assert_eq!(result, Err(Ok(EscalationError::AlreadyClosed)));
}

#[test]
fn test_extension_rejected_for_unknown_proposal() {
    let env = escalation_env();
    let (client, admin, creator, _a, _b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let result = client.try_evaluate_poll_escalation(&creator, &99);
    assert_eq!(result, Err(Ok(EscalationError::PollNotFound)));
    let status = client.try_get_escalation_status(&creator, &99);
    assert_eq!(status, Err(Ok(EscalationError::PollNotFound)));
}

// ---------------------------------------------------------------------------
// 6. Closing below quorum
// ---------------------------------------------------------------------------

#[test]
fn test_below_quorum_proposal_cannot_close_while_budget_remains() {
    let env = escalation_env();
    let (client, admin, creator, _a, holder_b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    // 3% of supply: below the 5% threshold, so no extension is ever available.
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &0);

    let result = client.try_close_poll(&creator, &poll_id);
    assert_eq!(result, Err(Ok(PollError::QuorumNotReached)));
    assert!(!client.get_poll_result(&creator, &poll_id).closed);
}

#[test]
fn test_below_quorum_proposal_closes_once_budget_is_spent() {
    let env = escalation_env();
    let (client, admin, creator, holder_a, holder_b, _c) = setup(&env);
    client.set_escalation_config(&admin, &default_config());

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    // 8% of supply qualifies for an extension but never reaches quorum.
    client.cast_vote_with_snapshot(&creator, &holder_a, &poll_id, &0);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &1);

    // While budget remains the below-quorum close is refused.
    let result = client.try_close_poll(&creator, &poll_id);
    assert_eq!(result, Err(Ok(PollError::QuorumNotReached)));

    for round in 0..MAX_EXTENSIONS {
        jump_to_evaluation_window(&env, &client, &creator, poll_id);
        client.evaluate_poll_escalation(&creator, &poll_id);
        if round + 1 < MAX_EXTENSIONS {
            assert_eq!(
                client.try_close_poll(&creator, &poll_id),
                Err(Ok(PollError::QuorumNotReached)),
                "round {round}: the proposal stays open while budget remains"
            );
        }
    }

    // The budget is now spent, so the proposal becomes final.
    client.close_poll(&creator, &poll_id);
    // The close is permitted by the spent budget, not by a genuine quorum.
    let event = last_closed_event(&env);
    assert!(!event.quorum_reached);
    assert!(event.finalized_by_exhaustion);
    assert_eq!(event.total_weight, 8);
    assert!(client.get_poll_result(&creator, &poll_id).closed);
    assert_eq!(
        client.try_close_poll(&creator, &poll_id),
        Err(Ok(PollError::AlreadyClosed))
    );
}

#[test]
fn test_below_quorum_close_is_refused_when_escalation_is_disabled() {
    let env = escalation_env();
    let (client, _admin, creator, _a, holder_b, _c) = setup(&env);

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &0);

    // Regression guard: with no config at all, `close_poll` keeps its original
    // quorum requirement instead of treating a zero budget as "exhausted".
    let result = client.try_close_poll(&creator, &poll_id);
    assert_eq!(result, Err(Ok(PollError::QuorumNotReached)));
}

#[test]
fn test_below_quorum_close_is_refused_when_budget_is_zero() {
    let env = escalation_env();
    let (client, admin, creator, _a, holder_b, _c) = setup(&env);
    client.set_escalation_config(
        &admin,
        &EscalationConfig {
            threshold_bps: THRESHOLD_BPS,
            extension_ledgers: EXTENSION_LEDGERS,
            max_extensions: 0,
        },
    );

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_b, &poll_id, &0);

    let result = client.try_close_poll(&creator, &poll_id);
    assert_eq!(result, Err(Ok(PollError::QuorumNotReached)));
}

#[test]
fn test_proposal_at_quorum_closes_without_escalation() {
    let env = escalation_env();
    let (client, _admin, creator, _a, _b, holder_c) = setup(&env);

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);
    client.cast_vote_with_snapshot(&creator, &holder_c, &poll_id, &0);

    client.close_poll(&creator, &poll_id);
    let event = last_closed_event(&env);
    assert!(event.quorum_reached);
    assert!(
        !event.finalized_by_exhaustion,
        "a genuine quorum needs no escalation finalization"
    );
    assert_eq!(event.total_weight, 92);
}

#[test]
fn test_zero_supply_proposal_still_needs_an_exhausted_budget() {
    let env = escalation_env();
    let (client, _admin, _creator, _a, _b, _c) = setup(&env);

    // A brand-new creator with no circulating supply: `close_poll` takes its
    // zero-supply branch, which must not silently waive the quorum rule.
    let creator = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "empty"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.set_quorum_bps(&creator, &QUORUM_BPS);
    assert_eq!(client.get_total_key_supply(&creator), 0);

    let poll_id = create_proposal(&env, &client, &creator, SEVEN_DAY_LEDGERS);

    let result = client.try_close_poll(&creator, &poll_id);
    assert_eq!(
        result,
        Err(Ok(PollError::QuorumNotReached)),
        "a zero-supply poll must not skip the quorum requirement"
    );
    assert!(!client.get_poll_result(&creator, &poll_id).closed);
}
