#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, Symbol, TryFromVal,
};

fn setup_test() -> (Env, BadgeContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(BadgeContract, ());
    let client = BadgeContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let auth_contract = Address::generate(&env);

    client.init(&admin);
    client.set_authorised_contract(&auth_contract);

    (env, client, admin, auth_contract)
}

#[test]
fn test_init_and_admin_management() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(BadgeContract, ());
    let client = BadgeContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    assert_eq!(client.get_admin(), None);

    client.init(&admin);
    assert_eq!(client.get_admin(), Some(admin.clone()));

    // Double init fails
    assert_eq!(
        client.try_init(&admin),
        Err(Ok(BadgeError::AlreadyInitialized))
    );

    // Rotate admin
    let new_admin = Address::generate(&env);
    client.set_admin(&new_admin);
    assert_eq!(client.get_admin(), Some(new_admin));
}

#[test]
fn test_authorised_contract_management() {
    let (env, client, _admin, auth_contract_1) = setup_test();

    assert!(client.is_authorised(&auth_contract_1));
    assert_eq!(
        client.get_authorised_contract(),
        Some(auth_contract_1.clone())
    );

    let auth_contract_2 = Address::generate(&env);
    assert!(!client.is_authorised(&auth_contract_2));

    // Add second authorised contract
    client.add_authorised_contract(&auth_contract_2);
    assert!(client.is_authorised(&auth_contract_2));

    let contracts = client.get_authorised_contracts();
    assert_eq!(contracts.len(), 2);

    // Adding existing authorised contract fails
    assert_eq!(
        client.try_add_authorised_contract(&auth_contract_2),
        Err(Ok(BadgeError::ContractAlreadyAuthorized))
    );

    // Remove contract
    client.remove_authorised_contract(&auth_contract_2);
    assert!(!client.is_authorised(&auth_contract_2));
    assert_eq!(client.get_authorised_contracts().len(), 1);

    // Removing un-authorised contract fails
    assert_eq!(
        client.try_remove_authorised_contract(&auth_contract_2),
        Err(Ok(BadgeError::ContractNotAuthorized))
    );
}

#[test]
fn test_mint_badge_restricted_to_authorised_contracts() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(BadgeContract, ());
    let client = BadgeContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let auth_contract = Address::generate(&env);
    let stranger = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.init(&admin);

    // Before any authorised contract is configured, mint_badge fails
    assert_eq!(
        client.try_mint_badge(&recipient, &BadgeType::FirstBuyer),
        Err(Ok(BadgeError::NoAuthorisedContract))
    );

    // Unauthorized caller via mint_badge_by fails
    assert_eq!(
        client.try_mint_badge_by(&stranger, &recipient, &BadgeType::FirstBuyer),
        Err(Ok(BadgeError::Unauthorized))
    );

    // Configure authorised contract
    client.set_authorised_contract(&auth_contract);

    // Authorised caller succeeds via mint_badge_by
    assert_eq!(
        client.try_mint_badge_by(&auth_contract, &recipient, &BadgeType::FirstBuyer),
        Ok(Ok(()))
    );
    assert!(client.has_badge(&recipient, &BadgeType::FirstBuyer));
}

#[test]
fn test_duplicate_badge_rejected_with_badge_already_earned() {
    let (env, client, _admin, _auth_contract) = setup_test();
    let recipient = Address::generate(&env);

    // First mint succeeds
    client.mint_badge(&recipient, &BadgeType::FirstBuyer);
    assert!(client.has_badge(&recipient, &BadgeType::FirstBuyer));

    // Duplicate mint of the same badge type to the same wallet must fail with BadgeAlreadyEarned
    assert_eq!(
        client.try_mint_badge(&recipient, &BadgeType::FirstBuyer),
        Err(Ok(BadgeError::BadgeAlreadyEarned))
    );

    // Duplicate via mint_badge_by also fails with BadgeAlreadyEarned
    let auth_contract = client.get_authorised_contract().unwrap();
    assert_eq!(
        client.try_mint_badge_by(&auth_contract, &recipient, &BadgeType::FirstBuyer),
        Err(Ok(BadgeError::BadgeAlreadyEarned))
    );
}

#[test]
fn test_all_five_badge_types_supported_without_error() {
    let (env, client, _admin, _auth_contract) = setup_test();
    let recipient = Address::generate(&env);

    let all_types = BadgeType::all();
    assert_eq!(all_types.len(), 5);

    // Mint each of the five badge types sequentially
    for badge_type in all_types.iter() {
        assert!(!client.has_badge(&recipient, badge_type));
        client.mint_badge(&recipient, badge_type);
        assert!(client.has_badge(&recipient, badge_type));
    }

    // Verify all 5 are recorded
    assert_eq!(client.get_wallet_badge_count(&recipient), 5);
    let badges = client.get_badges(&recipient);
    assert_eq!(badges.len(), 5);

    // Each badge type is duplicated and each must reject with BadgeAlreadyEarned
    for badge_type in all_types.iter() {
        assert_eq!(
            client.try_mint_badge(&recipient, badge_type),
            Err(Ok(BadgeError::BadgeAlreadyEarned))
        );
    }
}

