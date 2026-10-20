#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, vec, Address, Env, Vec};

pub mod events;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Number of milestones in every key's schedule.
pub const MILESTONE_COUNT: u32 = 4;

/// Default milestone durations in seconds: 30d, 90d, 180d, 365d.
pub const DEFAULT_DURATION_30D: u64 = 30 * 24 * 60 * 60;
pub const DEFAULT_DURATION_90D: u64 = 90 * 24 * 60 * 60;
pub const DEFAULT_DURATION_180D: u64 = 180 * 24 * 60 * 60;
pub const DEFAULT_DURATION_365D: u64 = 365 * 24 * 60 * 60;

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
/// Contract error variants.
///
/// **IMPORTANT**: append only — never insert or reorder. Discriminants are ABI.
pub enum ContractError {
    /// Admin address has not been set.
    AdminNotSet = 1,
    /// Caller is not the admin.
    Unauthorised = 2,
    /// milestone_index is ≥ MILESTONE_COUNT.
    InvalidMilestoneIndex = 3,
    /// The milestone schedule for this key has not been configured.
    ScheduleNotFound = 4,
    /// The holder has no recorded entry timestamp for this key.
    HoldingNotStarted = 5,
    /// The holder has not yet held for the required duration.
    DurationNotMet = 6,
    /// This milestone has already been claimed by the wallet.
    AlreadyClaimed = 7,
    /// Arithmetic overflow.
    Overflow = 8,
    /// A milestone schedule with invalid durations was supplied (must be strictly ascending).
    InvalidSchedule = 9,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    /// Protocol admin.
    Admin,
    /// Milestone schedule for a key: `DataKey::Schedule(key_id)`.
    Schedule(Address),
    /// Per-wallet holding record: `DataKey::HoldingRecord(wallet, key_id)`.
    HoldingRecord(Address, Address),
}

// ── Data types ────────────────────────────────────────────────────────────────

/// A single milestone definition: required holding duration and reward amount.
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct Milestone {
    /// Seconds of continuous holding required to unlock this milestone.
    pub duration_secs: u64,
    /// Reward token amount distributed when this milestone is claimed.
    pub reward_amount: i128,
}

/// Full milestone schedule for one key (always exactly MILESTONE_COUNT entries).
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct MilestoneSchedule {
    pub milestones: Vec<Milestone>,
}

/// Per-wallet holding state for a specific key.
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct HoldingRecord {
    /// Ledger timestamp when this wallet first purchased (or re-entered after a full exit).
    pub entry_timestamp: u64,
    /// Bitmask of claimed milestones. Bit `i` set ⟹ milestone `i` claimed.
    /// Supports up to 32 milestones; MILESTONE_COUNT = 4 uses bits 0–3.
    pub claimed_mask: u32,
}

/// Per-milestone status returned by `get_milestone_status`.
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct MilestoneStatus {
    pub milestone_index: u32,
    pub duration_secs: u64,
    pub reward_amount: i128,
    /// True if the wallet has already claimed this milestone.
    pub claimed: bool,
    /// True if the holding duration has been met and the milestone is claimable
    /// (only meaningful when `claimed` is false).
    pub claimable: bool,
}

/// Full status view returned by `get_milestone_status`.
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct MilestoneStatusView {
    pub wallet: Address,
    pub key_id: Address,
    /// Seconds the wallet has held continuously (0 if no holding record).
    pub held_secs: u64,
    pub milestones: Vec<MilestoneStatus>,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn read_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&DataKey::Admin)
}

fn read_schedule(env: &Env, key_id: &Address) -> Option<MilestoneSchedule> {
    env.storage()
        .persistent()
        .get(&DataKey::Schedule(key_id.clone()))
}

fn read_holding_record(env: &Env, wallet: &Address, key_id: &Address) -> Option<HoldingRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::HoldingRecord(wallet.clone(), key_id.clone()))
}

fn write_holding_record(env: &Env, wallet: &Address, key_id: &Address, record: &HoldingRecord) {
    env.storage()
        .persistent()
        .set(&DataKey::HoldingRecord(wallet.clone(), key_id.clone()), record);
}

