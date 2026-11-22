#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    vec, Address, BytesN, Env, IntoVal, String, Symbol,
};

fn create_test_env() -> (
    Env,
    TreasuryContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    let contract_id = env.register(TreasuryContract, ());
    let client = TreasuryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);
    let reviewer = Address::generate(&env);

    client.initialize(&admin);

    // Mock admin auth for set_reviewer_role
    env.mock_all_auths();
    client.set_reviewer_role(&reviewer);

    // Fund the treasury
    let key_id = String::from_str(&env, "key123");
    client.collect_fee(&key_id, &10000);

    (env, client, admin, recipient, reviewer)
}

fn create_milestone_input(env: &Env, amount: i128, desc: &str) -> MilestoneInput {
    let mut bytes = [0u8; 32];
    let desc_bytes = desc.as_bytes();
    for (i, b) in desc_bytes.iter().enumerate() {
        if i < 32 {
            bytes[i] = *b;
        }
    }
    MilestoneInput {
        amount,
        description_hash: BytesN::from_array(env, &bytes),
    }
}

#[test]
fn test_create_grant() {
    let (env, client, admin, recipient, _reviewer) = create_test_env();

    let milestones = vec![
        &env,
        create_milestone_input(&env, 3000, "Milestone 1"),
        create_milestone_input(&env, 7000, "Milestone 2"),
    ];

    let grant_id = client.create_grant(&admin, &recipient, &10000, &milestones, &false);

    assert_eq!(grant_id, 0);
    assert_eq!(client.get_treasury_balance(), 0);

    let status = client.grant_status(&grant_id);
    assert_eq!(status.id, 0);
    assert_eq!(status.admin, admin);
    assert_eq!(status.recipient, recipient);
    assert_eq!(status.total_amount, 10000);
    assert_eq!(status.released_amount, 0);
    assert_eq!(status.milestones.len(), 2);
    assert!(!status.cancelled);
}

#[test]
#[should_panic(expected = "MilestoneAmountMismatch")]
fn test_create_grant_mismatch_amount() {
    let (env, client, admin, recipient, _reviewer) = create_test_env();

    let milestones = vec![
        &env,
        create_milestone_input(&env, 3000, "Milestone 1"),
        create_milestone_input(&env, 5000, "Milestone 2"),
    ];

    client.create_grant(&admin, &recipient, &10000, &milestones, &false);
}

#[test]
#[should_panic(expected = "InsufficientBalance")]
fn test_create_grant_insufficient_balance() {
    let (env, client, admin, recipient, _reviewer) = create_test_env();

    let milestones = vec![&env, create_milestone_input(&env, 15000, "Milestone 1")];

    client.create_grant(&admin, &recipient, &15000, &milestones, &false);
}

#[test]
#[should_panic(expected = "InvalidMilestoneAmount")]
fn test_create_grant_zero_milestone() {
    let (env, client, admin, recipient, _reviewer) = create_test_env();

    let milestones = vec![&env, create_milestone_input(&env, 0, "Milestone 1")];

    client.create_grant(&admin, &recipient, &0, &milestones, &false);
}

#[test]
fn test_submit_and_approve_milestone() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![
        &env,
        create_milestone_input(&env, 3000, "Milestone 1"),
        create_milestone_input(&env, 7000, "Milestone 2"),
    ];

    let grant_id = client.create_grant(&admin, &recipient, &10000, &milestones, &false);

    // Submit milestone 0
    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence);

    // Approve milestone 0
    client.approve_milestone(&reviewer, &grant_id, &0);

    let status = client.grant_status(&grant_id);
    assert_eq!(status.released_amount, 3000);
    assert_eq!(
        status.milestones.get(0).unwrap().state,
        MilestoneState::Approved
    );
    assert_eq!(
        status.milestones.get(1).unwrap().state,
        MilestoneState::Pending
    );

    // Submit and approve milestone 1
    let mut evidence2 = BytesN::from_array(&env, &[0u8; 32]);
    evidence2.set(0, 2);
    client.submit_milestone(&recipient, &grant_id, &1, &evidence2);
    client.approve_milestone(&reviewer, &grant_id, &1);

    let status = client.grant_status(&grant_id);
    assert_eq!(status.released_amount, 10000);
    assert_eq!(
        status.milestones.get(1).unwrap().state,
        MilestoneState::Approved
    );
}

