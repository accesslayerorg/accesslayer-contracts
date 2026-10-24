#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, Symbol, Vec,
};

fn setup_test() -> (Env, AutoInvestRegistryContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AutoInvestRegistryContract, ());
    let client = AutoInvestRegistryContractClient::new(&env, &contract_id);
    let wallet = Address::generate(&env);
    (env, client, wallet)
}

fn valid_criteria() -> Criteria {
    Criteria {
        key_category: 1,
        min_rating: 400, // 4.00 stars
        max_price: 10_000,
        max_per_key: 5,
    }
}

fn rule_registered_events(env: &Env) -> Vec<RuleRegistered> {
    let mut matching = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            let sym: Symbol = first.into_val(env);
            if sym == RULE_REGISTERED_EVENT_NAME {
                let event: RuleRegistered = data.into_val(env);
                matching.push_back(event);
            }
        }
    }
    matching
}

fn rule_updated_events(env: &Env) -> Vec<RuleUpdated> {
    let mut matching = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            let sym: Symbol = first.into_val(env);
            if sym == RULE_UPDATED_EVENT_NAME {
                let event: RuleUpdated = data.into_val(env);
                matching.push_back(event);
            }
        }
    }
    matching
}

// ============================================================================
// Acceptance Criterion 1: Rule is registered correctly with all criteria fields
// ============================================================================

#[test]
fn test_register_rule_success_and_fields() {
    let (env, client, wallet) = setup_test();
    env.ledger().set_timestamp(1_000);

    let criteria = valid_criteria();
    let rule_id = client.register_rule(&wallet, &criteria);
    assert_eq!(rule_id, 1);

    let stored = client.get_rule(&rule_id);
    assert_eq!(stored.id, 1);
    assert_eq!(stored.owner, wallet);
    assert_eq!(stored.criteria, criteria);
    assert!(stored.active);
    assert_eq!(stored.created_at, 1_000);

    // Second registration increments rule ID
    let rule_id_2 = client.register_rule(&wallet, &criteria);
    assert_eq!(rule_id_2, 2);
}

// ============================================================================
// Acceptance Criterion 2: check_rule returns correct eligibility against all criteria
// ============================================================================

#[test]
fn test_check_rule_with_context_eligibility_all_criteria() {
    let (env, client, wallet) = setup_test();
    let key_id = Address::generate(&env);

    let criteria = Criteria {
        key_category: 2,
        min_rating: 350,
        max_price: 5_000,
        max_per_key: 3,
    };
    client.register_rule(&wallet, &criteria);

    // Matching key: eligible
    let result = client.check_rule_with_context(&wallet, &key_id, &2, &400, &4_000, &1);
    assert!(result.eligible);
    assert_eq!(result.rule_id.unwrap(), 1);
}

#[test]
fn test_check_rule_delegates_to_context() {
    let (env, client, wallet) = setup_test();
    let key_id = Address::generate(&env);

    let criteria = Criteria {
        key_category: 2,
        min_rating: 350,
        max_price: 5_000,
        max_per_key: 3,
    };
    client.register_rule(&wallet, &criteria);

    // check_rule with matching arguments delegates to check_rule_with_context
    let res = client.check_rule(&wallet, &key_id, &2, &400, &4_000, &1);
    assert!(res.eligible);
    assert_eq!(res.rule_id.unwrap(), 1);

    // check_rule with mismatching arguments returns ineligible
    let res_mismatch = client.check_rule(&wallet, &key_id, &2, &300, &4_000, &1);
    assert_eq!(
        res_mismatch,
        CheckResult {
            eligible: false,
            rule_id: None,
        }
    );
}

#[test]
fn test_check_rule_criteria_mismatches() {
    let (env, client, wallet) = setup_test();
    let key_id = Address::generate(&env);

    let criteria = Criteria {
        key_category: 2,
        min_rating: 350,
        max_price: 5_000,
        max_per_key: 3,
    };
    client.register_rule(&wallet, &criteria);

    // 1. Category mismatch
    let res = client.check_rule_with_context(&wallet, &key_id, &1, &400, &4_000, &1);
    assert_eq!(
        res,
        CheckResult {
            eligible: false,
            rule_id: None,
        }
    );

    // 2. Rating too low
    let res = client.check_rule_with_context(&wallet, &key_id, &2, &349, &4_000, &1);
    assert_eq!(
        res,
        CheckResult {
            eligible: false,
            rule_id: None,
        }
    );

    // 3. Price too high
    let res = client.check_rule_with_context(&wallet, &key_id, &2, &400, &5_001, &1);
    assert_eq!(
        res,
        CheckResult {
            eligible: false,
            rule_id: None,
        }
    );

    // 4. Holdings at max
    let res = client.check_rule_with_context(&wallet, &key_id, &2, &400, &4_000, &3);
    assert_eq!(
        res,
        CheckResult {
            eligible: false,
            rule_id: None,
        }
    );

    // 5. Holdings exceed max
    let res = client.check_rule_with_context(&wallet, &key_id, &2, &400, &4_000, &4);
    assert_eq!(
        res,
        CheckResult {
            eligible: false,
            rule_id: None,
        }
    );
}

