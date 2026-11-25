use crate::dynamic_fees::{self, VolumeBracket};
use crate::events::{self, FeeAdjustedEvent, FeeCollectedEvent};
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
    let treasury = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &1_000i128);
    client.set_curve_slope(&admin, &0i128);
    client.set_fee_config(&admin, &9_000u32, &1_000u32);
    client.set_protocol_fee(&admin, &Some(100u32), &treasury);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "feecreator"),
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

fn brackets(env: &Env, items: &[(i128, u32)]) -> Vec<VolumeBracket> {
    let mut result = Vec::new(env);
    for (min_volume, fee_bps) in items {
        result.push_back(VolumeBracket {
            min_volume: *min_volume,
            fee_bps: *fee_bps,
        });
    }
    result
}

fn fee_adjusted_events(env: &Env, key_id: &Address) -> Vec<FeeAdjustedEvent> {
    let mut found = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Option<Symbol> = topics.get(0).unwrap().try_into_val(env).ok();
        if name == Some(events::FEE_ADJUSTED_EVENT_NAME) {
            if let Some(topic) = topics.get(1) {
                let event_key: Option<Address> = topic.try_into_val(env).ok();
                if event_key == Some(key_id.clone()) {
                    found.push_back(data.try_into_val(env).unwrap());
                }
            }
        }
    }
    found
}

fn fee_collected_events(env: &Env) -> Vec<FeeCollectedEvent> {
    let mut found = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Option<Symbol> = topics.get(0).unwrap().try_into_val(env).ok();
        if name == Some(events::FEE_COLLECTED_EVENT_NAME) {
            found.push_back(data.try_into_val(env).unwrap());
        }
    }
    found
}

fn record_trade(setup: &Setup, key_id: &Address, volume: i128) {
    setup
        .env
        .as_contract(&setup.contract_id, || {
            dynamic_fees::record_trade(&setup.env, key_id, volume)
        })
        .unwrap();
}

#[test]
fn defaults_cover_all_fee_boundaries() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    env.as_contract(&contract_id, || {
        let brackets = dynamic_fees::default_volume_brackets(&env);
        for (volume, expected_fee) in [(0, 150), (99, 150), (100, 100), (1_000, 100), (1_001, 75)] {
            assert_eq!(dynamic_fees::fee_for_volume(&env, volume), expected_fee);
        }
        assert_eq!(brackets.len(), dynamic_fees::DEFAULT_VOLUME_BRACKETS);
    });
}

