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
    RegistryCount,
    CreatorKeys(Address),
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

fn read_registry_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::RegistryCount)
        .unwrap_or(0)
}

fn write_registry_count(env: &Env, count: u32) {
    env.storage()
        .instance()
        .set(&DataKey::RegistryCount, &count);
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

        let mut creator_keys = Self::get_keys_by_creator(env.clone(), creator.clone());
        creator_keys.push_back(key.clone());
        env.storage()
            .instance()
            .set(&DataKey::CreatorKeys(creator.clone()), &creator_keys);

        let registry_count = registry.len();
        write_registry_count(&env, registry_count);

        env.events().publish(
            (KEY_DEPLOYED_EVENT_NAME, creator.clone()),
            KeyDeployedEvent {
                key: key.clone(),
                creator,
            },
        );
        Ok(key)
    }

    /// Removes a key from the factory registry when the deployed contract is deprecated.
    /// Only the creator of that key or the admin may do so.
    pub fn remove_key(
        env: Env,
        caller: Address,
        creator: Address,
        key: Address,
    ) -> Result<(), FactoryError> {
        caller.require_auth();
        let admin = read_admin(&env)?;
        if caller != admin && caller != creator {
            return Err(FactoryError::Unauthorized);
        }

        let registry = Self::get_registry(env.clone());
        let creator_keys = Self::get_keys_by_creator(env.clone(), creator.clone());

        let mut filtered_registry = Vec::new(&env);
        for current in registry.iter() {
            if current != key {
                filtered_registry.push_back(current);
            }
        }

        let mut filtered_creator_keys = Vec::new(&env);
        for current in creator_keys.iter() {
            if current != key {
                filtered_creator_keys.push_back(current);
            }
        }

        env.storage()
            .instance()
            .set(&DataKey::Registry, &filtered_registry);
        env.storage().instance().set(
            &DataKey::CreatorKeys(creator.clone()),
            &filtered_creator_keys,
        );
        write_registry_count(&env, filtered_registry.len());
        Ok(())
    }

    /// Read-only view: every key contract address deployed by this factory.
    pub fn get_registry(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::Registry)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Read-only view: all deployed keys for a particular creator wallet.
    pub fn get_keys_by_creator(env: Env, creator: Address) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::CreatorKeys(creator))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Read-only view: a paginated slice of the registry ordered by deployment time.
    pub fn get_all_keys(env: Env, offset: u32, limit: u32) -> Vec<Address> {
        let registry = Self::get_registry(env.clone());
        let total = registry.len();
        if offset >= total || limit == 0 {
            return Vec::new(&env);
        }

        let start = offset;
        let end = if start.saturating_add(limit) > total {
            total
        } else {
            start.saturating_add(limit)
        };

        let mut page = Vec::new(&env);
        for index in start..end {
            page.push_back(registry.get_unchecked(index));
        }
        page
    }

    /// Read-only view: total number of active deployed key addresses in the registry.
    pub fn get_registry_count(env: Env) -> u32 {
        read_registry_count(&env)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Bytes;

    fn setup(env: &Env) -> (CreatorKeysFactoryClient<'_>, Address) {
        env.mock_all_auths();

        let wasm_bytes = Bytes::from_slice(env, &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
        let wasm_hash = env.deployer().upload_contract_wasm(wasm_bytes);

        let client = CreatorKeysFactoryClient::new(env, &env.register(CreatorKeysFactory, ()));
        let admin = Address::generate(env);
        client.initialise(&admin, &wasm_hash);
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
        let wasm_bytes = Bytes::from_slice(&env, &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
        let wasm_hash = env.deployer().upload_contract_wasm(wasm_bytes);
        assert_eq!(
            client.try_initialise(&admin, &wasm_hash),
            Err(Ok(FactoryError::AlreadyInitialised))
        );
    }

    #[test]
    fn registry_tracks_creator_filtered_keys_and_count() {
        let env = Env::default();
        let (client, admin) = setup(&env);
        let creator_a = Address::generate(&env);
        let creator_b = Address::generate(&env);
        client.set_creator_allowed(&admin, &creator_a, &true);
        client.set_creator_allowed(&admin, &creator_b, &true);

        let key_a_1 =
            client.deploy_key(&creator_a, &creator_a, &BytesN::from_array(&env, &[1; 32]));
        let key_a_2 =
            client.deploy_key(&creator_a, &creator_a, &BytesN::from_array(&env, &[2; 32]));
        let key_b_1 =
            client.deploy_key(&creator_b, &creator_b, &BytesN::from_array(&env, &[3; 32]));

        let a_keys = client.get_keys_by_creator(&creator_a);
        let b_keys = client.get_keys_by_creator(&creator_b);
        assert_eq!(a_keys.len(), 2);
        assert_eq!(b_keys.len(), 1);
        assert_eq!(a_keys.get_unchecked(0), key_a_1);
        assert_eq!(a_keys.get_unchecked(1), key_a_2);
        assert_eq!(b_keys.get_unchecked(0), key_b_1);
        assert_eq!(client.get_registry_count(), 3);
    }

    #[test]
    fn registry_paginates_all_keys_by_offset() {
        let env = Env::default();
        let (client, admin) = setup(&env);
        let creator = Address::generate(&env);
        client.set_creator_allowed(&admin, &creator, &true);

        let _key_1 = client.deploy_key(&creator, &creator, &BytesN::from_array(&env, &[11; 32]));
        let key_2 = client.deploy_key(&creator, &creator, &BytesN::from_array(&env, &[12; 32]));
        let key_3 = client.deploy_key(&creator, &creator, &BytesN::from_array(&env, &[13; 32]));
        let _key_4 = client.deploy_key(&creator, &creator, &BytesN::from_array(&env, &[14; 32]));

        let page = client.get_all_keys(&1u32, &2u32);
        assert_eq!(page.len(), 2);
        assert_eq!(page.get_unchecked(0), key_2);
        assert_eq!(page.get_unchecked(1), key_3);
        assert_eq!(client.get_registry_count(), 4);
    }

    #[test]
    fn deprecated_key_is_removed_from_registry() {
        let env = Env::default();
        let (client, admin) = setup(&env);
        let creator = Address::generate(&env);
        client.set_creator_allowed(&admin, &creator, &true);

        let key_1 = client.deploy_key(&creator, &creator, &BytesN::from_array(&env, &[21; 32]));
        let key_2 = client.deploy_key(&creator, &creator, &BytesN::from_array(&env, &[22; 32]));

        client.remove_key(&creator, &creator, &key_1);

        let creator_keys = client.get_keys_by_creator(&creator);
        assert_eq!(creator_keys.len(), 1);
        assert_eq!(creator_keys.get_unchecked(0), key_2);
        assert_eq!(client.get_registry_count(), 1);
        assert_eq!(client.get_all_keys(&0u32, &10u32).len(), 1);
    }
}
