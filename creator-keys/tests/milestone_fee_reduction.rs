//! Tests for the graduated supply milestone fee-reduction feature (#1076).
//!
//! Covers:
//! - fee reduction applied when supply crosses a milestone threshold
//! - no regression to a higher fee when supply decreases below a milestone
//! - `get_current_milestone` returns the accurate active tier and fee
//! - `set_milestones` is restricted to the protocol admin
//! - `MilestoneReached` is emitted with the correct key, tier, supply and fee

mod contract_test_env;

use contract_test_env::{
    assert_event, register_creator_keys, register_test_creator, set_key_price_for_tests,
    set_ledger_sequence, set_protocol_fee_bps, test_env_with_auths, test_wallet_address,
};
use creator_keys::{
    events, supply_milestones::MilestoneTier, ContractError, CreatorKeysContractClient,
};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

const BASE_PRICE: i128 = 100;

fn register_key_with_milestones(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
) -> (Address, Address, Address) {
    let admin = set_protocol_fee_bps(env, client, 9000u32, 1000u32);
    set_key_price_for_tests(env, client, BASE_PRICE);
    let creator = register_test_creator(env, client, "alice");
    let buyer = test_wallet_address(env, "buyer-1");
    (admin, creator, buyer)
}

fn configured_milestones(env: &Env) -> Vec<MilestoneTier> {
    Vec::from_array(
        env,
        [
            MilestoneTier {
                supply: 2,
                fee_bps: 500,
            },
            MilestoneTier {
                supply: 5,
                fee_bps: 300,
            },
        ],
    )
}

/// Buy a single key for `buyer` at the current on-chain buy quote, so the
/// payment always matches the bonding-curve price at the current supply.
fn buy_one_key(client: &CreatorKeysContractClient<'_>, creator: &Address, buyer: &Address) {
    let quote = client.get_buy_quote(creator);
    client.buy_key(creator, buyer, &quote.total_amount, &None);
}

#[test]
fn test_set_milestones_restricted_to_admin() {
    let env = test_env_with_auths();
    let (client, _contract_id) = register_creator_keys(&env);
    let admin = set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client, "alice");
    let non_admin = Address::generate(&env);
    let unregistered = Address::generate(&env);
    let milestones = configured_milestones(&env);

    // Non-admin callers are rejected.
    let result = client.try_set_milestones(&non_admin, &creator, &milestones);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));

    // Unregistered keys cannot be configured.
    let result = client.try_set_milestones(&admin, &unregistered, &milestones);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));

    // Admin can configure milestones at key creation.
    client.set_milestones(&admin, &creator, &milestones);

    // Getting the milestone on a key with no supply yet reports no active tier.
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (0u32, 0u32));
}

#[test]
fn test_set_milestones_validation() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client, "alice");

    // Empty milestone list is rejected.
    let result = client.try_set_milestones(&admin, &creator, &Vec::new(&env));
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));

    // Non-ascending supply thresholds are rejected.
    let descending = Vec::from_array(
        &env,
        [
            MilestoneTier {
                supply: 5,
                fee_bps: 300,
            },
            MilestoneTier {
                supply: 2,
                fee_bps: 500,
            },
        ],
    );
    let result = client.try_set_milestones(&admin, &creator, &descending);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));

    // Zero supply threshold is rejected.
    let zero = Vec::from_array(
        &env,
        [MilestoneTier {
            supply: 0,
            fee_bps: 500,
        }],
    );
    let result = client.try_set_milestones(&admin, &creator, &zero);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));

    // Fee above 10_000 bps is rejected.
    let too_high = Vec::from_array(
        &env,
        [MilestoneTier {
            supply: 2,
            fee_bps: 10_001,
        }],
    );
    let result = client.try_set_milestones(&admin, &creator, &too_high);
    assert_eq!(result, Err(Ok(ContractError::InvalidFeeConfig)));
}

