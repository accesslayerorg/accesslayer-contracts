//! Tests for issues #884 (upgrade authority and version), #885 (transfer guards),
//! #887 (supply milestone events) and #889 (pause state change event).

use crate::events::{MilestoneCrossedEvent, PauseStateChangedEvent};
use crate::{ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams};
use soroban_sdk::{
    testutils::Address as _, testutils::Events as _, testutils::Ledger as _, Address, BytesN, Env,
    IntoVal, String, Symbol, TryFromVal, Val, Vec,
};

fn setup_test() -> (Env, CreatorKeysContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_treasury_address(&admin, &treasury);
    client.set_key_price(&admin, &100i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_protocol_fee_recipient(&admin, &treasury);

    let creator = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    (env, client, admin, creator)
}

/// Returns the data of every event whose first topic is `name`.
fn events_named<T: IntoVal<Env, Val> + TryFromVal<Env, Val>>(env: &Env, name: &str) -> Vec<T> {
    let symbol = Symbol::new(env, name);
    let mut found = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            if Symbol::try_from_val(env, &first) == Ok(symbol.clone()) {
                found.push_back(T::try_from_val(env, &data).unwrap());
            }
        }
    }
    found
}

// --- #884 ---

#[test]
fn test_upgrade_rejected_for_non_admin() {
    let (env, client, _admin, _creator) = setup_test();
    let stranger = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[0u8; 32]);

    let result = client.try_upgrade(&stranger, &hash);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    assert_eq!(client.get_version(), 1);
}

#[test]
fn test_get_version_readable_without_auth() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    assert_eq!(client.get_version(), 1);
}

// --- #885 ---

#[test]
fn test_transfer_keys_standard_moves_balance() {
    let (env, client, _admin, creator) = setup_test();
    let from = Address::generate(&env);
    let to = Address::generate(&env);
    client.buy_key(&creator, &from, &1000i128, &None);
    client.buy_key(&creator, &from, &1000i128, &None);

    client.transfer_keys(&creator, &from, &to, &1u32);

    assert_eq!(client.get_key_balance(&creator, &from), 1);
    assert_eq!(client.get_key_balance(&creator, &to), 1);
}

#[test]
fn test_transfer_keys_frozen_sender_rejected() {
    let (env, client, _admin, creator) = setup_test();
    let from = Address::generate(&env);
    let to = Address::generate(&env);
    client.buy_key(&creator, &from, &1000i128, &None);
    client.buy_key(&creator, &from, &1000i128, &None);
    client.self_freeze(&creator, &from, &2u32);

    let result = client.try_transfer_keys(&creator, &from, &to, &1u32);
    assert_eq!(result, Err(Ok(ContractError::FrozenPosition)));
}

#[test]
fn test_transfer_keys_zero_address_rejected() {
    let (env, client, _admin, creator) = setup_test();
    let from = Address::generate(&env);
    let zero = Address::from_string(&String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ));
    client.buy_key(&creator, &from, &1000i128, &None);

    let result = client.try_transfer_keys(&creator, &from, &zero, &1u32);
    assert_eq!(result, Err(Ok(ContractError::ZeroAddress)));
}

#[test]
fn test_transfer_keys_blocked_during_cooldown() {
    let (env, client, _admin, creator) = setup_test();
    let from = Address::generate(&env);
    let to = Address::generate(&env);
    client.set_buy_cooldown(&creator, &10u32);
    client.buy_key(&creator, &from, &1000i128, &None);

    let result = client.try_transfer_keys(&creator, &from, &to, &1u32);
    assert_eq!(result, Err(Ok(ContractError::CooldownActive)));
}

// --- #887 ---

#[test]
fn test_milestone_single_crossing_up_and_down() {
    let (env, client, admin, creator) = setup_test();
    let buyer = Address::generate(&env);
    let mut milestones = Vec::new(&env);
    milestones.push_back(2u32);
    client.set_supply_milestones(&admin, &milestones);

    client.buy_key(&creator, &buyer, &1000i128, &None);
    assert_eq!(
        events_named::<MilestoneCrossedEvent>(&env, "mile_x").len(),
        0
    );

    client.buy_key(&creator, &buyer, &1000i128, &None);
    let up = events_named::<MilestoneCrossedEvent>(&env, "mile_x");
    assert_eq!(up.len(), 1);
    let event = up.get(0).unwrap();
    assert_eq!(event.tier, 1);
    assert_eq!(event.direction, Symbol::new(&env, "up"));
    assert_eq!(event.supply, 2);

    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.sell_key(&creator, &buyer, &None);
    let down = events_named::<MilestoneCrossedEvent>(&env, "mile_x");
    let event = down.get(down.len() - 1).unwrap();
    assert_eq!(event.direction, Symbol::new(&env, "down"));
    assert_eq!(event.supply, 1);
}

#[test]
fn test_milestone_multiple_crossings_in_one_trade() {
    let (env, client, admin, creator) = setup_test();
    let buyer = Address::generate(&env);
    let mut milestones = Vec::new(&env);
    milestones.push_back(1u32);
    milestones.push_back(2u32);
    milestones.push_back(3u32);
    client.set_supply_milestones(&admin, &milestones);

    client.buy_keys(&creator, &buyer, &3u32, &100_000i128, &None);

    assert_eq!(
        events_named::<MilestoneCrossedEvent>(&env, "mile_x").len(),
        3
    );
}

#[test]
fn test_milestone_no_crossing_emits_nothing() {
    let (env, client, admin, creator) = setup_test();
    let buyer = Address::generate(&env);
    let mut milestones = Vec::new(&env);
    milestones.push_back(10u32);
    client.set_supply_milestones(&admin, &milestones);

    client.buy_key(&creator, &buyer, &1000i128, &None);

    assert_eq!(
        events_named::<MilestoneCrossedEvent>(&env, "mile_x").len(),
        0
    );
}

// --- #889 ---

#[test]
fn test_paused_trades_rejected_and_unpause_restores() {
    let (env, client, admin, creator) = setup_test();
    let buyer = Address::generate(&env);
    let to = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000i128, &None);

    client.pause(&admin);
    let events = events_named::<PauseStateChangedEvent>(&env, "pause_chg");
    let event = events.get(events.len() - 1).unwrap();
    assert!(event.paused);
    assert_eq!(event.caller, admin);

    assert!(client.get_is_paused());
    assert_eq!(
        client.try_buy_key(&creator, &buyer, &1000i128, &None),
        Err(Ok(ContractError::ProtocolPaused))
    );
    assert_eq!(
        client.try_sell_key(&creator, &buyer, &None),
        Err(Ok(ContractError::ProtocolPaused))
    );
    assert_eq!(
        client.try_transfer_keys(&creator, &buyer, &to, &1u32),
        Err(Ok(ContractError::ProtocolPaused))
    );
    // View functions stay callable while paused.
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);

    client.unpause(&admin);
    let events = events_named::<PauseStateChangedEvent>(&env, "pause_chg");
    assert!(!events.get(events.len() - 1).unwrap().paused);
    client.buy_key(&creator, &buyer, &1000i128, &None);
    assert_eq!(client.get_key_balance(&creator, &buyer), 2);
}

#[test]
fn test_pause_rejected_for_non_admin() {
    let (env, client, _admin, _creator) = setup_test();
    let stranger = Address::generate(&env);
    assert_eq!(
        client.try_pause(&stranger),
        Err(Ok(ContractError::Unauthorized))
    );
}
