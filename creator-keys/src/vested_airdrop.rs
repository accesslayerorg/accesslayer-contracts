//! Vested Merkle airdrops (#1291).
//!
//! Claimed keys are credited to the recipient's key balance but locked from
//! sale and transfer until their cliff has passed and linear release has
//! progressed. Because the keys are part of the holder balance, they still
//! count toward governance voting weight. Creators may optionally retain the
//! right to revoke only the still-unvested portion.

use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol, Vec};

use crate::{
    append_holder_to_registry, constants, extend_key_ttl_to_full_window,
    issue_primitives::{airdrop_leaf, verify_merkle_proof},
    read_registered_creator_profile, settle_holder_dividends, ContractError, CreatorProfile,
};

pub const VESTED_CLAIM_EVENT: &str = "vested_claim";
pub const RELEASED_EVENT: &str = "released";
pub const VESTING_REVOKED_EVENT: &str = "vesting_revoked";

#[derive(Clone)]
#[contracttype]
pub enum VestedAirdropKey {
    NextAirdropId,
    NextPositionId,
    Airdrop(u32),
    Claimed(u32, BytesN<32>),
    Position(u32),
    HolderPositions(Address, Address),
    LockedBalance(Address, Address),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestedAirdrop {
    pub creator: Address,
    pub key_id: Address,
    pub root: BytesN<32>,
    pub total: u32,
    pub claimed: u32,
    pub cliff_ledgers: u32,
    pub vest_ledgers: u32,
    pub revocable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestedPosition {
    pub holder: Address,
    pub key_id: Address,
    pub airdrop_id: u32,
    /// Original number of keys credited by the claim.
    pub amount: u32,
    /// Keys already unlocked by [`release`].
    pub released: u32,
    /// Unvested keys removed by [`revoke_unvested`].
    pub revoked: u32,
    pub start_ledger: u32,
    pub cliff_ledger: u32,
    pub vest_ledgers: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestedClaimEvent {
    pub airdrop_id: u32,
    pub position_id: u32,
    pub amount: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReleasedEvent {
    pub position_id: u32,
    pub amount: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestingRevokedEvent {
    pub position_id: u32,
    pub amount: u32,
}

/// Creates an airdrop whose unvested keys cannot be revoked.
pub fn create_vested_airdrop(
    env: &Env,
    creator: &Address,
    key_id: &Address,
    root: BytesN<32>,
    total: u32,
    cliff_ledgers: u32,
    vest_ledgers: u32,
) -> Result<u32, ContractError> {
    create_vested_airdrop_with_revocability(
        env,
        creator,
        key_id,
        root,
        total,
        cliff_ledgers,
        vest_ledgers,
        false,
    )
}

/// Full creation entrypoint; `revocable` is fixed for the life of the airdrop.
#[allow(clippy::too_many_arguments)]
pub fn create_vested_airdrop_with_revocability(
    env: &Env,
    creator: &Address,
    key_id: &Address,
    root: BytesN<32>,
    total: u32,
    cliff_ledgers: u32,
    vest_ledgers: u32,
    revocable: bool,
) -> Result<u32, ContractError> {
    creator.require_auth();
    let profile = read_registered_creator_profile(env, key_id)?;
    if profile.creator != *creator {
        return Err(ContractError::Unauthorized);
    }
    if total == 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    if vest_ledgers == 0 && cliff_ledgers == 0 {
        return Err(ContractError::InvalidVestingSchedule);
    }

    let key = VestedAirdropKey::NextAirdropId;
    let id: u32 = env.storage().instance().get(&key).unwrap_or(1);
    let next = id.checked_add(1).ok_or(ContractError::Overflow)?;
    env.storage().instance().set(&key, &next);
    env.storage().persistent().set(
        &VestedAirdropKey::Airdrop(id),
        &VestedAirdrop {
            creator: creator.clone(),
            key_id: key_id.clone(),
            root,
            total,
            claimed: 0,
            cliff_ledgers,
            vest_ledgers,
            revocable,
        },
    );
    Ok(id)
}

/// Verifies a Merkle leaf and credits locked keys to the claimer.
pub fn claim_vested(
    env: &Env,
    claimer: &Address,
    airdrop_id: u32,
    amount: u32,
    proof: &Vec<BytesN<32>>,
) -> Result<u32, ContractError> {
    claimer.require_auth();
    crate::assert_not_paused(env)?;
    if amount == 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    let airdrop_key = VestedAirdropKey::Airdrop(airdrop_id);
    let mut airdrop: VestedAirdrop = env
        .storage()
        .persistent()
        .get(&airdrop_key)
        .ok_or(ContractError::VestedAirdropNotFound)?;
    let leaf = airdrop_leaf(env, claimer, amount as i128);
    if !verify_merkle_proof(env, &airdrop.root, leaf.clone(), proof) {
        return Err(ContractError::InvalidVestedAirdropProof);
    }
    let claimed_key = VestedAirdropKey::Claimed(airdrop_id, leaf);
    if env.storage().persistent().has(&claimed_key) {
        return Err(ContractError::VestedAirdropAlreadyClaimed);
    }
    let new_claimed = airdrop
        .claimed
        .checked_add(amount)
        .ok_or(ContractError::Overflow)?;
    if new_claimed > airdrop.total {
        return Err(ContractError::VestedAirdropExhausted);
    }

    let profile_key = constants::storage::creator(&airdrop.key_id);
    let mut profile: CreatorProfile = env
        .storage()
        .persistent()
        .get(&profile_key)
        .ok_or(ContractError::NotRegistered)?;
    if let Some(cap) = env
        .storage()
        .persistent()
        .get::<_, u32>(&constants::storage::max_supply(&airdrop.key_id))
    {
        if profile.supply.saturating_add(amount) > cap {
            return Err(ContractError::SupplyCapExceeded);
        }
    }

    let balance_key = constants::storage::holder_balance_key(&airdrop.key_id, claimer);
    let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
    settle_holder_dividends(env, &airdrop.key_id, claimer, balance)?;
    if balance == 0 {
        profile.holder_count = profile
            .holder_count
            .checked_add(1)
            .ok_or(ContractError::Overflow)?;
        append_holder_to_registry(env, &airdrop.key_id, claimer);
    }
    let new_balance = balance.checked_add(amount).ok_or(ContractError::Overflow)?;
    let locked_key = VestedAirdropKey::LockedBalance(airdrop.key_id.clone(), claimer.clone());
    let locked: u32 = env.storage().persistent().get(&locked_key).unwrap_or(0);
    env.storage().persistent().set(
        &locked_key,
        &locked.checked_add(amount).ok_or(ContractError::Overflow)?,
    );
    extend_key_ttl_to_full_window(env, &locked_key);
    env.storage().persistent().set(&balance_key, &new_balance);
    profile.supply = profile
        .supply
        .checked_add(amount)
        .ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&profile_key, &profile);

    let now = env.ledger().sequence();
    let position_key = VestedAirdropKey::NextPositionId;
    let position_id: u32 = env.storage().instance().get(&position_key).unwrap_or(1);
    env.storage().instance().set(
        &position_key,
        &position_id.checked_add(1).ok_or(ContractError::Overflow)?,
    );
    let position = VestedPosition {
        holder: claimer.clone(),
        key_id: airdrop.key_id.clone(),
        airdrop_id,
        amount,
        released: 0,
        revoked: 0,
        start_ledger: now,
        cliff_ledger: now.saturating_add(airdrop.cliff_ledgers),
        vest_ledgers: airdrop.vest_ledgers,
    };
    env.storage()
        .persistent()
        .set(&VestedAirdropKey::Position(position_id), &position);
    let positions_key = VestedAirdropKey::HolderPositions(claimer.clone(), airdrop.key_id.clone());
    let mut positions: Vec<u32> = env
        .storage()
        .persistent()
        .get(&positions_key)
        .unwrap_or_else(|| Vec::new(env));
    positions.push_back(position_id);
    env.storage().persistent().set(&positions_key, &positions);

    airdrop.claimed = new_claimed;
    env.storage().persistent().set(&claimed_key, &true);
    env.storage().persistent().set(&airdrop_key, &airdrop);

    env.events().publish(
        (
            Symbol::new(env, VESTED_CLAIM_EVENT),
            claimer.clone(),
            airdrop_id,
        ),
        VestedClaimEvent {
            airdrop_id,
            position_id,
            amount,
        },
    );
    Ok(position_id)
}

/// Linear vesting after the cliff. No key is releasable before the cliff.
pub fn vested_amount_at(position: &VestedPosition, ledger: u32) -> u32 {
    let cap = position.amount.saturating_sub(position.revoked);
    if ledger < position.cliff_ledger {
        return 0;
    }
    if position.vest_ledgers == 0 {
        return cap;
    }
    let elapsed = ledger.saturating_sub(position.cliff_ledger);
    if elapsed >= position.vest_ledgers {
        cap
    } else {
        (u64::from(cap) * u64::from(elapsed) / u64::from(position.vest_ledgers)) as u32
    }
}

/// Unlocks the vested portion of a position for sale and transfer.
pub fn release(env: &Env, holder: &Address, position_id: u32) -> Result<u32, ContractError> {
    holder.require_auth();
    let key = VestedAirdropKey::Position(position_id);
    let mut position: VestedPosition = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::VestedPositionNotFound)?;
    if position.holder != *holder {
        return Err(ContractError::VestedPositionNotOwned);
    }
    let vested = vested_amount_at(&position, env.ledger().sequence());
    let releasable = vested.saturating_sub(position.released);
    if releasable == 0 {
        return Err(ContractError::NothingToRelease);
    }
    position.released = position
        .released
        .checked_add(releasable)
        .ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&key, &position);
    subtract_locked(env, &position.key_id, holder, releasable)?;

    env.events().publish(
        (
            Symbol::new(env, RELEASED_EVENT),
            holder.clone(),
            position_id,
        ),
        ReleasedEvent {
            position_id,
            amount: releasable,
        },
    );
    Ok(releasable)
}

/// Removes only unvested keys, and only when the creator enabled revocability.
pub fn revoke_unvested(
    env: &Env,
    creator: &Address,
    position_id: u32,
) -> Result<u32, ContractError> {
    creator.require_auth();
    let position_key = VestedAirdropKey::Position(position_id);
    let mut position: VestedPosition = env
        .storage()
        .persistent()
        .get(&position_key)
        .ok_or(ContractError::VestedPositionNotFound)?;
    let airdrop: VestedAirdrop = env
        .storage()
        .persistent()
        .get(&VestedAirdropKey::Airdrop(position.airdrop_id))
        .ok_or(ContractError::VestedAirdropNotFound)?;
    if airdrop.creator != *creator {
        return Err(ContractError::Unauthorized);
    }
    if !airdrop.revocable {
        return Err(ContractError::RevocationNotEnabled);
    }
    let vested = vested_amount_at(&position, env.ledger().sequence());
    let unvested = position
        .amount
        .saturating_sub(position.revoked)
        .saturating_sub(vested);
    if unvested == 0 {
        return Err(ContractError::NothingToRevoke);
    }

    position.revoked = position
        .revoked
        .checked_add(unvested)
        .ok_or(ContractError::Overflow)?;
    env.storage().persistent().set(&position_key, &position);
    subtract_locked(env, &position.key_id, &position.holder, unvested)?;

    let balance_key = constants::storage::holder_balance_key(&position.key_id, &position.holder);
    let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
    let new_balance = balance.saturating_sub(unvested);
    env.storage().persistent().set(&balance_key, &new_balance);
    let profile_key = constants::storage::creator(&position.key_id);
    if let Some(mut profile) = env
        .storage()
        .persistent()
        .get::<_, CreatorProfile>(&profile_key)
    {
        profile.supply = profile.supply.saturating_sub(unvested);
        if new_balance == 0 {
            profile.holder_count = profile.holder_count.saturating_sub(1);
        }
        env.storage().persistent().set(&profile_key, &profile);
    }

    env.events().publish(
        (
            Symbol::new(env, VESTING_REVOKED_EVENT),
            creator.clone(),
            position_id,
        ),
        VestingRevokedEvent {
            position_id,
            amount: unvested,
        },
    );
    Ok(unvested)
}

/// Keys credited to `holder` that are still locked by vesting positions.
pub fn locked_balance(env: &Env, key_id: &Address, holder: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&VestedAirdropKey::LockedBalance(
            key_id.clone(),
            holder.clone(),
        ))
        .unwrap_or(0)
}

pub fn get_position(env: &Env, position_id: u32) -> Option<VestedPosition> {
    env.storage()
        .persistent()
        .get(&VestedAirdropKey::Position(position_id))
}

pub fn get_airdrop(env: &Env, airdrop_id: u32) -> Option<VestedAirdrop> {
    env.storage()
        .persistent()
        .get(&VestedAirdropKey::Airdrop(airdrop_id))
}

pub fn vested_amount(env: &Env, position_id: u32) -> u32 {
    get_position(env, position_id)
        .map(|position| vested_amount_at(&position, env.ledger().sequence()))
        .unwrap_or(0)
}

fn subtract_locked(
    env: &Env,
    key_id: &Address,
    holder: &Address,
    amount: u32,
) -> Result<(), ContractError> {
    let key = VestedAirdropKey::LockedBalance(key_id.clone(), holder.clone());
    let locked: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    if locked < amount {
        return Err(ContractError::Overflow);
    }
    let remaining = locked - amount;
    if remaining == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &remaining);
        extend_key_ttl_to_full_window(env, &key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{constants, CreatorKeysContract, CreatorKeysContractClient, CreatorProfile};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        String,
    };