#[test]
fn volume_is_key_scoped_and_expires_at_the_24_hour_boundary() {
    let setup = setup();
    let other_key = Address::generate(&setup.env);
    setup.client.register_creator(
        &RegisterCreatorParams {
            creator: other_key.clone(),
            handle: String::from_str(&setup.env, "othercreator"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    record_trade(&setup, &setup.creator, 100);

    assert_eq!(setup.client.get_key_rolling_volume(&setup.creator), 100);
    assert_eq!(setup.client.get_key_rolling_volume(&other_key), 0);
    assert_eq!(setup.client.get_current_fee_bps(&setup.creator), 100);
    assert_eq!(setup.client.get_current_fee_bps(&other_key), 150);

    setup
        .env
        .ledger()
        .set_timestamp(START_TIMESTAMP + dynamic_fees::ROLLING_WINDOW_SECS);
    assert_eq!(setup.client.get_current_fee_bps(&setup.creator), 100);
    setup
        .env
        .ledger()
        .set_timestamp(START_TIMESTAMP + dynamic_fees::ROLLING_WINDOW_SECS + 1);
    assert_eq!(setup.client.get_key_rolling_volume(&setup.creator), 0);
    assert_eq!(setup.client.get_current_fee_bps(&setup.creator), 150);
}

#[test]
fn admin_bracket_update_preserves_volume_and_rejects_invalid_or_unauthorized_updates() {
    let setup = setup();
    record_trade(&setup, &setup.creator, 500);
    let custom = brackets(&setup.env, &[(0, 200), (500, 50)]);
    setup.client.set_volume_brackets(&setup.admin, &custom);

    assert_eq!(setup.client.get_key_rolling_volume(&setup.creator), 500);
    assert_eq!(setup.client.get_current_fee_bps(&setup.creator), 50);
    assert_eq!(fee_adjusted_events(&setup.env, &setup.creator).len(), 0);
    record_trade(&setup, &setup.creator, 1);
    let changed = fee_adjusted_events(&setup.env, &setup.creator);
    assert_eq!(changed.len(), 1);
    assert_eq!(changed.get(0).unwrap().previous_fee_bps, 100);
    assert_eq!(changed.get(0).unwrap().new_fee_bps, 50);
    assert_eq!(setup.client.get_key_rolling_volume(&setup.creator), 501);

    let unauthorized = Address::generate(&setup.env);
    assert_eq!(
        setup.client.try_set_volume_brackets(&unauthorized, &custom),
        Err(Ok(ContractError::Unauthorized))
    );

    for invalid in [
        brackets(&setup.env, &[(1, 150)]),
        brackets(&setup.env, &[(0, 150), (0, 100)]),
        brackets(&setup.env, &[(0, 150), (100, 10_001)]),
        Vec::new(&setup.env),
    ] {
        assert_eq!(
            setup.client.try_set_volume_brackets(&setup.admin, &invalid),
            Err(Ok(ContractError::InvalidFeeConfig))
        );
    }
}

#[test]
fn fee_query_rejects_unregistered_key() {
    let setup = setup();
    let unknown_key = Address::generate(&setup.env);
    assert_eq!(
        setup.client.try_get_current_fee_bps(&unknown_key),
        Err(Ok(ContractError::NotRegistered))
    );
}

#[test]
fn swaps_use_pre_trade_volume_and_record_success_once() {
    let setup = setup();
    let buyer = Address::generate(&setup.env);

    setup
        .client
        .buy_key(&setup.creator, &buyer, &1_000i128, &None);
    let first_adjusted = fee_adjusted_events(&setup.env, &setup.creator);
    assert_eq!(first_adjusted.len(), 1);
    assert_eq!(first_adjusted.get(0).unwrap().new_fee_bps, 100);
    let first_fees = fee_collected_events(&setup.env);
    assert_eq!(first_fees.len(), 1);
    assert_eq!(first_fees.get(0).unwrap().fee, 15);
    assert_eq!(setup.client.get_key_rolling_volume(&setup.creator), 1_000);

    setup
        .client
        .buy_key(&setup.creator, &buyer, &1_000i128, &None);
    let adjusted = fee_adjusted_events(&setup.env, &setup.creator);
    assert_eq!(adjusted.len(), 1);
    assert_eq!(adjusted.get(0).unwrap().previous_fee_bps, 100);
    assert_eq!(adjusted.get(0).unwrap().new_fee_bps, 75);

    let fees = fee_collected_events(&setup.env);
    assert_eq!(fees.len(), 1);
    assert_eq!(fees.get(0).unwrap().fee, 10);
    assert_eq!(setup.client.get_key_rolling_volume(&setup.creator), 2_000);
    let mut ledger_info = setup.env.ledger().get();
    ledger_info.sequence_number += 1;
    ledger_info.timestamp += 300;
    setup.env.ledger().set(ledger_info);
    setup.client.sell_key(&setup.creator, &buyer, &None);
    let fees = fee_collected_events(&setup.env);
    assert_eq!(fees.len(), 1);
    assert_eq!(fees.get(0).unwrap().fee, 7);
    assert_eq!(setup.client.get_key_rolling_volume(&setup.creator), 3_000);

    let failed_buyer = Address::generate(&setup.env);
    assert_eq!(
        setup
            .client
            .try_buy_key(&setup.creator, &failed_buyer, &0i128, &None),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    assert_eq!(setup.client.get_key_rolling_volume(&setup.creator), 3_000);
}

#[test]
fn fee_adjusted_emits_once_for_bracket_crossings_and_expiry() {
    let setup = setup();
    record_trade(&setup, &setup.creator, 100);
    let first = fee_adjusted_events(&setup.env, &setup.creator);
    assert_eq!(first.len(), 1);
    assert_eq!(first.get(0).unwrap().previous_fee_bps, 150);
    assert_eq!(first.get(0).unwrap().new_fee_bps, 100);
    assert_eq!(first.get(0).unwrap().rolling_volume, 100);
    assert_eq!(first.get(0).unwrap().key_id, setup.creator);
    assert_eq!(first.get(0).unwrap().timestamp, START_TIMESTAMP);

    record_trade(&setup, &setup.creator, 50);
    assert_eq!(fee_adjusted_events(&setup.env, &setup.creator).len(), 0);

    setup
        .env
        .ledger()
        .set_timestamp(START_TIMESTAMP + dynamic_fees::ROLLING_WINDOW_SECS + 1);
    assert_eq!(setup.client.get_current_fee_bps(&setup.creator), 150);
    record_trade(&setup, &setup.creator, 10);
    let events = fee_adjusted_events(&setup.env, &setup.creator);
    assert_eq!(events.len(), 1);
    assert_eq!(events.get(0).unwrap().previous_fee_bps, 100);
    assert_eq!(events.get(0).unwrap().new_fee_bps, 150);
    assert_eq!(events.get(0).unwrap().rolling_volume, 0);
}

#[test]
fn rolling_volume_buckets_are_bounded() {
    assert_eq!(dynamic_fees::MAX_VOLUME_BUCKETS, 289);
    assert_eq!(dynamic_fees::VOLUME_BUCKET_SECS, 300);
}

#[test]
fn volume_accumulation_rejects_overflow_without_replacing_existing_volume() {
    let setup = setup();
    setup
        .env
        .as_contract(&setup.contract_id, || {
            dynamic_fees::record_trade(&setup.env, &setup.creator, i128::MAX).unwrap();
            dynamic_fees::record_trade(&setup.env, &setup.creator, 1)
        })
        .unwrap_err();
    assert_eq!(
        setup.client.get_key_rolling_volume(&setup.creator),
        i128::MAX
    );
}
