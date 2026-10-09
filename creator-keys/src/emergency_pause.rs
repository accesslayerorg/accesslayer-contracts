//! #1000 — Emergency pause for a platform-wide trading halt.
//!
//! The global-pause admin set (configured via `set_global_pause_admins`) can
//! halt every bonding-curve buy and sell in a single transaction by passing at
//! least [`crate::GLOBAL_PAUSE_THRESHOLD`] distinct signers. Lifting the halt
//! is a two-step flow: a resume is queued, and it can only be executed once
//! [`PLATFORM_RESUME_DELAY_SECS`] (24h) has elapsed. A per-key override lets
//! the same admins halt a single key independently of the platform state.

use crate::events::{
    self, KeyPauseOverrideEvent, PlatformPausedEvent, PlatformResumeQueuedEvent,
    PlatformResumedEvent,
};
use crate::{read_global_pause_admins, ContractError, GLOBAL_PAUSE_THRESHOLD};
use soroban_sdk::{contracterror, contracttype, Address, Env, Vec};

/// Timelock delay between queueing and executing a platform resume (24 hours).
pub const PLATFORM_RESUME_DELAY_SECS: u64 = 86_400;

/// Errors raised by the emergency-pause entrypoints.
///
/// Kept separate from [`ContractError`] so the platform-pause flow has its own
/// stable, self-describing error codes.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EmergencyPauseError {
    /// The admin set is unset or a signer is not a member of it.
    Unauthorized = 1,
    /// Fewer than the required number of distinct signers were supplied.
    InsufficientSigners = 2,
    /// The same signer appeared more than once.
    DuplicateSigner = 3,
    /// `pause_platform` was called while the platform is already paused.
    AlreadyPaused = 4,
    /// A resume action was attempted while the platform is not paused.
    NotPaused = 5,
    /// A resume is already queued.
    ResumeAlreadyQueued = 6,
    /// `resume_platform` was called without a queued resume.
    ResumeNotQueued = 7,
    /// `resume_platform` was called before the 24h timelock elapsed.
    TimelockNotElapsed = 8,
    /// Arithmetic overflow while computing the resume timestamp.
    Overflow = 9,
}

#[derive(Clone)]
#[contracttype]
pub enum EmergencyPauseDataKey {
    /// Whether the platform-wide trading halt is active.
    PlatformPaused,
    /// Ledger timestamp at which a queued resume becomes executable.
    ResumeEta,
    /// Per-key emergency pause override.
    KeyPaused(Address),
}

/// Validates that `signers` are distinct members of the global-pause admin set,
/// meet the threshold, and have all authorised the call. Returns the first
/// signer, reported as the event `actor`.
fn assert_multisig(env: &Env, signers: &Vec<Address>) -> Result<Address, EmergencyPauseError> {
    if signers.len() < GLOBAL_PAUSE_THRESHOLD {
        return Err(EmergencyPauseError::InsufficientSigners);
    }
    let config = read_global_pause_admins(env).map_err(|_| EmergencyPauseError::Unauthorized)?;

    for i in 0..signers.len() {
        let signer = signers.get_unchecked(i);
        if !config.admins.contains(&signer) {
            return Err(EmergencyPauseError::Unauthorized);
        }
        for j in 0..i {
            if signers.get_unchecked(j) == signer {
                return Err(EmergencyPauseError::DuplicateSigner);
            }
        }
        signer.require_auth();
    }

    Ok(signers.get_unchecked(0))
}

/// Read-only: whether the platform-wide emergency halt is active.
pub fn is_platform_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get(&EmergencyPauseDataKey::PlatformPaused)
        .unwrap_or(false)
}

/// Read-only: whether `key_id` has an active per-key emergency override.
pub fn is_key_paused(env: &Env, key_id: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&EmergencyPauseDataKey::KeyPaused(key_id.clone()))
        .unwrap_or(false)
}

