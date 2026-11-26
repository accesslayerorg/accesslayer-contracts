//! Integration tests for the atomic slippage-protected multi-buy (issue #717).

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_ledger_sequence, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{
    events,
    multi_buy::{MultiBuyLeg, MAX_MULTI_BUY_LEGS},
    ContractError, CreatorKeysContractClient,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token, Address, Env, IntoVal, Symbol, Vec,
};

const KEY_PRICE: i128 = 1_000;
const LOOSE_MAX: i128 = 1_000_000_000;

fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, 9_000, 1_000);
    client.set_curve_slope(&admin, &0);
    (client, admin)
}

fn creators(env: &Env, client: &CreatorKeysContractClient<'_>, n: u32) -> std::vec::Vec<Address> {
    (0..n)
        .map(|i| register_test_creator(env, client, &std::format!("mb_creator_{i}")))
        .collect()
}

fn leg(creator: &Address, amount: u32, max_price: i128) -> MultiBuyLeg {
    MultiBuyLeg {
        creator: creator.clone(),
        amount,
        max_price,
    }
}

fn deadline(env: &Env) -> u32 {
    env.ledger().get().sequence_number + 100
}

#[test]
fn three_legs_execute_and_emit_events() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let keys = creators(&env, &client, 3);
    let buyer = Address::generate(&env);
    let legs = Vec::from_array(
        &env,
        [
            leg(&keys[0], 1, LOOSE_MAX),
            leg(&keys[1], 2, LOOSE_MAX),
            leg(&keys[2], 1, LOOSE_MAX),
        ],
    );

    let results = client.multi_buy(&buyer, &legs, &deadline(&env));
    assert_eq!(results.len(), 3);
    let events_after_buy = env.events().all();
    for (i, amount) in [1u32, 2, 1].iter().enumerate() {
        assert_eq!(client.get_total_key_supply(&keys[i]), *amount);
        assert_eq!(client.get_key_balance(&keys[i], &buyer), *amount);
        assert_eq!(results.get(i as u32).unwrap().new_supply, *amount);
    }
    assert_eq!(
        events_after_buy
            .iter()
            .filter(|(_, topics, _)| {
                topics
                    .get(events::TOPIC_EVENT_NAME_INDEX)
                    .map(|v| {
                        let name: Symbol = v.into_val(&env);
                        name == events::MULTI_BUY_COMPLETED_EVENT_NAME
                    })
                    .unwrap_or(false)
            })
            .count(),
        1
    );
    let buy_events = events_after_buy
        .iter()
        .filter(|(_, topics, _)| {
            topics
                .get(events::TOPIC_EVENT_NAME_INDEX)
                .map(|v| {
                    let name: Symbol = v.into_val(&env);
                    name == events::BUY_EVENT_NAME
                })
                .unwrap_or(false)
        })
        .count();
    assert_eq!(buy_events, 3);
}

#[test]
fn second_leg_over_max_price_rolls_back_every_leg() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let keys = creators(&env, &client, 3);
    let buyer = Address::generate(&env);
    let legs = Vec::from_array(
        &env,
        [
            leg(&keys[0], 1, LOOSE_MAX),
            leg(&keys[1], 1, 1),
            leg(&keys[2], 1, LOOSE_MAX),
        ],
    );

    assert_eq!(
        client.try_multi_buy(&buyer, &legs, &deadline(&env)),
        Err(Ok(ContractError::SlippageExceeded))
    );
    for key in &keys {
        assert_eq!(client.get_total_key_supply(key), 0);
        assert_eq!(client.get_key_balance(key, &buyer), 0);
    }
}

#[test]
fn duplicate_creator_is_rejected() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let keys = creators(&env, &client, 2);
    let buyer = Address::generate(&env);
    let legs = Vec::from_array(
        &env,
        [
            leg(&keys[0], 1, LOOSE_MAX),
            leg(&keys[1], 1, LOOSE_MAX),
            leg(&keys[0], 1, LOOSE_MAX),
        ],
    );
    assert_eq!(
        client.try_multi_buy(&buyer, &legs, &deadline(&env)),
        Err(Ok(ContractError::DuplicateCreator))
    );
}

#[test]
fn more_than_ten_legs_is_rejected() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let keys = creators(&env, &client, MAX_MULTI_BUY_LEGS + 1);
    let buyer = Address::generate(&env);
    let mut legs = Vec::new(&env);
    for key in &keys {
        legs.push_back(leg(key, 1, LOOSE_MAX));
    }
    assert_eq!(
        client.try_multi_buy(&buyer, &legs, &deadline(&env)),
        Err(Ok(ContractError::TooManyLegs))
    );
}

#[test]
fn empty_legs_and_expired_deadline_are_rejected() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let keys = creators(&env, &client, 1);
    let buyer = Address::generate(&env);
    let empty = Vec::new(&env);
    assert_eq!(
        client.try_multi_buy(&buyer, &empty, &deadline(&env)),
        Err(Ok(ContractError::LegsEmpty))
    );

    set_ledger_sequence(&env, 500);
    let legs = Vec::from_array(&env, [leg(&keys[0], 1, LOOSE_MAX)]);
    assert_eq!(
        client.try_multi_buy(&buyer, &legs, &100),
        Err(Ok(ContractError::DeadlinePassed))
    );
}

#[test]
fn insufficient_worst_case_balance_is_rejected_before_any_purchase() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);
    let keys = creators(&env, &client, 2);
    let buyer = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.set_xlm_token_address(&_admin, &token);
    token::StellarAssetClient::new(&env, &token).mint(&buyer, &1_000);

    let legs = Vec::from_array(&env, [leg(&keys[0], 1, 1_000), leg(&keys[1], 1, 1_000)]);
    assert_eq!(
        client.try_multi_buy(&buyer, &legs, &deadline(&env)),
        Err(Ok(ContractError::InsufficientFunds))
    );
    for key in &keys {
        assert_eq!(client.get_total_key_supply(key), 0);
    }
}
