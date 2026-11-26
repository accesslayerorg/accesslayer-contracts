use crate::{constants, curve_subscriptions_swaps, ContractError};
use soroban_sdk::{contracttype, Address, Env, Symbol};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum GateType {
    Holder,
    Subscriber,
    Threshold,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct GateConfig {
    pub gate_type: GateType,
    /// Minimum balance for `Threshold`; ignored for other gate types.
    pub min_balance: u32,
}

#[derive(Clone)]
#[contracttype]
enum GateDataKey {
    Config(Address),
}

pub const ACCESS_GRANTED_EVENT_NAME: &str = "AccessGranted";
pub const ACCESS_DENIED_EVENT_NAME: &str = "AccessDenied";

fn balance(env: &Env, key_id: &Address, wallet: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&constants::storage::holder_balance_key(key_id, wallet))
        .unwrap_or(0)
}

pub fn set_gate(
    env: &Env,
    key_id: &Address,
    gate_type: GateType,
    min_balance: u32,
) -> Result<(), ContractError> {
    key_id.require_auth();
    crate::read_registered_creator_profile(env, key_id)?;
    if matches!(gate_type, GateType::Threshold) && min_balance == 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    env.storage().persistent().set(
        &GateDataKey::Config(key_id.clone()),
        &GateConfig {
            gate_type,
            min_balance,
        },
    );
    Ok(())
}

pub fn get_gate_config(env: &Env, key_id: &Address) -> Option<GateConfig> {
    env.storage()
        .persistent()
        .get(&GateDataKey::Config(key_id.clone()))
}

pub fn check_access(env: &Env, wallet: &Address, key_id: &Address) -> bool {
    let config = get_gate_config(env, key_id);
    let allowed = match config.as_ref() {
        Some(config) => {
            let current_balance = balance(env, key_id, wallet);
            match config.gate_type {
                GateType::Holder => current_balance > 0,
                GateType::Threshold => current_balance >= config.min_balance,
                GateType::Subscriber => {
                    curve_subscriptions_swaps::is_subscribed(env, key_id, wallet, current_balance)
                }
            }
        }
        None => false,
    };
    let gate_type = config.map(|c| c.gate_type).unwrap_or(GateType::Holder);
    let event = if allowed {
        Symbol::new(env, ACCESS_GRANTED_EVENT_NAME)
    } else {
        Symbol::new(env, ACCESS_DENIED_EVENT_NAME)
    };
    env.events()
        .publish((event, wallet.clone(), key_id.clone()), gate_type);
    allowed
}