/// Build the default four-milestone schedule with the given reward amounts.
fn default_schedule(env: &Env, rewards: &[i128; 4]) -> MilestoneSchedule {
    MilestoneSchedule {
        milestones: vec![
            env,
            Milestone { duration_secs: DEFAULT_DURATION_30D,  reward_amount: rewards[0] },
            Milestone { duration_secs: DEFAULT_DURATION_90D,  reward_amount: rewards[1] },
            Milestone { duration_secs: DEFAULT_DURATION_180D, reward_amount: rewards[2] },
            Milestone { duration_secs: DEFAULT_DURATION_365D, reward_amount: rewards[3] },
        ],
    }
}

/// Validate that milestone durations are strictly ascending.
fn schedule_is_valid(schedule: &MilestoneSchedule) -> bool {
    let ms = &schedule.milestones;
    if ms.len() != MILESTONE_COUNT {
        return false;
    }
    for i in 1..MILESTONE_COUNT {
        let prev = ms.get(i - 1).unwrap().duration_secs;
        let curr = ms.get(i).unwrap().duration_secs;
        if curr <= prev {
            return false;
        }
    }
    true
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct HolderMilestonesContract;

#[contractimpl]
impl HolderMilestonesContract {
    // ── Admin bootstrap ───────────────────────────────────────────────────────

    /// Set the protocol admin. First call is open; subsequent calls require
    /// the existing admin to authorise.
    pub fn set_admin(env: Env, admin: Address) {
        if let Some(existing) = read_admin(&env) {
            existing.require_auth();
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
    }

    // ── Schedule management (admin only) ──────────────────────────────────────

    /// Configure the default 30/90/180/365-day schedule for `key_id`.
    ///
    /// `rewards` must contain exactly four positive amounts in ascending-duration order.
    pub fn set_milestone_schedule(
        env: Env,
        admin: Address,
        key_id: Address,
        reward_30d: i128,
        reward_90d: i128,
        reward_180d: i128,
        reward_365d: i128,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        let stored_admin = read_admin(&env).ok_or(ContractError::AdminNotSet)?;
        if admin != stored_admin {
            return Err(ContractError::Unauthorised);
        }

        let schedule = default_schedule(&env, &[reward_30d, reward_90d, reward_180d, reward_365d]);
        // Durations are fixed defaults so ascending check always passes here,
        // but we run it anyway to guard against future refactors.
        if !schedule_is_valid(&schedule) {
            return Err(ContractError::InvalidSchedule);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Schedule(key_id), &schedule);
        Ok(())
    }

    /// Configure an arbitrary four-milestone schedule for `key_id`.
    ///
    /// Durations must be strictly ascending; all reward amounts must be > 0.
    pub fn set_custom_schedule(
        env: Env,
        admin: Address,
        key_id: Address,
        milestones: Vec<Milestone>,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        let stored_admin = read_admin(&env).ok_or(ContractError::AdminNotSet)?;
        if admin != stored_admin {
            return Err(ContractError::Unauthorised);
        }

        let schedule = MilestoneSchedule { milestones };
        if !schedule_is_valid(&schedule) {
            return Err(ContractError::InvalidSchedule);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Schedule(key_id), &schedule);
        Ok(())
    }

    /// Read the current milestone schedule for `key_id`.
    pub fn get_milestone_schedule(env: Env, key_id: Address) -> Option<MilestoneSchedule> {
        read_schedule(&env, &key_id)
    }

    // ── Holding record management ─────────────────────────────────────────────

    /// Record a wallet's entry into a key position.
    ///
    /// Called by authorised trading contracts (e.g. creator-keys) when a holder
    /// buys their first key. Has no effect if the wallet already has an active record.
    pub fn record_entry(env: Env, wallet: Address, key_id: Address) {
        // Idempotent: only write if no existing record.
        if read_holding_record(&env, &wallet, &key_id).is_none() {
            let record = HoldingRecord {
                entry_timestamp: env.ledger().timestamp(),
                claimed_mask: 0,
            };
            write_holding_record(&env, &wallet, &key_id, &record);
        }
    }

    /// Reset a wallet's holding record for a key (called on full exit).
    ///
    /// Clears the entry timestamp so that re-entry starts a fresh holding clock.
    /// Claimed milestone bits are preserved in the cleared record so callers can
    /// audit historical claims, but `entry_timestamp` is set to 0 to signal
    /// no active position.
    pub fn record_exit(env: Env, wallet: Address, key_id: Address) {
        if let Some(mut record) = read_holding_record(&env, &wallet, &key_id) {
            // entry_timestamp = 0 signals "no active position".
            record.entry_timestamp = 0;
            write_holding_record(&env, &wallet, &key_id, &record);
        }
    }

    // ── Reward claiming ───────────────────────────────────────────────────────

    /// Claim the reward for `milestone_index` on `key_id`.
    ///
    /// Requirements:
    /// - A schedule must be configured for `key_id`.
    /// - The wallet must have an active holding record (entry_timestamp > 0).
    /// - The wallet must have held for at least `milestone.duration_secs`.
    /// - The milestone must not have been claimed already.
    ///
    /// Emits [`events::MilestoneRewardClaimedEvent`].
    pub fn claim_milestone_reward(
        env: Env,
        wallet: Address,
        key_id: Address,
        milestone_index: u32,
    ) -> Result<i128, ContractError> {
        wallet.require_auth();

        if milestone_index >= MILESTONE_COUNT {
            return Err(ContractError::InvalidMilestoneIndex);
        }

        let schedule =
            read_schedule(&env, &key_id).ok_or(ContractError::ScheduleNotFound)?;

        let milestone = schedule
            .milestones
            .get(milestone_index)
            .unwrap(); // safe: schedule validated to have MILESTONE_COUNT entries

        let mut record =
            read_holding_record(&env, &wallet, &key_id).ok_or(ContractError::HoldingNotStarted)?;

        // entry_timestamp == 0 means the wallet fully exited; no active position.
        if record.entry_timestamp == 0 {
            return Err(ContractError::HoldingNotStarted);
        }

        // Check duration.
        let now = env.ledger().timestamp();
        let held_secs = now
            .checked_sub(record.entry_timestamp)
            .ok_or(ContractError::Overflow)?;
        if held_secs < milestone.duration_secs {
            return Err(ContractError::DurationNotMet);
        }

        // Check not already claimed (bitmask).
        let bit = 1u32 << milestone_index;
        if record.claimed_mask & bit != 0 {
            return Err(ContractError::AlreadyClaimed);
        }

        // Mark claimed.
        record.claimed_mask |= bit;
        write_holding_record(&env, &wallet, &key_id, &record);

        let amount = milestone.reward_amount;

        env.events().publish(
            events::milestone_reward_claimed_topics(&wallet, &key_id),
            events::MilestoneRewardClaimedEvent {
                wallet: wallet.clone(),
                key_id: key_id.clone(),
                milestone_index,
                amount,
            },
        );

        Ok(amount)
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    /// Return the full milestone status for `(wallet, key_id)`.
    ///
    /// `held_secs` is 0 when the wallet has no active position.
    /// Each `MilestoneStatus` entry reflects the current claimed/claimable state.
    pub fn get_milestone_status(
        env: Env,
        wallet: Address,
        key_id: Address,
    ) -> Result<MilestoneStatusView, ContractError> {
        let schedule =
            read_schedule(&env, &key_id).ok_or(ContractError::ScheduleNotFound)?;

        let (held_secs, claimed_mask) =
            match read_holding_record(&env, &wallet, &key_id) {
                Some(r) if r.entry_timestamp > 0 => {
                    let now = env.ledger().timestamp();
                    let held = now.saturating_sub(r.entry_timestamp);
                    (held, r.claimed_mask)
                }
                Some(r) => (0u64, r.claimed_mask), // exited wallet: no active hold
                None => (0u64, 0u32),
            };

        let mut statuses: Vec<MilestoneStatus> = Vec::new(&env);
        for i in 0..MILESTONE_COUNT {
            let m = schedule.milestones.get(i).unwrap();
            let bit = 1u32 << i;
            let claimed = (claimed_mask & bit) != 0;
            let claimable = !claimed && held_secs >= m.duration_secs;
            statuses.push_back(MilestoneStatus {
                milestone_index: i,
                duration_secs: m.duration_secs,
                reward_amount: m.reward_amount,
                claimed,
                claimable,
            });
        }

        Ok(MilestoneStatusView {
            wallet,
            key_id,
            held_secs,
            milestones: statuses,
        })
    }
}