#[test]
fn test_reject_milestone() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![
        &env,
        create_milestone_input(&env, 3000, "Milestone 1"),
        create_milestone_input(&env, 7000, "Milestone 2"),
    ];

    let grant_id = client.create_grant(&admin, &recipient, &10000, &milestones, &false);

    // Submit milestone 0
    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence);

    // Reject milestone 0
    let mut reason = BytesN::from_array(&env, &[0u8; 32]);
    reason.set(0, 99);
    client.reject_milestone(&reviewer, &grant_id, &0, &reason);

    let status = client.grant_status(&grant_id);
    assert_eq!(status.released_amount, 0);
    assert_eq!(
        status.milestones.get(0).unwrap().state,
        MilestoneState::Rejected
    );
    assert_eq!(
        status
            .milestones
            .get(0)
            .unwrap()
            .reject_reason_hash
            .unwrap(),
        reason
    );

    // Milestone 1 can still be submitted and approved
    let mut evidence2 = BytesN::from_array(&env, &[0u8; 32]);
    evidence2.set(0, 2);
    client.submit_milestone(&recipient, &grant_id, &1, &evidence2);
    client.approve_milestone(&reviewer, &grant_id, &1);

    let status = client.grant_status(&grant_id);
    assert_eq!(status.released_amount, 7000);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_approve_milestone_unauthorized() {
    let (env, client, admin, recipient, _reviewer) = create_test_env();

    let milestones = vec![&env, create_milestone_input(&env, 3000, "Milestone 1")];

    let grant_id = client.create_grant(&admin, &recipient, &3000, &milestones, &false);

    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence);

    // Try to approve with non-reviewer
    let fake_reviewer = Address::generate(&env);
    client.approve_milestone(&fake_reviewer, &grant_id, &0);
}

#[test]
#[should_panic(expected = "MilestoneNotSubmitted")]
fn test_approve_milestone_not_submitted() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![&env, create_milestone_input(&env, 3000, "Milestone 1")];

    let grant_id = client.create_grant(&admin, &recipient, &3000, &milestones, &false);

    // Try to approve without submitting
    client.approve_milestone(&reviewer, &grant_id, &0);
}

#[test]
#[should_panic(expected = "MilestoneAlreadyApproved")]
fn test_approve_milestone_already_approved() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![&env, create_milestone_input(&env, 3000, "Milestone 1")];

    let grant_id = client.create_grant(&admin, &recipient, &3000, &milestones, &false);

    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence);
    client.approve_milestone(&reviewer, &grant_id, &0);

    // Try to approve again
    client.approve_milestone(&reviewer, &grant_id, &0);
}

#[test]
fn test_cancel_grant() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![
        &env,
        create_milestone_input(&env, 3000, "Milestone 1"),
        create_milestone_input(&env, 7000, "Milestone 2"),
    ];

    let grant_id = client.create_grant(&admin, &recipient, &10000, &milestones, &false);

    // Submit and approve milestone 0
    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence);
    client.approve_milestone(&reviewer, &grant_id, &0);

    // Cancel grant - should return unreleased 7000
    client.cancel_grant(&admin, &grant_id);

    assert_eq!(client.get_treasury_balance(), 7000);

    let status = client.grant_status(&grant_id);
    assert!(status.cancelled);
    assert_eq!(status.released_amount, 3000);
}

#[test]
#[should_panic(expected = "NothingToCancel")]
fn test_cancel_grant_nothing_to_cancel() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![&env, create_milestone_input(&env, 3000, "Milestone 1")];

    let grant_id = client.create_grant(&admin, &recipient, &3000, &milestones, &false);

    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence);
    client.approve_milestone(&reviewer, &grant_id, &0);

    // All funds released, nothing to cancel
    client.cancel_grant(&admin, &grant_id);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_cancel_grant_unauthorized() {
    let (env, client, admin, recipient, _reviewer) = create_test_env();

    let milestones = vec![
        &env,
        create_milestone_input(&env, 3000, "Milestone 1"),
        create_milestone_input(&env, 7000, "Milestone 2"),
    ];

    let grant_id = client.create_grant(&admin, &recipient, &10000, &milestones, &false);

    let fake_admin = Address::generate(&env);
    client.cancel_grant(&fake_admin, &grant_id);
}

#[test]
fn test_sequential_submission() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![
        &env,
        create_milestone_input(&env, 3000, "Milestone 1"),
        create_milestone_input(&env, 7000, "Milestone 2"),
    ];

    let grant_id = client.create_grant(&admin, &recipient, &10000, &milestones, &true);

    // Try to submit milestone 1 before milestone 0 is approved - should fail
    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 2);

    let result = client.try_submit_milestone(&recipient, &grant_id, &1, &evidence);
    assert!(result.is_err());

    // Submit milestone 0
    let mut evidence0 = BytesN::from_array(&env, &[0u8; 32]);
    evidence0.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence0);

    // Try to submit milestone 1 before milestone 0 is approved - should fail
    let result = client.try_submit_milestone(&recipient, &grant_id, &1, &evidence);
    assert!(result.is_err());

    // Approve milestone 0
    client.approve_milestone(&reviewer, &grant_id, &0);

    // Now submit milestone 1 - should succeed
    client.submit_milestone(&recipient, &grant_id, &1, &evidence);
    client.approve_milestone(&reviewer, &grant_id, &1);

    let status = client.grant_status(&grant_id);
    assert_eq!(status.released_amount, 10000);
}