#[test]
fn test_non_wildcard_category_behavior() {
    let (env, client, wallet) = setup_test();
    let key_id = Address::generate(&env);

    // Rule specifies non-wildcard category 5
    let criteria = Criteria {
        key_category: 5,
        min_rating: 300,
        max_price: 10_000,
        max_per_key: 5,
    };
    client.register_rule(&wallet, &criteria);

    // Exact category match (5 == 5) -> eligible
    let res_match = client.check_rule_with_context(&wallet, &key_id, &5, &350, &5_000, &0);
    assert!(res_match.eligible);

    // Different category (3 != 5) -> ineligible
    let res_diff = client.check_rule_with_context(&wallet, &key_id, &3, &350, &5_000, &0);
    assert_eq!(
        res_diff,
        CheckResult {
            eligible: false,
            rule_id: None,
        }
    );

    // Caller supplying category 0 against a non-wildcard rule (0 != 5) -> ineligible
    let res_zero = client.check_rule_with_context(&wallet, &key_id, &0, &350, &5_000, &0);
    assert_eq!(
        res_zero,
        CheckResult {
            eligible: false,
            rule_id: None,
        }
    );
}

#[test]
fn test_check_rule_boundary_values() {
    let (env, client, wallet) = setup_test();
    let key_id = Address::generate(&env);

    let criteria = Criteria {
        key_category: 2,
        min_rating: 400,
        max_price: 5_000,
        max_per_key: 3,
    };
    client.register_rule(&wallet, &criteria);

    // Exact min rating boundary (400) -> eligible
    let res = client.check_rule_with_context(&wallet, &key_id, &2, &400, &3_000, &0);
    assert!(res.eligible);

    // Exact max price boundary (5_000) -> eligible
    let res = client.check_rule_with_context(&wallet, &key_id, &2, &450, &5_000, &0);
    assert!(res.eligible);

    // Current holdings one below max (2 < 3) -> eligible
    let res = client.check_rule_with_context(&wallet, &key_id, &2, &450, &3_000, &2);
    assert!(res.eligible);

    // Current holdings equal to max (3 < 3 is false) -> ineligible
    let res = client.check_rule_with_context(&wallet, &key_id, &2, &450, &3_000, &3);
    assert!(!res.eligible);
}

#[test]
fn test_check_rule_wildcard_category() {
    let (env, client, wallet) = setup_test();
    let key_id = Address::generate(&env);

    // Category 0 acts as a wildcard matching any category
    let criteria = Criteria {
        key_category: 0,
        min_rating: 300,
        max_price: 10_000,
        max_per_key: 5,
    };
    client.register_rule(&wallet, &criteria);

    assert!(
        client
            .check_rule_with_context(&wallet, &key_id, &1, &350, &5_000, &0)
            .eligible
    );
    assert!(
        client
            .check_rule_with_context(&wallet, &key_id, &99, &350, &5_000, &0)
            .eligible
    );
    assert!(
        client
            .check_rule_with_context(&wallet, &key_id, &1234, &350, &5_000, &0)
            .eligible
    );
}

// ============================================================================
// Acceptance Criterion 3: Deactivated rules are excluded from check_rule evaluation
// ============================================================================

#[test]
fn test_deactivated_rules_excluded_from_check_rule() {
    let (env, client, wallet) = setup_test();
    let key_id = Address::generate(&env);

    let criteria = valid_criteria();
    let rule_id = client.register_rule(&wallet, &criteria);

    // Initially matches
    let res1 = client.check_rule_with_context(&wallet, &key_id, &1, &450, &5_000, &0);
    assert!(res1.eligible);

    // Deactivate rule
    client.deactivate_rule(&rule_id);

    let stored = client.get_rule(&rule_id);
    assert!(!stored.active);

    // Excluded from check_rule evaluation
    let res2 = client.check_rule_with_context(&wallet, &key_id, &1, &450, &5_000, &0);
    assert_eq!(
        res2,
        CheckResult {
            eligible: false,
            rule_id: None,
        }
    );
}

