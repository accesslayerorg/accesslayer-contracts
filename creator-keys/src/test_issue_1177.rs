//! Tests for the active fee tier view and `FeeTierChanged` event (#1177).

use crate::dynamic_fees::{self, ActiveFeeTier, VolumeBracket};
use crate::events::{self, KeyFeeTierChangedEvent};
use crate::{ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, String, Symbol, TryIntoVal, Vec,
};

const START_TIMESTAMP: u64 = 1_200;

struct Setup {
    env: Env,
    contract_id: Address,
    client: CreatorKeysContractClient<'static>,
    admin: Address,
    creator: Address,
}

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(START_TIMESTAMP);
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "tiercreator"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    Setup {
        env,
        contract_id,
        client,
        admin,
        creator,
    }
}

fn record_trade(setup: &Setup, volume: i128) {
    setup
        .env
        .as_contract(&setup.contract_id, || {
            dynamic_fees::record_trade(&setup.env, &setup.creator, volume)
        })
        .unwrap();
}

fn tier_changed_events(setup: &Setup) -> Vec<KeyFeeTierChangedEvent> {
    let env = &setup.env;
    let mut found = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Option<Symbol> = topics.get(0).unwrap().try_into_val(env).ok();
        if name == Some(events::KEY_FEE_TIER_CHANGED_EVENT_NAME) {
            found.push_back(data.try_into_val(env).unwrap());
        }
    }
    found
}

fn active(setup: &Setup) -> ActiveFeeTier {
    setup.client.get_active_fee_tier(&setup.creator)
}

#[test]
fn active_tier_is_correct_at_each_threshold_boundary() {
    let setup = setup();
    // Default brackets: 0 -> 150, 100 -> 100, 1_001 -> 75.
    assert_eq!(
        active(&setup),
        ActiveFeeTier {
            tier_index: 0,
            fee_bps: 150,
            rolling_volume: 0,
            volume_to_next_tier: 100,
        }
    );

    record_trade(&setup, 99);
    let tier = active(&setup);
    assert_eq!((tier.tier_index, tier.fee_bps), (0, 150));
    assert_eq!(tier.volume_to_next_tier, 1);

    record_trade(&setup, 1);
    let tier = active(&setup);
    assert_eq!((tier.tier_index, tier.fee_bps), (1, 100));
    assert_eq!(tier.rolling_volume, 100);
    assert_eq!(tier.volume_to_next_tier, 901);

    record_trade(&setup, 900);
    let tier = active(&setup);
    assert_eq!((tier.tier_index, tier.fee_bps), (1, 100));
    assert_eq!(tier.volume_to_next_tier, 1);

    record_trade(&setup, 1);
    let tier = active(&setup);
    assert_eq!((tier.tier_index, tier.fee_bps), (2, 75));
    assert_eq!(tier.volume_to_next_tier, 0);
}

#[test]
fn active_tier_downgrades_when_volume_decays() {
    let setup = setup();
    record_trade(&setup, 1_001);
    assert_eq!(active(&setup).fee_bps, 75);

    setup
        .env
        .ledger()
        .set_timestamp(START_TIMESTAMP + dynamic_fees::ROLLING_WINDOW_SECS);
    assert_eq!(active(&setup).fee_bps, 75);

    setup
        .env
        .ledger()
        .set_timestamp(START_TIMESTAMP + dynamic_fees::ROLLING_WINDOW_SECS + 1);
    assert_eq!(
        active(&setup),
        ActiveFeeTier {
            tier_index: 0,
            fee_bps: 150,
            rolling_volume: 0,
            volume_to_next_tier: 100,
        }
    );
}

#[test]
fn active_tier_follows_admin_configured_brackets() {
    let setup = setup();
    let mut brackets = Vec::new(&setup.env);
    for (min_volume, fee_bps) in [(0i128, 200u32), (5_000, 50)] {
        brackets.push_back(VolumeBracket {
            min_volume,
            fee_bps,
        });
    }
    setup.client.set_volume_brackets(&setup.admin, &brackets);

    record_trade(&setup, 1_200);
    let tier = active(&setup);
    assert_eq!((tier.tier_index, tier.fee_bps), (0, 200));
    assert_eq!(tier.volume_to_next_tier, 3_800);
}

#[test]
fn active_tier_rejects_unregistered_key() {
    let setup = setup();
    let stranger = Address::generate(&setup.env);
    assert_eq!(
        setup.client.try_get_active_fee_tier(&stranger),
        Err(Ok(ContractError::NotRegistered))
    );
}

#[test]
fn fee_tier_changed_emits_only_on_tier_transitions() {
    let setup = setup();

    record_trade(&setup, 50);
    assert_eq!(tier_changed_events(&setup).len(), 0);

    record_trade(&setup, 50);
    let events = tier_changed_events(&setup);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.key_id, setup.creator);
    assert_eq!(event.new_tier_index, 1);
    assert_eq!((event.previous_fee_bps, event.new_fee_bps), (150, 100));
    assert_eq!(event.rolling_volume, 100);
    assert_eq!(event.timestamp, START_TIMESTAMP);

    // Stays inside the same tier: nothing new is emitted.
    record_trade(&setup, 10);
    assert_eq!(tier_changed_events(&setup).len(), 0);

    // Crossing into the top tier.
    record_trade(&setup, 900);
    let event = tier_changed_events(&setup).get(0).unwrap();
    assert_eq!(event.new_tier_index, 2);
    assert_eq!((event.previous_fee_bps, event.new_fee_bps), (100, 75));
}

#[test]
fn fee_tier_changed_emits_on_downgrade_after_decay() {
    let setup = setup();
    record_trade(&setup, 1_001);

    setup
        .env
        .ledger()
        .set_timestamp(START_TIMESTAMP + dynamic_fees::ROLLING_WINDOW_SECS + 1);
    record_trade(&setup, 10);

    let events = tier_changed_events(&setup);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.new_tier_index, 0);
    assert_eq!((event.previous_fee_bps, event.new_fee_bps), (75, 150));
    assert_eq!(event.rolling_volume, 0);
}