#[test]
fn test_non_sequential_submission() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![
        &env,
        create_milestone_input(&env, 3000, "Milestone 1"),
        create_milestone_input(&env, 7000, "Milestone 2"),
    ];

    let grant_id = client.create_grant(&admin, &recipient, &10000, &milestones, &false);

    // Submit milestone 1 first (out of order) - should succeed with non-sequential
    let mut evidence1 = BytesN::from_array(&env, &[0u8; 32]);
    evidence1.set(0, 2);
    client.submit_milestone(&recipient, &grant_id, &1, &evidence1);
    client.approve_milestone(&reviewer, &grant_id, &1);

    // Then submit milestone 0
    let mut evidence0 = BytesN::from_array(&env, &[0u8; 32]);
    evidence0.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence0);
    client.approve_milestone(&reviewer, &grant_id, &0);

    let status = client.grant_status(&grant_id);
    assert_eq!(status.released_amount, 10000);
}

#[test]
fn test_submit_milestone_wrong_recipient() {
    let (env, client, admin, recipient, _reviewer) = create_test_env();

    let milestones = vec![&env, create_milestone_input(&env, 3000, "Milestone 1")];

    let grant_id = client.create_grant(&admin, &recipient, &3000, &milestones, &false);

    let fake_recipient = Address::generate(&env);
    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);

    let result = client.try_submit_milestone(&fake_recipient, &grant_id, &0, &evidence);
    assert!(result.is_err());
}

#[test]
fn test_grant_status_view() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![
        &env,
        create_milestone_input(&env, 3000, "Milestone 1"),
        create_milestone_input(&env, 7000, "Milestone 2"),
    ];

    let grant_id = client.create_grant(&admin, &recipient, &10000, &milestones, &false);

    let status = client.grant_status(&grant_id);
    assert_eq!(status.id, 0);
    assert_eq!(status.total_amount, 10000);
    assert_eq!(status.released_amount, 0);
    assert_eq!(status.milestones.len(), 2);
    assert!(!status.cancelled);

    // Check milestone states
    assert_eq!(
        status.milestones.get(0).unwrap().state,
        MilestoneState::Pending
    );
    assert_eq!(
        status.milestones.get(1).unwrap().state,
        MilestoneState::Pending
    );

    // Submit and approve first milestone
    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence);

    let status = client.grant_status(&grant_id);
    assert_eq!(
        status.milestones.get(0).unwrap().state,
        MilestoneState::Submitted
    );
    assert_eq!(
        status.milestones.get(0).unwrap().evidence_hash.unwrap(),
        evidence
    );

    client.approve_milestone(&reviewer, &grant_id, &0);

    let status = client.grant_status(&grant_id);
    assert_eq!(
        status.milestones.get(0).unwrap().state,
        MilestoneState::Approved
    );
    assert_eq!(status.released_amount, 3000);
}

#[test]
fn test_multiple_grants() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones1 = vec![
        &env,
        create_milestone_input(&env, 3000, "Grant 1 - Milestone 1"),
    ];
    let grant_id1 = client.create_grant(&admin, &recipient, &3000, &milestones1, &false);

    let milestones2 = vec![
        &env,
        create_milestone_input(&env, 2000, "Grant 2 - Milestone 1"),
        create_milestone_input(&env, 5000, "Grant 2 - Milestone 2"),
    ];
    let grant_id2 = client.create_grant(&admin, &recipient, &7000, &milestones2, &false);

    assert_eq!(grant_id1, 0);
    assert_eq!(grant_id2, 1);
    assert_eq!(client.get_treasury_balance(), 0);

    let status1 = client.grant_status(&grant_id1);
    assert_eq!(status1.total_amount, 3000);

    let status2 = client.grant_status(&grant_id2);
    assert_eq!(status2.total_amount, 7000);

    // Approve grant 1
    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);
    client.submit_milestone(&recipient, &grant_id1, &0, &evidence);
    client.approve_milestone(&reviewer, &grant_id1, &0);

    // Approve grant 2 milestone 0
    let mut evidence2 = BytesN::from_array(&env, &[0u8; 32]);
    evidence2.set(0, 2);
    client.submit_milestone(&recipient, &grant_id2, &0, &evidence2);
    client.approve_milestone(&reviewer, &grant_id2, &0);

    let status1 = client.grant_status(&grant_id1);
    assert_eq!(status1.released_amount, 3000);

    let status2 = client.grant_status(&grant_id2);
    assert_eq!(status2.released_amount, 2000);
}

