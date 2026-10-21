//! Upgrade proxy contract for bonding curve logic migration (#1055).
//!
//! Wraps the bonding curve (`creator-keys`) contract behind an upgrade proxy pattern,
//! allowing the admin to migrate to a new implementation while preserving all state,
//! balances, and continuous trading operations.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Val,
    Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ProxyError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidInterface = 4,
    ContractPaused = 5,
}

#[derive(Clone)]
#[contracttype]
pub enum ProxyDataKey {
    Admin,
    Implementation,
    IsPaused,
    Initialized,
}

/// Event emitted when an implementation upgrade is executed.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct UpgradeExecuted {
    pub old_implementation: Address,
    pub new_implementation: Address,
    pub executed_at: u64,
}

pub const UPGRADE_EXECUTED_EVENT_NAME: Symbol = symbol_short!("upg_exec");

#[contract]
pub struct BondingCurveProxy;

fn assert_admin(env: &Env, caller: &Address) -> Result<(), ProxyError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&ProxyDataKey::Admin)
        .ok_or(ProxyError::NotInitialized)?;
    if *caller != admin {
        return Err(ProxyError::Unauthorized);
    }
    Ok(())
}

fn validate_interface(env: &Env, implementation: &Address) -> Result<(), ProxyError> {
    // Interface verification: ensure implementation responds to bonding curve contract calls
    let fn_name = Symbol::new(env, "get_timelock_delay");
    let res = env.try_invoke_contract::<Val, Val>(implementation, &fn_name, Vec::new(env));
    if res.is_err() {
        let alt_fn = Symbol::new(env, "get_current_fee");
        let alt_res = env.try_invoke_contract::<Val, Val>(implementation, &alt_fn, Vec::new(env));
        if alt_res.is_err() {
            return Err(ProxyError::InvalidInterface);
        }
    }
    Ok(())
}

fn emit_upgrade_executed(env: &Env, old_impl: &Address, new_impl: &Address) {
    let payload = UpgradeExecuted {
        old_implementation: old_impl.clone(),
        new_implementation: new_impl.clone(),
        executed_at: env.ledger().timestamp(),
    };
    env.events()
        .publish((UPGRADE_EXECUTED_EVENT_NAME, old_impl, new_impl), payload);
}

#[contractimpl]
impl BondingCurveProxy {
    /// Initializes the proxy with an admin and the initial bonding curve implementation contract.
    pub fn initialize(
        env: Env,
        admin: Address,
        initial_implementation: Address,
    ) -> Result<(), ProxyError> {
        let storage = env.storage().instance();
        if storage
            .get::<_, bool>(&ProxyDataKey::Initialized)
            .unwrap_or(false)
        {
            return Err(ProxyError::AlreadyInitialized);
        }

        validate_interface(&env, &initial_implementation)?;

        storage.set(&ProxyDataKey::Admin, &admin);
        storage.set(&ProxyDataKey::Implementation, &initial_implementation);
        storage.set(&ProxyDataKey::IsPaused, &false);
        storage.set(&ProxyDataKey::Initialized, &true);

        emit_upgrade_executed(&env, &initial_implementation, &initial_implementation);

        Ok(())
    }

    /// Upgrades the bonding curve contract to a new implementation.
    ///
    /// Requires admin authorization and performs interface validation against the new contract.
    /// Emits `UpgradeExecuted` event upon successful upgrade.
    pub fn upgrade_implementation(
        env: Env,
        caller: Address,
        new_implementation: Address,
    ) -> Result<(), ProxyError> {
        assert_admin(&env, &caller)?;

        let storage = env.storage().instance();
        let old_impl: Address = storage
            .get(&ProxyDataKey::Implementation)
            .ok_or(ProxyError::NotInitialized)?;

        validate_interface(&env, &new_implementation)?;

        storage.set(&ProxyDataKey::Implementation, &new_implementation);

        emit_upgrade_executed(&env, &old_impl, &new_implementation);

        Ok(())
    }

    /// Emergency pause for the proxy to guard trading operations during sensitive migrations.
    pub fn set_paused(env: Env, caller: Address, paused: bool) -> Result<(), ProxyError> {
        assert_admin(&env, &caller)?;
        env.storage()
            .instance()
            .set(&ProxyDataKey::IsPaused, &paused);
        Ok(())
    }

    /// Returns whether the proxy is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&ProxyDataKey::IsPaused)
            .unwrap_or(false)
    }

    /// Returns the current active implementation address.
    pub fn get_implementation(env: Env) -> Result<Address, ProxyError> {
        env.storage()
            .instance()
            .get(&ProxyDataKey::Implementation)
            .ok_or(ProxyError::NotInitialized)
    }

    /// Returns the current admin address.
    pub fn get_admin(env: Env) -> Result<Address, ProxyError> {
        env.storage()
            .instance()
            .get(&ProxyDataKey::Admin)
            .ok_or(ProxyError::NotInitialized)
    }

    /// Fallback dispatcher: delegates call to current implementation.
    /// Rejects calls if the proxy is paused or not initialized.
    pub fn dispatch(env: Env, function: Symbol, args: Vec<Val>) -> Result<Val, ProxyError> {
        let storage = env.storage().instance();
        let is_paused: bool = storage.get(&ProxyDataKey::IsPaused).unwrap_or(false);
        if is_paused {
            return Err(ProxyError::ContractPaused);
        }

        let impl_addr: Address = storage
            .get(&ProxyDataKey::Implementation)
            .ok_or(ProxyError::NotInitialized)?;

        Ok(env.invoke_contract(&impl_addr, &function, args))
    }
}
