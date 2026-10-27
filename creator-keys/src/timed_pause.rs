//! An expiring admin halt, independent of the existing multisig pause controls.
use crate::{
    assert_is_admin, extend_key_ttl_to_full_window, ContractError, CreatorKeysContract,
    CreatorKeysContractArgs, CreatorKeysContractClient,
};
use soroban_sdk::{contractimpl, contracttype, symbol_short, Address, Env, String};

pub const MAX_PAUSE_SECONDS: u64 = 7 * 24 * 60 * 60;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TimedPlatformPause {
    pub reason: String,
    pub paused_at: u64,
    pub resume_at: u64,
    pub active: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum TimedPauseKey {
    State,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TimedResumeEvent {
    pub reason: String,
    pub resume_at: u64,
    pub timestamp: u64,
    pub automatic: bool,
}

fn state(env: &Env) -> Option<TimedPlatformPause> {
    let value = env.storage().persistent().get(&TimedPauseKey::State);
    if value.is_some() {
        extend_key_ttl_to_full_window(env, &TimedPauseKey::State);
    }
    value
}

fn resume(env: &Env, mut pause: TimedPlatformPause, automatic: bool) {
    pause.active = false;
    env.storage()
        .persistent()
        .set(&TimedPauseKey::State, &pause);
    extend_key_ttl_to_full_window(env, &TimedPauseKey::State);
    env.events().publish(
        (symbol_short!("all_resum"),),
        TimedResumeEvent {
            reason: pause.reason,
            resume_at: pause.resume_at,
            timestamp: env.ledger().timestamp(),
            automatic,
        },
    );
}

/// Expiry is enforced by timestamp, without requiring a keeper transaction.
/// The first guarded call after expiry records the automatic-resume event.
pub(crate) fn assert_not_paused(env: &Env) -> Result<(), ContractError> {
    if let Some(pause) = state(env) {
        if pause.active {
            if env.ledger().timestamp() < pause.resume_at {
                return Err(ContractError::PlatformPaused);
            }
            resume(env, pause, true);
        }
    }
    Ok(())
}

#[contractimpl]
impl CreatorKeysContract {
    /// Halt trading for a bounded duration. An active halt cannot be extended.
    pub fn pause_all(
        env: Env,
        admin: Address,
        reason: String,
        max_duration_seconds: u64,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        if reason.is_empty()
            || reason.len() > 512
            || max_duration_seconds == 0
            || max_duration_seconds > MAX_PAUSE_SECONDS
        {
            return Err(ContractError::NotPositiveAmount);
        }
        assert_not_paused(&env)?;
        let paused_at = env.ledger().timestamp();
        let resume_at = paused_at
            .checked_add(max_duration_seconds)
            .ok_or(ContractError::Overflow)?;
        let pause = TimedPlatformPause {
            reason,
            paused_at,
            resume_at,
            active: true,
        };
        env.storage()
            .persistent()
            .set(&TimedPauseKey::State, &pause);
        extend_key_ttl_to_full_window(&env, &TimedPauseKey::State);
        env.events()
            .publish((symbol_short!("all_pause"), admin), pause);
        Ok(())
    }

    pub fn resume_all(env: Env, admin: Address) -> Result<(), ContractError> {
        admin.require_auth();
        assert_is_admin(&env, &admin)?;
        let pause = state(&env).ok_or(ContractError::ActionNotPending)?;
        if !pause.active || env.ledger().timestamp() >= pause.resume_at {
            return Err(ContractError::ActionNotPending);
        }
        resume(&env, pause, false);
        Ok(())
    }

    /// Error code `PlatformPaused` is numeric; this view returns its context.
    pub fn get_platform_pause(env: Env) -> Option<TimedPlatformPause> {
        state(&env).map(|mut pause| {
            pause.active = pause.active && env.ledger().timestamp() < pause.resume_at;
            pause
        })
    }
}