/// Read-only: the timestamp at which a queued resume becomes executable.
pub fn resume_eta(env: &Env) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&EmergencyPauseDataKey::ResumeEta)
}

/// Rejects a buy or sell on `key_id` while the platform halt or the key's
/// override is active.
pub fn assert_trading_allowed(env: &Env, key_id: &Address) -> Result<(), ContractError> {
    if is_platform_paused(env) || is_key_paused(env, key_id) {
        return Err(ContractError::GlobalTradingHalted);
    }
    Ok(())
}

/// Halts all buys and sells across every key, effective immediately.
pub fn pause_platform(env: &Env, signers: &Vec<Address>) -> Result<(), EmergencyPauseError> {
    let actor = assert_multisig(env, signers)?;
    if is_platform_paused(env) {
        return Err(EmergencyPauseError::AlreadyPaused);
    }

    let storage = env.storage().persistent();
    storage.set(&EmergencyPauseDataKey::PlatformPaused, &true);
    storage.remove(&EmergencyPauseDataKey::ResumeEta);

    env.events().publish(
        events::platform_paused_topics(&actor),
        PlatformPausedEvent {
            actor,
            timestamp: env.ledger().timestamp(),
        },
    );
    Ok(())
}

/// Queues a platform resume that becomes executable after the 24h timelock.
/// Returns the timestamp at which `resume_platform` may be called.
pub fn queue_platform_resume(
    env: &Env,
    signers: &Vec<Address>,
) -> Result<u64, EmergencyPauseError> {
    let actor = assert_multisig(env, signers)?;
    if !is_platform_paused(env) {
        return Err(EmergencyPauseError::NotPaused);
    }
    if resume_eta(env).is_some() {
        return Err(EmergencyPauseError::ResumeAlreadyQueued);
    }

    let executable_at = env
        .ledger()
        .timestamp()
        .checked_add(PLATFORM_RESUME_DELAY_SECS)
        .ok_or(EmergencyPauseError::Overflow)?;
    env.storage()
        .persistent()
        .set(&EmergencyPauseDataKey::ResumeEta, &executable_at);

    env.events().publish(
        events::platform_resume_queued_topics(&actor),
        PlatformResumeQueuedEvent {
            actor,
            executable_at,
        },
    );
    Ok(executable_at)
}

/// Lifts the platform halt once a queued resume's timelock has elapsed.
pub fn resume_platform(env: &Env, signers: &Vec<Address>) -> Result<(), EmergencyPauseError> {
    let actor = assert_multisig(env, signers)?;
    if !is_platform_paused(env) {
        return Err(EmergencyPauseError::NotPaused);
    }
    let executable_at = resume_eta(env).ok_or(EmergencyPauseError::ResumeNotQueued)?;
    let now = env.ledger().timestamp();
    if now < executable_at {
        return Err(EmergencyPauseError::TimelockNotElapsed);
    }

    let storage = env.storage().persistent();
    storage.set(&EmergencyPauseDataKey::PlatformPaused, &false);
    storage.remove(&EmergencyPauseDataKey::ResumeEta);

    env.events().publish(
        events::platform_resumed_topics(&actor),
        PlatformResumedEvent {
            actor,
            timestamp: now,
        },
    );
    Ok(())
}

/// Sets or clears the emergency pause override for a single key. Independent of
/// the platform halt: neither state clears or bypasses the other.
pub fn set_key_pause_override(
    env: &Env,
    signers: &Vec<Address>,
    key_id: &Address,
    paused: bool,
) -> Result<(), EmergencyPauseError> {
    let actor = assert_multisig(env, signers)?;

    let key = EmergencyPauseDataKey::KeyPaused(key_id.clone());
    if paused {
        env.storage().persistent().set(&key, &true);
    } else {
        env.storage().persistent().remove(&key);
    }

    env.events().publish(
        events::key_pause_override_topics(key_id),
        KeyPauseOverrideEvent {
            key_id: key_id.clone(),
            paused,
            actor,
        },
    );
    Ok(())
}
