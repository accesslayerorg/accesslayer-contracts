//! Shared setup helpers for `key-reputation` integration tests.
#![allow(dead_code)]

use key_reputation::{KeyReputationContract, KeyReputationContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

/// Default [`Env`] for tests with mocked authorisation.
pub fn test_env_with_auths() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

/// Register [`KeyReputationContract`] and return a client and contract id.
pub fn register_key_reputation<'a>(
    env: &'a Env,
) -> (KeyReputationContractClient<'a>, Address) {
    let id = env.register(KeyReputationContract, ());
    let client = KeyReputationContractClient::new(env, &id);
    (client, id)
}

/// Bootstrap: register contract, set admin, return (client, contract_id, admin).
pub fn setup_with_admin(env: &Env) -> (KeyReputationContractClient<'_>, Address, Address) {
    let (client, id) = register_key_reputation(env);
    let admin = Address::generate(env);
    client.set_admin(&admin);
    (client, id, admin)
}

/// Add a freshly generated address as an authorised caller and return it.
pub fn add_fresh_caller(
    env: &Env,
    client: &KeyReputationContractClient<'_>,
    admin: &Address,
) -> Address {
    let caller = Address::generate(env);
    client.add_authorised_caller(admin, &caller);
    caller
}
