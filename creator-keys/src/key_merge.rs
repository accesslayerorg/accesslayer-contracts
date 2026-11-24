use crate::{constants, read_registered_creator_profile, ContractError, CreatorProfile};
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

pub const MIGRATION_WINDOW_LEDGERS: u32 = 17_280;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MergeStatus {
    pub key_a: Address,
    pub key_b: Address,
    pub merged_key_id: Address,
    pub migration_expiry: u32,
    pub deprecated: bool,
}

#[derive(Clone)]
#[contracttype]
enum MergeDataKey {
    Status(Address, Address),
}

fn status_key(key_a: &Address, key_b: &Address) -> MergeDataKey {
    MergeDataKey::Status(key_a.clone(), key_b.clone())
}

fn scaled(value: u32, ratio_bps: u32) -> Result<u32, ContractError> {
    u32::try_from(
        u64::from(value)
            .checked_mul(u64::from(ratio_bps))
            .ok_or(ContractError::Overflow)?
            / 10_000,
    )
    .map_err(|_| ContractError::Overflow)
}

pub fn merge(
    env: &Env,
    key_a: &Address,
    key_b: &Address,
    merge_ratio_a_bps: u32,
) -> Result<MergeStatus, ContractError> {
    key_a.require_auth();
    key_b.require_auth();
    if key_a == key_b || merge_ratio_a_bps == 0 || merge_ratio_a_bps > 10_000 {
        return Err(ContractError::InvalidFeeConfig);
    }
    let mut profile_a: CreatorProfile = read_registered_creator_profile(env, key_a)?;
    let mut profile_b: CreatorProfile = read_registered_creator_profile(env, key_b)?;
    if env
        .storage()
        .persistent()
        .get::<_, MergeStatus>(&status_key(key_a, key_b))
        .is_some()
    {
        return Err(ContractError::AlreadyRegistered);
    }

    let mut holders: Vec<Address> = env
        .storage()
        .persistent()
        .get(&constants::storage::holder_registry(key_a))
        .unwrap_or(Vec::new(env));
    let b_holders: Vec<Address> = env
        .storage()
        .persistent()
        .get(&constants::storage::holder_registry(key_b))
        .unwrap_or(Vec::new(env));
    for holder in b_holders.iter() {
        if !holders.contains(&holder) {
            holders.push_back(holder);
        }
    }

    for holder in holders.iter() {
        let a_key = constants::storage::holder_balance_key(key_a, &holder);
        let b_key = constants::storage::holder_balance_key(key_b, &holder);
        let a: u32 = env.storage().persistent().get(&a_key).unwrap_or(0);
        let b: u32 = env.storage().persistent().get(&b_key).unwrap_or(0);
        let migrated = a
            .checked_add(scaled(b, merge_ratio_a_bps)?)
            .ok_or(ContractError::Overflow)?;
        if migrated > 0 {
            env.storage().persistent().set(&a_key, &migrated);
        } else {
            env.storage().persistent().remove(&a_key);
        }
        env.storage().persistent().remove(&b_key);

        let a_staked_key = constants::storage::staked_balance(key_a, &holder);
        let b_staked_key = constants::storage::staked_balance(key_b, &holder);
        let a_staked: u32 = env.storage().persistent().get(&a_staked_key).unwrap_or(0);
        let b_staked: u32 = env.storage().persistent().get(&b_staked_key).unwrap_or(0);
        let migrated_staked = a_staked
            .checked_add(scaled(b_staked, merge_ratio_a_bps)?)
            .ok_or(ContractError::Overflow)?;
        if migrated_staked > 0 {
            env.storage()
                .persistent()
                .set(&a_staked_key, &migrated_staked);
        } else {
            env.storage().persistent().remove(&a_staked_key);
        }
        env.storage().persistent().remove(&b_staked_key);
    }

    profile_a.supply = profile_a
        .supply
        .checked_add(scaled(profile_b.supply, merge_ratio_a_bps)?)
        .ok_or(ContractError::Overflow)?;
    profile_a.holder_count = holders
        .iter()
        .filter(|holder| {
            env.storage()
                .persistent()
                .get::<_, u32>(&constants::storage::holder_balance_key(key_a, holder))
                .unwrap_or(0)
                > 0
        })
        .count() as u32;
    profile_b.supply = 0;
    profile_b.holder_count = 0;
    env.storage()
        .persistent()
        .set(&constants::storage::creator(key_a), &profile_a);
    env.storage()
        .persistent()
        .set(&constants::storage::creator(key_b), &profile_b);

    let status = MergeStatus {
        key_a: key_a.clone(),
        key_b: key_b.clone(),
        merged_key_id: key_a.clone(),
        migration_expiry: env
            .ledger()
            .sequence()
            .checked_add(MIGRATION_WINDOW_LEDGERS)
            .ok_or(ContractError::Overflow)?,
        deprecated: false,
    };
    env.storage()
        .persistent()
        .set(&status_key(key_a, key_b), &status);
    env.events().publish(
        (
            Symbol::new(env, "KeysMerged"),
            key_a.clone(),
            key_b.clone(),
            key_a.clone(),
        ),
        status.migration_expiry,
    );
    Ok(status)
}

pub fn finalize_merge(env: &Env, key_a: &Address, key_b: &Address) -> Result<(), ContractError> {
    let key = status_key(key_a, key_b);
    let mut status: MergeStatus = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::NotRegistered)?;
    if env.ledger().sequence() < status.migration_expiry {
        return Err(ContractError::TimelockNotElapsed);
    }
    if !status.deprecated {
        env.storage()
            .persistent()
            .set(&constants::storage::deprecated_key(key_b), &0i128);
        status.deprecated = true;
        env.storage().persistent().set(&key, &status);
    }
    Ok(())
}

pub fn get_merge_status(env: &Env, key_a: &Address, key_b: &Address) -> Option<MergeStatus> {
    env.storage().persistent().get(&status_key(key_a, key_b))
}
