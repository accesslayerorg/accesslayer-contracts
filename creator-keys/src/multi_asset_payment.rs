use crate::{ContractError, CreatorKeysContract};
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

pub const INTERNAL_DECIMALS: u32 = 7;
pub const DEFAULT_RATE_STALENESS_SECS: u64 = 3_600;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PaymentAssetInfo {
    pub token: Address,
    pub decimals: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PaymentAssetConfig {
    pub token: Address,
    pub decimals: u32,
    pub rate_oracle: Address,
    pub is_active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AssetRate {
    pub rate: i128,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AssetRateView {
    pub rate: i128,
    pub updated_at: u64,
    pub is_stale: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum MultiAssetDataKey {
    AssetConfig(Address),
    AssetList,
    AssetRate(Address),
    AssetStalenessThreshold(Address),
    AssetReserve(Address),
    BaseReserve,
}

pub const PAYMENT_ASSET_ADDED_EVENT: &str = "payment_asset_added";
pub const PAYMENT_ASSET_REMOVED_EVENT: &str = "payment_asset_removed";
pub const BOUGHT_WITH_EVENT: &str = "bought_with";

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PaymentAssetAddedEvent {
    pub admin: Address,
    pub token: Address,
    pub decimals: u32,
    pub rate_oracle: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PaymentAssetRemovedEvent {
    pub admin: Address,
    pub token: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BoughtWithEvent {
    pub buyer: Address,
    pub key_id: Address,
    pub amount: u32,
    pub token: Address,
    pub token_paid: i128,
    pub base_cost: i128,
}

/// Normalizes external token amounts to 7-decimal internal accounting.
pub fn to_internal(amount: i128, decimals: u32) -> Result<i128, ContractError> {
    if amount < 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    if decimals == INTERNAL_DECIMALS {
        Ok(amount)
    } else if decimals < INTERNAL_DECIMALS {
        let diff = INTERNAL_DECIMALS - decimals;
        let factor = 10i128.checked_pow(diff).ok_or(ContractError::Overflow)?;
        amount.checked_mul(factor).ok_or(ContractError::Overflow)
    } else {
        let diff = decimals - INTERNAL_DECIMALS;
        let factor = 10i128.checked_pow(diff).ok_or(ContractError::Overflow)?;
        Ok(amount / factor)
    }
}

/// Converts from 7-decimal internal accounting to external token decimals,
/// rounding in favor of the protocol.
pub fn from_internal(amount: i128, decimals: u32) -> Result<i128, ContractError> {
    if amount < 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    if decimals == INTERNAL_DECIMALS {
        Ok(amount)
    } else if decimals < INTERNAL_DECIMALS {
        let diff = INTERNAL_DECIMALS - decimals;
        let factor = 10i128.checked_pow(diff).ok_or(ContractError::Overflow)?;
        let rem = amount % factor;
        let div = amount / factor;
        if rem > 0 {
            div.checked_add(1).ok_or(ContractError::Overflow)
        } else {
            Ok(div)
        }
    } else {
        let diff = decimals - INTERNAL_DECIMALS;
        let factor = 10i128.checked_pow(diff).ok_or(ContractError::Overflow)?;
        amount.checked_mul(factor).ok_or(ContractError::Overflow)
    }
}

/// Registers a payment asset with decimal normalization and associated rate oracle.
pub fn add_payment_asset(
    env: &Env,
    admin: &Address,
    token: &Address,
    decimals: u32,
    rate_oracle: &Address,
) -> Result<(), ContractError> {
    admin.require_auth();
    let configured_admin: Address = env
        .storage()
        .persistent()
        .get(&crate::constants::storage::ADMIN_ADDRESS)
        .ok_or(ContractError::Unauthorized)?;
    if *admin != configured_admin {
        return Err(ContractError::Unauthorized);
    }

    if decimals == 0 || decimals > 24 {
        return Err(ContractError::InvalidVaultInput);
    }

    let config = PaymentAssetConfig {
        token: token.clone(),
        decimals,
        rate_oracle: rate_oracle.clone(),
        is_active: true,
    };
    env.storage()
        .persistent()
        .set(&MultiAssetDataKey::AssetConfig(token.clone()), &config);

    let mut list: Vec<Address> = env
        .storage()
        .persistent()
        .get(&MultiAssetDataKey::AssetList)
        .unwrap_or_else(|| Vec::new(env));
    let mut exists = false;
    for i in 0..list.len() {
        if list.get(i).unwrap() == *token {
            exists = true;
            break;
        }
    }
    if !exists {
        list.push_back(token.clone());
        env.storage()
            .persistent()
            .set(&MultiAssetDataKey::AssetList, &list);
    }

    let rate_key = MultiAssetDataKey::AssetRate(token.clone());
    if !env.storage().persistent().has(&rate_key) {
        env.storage().persistent().set(
            &rate_key,
            &AssetRate {
                rate: 10_000_000i128,
                updated_at: env.ledger().timestamp(),
            },
        );
    }

    env.events().publish(
        (Symbol::new(env, PAYMENT_ASSET_ADDED_EVENT), token.clone()),
        PaymentAssetAddedEvent {
            admin: admin.clone(),
            token: token.clone(),
            decimals,
            rate_oracle: rate_oracle.clone(),
        },
    );

    Ok(())
}

/// Deactivates a payment asset from accepting new buys, preserving balance redemptions.
pub fn remove_payment_asset(
    env: &Env,
    admin: &Address,
    token: &Address,
) -> Result<(), ContractError> {
    admin.require_auth();
    let configured_admin: Address = env
        .storage()
        .persistent()
        .get(&crate::constants::storage::ADMIN_ADDRESS)
        .ok_or(ContractError::Unauthorized)?;
    if *admin != configured_admin {
        return Err(ContractError::Unauthorized);
    }

    let config_key = MultiAssetDataKey::AssetConfig(token.clone());
    let mut config: PaymentAssetConfig = env
        .storage()
        .persistent()
        .get(&config_key)
        .ok_or(ContractError::NotRegistered)?;

    config.is_active = false;
    env.storage().persistent().set(&config_key, &config);

    let list: Vec<Address> = env
        .storage()
        .persistent()
        .get(&MultiAssetDataKey::AssetList)
        .unwrap_or_else(|| Vec::new(env));
    let mut new_list = Vec::new(env);
    for i in 0..list.len() {
        let t = list.get(i).unwrap();
        if t != *token {
            new_list.push_back(t);
        }
    }
    env.storage()
        .persistent()
        .set(&MultiAssetDataKey::AssetList, &new_list);

    env.events().publish(
        (Symbol::new(env, PAYMENT_ASSET_REMOVED_EVENT), token.clone()),
        PaymentAssetRemovedEvent {
            admin: admin.clone(),
            token: token.clone(),
        },
    );

    Ok(())
}

/// Returns the allowed payment assets with their token address and decimals.
pub fn payment_assets(env: &Env) -> Vec<PaymentAssetInfo> {
    let list: Vec<Address> = env
        .storage()
        .persistent()
        .get(&MultiAssetDataKey::AssetList)
        .unwrap_or_else(|| Vec::new(env));
    let mut result = Vec::new(env);
    for i in 0..list.len() {
        let token = list.get(i).unwrap();
        if let Some(config) = env
            .storage()
            .persistent()
            .get::<_, PaymentAssetConfig>(&MultiAssetDataKey::AssetConfig(token.clone()))
        {
            if config.is_active {
                result.push_back(PaymentAssetInfo {
                    token: config.token,
                    decimals: config.decimals,
                });
            }
        }
    }
    result
}

/// Retrieves the configured rate with staleness checking.
pub fn get_asset_rate(env: &Env, token: &Address) -> Result<AssetRateView, ContractError> {
    let config: PaymentAssetConfig = env
        .storage()
        .persistent()
        .get(&MultiAssetDataKey::AssetConfig(token.clone()))
        .ok_or(ContractError::Unauthorized)?;

    let threshold: u64 = env
        .storage()
        .persistent()
        .get(&MultiAssetDataKey::AssetStalenessThreshold(token.clone()))
        .unwrap_or(DEFAULT_RATE_STALENESS_SECS);

    let (rate, updated_at) = if let Ok(Ok(view)) = env
        .try_invoke_contract::<crate::OraclePriceView, soroban_sdk::Error>(
            &config.rate_oracle,
            &Symbol::new(env, "get_oracle_price"),
            Vec::new(env),
        ) {
        (view.price, view.updated_at)
    } else if let Ok(Ok(price)) = env.try_invoke_contract::<i128, soroban_sdk::Error>(
        &config.rate_oracle,
        &Symbol::new(env, "get_price"),
        Vec::new(env),
    ) {
        (price, env.ledger().timestamp())
    } else {
        let stored: AssetRate = env
            .storage()
            .persistent()
            .get(&MultiAssetDataKey::AssetRate(token.clone()))
            .ok_or(ContractError::OraclePriceNotSet)?;
        (stored.rate, stored.updated_at)
    };

    if rate <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    let age = env.ledger().timestamp().saturating_sub(updated_at);
    let is_stale = age > threshold;

    Ok(AssetRateView {
        rate,
        updated_at,
        is_stale,
    })
}

/// Sets the asset rate by an authorized rate oracle.
pub fn set_asset_rate(
    env: &Env,
    oracle: &Address,
    token: &Address,
    rate: i128,
) -> Result<(), ContractError> {
    oracle.require_auth();
    let config: PaymentAssetConfig = env
        .storage()
        .persistent()
        .get(&MultiAssetDataKey::AssetConfig(token.clone()))
        .ok_or(ContractError::Unauthorized)?;

    if *oracle != config.rate_oracle {
        return Err(ContractError::Unauthorized);
    }
    if rate <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    let timestamp = env.ledger().timestamp();
    env.storage().persistent().set(
        &MultiAssetDataKey::AssetRate(token.clone()),
        &AssetRate {
            rate,
            updated_at: timestamp,
        },
    );

    Ok(())
}

/// Configures the staleness threshold in seconds for a token rate.
pub fn set_asset_staleness(
    env: &Env,
    admin: &Address,
    token: &Address,
    threshold_secs: u64,
) -> Result<(), ContractError> {
    admin.require_auth();
    let configured_admin: Address = env
        .storage()
        .persistent()
        .get(&crate::constants::storage::ADMIN_ADDRESS)
        .ok_or(ContractError::Unauthorized)?;
    if *admin != configured_admin {
        return Err(ContractError::Unauthorized);
    }

    env.storage().persistent().set(
        &MultiAssetDataKey::AssetStalenessThreshold(token.clone()),
        &threshold_secs,
    );
    Ok(())
}

/// Purchases keys using an allowlisted payment token with decimal normalization,
/// oracle conversion, protocol-favorable rounding, and max_pay protection.
pub fn buy_with(
    env: &Env,
    buyer: &Address,
    key_id: &Address,
    amount: u32,
    token: &Address,
    max_pay: i128,
) -> Result<u32, ContractError> {
    buyer.require_auth();

    if amount == 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    if max_pay <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    let config: PaymentAssetConfig = env
        .storage()
        .persistent()
        .get(&MultiAssetDataKey::AssetConfig(token.clone()))
        .ok_or(ContractError::Unauthorized)?;

    if !config.is_active {
        return Err(ContractError::Unauthorized);
    }

    let rate_view = get_asset_rate(env, token)?;
    if rate_view.is_stale {
        return Err(ContractError::OraclePriceNotSet);
    }
    let rate = rate_view.rate;

    let sim = CreatorKeysContract::simulate_buy(env.clone(), key_id.clone(), amount);
    let base_cost = sim.total_cost;
    if base_cost <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    let numerator = base_cost
        .checked_mul(10_000_000)
        .ok_or(ContractError::Overflow)?;
    let required_token_internal = numerator
        .checked_add(rate - 1)
        .ok_or(ContractError::Overflow)?
        .checked_div(rate)
        .ok_or(ContractError::Overflow)?;

    let token_amount = from_internal(required_token_internal, config.decimals)?;

    if token_amount > max_pay {
        return Err(ContractError::SlippageExceeded);
    }

    let outcome = CreatorKeysContract::buy_keys_core(
        env.clone(),
        key_id.clone(),
        buyer.clone(),
        amount,
        base_cost,
        None,
        None,
        None,
    )?;

    let reserve_key = MultiAssetDataKey::AssetReserve(token.clone());
    let current_asset_res: i128 = env.storage().persistent().get(&reserve_key).unwrap_or(0);
    let new_asset_res = current_asset_res
        .checked_add(token_amount)
        .ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&reserve_key, &new_asset_res);

    let base_reserve_key = MultiAssetDataKey::BaseReserve;
    let current_base_res: i128 = env
        .storage()
        .persistent()
        .get(&base_reserve_key)
        .unwrap_or(0);
    let new_base_res = current_base_res
        .checked_add(base_cost)
        .ok_or(ContractError::Overflow)?;
    env.storage()
        .persistent()
        .set(&base_reserve_key, &new_base_res);

    env.events().publish(
        (
            Symbol::new(env, BOUGHT_WITH_EVENT),
            buyer.clone(),
            key_id.clone(),
        ),
        BoughtWithEvent {
            buyer: buyer.clone(),
            key_id: key_id.clone(),
            amount,
            token: token.clone(),
            token_paid: token_amount,
            base_cost,
        },
    );

    Ok(outcome.new_supply)
}

/// Allows redeeming accumulated reserve balance of a payment asset (even if removed).
pub fn redeem_payment_asset(
    env: &Env,
    admin: &Address,
    token: &Address,
    amount: i128,
) -> Result<i128, ContractError> {
    admin.require_auth();
    let configured_admin: Address = env
        .storage()
        .persistent()
        .get(&crate::constants::storage::ADMIN_ADDRESS)
        .ok_or(ContractError::Unauthorized)?;
    if *admin != configured_admin {
        return Err(ContractError::Unauthorized);
    }

    if amount <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    let reserve_key = MultiAssetDataKey::AssetReserve(token.clone());
    let current_asset_res: i128 = env.storage().persistent().get(&reserve_key).unwrap_or(0);
    if amount > current_asset_res {
        return Err(ContractError::InsufficientBalance);
    }

    let new_res = current_asset_res - amount;
    env.storage().persistent().set(&reserve_key, &new_res);

    Ok(amount)
}

/// Retrieves the recorded reserve balance for a payment asset.
pub fn get_asset_reserve(env: &Env, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&MultiAssetDataKey::AssetReserve(token.clone()))
        .unwrap_or(0)
}

/// Retrieves the recorded total base reserve balance.
pub fn get_base_reserve(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&MultiAssetDataKey::BaseReserve)
        .unwrap_or(0)
}