#[test]
fn test_check_rule_falls_through_deactivated_rule_to_next() {
    let (env, client, wallet) = setup_test();
    let key_id = Address::generate(&env);

    // Rule 1: Category 1, min_rating 400
    let r1 = client.register_rule(
        &wallet,
        &Criteria {
            key_category: 1,
            min_rating: 400,
            max_price: 10_000,
            max_per_key: 5,
        },
    );

    // Rule 2: Category 1, min_rating 300
    let r2 = client.register_rule(
        &wallet,
        &Criteria {
            key_category: 1,
            min_rating: 300,
            max_price: 10_000,
            max_per_key: 5,
        },
    );

    // Both match, Rule 1 is returned by FIFO registration order
    let res = client.check_rule_with_context(&wallet, &key_id, &1, &450, &5_000, &0);
    assert_eq!(res.rule_id.unwrap(), r1);

    // Deactivate Rule 1 -> now Rule 2 is returned
    client.deactivate_rule(&r1);
    let res2 = client.check_rule_with_context(&wallet, &key_id, &1, &450, &5_000, &0);
    assert_eq!(res2.rule_id.unwrap(), r2);
}

// ============================================================================
// Acceptance Criterion 4: get_rules returns correct rules in registration order
// ============================================================================

#[test]
fn test_get_rules_registration_order() {
    let (_env, client, wallet) = setup_test();

    let c1 = Criteria {
        key_category: 1,
        min_rating: 200,
        max_price: 1_000,
        max_per_key: 1,
    };
    let c2 = Criteria {
        key_category: 2,
        min_rating: 300,
        max_price: 2_000,
        max_per_key: 2,
    };
    let c3 = Criteria {
        key_category: 3,
        min_rating: 400,
        max_price: 3_000,
        max_per_key: 3,
    };

    let id1 = client.register_rule(&wallet, &c1);
    let id2 = client.register_rule(&wallet, &c2);
    let id3 = client.register_rule(&wallet, &c3);

    let rules = client.get_rules(&wallet);
    assert_eq!(rules.len(), 3);
    assert_eq!(rules.get(0).unwrap().id, id1);
    assert_eq!(rules.get(1).unwrap().id, id2);
    assert_eq!(rules.get(2).unwrap().id, id3);

    // Deactivating a rule does not remove it from get_rules nor change ordering
    client.deactivate_rule(&id2);

    let updated_rules = client.get_rules(&wallet);
    assert_eq!(updated_rules.len(), 3);
    assert_eq!(updated_rules.get(0).unwrap().id, id1);
    assert_eq!(updated_rules.get(1).unwrap().id, id2);
    assert!(!updated_rules.get(1).unwrap().active);
    assert_eq!(updated_rules.get(2).unwrap().id, id3);
}

#[test]
fn test_get_rules_empty_for_new_wallet() {
    let (_env, client, wallet) = setup_test();
    let rules = client.get_rules(&wallet);
    assert_eq!(rules.len(), 0);
}

// ============================================================================
// Acceptance Criterion 5: RuleRegistered and RuleUpdated events are emitted correctly
// ============================================================================

#[test]
fn test_rule_registered_event_emission() {
    let (env, client, wallet) = setup_test();
    let criteria = valid_criteria();

    let rule_id = client.register_rule(&wallet, &criteria);

    let events = rule_registered_events(&env);
    assert_eq!(events.len(), 1);

    let event = events.get(0).unwrap();
    assert_eq!(event.rule_id, rule_id);
    assert_eq!(event.wallet, wallet);
    assert_eq!(event.criteria, criteria);
}

#[test]
fn test_rule_updated_event_emission() {
    let (env, client, wallet) = setup_test();
    let criteria1 = valid_criteria();
    let rule_id = client.register_rule(&wallet, &criteria1);

    let criteria2 = Criteria {
        key_category: 3,
        min_rating: 450,
        max_price: 25_000,
        max_per_key: 10,
    };
    client.update_rule(&rule_id, &criteria2);

    let events = rule_updated_events(&env);
    assert_eq!(events.len(), 1);

    let event = events.get(0).unwrap();
    assert_eq!(event.rule_id, rule_id);
    assert_eq!(event.wallet, wallet);
    assert_eq!(event.criteria, criteria2);
}

// ============================================================================
// Edge cases & Validations
// ============================================================================

#[test]
fn test_update_rule_success_and_immutability() {
    let (env, client, wallet) = setup_test();
    env.ledger().set_timestamp(2_000);

    let c1 = valid_criteria();
    let rule_id = client.register_rule(&wallet, &c1);

    let c2 = Criteria {
        key_category: 9,
        min_rating: 500,
        max_price: 99_000,
        max_per_key: 20,
    };

    env.ledger().set_timestamp(3_000);
    client.update_rule(&rule_id, &c2);

    let updated = client.get_rule(&rule_id);
    assert_eq!(updated.criteria, c2);
    // Owner, ID, active flag, and created_at remain unchanged (immutable)
    assert_eq!(updated.id, rule_id);
    assert_eq!(updated.owner, wallet);
    assert!(updated.active);
    assert_eq!(updated.created_at, 2_000);
}

