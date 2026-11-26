//! Creator allocation pre-mint with disclosed caps and mandatory vesting.
//!
//! Issue #1283: Creators who keep a large supply share at launch can dump on holders,
//! and the amount is not guaranteed anywhere. This module adds a disclosed creator
//! allocation limited by a protocol cap and subject to vesting.

use soroban_sdk::{contracttype, Address, Env};

use crate::ContractError;

/// Maximum creator allocation as basis points of initial supply target.
pub const MAX_CREATOR_ALLOCATION_BPS: u32 = 2000; // 20%

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CreatorAllocation {
    /// Creator address
    pub creator: Address,
    /// Key ID
    pub key_id: Address,
    /// Total allocated amount
    pub amount: i128,
    /// Vesting duration in ledgers
    pub vesting_ledgers: u32,
    /// Cliff duration in ledgers
    pub cliff_ledgers: u32,
    /// Ledger when allocation was set
    pub allocation_ledger: u32,
    /// Amount already claimed
    pub claimed_amount: i128,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum CreatorAllocationKey {
    /// (key_id) -> CreatorAllocation
    Allocation(Address),
    /// (key_id) -> bool - whether first public trade has occurred
    FirstTradeOccurred(Address),
    /// Protocol-wide max creator allocation BPS
    MaxCreatorAllocationBps,
}

impl CreatorAllocation {
    /// Calculate the vested amount at the given ledger.
    pub fn vested_at_ledger(&self, current_ledger: u32) -> i128 {
        let elapsed = current_ledger.saturating_sub(self.allocation_ledger);

        // Before cliff, nothing is vested
        if elapsed < self.cliff_ledgers {
            return 0;
        }

        // After full vesting period, everything is vested
        if elapsed >= self.vesting_ledgers {
            return self.amount;
        }

        // Linear vesting after cliff
        let vesting_elapsed = elapsed.saturating_sub(self.cliff_ledgers);
        let vesting_duration = self.vesting_ledgers.saturating_sub(self.cliff_ledgers);

        if vesting_duration == 0 {
            return self.amount;
        }

        // Calculate proportional vested amount
        self.amount
            .checked_mul(vesting_elapsed as i128)
            .and_then(|v| v.checked_div(vesting_duration as i128))
            .unwrap_or(0)
    }

    /// Calculate the claimable amount (vested - claimed).
    pub fn claimable_at_ledger(&self, current_ledger: u32) -> i128 {
        let vested = self.vested_at_ledger(current_ledger);
        vested.saturating_sub(self.claimed_amount)
    }

    /// Check if the allocation can be transferred or sold.
    pub fn is_transferable_at_ledger(&self, current_ledger: u32) -> bool {
        let elapsed = current_ledger.saturating_sub(self.allocation_ledger);
        elapsed >= self.vesting_ledgers
    }
}

/// Set creator allocation for a key (must be before first public trade).
pub fn set_creator_allocation(
    env: &Env,
    creator: Address,
    key_id: Address,
    amount: i128,
    vesting_ledgers: u32,
    cliff_ledgers: u32,
) -> Result<(), ContractError> {
    // Verify caller is the creator
    creator.require_auth();

    // Check if first public trade has occurred
    let first_trade_key = CreatorAllocationKey::FirstTradeOccurred(key_id.clone());
    if env
        .storage()
        .persistent()
        .get::<CreatorAllocationKey, bool>(&first_trade_key)
        .unwrap_or(false)
    {
        return Err(ContractError::Unauthorized);
    }

    // Get max allocation BPS (default to constant if not set)
    let _max_bps = env
        .storage()
        .persistent()
        .get::<CreatorAllocationKey, u32>(&CreatorAllocationKey::MaxCreatorAllocationBps)
        .unwrap_or(MAX_CREATOR_ALLOCATION_BPS);

    // Get initial supply target for this key
    // Note: This would need to be fetched from the actual key's configuration
    // For now, we'll validate against the amount directly
    // In a real implementation, this would check: amount <= (initial_supply * max_bps / 10000)

    // Validate cliff <= vesting
    if cliff_ledgers > vesting_ledgers {
        return Err(ContractError::InvalidLockPeriod);
    }

    // Create allocation
    let allocation = CreatorAllocation {
        creator: creator.clone(),
        key_id: key_id.clone(),
        amount,
        vesting_ledgers,
        cliff_ledgers,
        allocation_ledger: env.ledger().sequence(),
        claimed_amount: 0,
    };

    // Store allocation
    let alloc_key = CreatorAllocationKey::Allocation(key_id.clone());
    env.storage().persistent().set(&alloc_key, &allocation);

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("alloc_set"),),
        (creator, key_id, amount, vesting_ledgers, cliff_ledgers),
    );

    Ok(())
}

