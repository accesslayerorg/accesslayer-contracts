//! Shared setup helpers for `holder-milestones` integration tests.
#![allow(dead_code)]

use holder_milestones::{HolderMilestonesContract, HolderMilestonesContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

/// Default reward amounts for each milestone tier.
pub const REWARD_30D: i128 = 100;
pub const REWARD_90D: i128 = 300;
pub const REWARD_180D: i128 = 600;
pub const REWARD_365D: i128 = 1_200;

/// Default [`Env`] with mocked authorisation.
pub fn test_env_with_auths() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

/// Register [`HolderMilestonesContract`] and return a client and the contract id.
pub fn register_holder_milestones(env: &Env) -> (HolderMilestonesContractClient<'_>, Address) {
    let id = env.register(HolderMilestonesContract, ());
    let client = HolderMilestonesContractClient::new(env, &id);
    (client, id)
}

/// Bootstrap: register contract + set admin. Returns (client, id, admin).
pub fn setup_with_admin(env: &Env) -> (HolderMilestonesContractClient<'_>, Address, Address) {
    let (client, id) = register_holder_milestones(env);
    let admin = Address::generate(env);
    client.set_admin(&admin);
    (client, id, admin)
}

/// Bootstrap with a configured schedule for a fresh key_id.
/// Returns (client, id, admin, key_id).
pub fn setup_with_schedule(
    env: &Env,
) -> (HolderMilestonesContractClient<'_>, Address, Address, Address) {
    let (client, id, admin) = setup_with_admin(env);
    let key_id = Address::generate(env);
    client.set_milestone_schedule(
        &admin,
        &key_id,
        &REWARD_30D,
        &REWARD_90D,
        &REWARD_180D,
        &REWARD_365D,
    );
    (client, id, admin, key_id)
}

/// Register entry for a fresh wallet and return it.
pub fn enter_wallet(
    env: &Env,
    client: &HolderMilestonesContractClient<'_>,
    key_id: &Address,
) -> Address {
    let wallet = Address::generate(env);
    client.record_entry(&wallet, key_id);
    wallet
}

/// Advance the ledger timestamp by `seconds`.
pub fn advance_time(env: &Env, seconds: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = ledger.timestamp.saturating_add(seconds);
    env.ledger().set(ledger);
}
