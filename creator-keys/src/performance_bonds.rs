//! XLM performance bonds for creator keys.

use crate::{
    assert_is_admin, constants, extend_key_ttl_to_full_window, read_registered_creator_profile,
    BondError, FeatureKey,
};
use soroban_sdk::{contracttype, token::TokenClient, Address, Env, String, Symbol};

const BPS_DENOMINATOR: i128 = 10_000;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BondStatus {
    pub deposited: i128,
    pub slashed: i128,
    pub releasable: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BondSlashedEvent {
    pub key_id: Address,
    pub reason: String,
    pub slash_bps: u32,
    pub slashed_amount: i128,
}

fn xlm_token(env: &Env) -> Result<Address, BondError> {
    env.storage()
        .persistent()
        .get(&FeatureKey::XlmTokenAddress)
        .ok_or(BondError::TokenNotConfigured)
}

fn deposited(env: &Env, key_id: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&FeatureKey::PerformanceBond(key_id.clone()))
        .unwrap_or(0)
}

fn slashed(env: &Env, key_id: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&FeatureKey::BondSlashed(key_id.clone()))
        .unwrap_or(0)
}

fn released(env: &Env, key_id: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&FeatureKey::BondReleased(key_id.clone()))
        .unwrap_or(0)
}

fn releasable(env: &Env, key_id: &Address) -> Result<i128, BondError> {
    deposited(env, key_id)
        .checked_sub(slashed(env, key_id))
        .and_then(|amount| amount.checked_sub(released(env, key_id)))
        .ok_or(BondError::Overflow)
}

fn save_bond_value(env: &Env, key: FeatureKey, amount: i128) {
    env.storage().persistent().set(&key, &amount);
    extend_key_ttl_to_full_window(env, &key);
}

pub fn set_xlm_token_address(
    env: &Env,
    admin: &Address,
    token_address: &Address,
) -> Result<(), BondError> {
    admin.require_auth();
    assert_is_admin(env, admin).map_err(|_| BondError::Unauthorized)?;
    let key = FeatureKey::XlmTokenAddress;
    if env.storage().persistent().has(&key) {
        return Err(BondError::TokenAlreadyConfigured);
    }
    env.storage().persistent().set(&key, token_address);
    extend_key_ttl_to_full_window(env, &key);
    Ok(())
}

pub fn set_minimum_bond(env: &Env, admin: &Address, amount: i128) -> Result<(), BondError> {
    admin.require_auth();
    assert_is_admin(env, admin).map_err(|_| BondError::Unauthorized)?;
    if amount <= 0 {
        return Err(BondError::NotPositiveAmount);
    }
    let key = FeatureKey::MinPerformanceBond;
    save_bond_value(env, key, amount);
    Ok(())
}

/// Deposit the creator's XLM SAC tokens into this contract's bond escrow.
pub fn deposit(env: &Env, key_id: &Address, amount: i128) -> Result<(), BondError> {
    key_id.require_auth();
    read_registered_creator_profile(env, key_id).map_err(|_| BondError::NotRegistered)?;
    let trade_count: u64 = env
        .storage()
        .persistent()
        .get(&constants::storage::trade_count(key_id))
        .unwrap_or(0);
    if trade_count > 0 {
        return Err(BondError::AlreadyTrading);
    }
    if amount <= 0 {
        return Err(BondError::NotPositiveAmount);
    }
    let bond_key = FeatureKey::PerformanceBond(key_id.clone());
    if env.storage().persistent().has(&bond_key) {
        return Err(BondError::AlreadyDeposited);
    }
    let minimum: i128 = env
        .storage()
        .persistent()
        .get(&FeatureKey::MinPerformanceBond)
        .ok_or(BondError::BelowMinimumBond)?;
    if amount < minimum {
        return Err(BondError::BelowMinimumBond);
    }
    let token = xlm_token(env)?;
    TokenClient::new(env, &token).transfer(key_id, &env.current_contract_address(), &amount);
    save_bond_value(env, bond_key, amount);
    Ok(())
}

/// Prevent a creator from beginning trades without the configured escrow.
/// When no minimum has been configured, legacy keys retain their current behavior.
pub(crate) fn assert_trade_eligible(
    env: &Env,
    key_id: &Address,
) -> Result<(), crate::ContractError> {
    if let Some(minimum) = env
        .storage()
        .persistent()
        .get::<_, i128>(&FeatureKey::MinPerformanceBond)
    {
        if deposited(env, key_id) < minimum {
            return Err(crate::ContractError::PerformanceBondRequired);
        }
    }
    Ok(())
}

/// Called only after the configured protocol governance vote passes.
pub(crate) fn slash_after_governance(
    env: &Env,
    key_id: &Address,
    slash_bps: u32,
    reason: &String,
) -> Result<i128, BondError> {
    if slash_bps == 0 || slash_bps > 10_000 {
        return Err(BondError::InvalidSlashBps);
    }
    let remaining = releasable(env, key_id)?;
    if remaining == 0 {
        return Err(BondError::BondNotFound);
    }
    let slash_amount = remaining
        .checked_mul(i128::from(slash_bps))
        .and_then(|value| value.checked_div(BPS_DENOMINATOR))
        .ok_or(BondError::Overflow)?;
    if slash_amount == 0 {
        return Err(BondError::NotPositiveAmount);
    }
    let treasury = env
        .storage()
        .persistent()
        .get::<_, Address>(&constants::storage::TREASURY_ADDRESS)
        .ok_or(BondError::TreasuryNotConfigured)?;
    let token = xlm_token(env)?;
    TokenClient::new(env, &token).transfer(
        &env.current_contract_address(),
        &treasury,
        &slash_amount,
    );

    let next_slashed = slashed(env, key_id)
        .checked_add(slash_amount)
        .ok_or(BondError::Overflow)?;
    save_bond_value(env, FeatureKey::BondSlashed(key_id.clone()), next_slashed);
    env.events().publish(
        (Symbol::new(env, "BondSlashed"), key_id.clone()),
        BondSlashedEvent {
            key_id: key_id.clone(),
            reason: reason.clone(),
            slash_bps,
            slashed_amount: slash_amount,
        },
    );
    Ok(slash_amount)
}

/// Release the remaining escrow after a key has been retired or deprecated.
pub fn finalize_deprecation(env: &Env, key_id: &Address) -> Result<i128, BondError> {
    if !env
        .storage()
        .persistent()
        .has(&constants::storage::deprecated_key(key_id))
    {
        return Err(BondError::NotDeprecated);
    }
    release_remaining(env, key_id)
}

pub(crate) fn release_after_deprecation(env: &Env, key_id: &Address) -> Result<i128, BondError> {
    release_remaining(env, key_id)
}

fn release_remaining(env: &Env, key_id: &Address) -> Result<i128, BondError> {
    let amount = releasable(env, key_id)?;
    if amount == 0 {
        return Ok(0);
    }
    let token = xlm_token(env)?;
    TokenClient::new(env, &token).transfer(&env.current_contract_address(), key_id, &amount);
    let next_released = released(env, key_id)
        .checked_add(amount)
        .ok_or(BondError::Overflow)?;
    save_bond_value(env, FeatureKey::BondReleased(key_id.clone()), next_released);
    Ok(amount)
}

pub fn get_status(env: &Env, key_id: &Address) -> Result<BondStatus, BondError> {
    read_registered_creator_profile(env, key_id).map_err(|_| BondError::NotRegistered)?;
    Ok(BondStatus {
        deposited: deposited(env, key_id),
        slashed: slashed(env, key_id),
        releasable: releasable(env, key_id)?,
    })
}