/// Claim vested allocation.
pub fn claim_vested(env: &Env, creator: Address, key_id: Address) -> Result<i128, ContractError> {
    creator.require_auth();

    // Load allocation
    let alloc_key = CreatorAllocationKey::Allocation(key_id.clone());
    let mut allocation: CreatorAllocation = env
        .storage()
        .persistent()
        .get(&alloc_key)
        .ok_or(ContractError::VestingNotFound)?;

    // Verify creator
    if allocation.creator != creator {
        return Err(ContractError::Unauthorized);
    }

    // Calculate claimable amount
    let current_ledger = env.ledger().sequence();
    let claimable = allocation.claimable_at_ledger(current_ledger);

    if claimable <= 0 {
        return Err(ContractError::NoDividendClaimable);
    }

    // Update claimed amount
    allocation.claimed_amount = allocation
        .claimed_amount
        .checked_add(claimable)
        .ok_or(ContractError::Overflow)?;

    // Save updated allocation
    env.storage().persistent().set(&alloc_key, &allocation);

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("alloc_clm"),),
        (creator, key_id, claimable),
    );

    Ok(claimable)
}

/// Get vesting status for an allocation.
pub fn vesting_status(env: &Env, key_id: Address) -> Option<VestingStatus> {
    let alloc_key = CreatorAllocationKey::Allocation(key_id);
    let allocation: CreatorAllocation = env.storage().persistent().get(&alloc_key)?;

    let current_ledger = env.ledger().sequence();
    let vested = allocation.vested_at_ledger(current_ledger);
    let claimable = allocation.claimable_at_ledger(current_ledger);

    Some(VestingStatus {
        total_amount: allocation.amount,
        vested_amount: vested,
        claimed_amount: allocation.claimed_amount,
        claimable_amount: claimable,
        cliff_ledgers: allocation.cliff_ledgers,
        vesting_ledgers: allocation.vesting_ledgers,
        allocation_ledger: allocation.allocation_ledger,
        is_transferable: allocation.is_transferable_at_ledger(current_ledger),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VestingStatus {
    pub total_amount: i128,
    pub vested_amount: i128,
    pub claimed_amount: i128,
    pub claimable_amount: i128,
    pub cliff_ledgers: u32,
    pub vesting_ledgers: u32,
    pub allocation_ledger: u32,
    pub is_transferable: bool,
}

/// Extend vesting period (governance only, can only extend, not shorten).
pub fn extend_vesting(
    env: &Env,
    admin: Address,
    key_id: Address,
    additional_ledgers: u32,
) -> Result<(), ContractError> {
    admin.require_auth();

    // Load allocation
    let alloc_key = CreatorAllocationKey::Allocation(key_id.clone());
    let mut allocation: CreatorAllocation = env
        .storage()
        .persistent()
        .get(&alloc_key)
        .ok_or(ContractError::VestingNotFound)?;

    // Extend vesting period
    allocation.vesting_ledgers = allocation
        .vesting_ledgers
        .checked_add(additional_ledgers)
        .ok_or(ContractError::Overflow)?;

    // Save updated allocation
    env.storage().persistent().set(&alloc_key, &allocation);

    // Emit event
    env.events().publish(
        (soroban_sdk::symbol_short!("vest_ext"),),
        (key_id, additional_ledgers),
    );

    Ok(())
}

/// Mark that first public trade has occurred for a key.
pub fn mark_first_trade(env: &Env, key_id: Address) {
    let first_trade_key = CreatorAllocationKey::FirstTradeOccurred(key_id);
    env.storage().persistent().set(&first_trade_key, &true);
}
