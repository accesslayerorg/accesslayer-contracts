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

/// Gate types supported by the content gating layer.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum GateType {
    /// Requires any non-zero balance.
    Holder,
    /// Requires an active subscription.
    Subscriber,
    /// Requires balance of at least `min_balance`.
    Threshold,
}

/// Gate configuration parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct GateConfig {
    pub gate_type: GateType,
    pub min_balance: i128,
}

#[derive(Clone)]
#[contracttype]
pub enum GateDataKey {
    Creator(u64),
    Gate(u64),
}

/// Emitted when access is granted.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AccessGranted {
    pub wallet: Address,
    pub key_id: u64,
    pub gate_type: GateType,
}

/// Emitted when access is denied.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AccessDenied {
    pub wallet: Address,
    pub key_id: u64,
    pub gate_type: GateType,
}

pub const ACCESS_GRANTED_EVENT_NAME: Symbol = symbol_short!("acc_grnt");
pub const ACCESS_DENIED_EVENT_NAME: Symbol = symbol_short!("acc_deny");

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

    /// Configures the access gate for a key. Only the key creator may call this.
    pub fn set_gate(
        env: Env,
        caller: Address,
        key_id: u64,
        config: GateConfig,
    ) -> Result<(), ProxyError> {
        caller.require_auth();
        let creator: Address = env
            .storage()
            .persistent()
            .get(&GateDataKey::Creator(key_id))
            .ok_or(ProxyError::Unauthorized)?;
        if caller != creator {
            return Err(ProxyError::Unauthorized);
        }
        env.storage()
            .persistent()
            .set(&GateDataKey::Gate(key_id), &config);
        Ok(())
    }

    /// Returns the current gate configuration for a key.
    pub fn get_gate_config(env: Env, key_id: u64) -> Result<GateConfig, ProxyError> {
        env.storage()
            .persistent()
            .get(&GateDataKey::Gate(key_id))
            .ok_or(ProxyError::NotInitialized)
    }

    /// Checks whether `wallet` may access gated content for `key_id`.
    /// `balance` and `is_subscriber` are supplied by the caller's verified
    /// view of the bonding curve state. Emits `AccessGranted` or `AccessDenied`.
    pub fn check_access(
        env: Env,
        wallet: Address,
        key_id: u64,
        balance: i128,
        is_subscriber: bool,
    ) -> Result<bool, ProxyError> {
        let config: GateConfig = env
            .storage()
            .persistent()
            .get(&GateDataKey::Gate(key_id))
            .ok_or(ProxyError::NotInitialized)?;

        let allowed = match config.gate_type {
            GateType::Holder => balance > 0,
            GateType::Subscriber => is_subscriber,
            GateType::Threshold => balance >= config.min_balance,
        };

        if allowed {
            env.events().publish(
                (ACCESS_GRANTED_EVENT_NAME, wallet.clone(), key_id),
                AccessGranted {
                    wallet,
                    key_id,
                    gate_type: config.gate_type,
                },
            );
        } else {
            env.events().publish(
                (ACCESS_DENIED_EVENT_NAME, wallet.clone(), key_id),
                AccessDenied {
                    wallet,
                    key_id,
                    gate_type: config.gate_type,
                },
            );
        }

        Ok(allowed)
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