#[test]
fn test_update_deactivated_rule_fails() {
    let (_env, client, wallet) = setup_test();
    let rule_id = client.register_rule(&wallet, &valid_criteria());

    client.deactivate_rule(&rule_id);

    let res = client.try_update_rule(&rule_id, &valid_criteria());
    assert_eq!(res, Err(Ok(AutoInvestError::Deactivated)));
}

#[test]
fn test_update_nonexistent_rule_fails() {
    let (_env, client, _wallet) = setup_test();
    let res = client.try_update_rule(&999, &valid_criteria());
    assert_eq!(res, Err(Ok(AutoInvestError::NotFound)));
}

#[test]
fn test_double_deactivate_fails() {
    let (_env, client, wallet) = setup_test();
    let rule_id = client.register_rule(&wallet, &valid_criteria());

    // First deactivation succeeds
    client.deactivate_rule(&rule_id);

    // Second deactivation fails with AlreadyInactive
    let res = client.try_deactivate_rule(&rule_id);
    assert_eq!(res, Err(Ok(AutoInvestError::AlreadyInactive)));
}

#[test]
fn test_deactivate_nonexistent_rule_fails() {
    let (_env, client, _wallet) = setup_test();
    let res = client.try_deactivate_rule(&999);
    assert_eq!(res, Err(Ok(AutoInvestError::NotFound)));
}

#[test]
fn test_invalid_criteria_rejections() {
    let (_env, client, wallet) = setup_test();

    // Rating < 100
    let mut bad = valid_criteria();
    bad.min_rating = 99;
    assert_eq!(
        client.try_register_rule(&wallet, &bad),
        Err(Ok(AutoInvestError::MinRatingOutOfRange))
    );

    // Rating > 500
    bad = valid_criteria();
    bad.min_rating = 501;
    assert_eq!(
        client.try_register_rule(&wallet, &bad),
        Err(Ok(AutoInvestError::MinRatingOutOfRange))
    );

    // Max price == 0
    bad = valid_criteria();
    bad.max_price = 0;
    assert_eq!(
        client.try_register_rule(&wallet, &bad),
        Err(Ok(AutoInvestError::MaxPriceInvalid))
    );

    // Max price < 0
    bad = valid_criteria();
    bad.max_price = -100;
    assert_eq!(
        client.try_register_rule(&wallet, &bad),
        Err(Ok(AutoInvestError::MaxPriceInvalid))
    );

    // Max per key == 0
    bad = valid_criteria();
    bad.max_per_key = 0;
    assert_eq!(
        client.try_register_rule(&wallet, &bad),
        Err(Ok(AutoInvestError::MaxPerKeyZero))
    );

    // Same criteria validation applies on update_rule
    let valid_id = client.register_rule(&wallet, &valid_criteria());
    bad = valid_criteria();
    bad.min_rating = 50;
    assert_eq!(
        client.try_update_rule(&valid_id, &bad),
        Err(Ok(AutoInvestError::MinRatingOutOfRange))
    );
}

#[test]
fn test_max_rules_per_wallet_boundary() {
    let (_env, client, wallet) = setup_test();

    // Exactly 50 rules can be registered
    for _ in 0..MAX_RULES_PER_WALLET {
        client.register_rule(&wallet, &valid_criteria());
    }
    assert_eq!(client.get_rules(&wallet).len(), MAX_RULES_PER_WALLET);

    // 51st rule registration fails with TooManyRules
    let res = client.try_register_rule(&wallet, &valid_criteria());
    assert_eq!(res, Err(Ok(AutoInvestError::TooManyRules)));
}

#[test]
fn test_unauthorized_update_and_deactivate() {
    let (env, client, wallet) = setup_test();
    let rule_id = client.register_rule(&wallet, &valid_criteria());

    // Clear all mocked authorizations
    env.mock_auths(&[]);

    assert!(client.try_update_rule(&rule_id, &valid_criteria()).is_err());
    assert!(client.try_deactivate_rule(&rule_id).is_err());
}

#[test]
fn test_check_rule_no_rules_returns_ineligible() {
    let (env, client, wallet) = setup_test();
    let key_id = Address::generate(&env);

    let res = client.check_rule_with_context(&wallet, &key_id, &1, &450, &1_000, &0);
    assert_eq!(
        res,
        CheckResult {
            eligible: false,
            rule_id: None,
        }
    );
}