    struct Setup<'a> {
        env: Env,
        client: CreatorKeysContractClient<'a>,
        creator: Address,
        key_id: Address,
        claimer: Address,
        airdrop_id: u32,
        position_id: u32,
    }

    fn setup(revocable: bool, cliff_ledgers: u32, vest_ledgers: u32) -> Setup<'static> {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.sequence_number = 10);

        let contract = env.register(CreatorKeysContract, ());
        let client = CreatorKeysContractClient::new(&env, &contract);
        let creator = Address::generate(&env);
        let key_id = Address::generate(&env);
        let claimer = Address::generate(&env);
        let amount: u32 = 100;

        let fee_recipient = Address::generate(&env);
        env.as_contract(&contract, || {
            env.storage().persistent().set(
                &constants::storage::creator(&key_id),
                &CreatorProfile {
                    creator: creator.clone(),
                    handle: String::from_str(&env, "vested-key"),
                    supply: 0,
                    holder_count: 0,
                    fee_recipient,
                    registered_at: 10,
                },
            );
        });

        let root = airdrop_leaf(&env, &claimer, i128::from(amount));
        let airdrop_id = if revocable {
            client.create_revocable_vested_airdrop(
                &creator,
                &key_id,
                &root,
                &amount,
                &cliff_ledgers,
                &vest_ledgers,
                &true,
            )
        } else {
            client.create_vested_airdrop(
                &creator,
                &key_id,
                &root,
                &amount,
                &cliff_ledgers,
                &vest_ledgers,
            )
        };
        let position_id = client.claim_vested(
            &claimer,
            &airdrop_id,
            &amount,
            &Vec::<BytesN<32>>::new(&env),
        );

        Setup {
            env,
            client,
            creator,
            key_id,
            claimer,
            airdrop_id,
            position_id,
        }
    }

    #[test]
    fn linear_release_starts_at_zero_after_cliff() {
        let position = VestedPosition {
            holder: Address::generate(&Env::default()),
            key_id: Address::generate(&Env::default()),
            airdrop_id: 1,
            amount: 1_000,
            released: 0,
            revoked: 0,
            start_ledger: 0,
            cliff_ledger: 100,
            vest_ledgers: 100,
        };
        assert_eq!(vested_amount_at(&position, 99), 0);
        assert_eq!(vested_amount_at(&position, 100), 0);
        assert_eq!(vested_amount_at(&position, 150), 500);
        assert_eq!(vested_amount_at(&position, 200), 1_000);
    }

    #[test]
    fn claimed_keys_are_locked_but_count_for_voting() {
        let s = setup(false, 10, 100);
        assert_eq!(s.client.get_locked_key_balance(&s.key_id, &s.claimer), 100);
        assert_eq!(s.client.get_liquid_balance(&s.key_id, &s.claimer), 0);
        assert_eq!(s.client.get_voting_weight(&s.claimer, &s.key_id), 100);

        let recipient = Address::generate(&s.env);
        assert!(s
            .client
            .try_transfer_keys(&s.key_id, &s.claimer, &recipient, &1)
            .is_err());
        assert!(s
            .client
            .try_claim_vested(
                &s.claimer,
                &s.airdrop_id,
                &100,
                &Vec::<BytesN<32>>::new(&s.env),
            )
            .is_err());
        assert!(s.client.try_release(&s.claimer, &s.position_id).is_err());

        s.env.ledger().with_mut(|l| l.sequence_number = 20);
        assert_eq!(s.client.vested_amount(&s.position_id), 0);
        assert!(s.client.try_release(&s.claimer, &s.position_id).is_err());

        s.env.ledger().with_mut(|l| l.sequence_number = 70);
        assert_eq!(s.client.vested_amount(&s.position_id), 50);
        assert_eq!(s.client.release(&s.claimer, &s.position_id), 50);
        assert_eq!(s.client.get_locked_key_balance(&s.key_id, &s.claimer), 50);
        assert_eq!(s.client.get_liquid_balance(&s.key_id, &s.claimer), 50);
        assert_eq!(s.client.get_voting_weight(&s.claimer, &s.key_id), 100);

        s.client
            .transfer_keys(&s.key_id, &s.claimer, &recipient, &50);
        assert_eq!(s.client.get_locked_key_balance(&s.key_id, &s.claimer), 50);

        s.env.ledger().with_mut(|l| l.sequence_number = 120);
        assert_eq!(s.client.release(&s.claimer, &s.position_id), 50);
        assert_eq!(s.client.get_locked_key_balance(&s.key_id, &s.claimer), 0);
        assert_eq!(s.client.vested_amount(&s.position_id), 100);
    }

    #[test]
    fn revocation_is_opt_in_and_only_removes_unvested_keys() {
        let not_revocable = setup(false, 10, 100);
        assert!(not_revocable
            .client
            .try_revoke_unvested(&not_revocable.creator, &not_revocable.position_id)
            .is_err());
        assert_eq!(
            not_revocable
                .client
                .get_locked_key_balance(&not_revocable.key_id, &not_revocable.claimer),
            100
        );

        let s = setup(true, 10, 100);
        s.env.ledger().with_mut(|l| l.sequence_number = 70);
        assert_eq!(s.client.release(&s.claimer, &s.position_id), 50);
        assert_eq!(s.client.revoke_unvested(&s.creator, &s.position_id), 50);

        let position = s.client.get_vested_position(&s.position_id).unwrap();
        assert_eq!(position.released, 50);
        assert_eq!(position.revoked, 50);
        assert_eq!(s.client.get_locked_key_balance(&s.key_id, &s.claimer), 0);
        assert_eq!(s.client.get_voting_weight(&s.claimer, &s.key_id), 50);
        assert!(s
            .client
            .try_revoke_unvested(&s.creator, &s.position_id)
            .is_err());
    }
}
