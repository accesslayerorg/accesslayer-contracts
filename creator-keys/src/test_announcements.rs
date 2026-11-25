use crate::{
    events, AnnouncementCategory, ContractError, CreatorKeysContract, CreatorKeysContractClient,
    RegisterCreatorParams, MAX_ANNOUNCEMENT_PAGE_SIZE,
};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, String, Symbol, TryIntoVal,
};

fn setup() -> (Env, CreatorKeysContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "creator"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    (env, client, creator)
}

#[test]
fn announcements_store_all_categories_and_emit_metadata() {
    let (env, client, creator) = setup();
    env.ledger().set_timestamp(1_234_567);
    let categories = [
        AnnouncementCategory::Update,
        AnnouncementCategory::Milestone,
        AnnouncementCategory::Warning,
        AnnouncementCategory::Event,
    ];

    for (index, category) in categories.iter().enumerate() {
        let title = String::from_str(&env, "Creator update");
        let content_hash = String::from_str(&env, "bafy-content");
        let id = client.post_announcement(&creator, &title, &content_hash, category);
        assert_eq!(id, index as u32);

        let events = env.events().all();
        let (_, topics, data) = events.last().unwrap();
        let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
        let event_key: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
        let event_id: u32 = topics.get(2).unwrap().try_into_val(&env).unwrap();
        let payload: events::AnnouncementPostedEvent = data.try_into_val(&env).unwrap();
        assert_eq!(name, events::ANNOUNCEMENT_POSTED_EVENT_NAME);
        assert_eq!(event_key, creator);
        assert_eq!(event_id, id);
        assert_eq!(payload.key_id, creator);
        assert_eq!(payload.title, title);
        assert_eq!(payload.content_hash, content_hash);
        assert_eq!(payload.category, *category);
        assert_eq!(payload.posted_at, 1_234_567);
    }

    let all = client.get_announcements(&creator, &0, &4);
    assert_eq!(all.len(), 4);
    for (index, category) in categories.iter().enumerate() {
        let announcement = all.get(index as u32).unwrap();
        assert_eq!(announcement.announcement_id, index as u32);
        assert_eq!(announcement.key_id, creator);
        assert_eq!(announcement.title, String::from_str(&env, "Creator update"));
        assert_eq!(
            announcement.content_hash,
            String::from_str(&env, "bafy-content")
        );
        assert_eq!(announcement.category, *category);
        assert_eq!(announcement.posted_at, 1_234_567);
    }
}

#[test]
fn announcement_pages_are_ordered_and_bounded() {
    let (env, client, creator) = setup();
    for _ in 0..3 {
        client.post_announcement(
            &creator,
            &String::from_str(&env, "title"),
            &String::from_str(&env, "hash"),
            &AnnouncementCategory::Update,
        );
    }

    let first_page = client.get_announcements(&creator, &0, &2);
    let second_page = client.get_announcements(&creator, &2, &2);
    assert_eq!(first_page.len(), 2);
    assert_eq!(second_page.len(), 1);
    assert_eq!(first_page.get(0).unwrap().announcement_id, 0);
    assert_eq!(first_page.get(1).unwrap().announcement_id, 1);
    assert_eq!(second_page.get(0).unwrap().announcement_id, 2);
    assert!(client.get_announcements(&creator, &3, &1).is_empty());
    assert!(client.get_announcements(&creator, &0, &0).is_empty());
    assert_eq!(
        client.try_get_announcements(&creator, &0, &(MAX_ANNOUNCEMENT_PAGE_SIZE + 1)),
        Err(Ok(ContractError::AnnouncementPageSizeExceeded))
    );
}

#[test]
fn posting_requires_creator_authorization() {
    let (env, client, creator) = setup();
    env.mock_auths(&[]);

    assert!(client
        .try_post_announcement(
            &creator,
            &String::from_str(&env, "title"),
            &String::from_str(&env, "hash"),
            &AnnouncementCategory::Update,
        )
        .is_err());
}