#[test]
fn test_get_badges_returns_accurate_list_and_timestamps() {
    let (env, client, _admin, _auth_contract) = setup_test();
    let recipient = Address::generate(&env);

    // Initially empty
    let empty_badges = client.get_badges(&recipient);
    assert_eq!(empty_badges.len(), 0);
    assert_eq!(client.get_wallet_badge_count(&recipient), 0);

    // Mint at timestamp 1,000
    env.ledger().set_timestamp(1_000);
    client.mint_badge(&recipient, &BadgeType::FirstBuyer);

    // Mint at timestamp 2,500
    env.ledger().set_timestamp(2_500);
    client.mint_badge(&recipient, &BadgeType::WhaleHolder);

    // Mint at timestamp 5,000
    env.ledger().set_timestamp(5_000);
    client.mint_badge(&recipient, &BadgeType::GovernanceVoter);

    let badges = client.get_badges(&recipient);
    assert_eq!(badges.len(), 3);

    let b0 = badges.get(0).unwrap();
    assert_eq!(b0.badge_type, BadgeType::FirstBuyer);
    assert_eq!(b0.mint_timestamp, 1_000);
    assert_eq!(b0.timestamp(), 1_000);

    let b1 = badges.get(1).unwrap();
    assert_eq!(b1.badge_type, BadgeType::WhaleHolder);
    assert_eq!(b1.mint_timestamp, 2_500);

    let b2 = badges.get(2).unwrap();
    assert_eq!(b2.badge_type, BadgeType::GovernanceVoter);
    assert_eq!(b2.mint_timestamp, 5_000);

    // Verify direct timestamp queries
    assert_eq!(
        client.get_badge_timestamp(&recipient, &BadgeType::FirstBuyer),
        Some(1_000)
    );
    assert_eq!(
        client.get_badge_timestamp(&recipient, &BadgeType::WhaleHolder),
        Some(2_500)
    );
    assert_eq!(
        client.get_badge_timestamp(&recipient, &BadgeType::GovernanceVoter),
        Some(5_000)
    );
    assert_eq!(
        client.get_badge_timestamp(&recipient, &BadgeType::TopTrader),
        None
    );
}

#[test]
fn test_badge_minted_event_emitted_correctly() {
    let (env, client, _admin, _auth_contract) = setup_test();
    let recipient = Address::generate(&env);

    client.mint_badge(&recipient, &BadgeType::TopTrader);

    // Filter events for BadgeMinted
    let mut matching_events = soroban_sdk::Vec::new(&env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            if Symbol::try_from_val(&env, &first) == Ok(Symbol::new(&env, BADGE_MINTED_EVENT_NAME))
            {
                if let Ok(event) = BadgeMinted::try_from_val(&env, &data) {
                    matching_events.push_back(event);
                }
            }
        }
    }

    assert_eq!(matching_events.len(), 1);
    let event = matching_events.get(0).unwrap();
    assert_eq!(event.wallet, recipient);
    assert_eq!(event.badge_type, BadgeType::TopTrader);
}

#[test]
fn test_multiple_wallets_independent_badges() {
    let (env, client, _admin, _auth_contract) = setup_test();
    let wallet_a = Address::generate(&env);
    let wallet_b = Address::generate(&env);

    env.ledger().set_timestamp(100);
    client.mint_badge(&wallet_a, &BadgeType::FirstBuyer);

    env.ledger().set_timestamp(200);
    client.mint_badge(&wallet_b, &BadgeType::FirstBuyer);

    // Both wallets earned FirstBuyer independently
    assert!(client.has_badge(&wallet_a, &BadgeType::FirstBuyer));
    assert!(client.has_badge(&wallet_b, &BadgeType::FirstBuyer));

    assert_eq!(
        client.get_badge_timestamp(&wallet_a, &BadgeType::FirstBuyer),
        Some(100)
    );
    assert_eq!(
        client.get_badge_timestamp(&wallet_b, &BadgeType::FirstBuyer),
        Some(200)
    );

    // Total count is 2
    assert_eq!(client.get_total_badges(), 2);
}

#[test]
fn test_badge_type_constants_and_names() {
    assert_eq!(BadgeType::FIRST_BUYER, BadgeType::FirstBuyer);
    assert_eq!(BadgeType::WHALE_HOLDER, BadgeType::WhaleHolder);
    assert_eq!(BadgeType::GOVERNANCE_VOTER, BadgeType::GovernanceVoter);
    assert_eq!(BadgeType::LONG_TERM_HOLDER, BadgeType::LongTermHolder);
    assert_eq!(BadgeType::TOP_TRADER, BadgeType::TopTrader);

    assert_eq!(BadgeType::FirstBuyer.name(), "FIRST_BUYER");
    assert_eq!(BadgeType::WhaleHolder.name(), "WHALE_HOLDER");
    assert_eq!(BadgeType::GovernanceVoter.name(), "GOVERNANCE_VOTER");
    assert_eq!(BadgeType::LongTermHolder.name(), "LONG_TERM_HOLDER");
    assert_eq!(BadgeType::TopTrader.name(), "TOP_TRADER");
}