#[test]
fn test_reject_then_resubmit_not_allowed() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![&env, create_milestone_input(&env, 3000, "Milestone 1")];

    let grant_id = client.create_grant(&admin, &recipient, &3000, &milestones, &false);

    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence);

    let mut reason = BytesN::from_array(&env, &[0u8; 32]);
    reason.set(0, 99);
    client.reject_milestone(&reviewer, &grant_id, &0, &reason);

    // Try to submit again - should fail
    let mut evidence2 = BytesN::from_array(&env, &[0u8; 32]);
    evidence2.set(0, 2);
    let result = client.try_submit_milestone(&recipient, &grant_id, &0, &evidence2);
    assert!(result.is_err());

    let status = client.grant_status(&grant_id);
    assert_eq!(
        status.milestones.get(0).unwrap().state,
        MilestoneState::Rejected
    );
}

#[test]
fn test_original_treasury_functions_still_work() {
    let (env, client, _admin, _recipient, _reviewer) = create_test_env();

    // Test original fee collection and distribution
    let key_id = String::from_str(&env, "key456");
    client.collect_fee(&key_id, &5000);
    assert_eq!(client.get_treasury_balance(), 5000);

    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    let recipients = vec![&env, recipient1.clone(), recipient2.clone()];
    let amounts = vec![&env, 3000, 1000];

    env.mock_all_auths();
    client.distribute(&recipients, &amounts);

    assert_eq!(client.get_treasury_balance(), 1000);

    let history = client.get_distribution_history(&1).unwrap();
    assert_eq!(history.epoch, 1);
    assert_eq!(history.recipients, recipients);
    assert_eq!(history.amounts, amounts);
}

#[test]
fn test_events_emitted() {
    let (env, client, admin, recipient, reviewer) = create_test_env();

    let milestones = vec![&env, create_milestone_input(&env, 3000, "Milestone 1")];

    let grant_id = client.create_grant(&admin, &recipient, &3000, &milestones, &false);

    // Check grant_created event
    let events = env.events().all();
    let grant_created_event = events
        .iter()
        .find(|e| {
            let topics = e.1.clone();
            let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
            event_name == Symbol::new(&env, "grant_created")
        })
        .expect("grant_created event not found");
    let topics = grant_created_event.1.clone();
    let event_grant_id: u32 = topics.get(1).unwrap().into_val(&env);
    assert_eq!(event_grant_id, grant_id);

    // Submit milestone
    let mut evidence = BytesN::from_array(&env, &[0u8; 32]);
    evidence.set(0, 1);
    client.submit_milestone(&recipient, &grant_id, &0, &evidence);

    // Check milestone_submitted event
    let events = env.events().all();
    let submitted_event = events
        .iter()
        .find(|e| {
            let topics = e.1.clone();
            let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
            event_name == Symbol::new(&env, "milestone_submitted")
        })
        .expect("milestone_submitted event not found");
    let topics = submitted_event.1.clone();
    let event_grant_id: u32 = topics.get(1).unwrap().into_val(&env);
    assert_eq!(event_grant_id, grant_id);

    // Approve milestone
    client.approve_milestone(&reviewer, &grant_id, &0);

    // Check milestone_approved event
    let events = env.events().all();
    let approved_event = events
        .iter()
        .find(|e| {
            let topics = e.1.clone();
            let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
            event_name == Symbol::new(&env, "milestone_approved")
        })
        .expect("milestone_approved event not found");
    let topics = approved_event.1.clone();
    let event_grant_id: u32 = topics.get(1).unwrap().into_val(&env);
    assert_eq!(event_grant_id, grant_id);

    // Cancel grant
    client.cancel_grant(&admin, &grant_id);

    // Check grant_cancelled event
    let events = env.events().all();
    let cancelled_event = events
        .iter()
        .find(|e| {
            let topics = e.1.clone();
            let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
            event_name == Symbol::new(&env, "grant_cancelled")
        })
        .expect("grant_cancelled event not found");
    let topics = cancelled_event.1.clone();
    let event_grant_id: u32 = topics.get(1).unwrap().into_val(&env);
    assert_eq!(event_grant_id, grant_id);
}
