//! Integration tests for role-scoped co-creators (issue #1132).

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::{
    co_creators::{CoCreatorRole, CreatorAction},
    events, ContractError, FeatureError,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, IntoVal, String, Symbol,
};

fn setup(env: &soroban_sdk::Env) -> (creator_keys::CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_pricing_and_fees(env, &client, 1_000, 9_000, 1_000);
    let creator = register_test_creator(env, &client, "co_creator_key");
    (client, creator)
}

#[test]
fn only_primary_creator_can_add_and_remove_co_creators() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let attacker = Address::generate(&env);
    let content = Address::generate(&env);

    assert_eq!(
        client.try_add_co_creator(&creator, &attacker, &content, &CoCreatorRole::Content),
        Err(Ok(FeatureError::Unauthorized))
    );
    client.add_co_creator(&creator, &creator, &content, &CoCreatorRole::Content);
    assert_eq!(
        client.get_co_creator_role(&creator, &content),
        Some(CoCreatorRole::Content)
    );
    assert_eq!(client.get_co_creators(&creator).len(), 1);

    assert_eq!(
        client.try_add_co_creator(&creator, &attacker, &attacker, &CoCreatorRole::Admin),
        Err(Ok(FeatureError::Unauthorized))
    );
    client.remove_co_creator(&creator, &content);
    assert_eq!(client.get_co_creator_role(&creator, &content), None);
    assert_eq!(client.get_co_creators(&creator).len(), 0);
}

#[test]
fn add_and_remove_emit_events_with_wallet_and_role() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let admin = Address::generate(&env);

    client.add_co_creator(&creator, &creator, &admin, &CoCreatorRole::Admin);
    let added: std::vec::Vec<_> = env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            let name: Symbol = topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .unwrap()
                .into_val(&env);
            name == events::CO_CREATOR_ADDED_EVENT_NAME
        })
        .collect();
    assert_eq!(added.len(), 1);
    let payload: events::CoCreatorAddedEvent = added[0].2.clone().into_val(&env);
    assert_eq!(payload.key_id, creator);
    assert_eq!(payload.wallet, admin);
    assert_eq!(payload.role, CoCreatorRole::Admin);

    client.remove_co_creator(&creator, &admin);
    let removed: std::vec::Vec<_> = env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            let name: Symbol = topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .unwrap()
                .into_val(&env);
            name == events::CO_CREATOR_ROLE_REMOVED_EVENT_NAME
        })
        .collect();
    assert_eq!(removed.len(), 1);
    let payload: events::CoCreatorRoleRemovedEvent = removed[0].2.clone().into_val(&env);
    assert_eq!(payload.wallet, admin);
    assert_eq!(payload.role, CoCreatorRole::Admin);
}

#[test]
fn content_role_can_announce_but_not_handle_revenue() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let content = Address::generate(&env);
    client.add_co_creator(&creator, &creator, &content, &CoCreatorRole::Content);

    assert!(client.can_co_creator_perform(&creator, &content, &CreatorAction::Announcement));
    assert!(!client.can_co_creator_perform(&creator, &content, &CreatorAction::Revenue));

    let id = client.post_announcement_as_co_creator(
        &creator,
        &content,
        &String::from_str(&env, "Title"),
        &String::from_str(&env, "hash"),
        &creator_keys::AnnouncementCategory::Update,
    );
    assert_eq!(id, 0);
    let announcements = client.get_announcements(&creator, &0, &10);
    assert_eq!(announcements.len(), 1);

    assert_eq!(
        client.try_claim_royalties_as_co_creator(&creator, &content),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        client.try_claim_royalties_as_co_creator(&creator, &Address::generate(&env)),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn revenue_role_can_claim_but_not_announce() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let revenue = Address::generate(&env);
    client.add_co_creator(&creator, &creator, &revenue, &CoCreatorRole::Revenue);

    assert!(client.can_co_creator_perform(&creator, &revenue, &CreatorAction::Revenue));
    assert!(!client.can_co_creator_perform(&creator, &revenue, &CreatorAction::Announcement));
    assert_eq!(
        client.try_claim_royalties_as_co_creator(&creator, &revenue),
        Err(Ok(ContractError::NoDividendClaimable))
    );
    assert_eq!(
        client.try_post_announcement_as_co_creator(
            &creator,
            &revenue,
            &String::from_str(&env, "Title"),
            &String::from_str(&env, "hash"),
            &creator_keys::AnnouncementCategory::Update,
        ),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn admin_role_can_perform_every_creator_action() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let admin = Address::generate(&env);
    client.add_co_creator(&creator, &creator, &admin, &CoCreatorRole::Admin);

    for action in [
        CreatorAction::Announcement,
        CreatorAction::Revenue,
        CreatorAction::All,
    ] {
        assert!(client.can_co_creator_perform(&creator, &admin, &action));
    }
    client.post_announcement_as_co_creator(
        &creator,
        &admin,
        &String::from_str(&env, "Title"),
        &String::from_str(&env, "hash"),
        &creator_keys::AnnouncementCategory::Update,
    );
    assert_eq!(client.get_announcements(&creator, &0, &10).len(), 1);
}
