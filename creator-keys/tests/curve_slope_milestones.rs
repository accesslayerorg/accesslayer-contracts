//! Regression tests for supply-triggered curve slope changes (#1173).

mod contract_test_env;

use contract_test_env::{
    assert_event, register_creator_keys, register_test_creator, set_key_price_for_tests,
    set_protocol_fee_bps, test_env_with_auths, test_wallet_address,
};
use creator_keys::{
    supply_milestones::{CurveSlopeTier, SupplyMilestoneUnlockedEvent},
    CreatorKeysContractClient, FeatureError,
};
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    Address, Env, Symbol, TryFromVal, Vec,
};

fn configured_tiers(env: &Env) -> Vec<CurveSlopeTier> {
    Vec::from_array(
        env,
        [
            CurveSlopeTier {
                supply: 2,
                slope: 0,
            },
            CurveSlopeTier {
                supply: 5,
                slope: 50,
            },
        ],
    )
}

fn buy_one(client: &CreatorKeysContractClient<'_>, creator: &Address, buyer: &Address) {
    let quote = client.get_buy_quote(creator);
    client.buy_key(creator, buyer, &quote.total_amount, &None);
}

#[test]
fn slope_changes_once_when_supply_crosses_threshold_and_emits_event() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let admin = set_protocol_fee_bps(&env, &client, 9000, 1000);
    set_key_price_for_tests(&env, &client, 100);
    client.set_circuit_breaker_threshold(&admin, &10_000_u32);
    client.set_curve_slope(&admin, &100_i128);
    let creator = register_test_creator(&env, &client, "slopeowner");
    let buyer = test_wallet_address(&env, "slope-buyer");
    client.set_curve_slope_milestones(&admin, &creator, &configured_tiers(&env));

    assert_eq!(client.current_curve_slope_milestone(&creator), (0, 100));
    buy_one(&client, &creator, &buyer);
    assert_eq!(client.current_curve_slope_milestone(&creator), (0, 100));
    buy_one(&client, &creator, &buyer);

    assert_event(
        &env,
        &contract_id,
        Symbol::new(&env, "SupplyMilestoneUnlocked"),
        &SupplyMilestoneUnlockedEvent {
            milestone_index: 1,
            supply: 2,
            new_slope: 0,
        },
    );
    let unlock_events = env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|topic| Symbol::try_from_val(&env, &topic).ok())
                == Some(Symbol::new(&env, "SupplyMilestoneUnlocked"))
        })
        .count();
    assert_eq!(unlock_events, 1);
    assert_eq!(client.current_curve_slope_milestone(&creator), (1, 0));

    buy_one(&client, &creator, &buyer);
    assert_eq!(client.current_curve_slope_milestone(&creator), (1, 0));
}

#[test]
fn slope_schedule_is_immutable_and_validated() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_fee_bps(&env, &client, 9000, 1000);
    set_key_price_for_tests(&env, &client, 100);
    client.set_circuit_breaker_threshold(&admin, &10_000_u32);
    let creator = register_test_creator(&env, &client, "immutableowner");
    let tiers = configured_tiers(&env);

    client.set_curve_slope_milestones(&admin, &creator, &tiers);
    assert_eq!(
        client.try_set_curve_slope_milestones(&admin, &creator, &tiers),
        Err(Ok(FeatureError::AlreadyConfigured)),
    );

    let invalid = Vec::from_array(
        &env,
        [
            CurveSlopeTier {
                supply: 3,
                slope: 10,
            },
            CurveSlopeTier {
                supply: 2,
                slope: 20,
            },
        ],
    );
    let another_creator = register_test_creator(&env, &client, "invalidowner");
    assert_eq!(
        client.try_set_curve_slope_milestones(&admin, &another_creator, &invalid),
        Err(Ok(FeatureError::InvalidCurveMilestones)),
    );
    assert_eq!(
        client.try_set_curve_slope_milestones(&admin, &Address::generate(&env), &invalid),
        Err(Ok(FeatureError::NotRegistered)),
    );
}