#[test]
fn test_fee_reduction_applied_when_supply_crosses_milestone() {
    let env = test_env_with_auths();
    let (client, contract_id) = register_creator_keys(&env);
    let (admin, creator, buyer) = register_key_with_milestones(&env, &client);
    let milestones = configured_milestones(&env);
    client.set_milestones(&admin, &creator, &milestones);

    // Supply 0 -> 1: no tier reached yet.
    buy_one_key(&client, &creator, &buyer);
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (0u32, 0u32));

    // Supply 1 -> 2: crosses tier 1 (supply 2), fee becomes 500 bps.
    set_ledger_sequence(&env, 42);
    buy_one_key(&client, &creator, &buyer);
    assert_event(
        &env,
        &contract_id,
        events::MILESTONE_REACHED_EVENT_NAME,
        &events::MilestoneReachedEvent {
            key_id: creator.clone(),
            tier: 1,
            supply: 2,
            new_fee_bps: 500,
            ledger: 42,
        },
    );
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (1u32, 500u32));

    // Supply 2 -> 3 and 3 -> 4: tier 1 already applied, no change.
    buy_one_key(&client, &creator, &buyer);
    buy_one_key(&client, &creator, &buyer);
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (1u32, 500u32));

    // Supply 4 -> 5: crosses tier 2 (supply 5), fee drops to 300 bps.
    buy_one_key(&client, &creator, &buyer);
    assert_event(
        &env,
        &contract_id,
        events::MILESTONE_REACHED_EVENT_NAME,
        &events::MilestoneReachedEvent {
            key_id: creator.clone(),
            tier: 2,
            supply: 5,
            new_fee_bps: 300,
            ledger: 42,
        },
    );
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (2u32, 300u32));
}

#[test]
fn test_no_fee_regression_when_supply_decreases_below_milestone() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (admin, creator, buyer) = register_key_with_milestones(&env, &client);
    let milestones = configured_milestones(&env);
    client.set_milestones(&admin, &creator, &milestones);

    // Buy up to and past tier 2 (supply 5, fee 300 bps). Buys share a ledger;
    // sells must move to a later ledger to satisfy the flash-loan guard.
    set_ledger_sequence(&env, 100);
    for _ in 0..5 {
        buy_one_key(&client, &creator, &buyer);
    }
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (2u32, 300u32));

    // Sell back below tier 2 (supply 4, then 3): fee must stay at 300 bps.
    set_ledger_sequence(&env, 200);
    client.sell_key(&creator, &buyer, &None);
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (2u32, 300u32));

    client.sell_key(&creator, &buyer, &None);
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (2u32, 300u32));

    // Sell back to supply 2 (tier 1 threshold still met) and then to supply 1
    // (below tier 1): fee still must not regress back to 500 bps.
    client.sell_key(&creator, &buyer, &None);
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (2u32, 300u32));

    client.sell_key(&creator, &buyer, &None);
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (2u32, 300u32));
}

#[test]
fn test_get_current_milestone_without_config() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let _admin = set_protocol_fee_bps(&env, &client, 9000u32, 1000u32);
    let creator = register_test_creator(&env, &client, "alice");
    let unregistered = Address::generate(&env);

    // Registered key with no milestone config reports no active tier.
    let (tier, fee_bps) = client.get_current_milestone(&creator);
    assert_eq!((tier, fee_bps), (0u32, 0u32));

    // Unregistered key returns NotRegistered.
    let result = client.try_get_current_milestone(&unregistered);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_check_and_apply_milestone_is_idempotent() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let (admin, creator, buyer) = register_key_with_milestones(&env, &client);
    let milestones = configured_milestones(&env);
    client.set_milestones(&admin, &creator, &milestones);

    buy_one_key(&client, &creator, &buyer);
    buy_one_key(&client, &creator, &buyer);

    // Re-applying with no new supply change is a no-op and returns the same tier.
    let first = client.check_and_apply_milestone(&creator);
    assert_eq!(first, (1u32, 500u32));
    let second = client.check_and_apply_milestone(&creator);
    assert_eq!(second, (1u32, 500u32));
}
