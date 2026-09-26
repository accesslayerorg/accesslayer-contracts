#![no_std]
//! Minimal factory that deploys `creator-keys` contract instances and keeps a
//! registry of the deployed addresses.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    Symbol, Vec,
};

pub const KEY_DEPLOYED_EVENT_NAME: Symbol = symbol_short!("key_depl");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FactoryError {
    AlreadyInitialised = 1,
    NotInitialised = 2,
    Unauthorized = 3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyDeployedEvent {
    pub key: Address,
    pub creator: Address,
}

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Admin,
    WasmHash,
    Registry,
    Allowed(Address),
}

#[contract]
pub struct CreatorKeysFactory;

fn read_admin(env: &Env) -> Result<Address, FactoryError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(FactoryError::NotInitialised)
}

#[contractimpl]
impl CreatorKeysFactory {
    /// One-time setup: the admin and the uploaded `creator-keys` wasm hash to deploy.
    pub fn initialise(env: Env, admin: Address, wasm_hash: BytesN<32>) -> Result<(), FactoryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(FactoryError::AlreadyInitialised);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::WasmHash, &wasm_hash);
        Ok(())
    }

    /// Admin allows or disallows a creator wallet to call `deploy_key`.
    pub fn set_creator_allowed(
        env: Env,
        admin: Address,
        creator: Address,
        allowed: bool,
    ) -> Result<(), FactoryError> {
        admin.require_auth();
        if admin != read_admin(&env)? {
            return Err(FactoryError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::Allowed(creator), &allowed);
        Ok(())
    }

    /// Deploys a new key contract for `creator` and records its address.
    /// Only the admin or a whitelisted creator may call it.
    pub fn deploy_key(
        env: Env,
        caller: Address,
        creator: Address,
        salt: BytesN<32>,
    ) -> Result<Address, FactoryError> {
        caller.require_auth();
        let is_allowed: bool = env
            .storage()
            .instance()
            .get(&DataKey::Allowed(caller.clone()))
            .unwrap_or(false);
        if caller != read_admin(&env)? && !is_allowed {
            return Err(FactoryError::Unauthorized);
        }
        let wasm_hash: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::WasmHash)
            .ok_or(FactoryError::NotInitialised)?;

        let key = env
            .deployer()
            .with_current_contract(salt)
            .deploy_v2(wasm_hash, ());

        let mut registry = Self::get_registry(env.clone());
        registry.push_back(key.clone());
        env.storage().instance().set(&DataKey::Registry, &registry);

        env.events().publish(
            (KEY_DEPLOYED_EVENT_NAME, creator.clone()),
            KeyDeployedEvent {
                key: key.clone(),
                creator,
            },
        );
        Ok(key)
    }

    /// Read-only view: every key contract address deployed by this factory.
    pub fn get_registry(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::Registry)
            .unwrap_or_else(|| Vec::new(&env))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup(env: &Env) -> (CreatorKeysFactoryClient<'_>, Address) {
        env.mock_all_auths();
        let client = CreatorKeysFactoryClient::new(env, &env.register(CreatorKeysFactory, ()));
        let admin = Address::generate(env);
        client.initialise(&admin, &BytesN::from_array(env, &[7; 32]));
        (client, admin)
    }

    #[test]
    fn unauthorised_deploy_is_rejected() {
        let env = Env::default();
        let (client, _admin) = setup(&env);
        let stranger = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[1; 32]);
        assert_eq!(
            client.try_deploy_key(&stranger, &stranger, &salt),
            Err(Ok(FactoryError::Unauthorized))
        );
        assert_eq!(client.get_registry().len(), 0);
    }

    #[test]
    fn only_admin_manages_allowed_creators_and_init_is_once() {
        let env = Env::default();
        let (client, admin) = setup(&env);
        let stranger = Address::generate(&env);
        assert_eq!(
            client.try_set_creator_allowed(&stranger, &stranger, &true),
            Err(Ok(FactoryError::Unauthorized))
        );
        client.set_creator_allowed(&admin, &stranger, &true);
        assert_eq!(
            client.try_initialise(&admin, &BytesN::from_array(&env, &[7; 32])),
            Err(Ok(FactoryError::AlreadyInitialised))
        );
    }
}
