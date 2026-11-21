use crate::ContractError;
use soroban_sdk::{contracttype, Address, Env, Vec};

/// Recovery delay in ledgers (3 days on Stellar ≈ 17,280 ledgers at ~15.5 seconds per ledger)
pub const RECOVERY_DELAY_LEDGERS: u32 = 17280;
pub const MIN_GUARDIANS: u32 = 3;
pub const MAX_GUARDIANS: u32 = 7;
pub const MIN_THRESHOLD: u32 = 2;

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct GuardianSet {
    pub guardians: Vec<Address>,
    pub threshold: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct RecoveryState {
    pub owner: Address,
    pub new_address: Address,
    pub initiated_ledger: u32,
    pub approvers: Vec<Address>,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct RecoveryInitiatedEvent {
    pub owner: Address,
    pub guardian: Address,
    pub new_address: Address,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct RecoveryApprovedEvent {
    pub owner: Address,
    pub guardian: Address,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct RecoveryCancelledEvent {
    pub owner: Address,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct RecoveryExecutedEvent {
    pub owner: Address,
    pub new_address: Address,
}

pub fn set_guardians(
    env: &Env,
    owner: &Address,
    guardians: Vec<Address>,
    threshold: u32,
) -> Result<(), ContractError> {
    owner.require_auth();

    // Validate guardian count
    if guardians.len() < MIN_GUARDIANS || guardians.len() > MAX_GUARDIANS {
        return Err(ContractError::InvalidThreshold);
    }

    // Validate threshold
    if threshold < MIN_THRESHOLD || threshold > guardians.len() {
        return Err(ContractError::InvalidThreshold);
    }

    // Check if recovery is pending
    let recovery_key = soroban_sdk::Symbol::new(env, "recovery_state");
    let recovery_result: Option<RecoveryState> = env
        .storage()
        .persistent()
        .get(&(recovery_key.clone(), owner))
        .unwrap_or(None);
    if recovery_result.is_some() {
        return Err(ContractError::RecoveryAlreadyPending);
    }

    // Store guardian set
    let guardian_key = soroban_sdk::Symbol::new(env, "guardian_set");
    let guardian_set = GuardianSet {
        guardians,
        threshold,
    };
    env.storage()
        .persistent()
        .set(&(guardian_key, owner), &guardian_set);

    Ok(())
}

pub fn initiate_recovery(
    env: &Env,
    guardian: &Address,
    owner: &Address,
    new_address: &Address,
) -> Result<(), ContractError> {
    guardian.require_auth();

    if new_address == owner {
        return Err(ContractError::ZeroAddress);
    }

    // Get guardian set
    let guardian_key = soroban_sdk::Symbol::new(env, "guardian_set");
    let guardian_set: GuardianSet = env
        .storage()
        .persistent()
        .get(&(guardian_key, owner))
        .unwrap_or(None)
        .ok_or(ContractError::NoGuardians)?;

    // Check if guardian is in the set - compare by value
    let mut is_guardian = false;
    for g in guardian_set.guardians.iter() {
        let g_clone = g.clone();
        if g_clone == guardian.clone() {
            is_guardian = true;
            break;
        }
    }
    if !is_guardian {
        return Err(ContractError::Unauthorized);
    }

    // Check no recovery already pending
    let recovery_key = soroban_sdk::Symbol::new(env, "recovery_state");
    let recovery_result: Option<RecoveryState> = env
        .storage()
        .persistent()
        .get(&(recovery_key.clone(), owner))
        .unwrap_or(None);
    if recovery_result.is_some() {
        return Err(ContractError::RecoveryAlreadyPending);
    }

    // Create new recovery state with first approver
    let current_ledger = env.ledger().sequence();
    let mut approvers = Vec::new(env);
    approvers.push_back(guardian.clone());

    let recovery_state = RecoveryState {
        owner: owner.clone(),
        new_address: new_address.clone(),
        initiated_ledger: current_ledger,
        approvers,
    };

    env.storage()
        .persistent()
        .set(&(recovery_key, owner), &recovery_state);

    // Emit event
    env.events().publish(
        (soroban_sdk::Symbol::new(env, "recovery_initiated"), owner),
        RecoveryInitiatedEvent {
            owner: owner.clone(),
            guardian: guardian.clone(),
            new_address: new_address.clone(),
        },
    );

    Ok(())
}

pub fn approve_recovery(
    env: &Env,
    guardian: &Address,
    owner: &Address,
) -> Result<(), ContractError> {
    guardian.require_auth();

    // Get guardian set
    let guardian_key = soroban_sdk::Symbol::new(env, "guardian_set");
    let guardian_set: GuardianSet = env
        .storage()
        .persistent()
        .get(&(guardian_key, owner))
        .unwrap_or(None)
        .ok_or(ContractError::NoGuardians)?;

    // Check if guardian is in the set
    let mut is_guardian = false;
    for g in guardian_set.guardians.iter() {
        let g_clone = g.clone();
        if g_clone == guardian.clone() {
            is_guardian = true;
            break;
        }
    }
    if !is_guardian {
        return Err(ContractError::Unauthorized);
    }

    // Get recovery state
    let recovery_key = soroban_sdk::Symbol::new(env, "recovery_state");
    let mut recovery_state: RecoveryState = env
        .storage()
        .persistent()
        .get(&(recovery_key.clone(), owner))
        .unwrap_or(None)
        .ok_or(ContractError::NoRecoveryPending)?;

    // Check if already approved by this guardian
    let mut already_approved = false;
    for a in recovery_state.approvers.iter() {
        let a_clone = a.clone();
        if a_clone == guardian.clone() {
            already_approved = true;
            break;
        }
    }
    if already_approved {
        return Err(ContractError::AlreadyApproved);
    }

    // Add approval
    recovery_state.approvers.push_back(guardian.clone());
    env.storage()
        .persistent()
        .set(&(recovery_key, owner), &recovery_state);

    // Emit event
    env.events().publish(
        (soroban_sdk::Symbol::new(env, "recovery_approved"), owner),
        RecoveryApprovedEvent {
            owner: owner.clone(),
            guardian: guardian.clone(),
        },
    );

    Ok(())
}

pub fn execute_recovery(env: &Env, owner: &Address) -> Result<(), ContractError> {
    owner.require_auth();

    // Get recovery state
    let recovery_key = soroban_sdk::Symbol::new(env, "recovery_state");
    let recovery_state: RecoveryState = env
        .storage()
        .persistent()
        .get(&(recovery_key.clone(), owner))
        .unwrap_or(None)
        .ok_or(ContractError::NoRecoveryPending)?;

    // Get guardian set to check threshold
    let guardian_key = soroban_sdk::Symbol::new(env, "guardian_set");
    let guardian_set: GuardianSet = env
        .storage()
        .persistent()
        .get(&(guardian_key, owner))
        .unwrap_or(None)
        .ok_or(ContractError::NoGuardians)?;

    let current_ledger = env.ledger().sequence();

    // Check delay has elapsed
    if current_ledger < recovery_state.initiated_ledger + RECOVERY_DELAY_LEDGERS {
        return Err(ContractError::TimelockNotElapsed);
    }

    // Check threshold
    if recovery_state.approvers.len() < guardian_set.threshold {
        return Err(ContractError::RecoveryThresholdNotMet);
    }

    // Clear recovery state - actual balance transfers handled separately by caller
    // or via indexer, as storage iteration is not available
    env.storage().persistent().remove(&(recovery_key, owner));

    // Emit event
    env.events().publish(
        (soroban_sdk::Symbol::new(env, "recovery_executed"), owner),
        RecoveryExecutedEvent {
            owner: owner.clone(),
            new_address: recovery_state.new_address.clone(),
        },
    );

    Ok(())
}

pub fn cancel_recovery(env: &Env, owner: &Address) -> Result<(), ContractError> {
    owner.require_auth();

    // Get recovery state
    let recovery_key = soroban_sdk::Symbol::new(env, "recovery_state");
    let _recovery_state: RecoveryState = env
        .storage()
        .persistent()
        .get(&(recovery_key.clone(), owner))
        .unwrap_or(None)
        .ok_or(ContractError::NoRecoveryPending)?;

    // Clear recovery state
    env.storage().persistent().remove(&(recovery_key, owner));

    // Emit event
    env.events().publish(
        (soroban_sdk::Symbol::new(env, "recovery_cancelled"), owner),
        RecoveryCancelledEvent {
            owner: owner.clone(),
        },
    );

    Ok(())
}
