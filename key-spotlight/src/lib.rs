#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, vec, Address, Env, Vec};

pub mod events;

// ── Constants ─────────────────────────────────────────────────────────────────

/// One day in seconds.
pub const SECS_PER_DAY: u64 = 86_400;

/// Maximum spotlight duration a single bid may cover.
pub const MAX_DURATION_DAYS: u32 = 30;

/// Minimum bid amount (prevents zero/dust bids).
pub const MIN_BID_AMOUNT: i128 = 1;

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
    /// bid_amount is below the minimum or not greater than the current leader.
    BidTooLow = 3,
    /// duration_days is zero or exceeds MAX_DURATION_DAYS.
    InvalidDuration = 4,
    /// start_time is in the past.
    StartTimeInPast = 5,
    /// Treasury address has not been set.
    TreasuryNotSet = 6,
    /// Arithmetic overflow.
    Overflow = 7,
    /// The slot has already started; no more bids accepted.
    SlotAlreadyStarted = 8,
    /// No spotlight is currently active.
    NoActiveSpotlight = 9,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    /// Protocol admin.
    Admin,
    /// Protocol treasury address.
    Treasury,
    /// Leading bid for a slot: `DataKey::SlotLeader(slot_id)`.
    SlotLeader(u64),
    /// All bids for a slot: `DataKey::SlotBids(slot_id)`.
    SlotBids(u64),
    /// Whether a slot has been finalised: `DataKey::SlotFinalised(slot_id)`.
    SlotFinalised(u64),
}

// ── Data types ────────────────────────────────────────────────────────────────

/// A single bid placed on a spotlight slot.
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct SpotlightBid {
    pub bidder: Address,
    pub key_id: Address,
    pub bid_amount: i128,
    pub duration_days: u32,
}

/// The current leading bid for a slot.
///
/// Duplicates fields from `SpotlightBid` for efficient reads of the winner without
/// scanning the full bids list.
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct SlotLeader {
    pub slot_id: u64,
    pub bidder: Address,
    pub key_id: Address,
    pub bid_amount: i128,
    pub duration_days: u32,
}

/// View returned by `get_active_spotlight`.
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct ActiveSpotlightView {
    pub key_id: Address,
    pub slot_id: u64,
    pub bid_amount: i128,
    pub duration_days: u32,
    /// Seconds remaining in the active spotlight window (0 if just ended).
    pub remaining_secs: u64,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn read_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&DataKey::Admin)
}

fn read_treasury(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&DataKey::Treasury)
}

fn read_slot_leader(env: &Env, slot_id: u64) -> Option<SlotLeader> {
    env.storage()
        .persistent()
        .get(&DataKey::SlotLeader(slot_id))
}

fn read_slot_bids(env: &Env, slot_id: u64) -> Vec<SpotlightBid> {
    env.storage()
        .persistent()
        .get(&DataKey::SlotBids(slot_id))
        .unwrap_or_else(|| Vec::new(env))
}

fn write_slot_bids(env: &Env, slot_id: u64, bids: &Vec<SpotlightBid>) {
    env.storage()
        .persistent()
        .set(&DataKey::SlotBids(slot_id), bids);
}

/// Normalise a unix timestamp to the start of its UTC day (floor to day boundary).
///
/// `slot_id` is the canonical identifier for a spotlight time slot.
pub fn day_floor(ts: u64) -> u64 {
    (ts / SECS_PER_DAY) * SECS_PER_DAY
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct KeySpotlightContract;

#[contractimpl]
impl KeySpotlightContract {
    // ── Admin bootstrap ───────────────────────────────────────────────────────

    /// Set the protocol admin. First call is open; subsequent calls require
    /// the existing admin to authorise.
    pub fn set_admin(env: Env, admin: Address) {
        if let Some(existing) = read_admin(&env) {
            existing.require_auth();
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
    }

    /// Set the treasury address that receives winning bid proceeds.
    pub fn set_treasury(
        env: Env,
        admin: Address,
        treasury: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        let stored = read_admin(&env).ok_or(ContractError::AdminNotSet)?;
        if admin != stored {
            return Err(ContractError::Unauthorised);
        }
        env.storage().persistent().set(&DataKey::Treasury, &treasury);
        Ok(())
    }

    pub fn get_treasury(env: Env) -> Option<Address> {
        read_treasury(&env)
    }

    // ── Bidding ───────────────────────────────────────────────────────────────

    /// Place or raise a bid for a spotlight slot.
    ///
    /// - `key_id`: the creator key being promoted.
    /// - `start_time`: unix timestamp for the desired slot start; floored to a
    ///   day boundary internally so slot ids are always day-aligned.
    /// - `duration_days`: how many days of spotlight coverage (1–MAX_DURATION_DAYS).
    /// - `bid_amount`: must exceed the current slot leader's bid (or MIN_BID_AMOUNT
    ///   if no bids yet).
    ///
    /// When a new bid displaces the current leader the outbid amount is recorded
    /// on the previous leader's bid entry so the caller can verify it via
    /// `get_spotlight_bids`. Actual token transfers are handled off-chain or by a
    /// wrapper contract that reads these records; the on-chain state tracks amounts
    /// and winners only.
    ///
    /// Emits [`events::SpotlightBidPlacedEvent`].
    pub fn place_spotlight_bid(
        env: Env,
        key_id: Address,
        creator: Address,
        start_time: u64,
        duration_days: u32,
        bid_amount: i128,
    ) -> Result<SlotLeader, ContractError> {
        creator.require_auth();

        let now = env.ledger().timestamp();
        if start_time < now {
            return Err(ContractError::StartTimeInPast);
        }
        if duration_days == 0 || duration_days > MAX_DURATION_DAYS {
            return Err(ContractError::InvalidDuration);
        }
        if bid_amount < MIN_BID_AMOUNT {
            return Err(ContractError::BidTooLow);
        }

        let slot_id = day_floor(start_time);

        // Reject bids on slots that have already started.
        if slot_id <= now {
            return Err(ContractError::SlotAlreadyStarted);
        }

        // Must strictly exceed the current leader.
        if let Some(ref leader) = read_slot_leader(&env, slot_id) {
            if bid_amount <= leader.bid_amount {
                return Err(ContractError::BidTooLow);
            }
        }

        // Append this bid to the slot's history.
        let mut bids = read_slot_bids(&env, slot_id);
        bids.push_back(SpotlightBid {
            bidder: creator.clone(),
            key_id: key_id.clone(),
            bid_amount,
            duration_days,
        });
        write_slot_bids(&env, slot_id, &bids);

        // Update leader record.
        let leader = SlotLeader {
            slot_id,
            bidder: creator.clone(),
            key_id: key_id.clone(),
            bid_amount,
            duration_days,
        };
        env.storage()
            .persistent()
            .set(&DataKey::SlotLeader(slot_id), &leader);

        env.events().publish(
            events::spotlight_bid_placed_topics(&key_id, slot_id),
            events::SpotlightBidPlacedEvent {
                key_id: key_id.clone(),
                slot_id,
                bid_amount,
                bidder: creator,
            },
        );

        Ok(leader)
    }

    // ── Slot finalisation ─────────────────────────────────────────────────────

    /// Finalise a slot once its start_time has been reached.
    ///
    /// Marks the slot as finalised and emits `SpotlightAwarded`. This is the
    /// trigger point at which the winning bid proceeds should be routed to the
    /// treasury and outbid amounts returned to losing bidders. Token movement
    /// itself is handled by a wrapper or the caller; this method records the
    /// outcome on-chain and emits the canonical event.
    ///
    /// Can be called by anyone once `slot_id <= now` (permissionless finalisation).
    pub fn finalise_slot(env: Env, slot_id: u64) -> Result<SlotLeader, ContractError> {
        let now = env.ledger().timestamp();
        if slot_id > now {
            return Err(ContractError::SlotAlreadyStarted); // slot hasn't started yet
        }

        // Idempotency: if already finalised, just return the leader.
        let already: bool = env
            .storage()
            .persistent()
            .get(&DataKey::SlotFinalised(slot_id))
            .unwrap_or(false);

        let leader =
            read_slot_leader(&env, slot_id).ok_or(ContractError::NoActiveSpotlight)?;

        if !already {
            env.storage()
                .persistent()
                .set(&DataKey::SlotFinalised(slot_id), &true);

            env.events().publish(
                events::spotlight_awarded_topics(&leader.key_id, slot_id),
                events::SpotlightAwardedEvent {
                    key_id: leader.key_id.clone(),
                    slot_id,
                    winning_amount: leader.bid_amount,
                    duration_days: leader.duration_days,
                },
            );
        }

        Ok(leader)
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    /// Return the currently active spotlight winner, if any.
    ///
    /// An active spotlight is a finalised slot whose coverage window
    /// (`slot_id` .. `slot_id + duration_days * SECS_PER_DAY`) contains `now`.
    ///
    /// Callers are expected to track which slots have been finalised and pass the
    /// relevant `slot_id`. This design avoids unbounded on-chain iteration.
    pub fn get_active_spotlight(
        env: Env,
        slot_id: u64,
    ) -> Result<ActiveSpotlightView, ContractError> {
        let now = env.ledger().timestamp();

        let finalised: bool = env
            .storage()
            .persistent()
            .get(&DataKey::SlotFinalised(slot_id))
            .unwrap_or(false);

        if !finalised {
            return Err(ContractError::NoActiveSpotlight);
        }

        let leader =
            read_slot_leader(&env, slot_id).ok_or(ContractError::NoActiveSpotlight)?;

        let end_time = slot_id
            .checked_add(leader.duration_days as u64 * SECS_PER_DAY)
            .ok_or(ContractError::Overflow)?;

        if now >= end_time {
            return Err(ContractError::NoActiveSpotlight);
        }

        let remaining_secs = end_time.saturating_sub(now);

        Ok(ActiveSpotlightView {
            key_id: leader.key_id,
            slot_id,
            bid_amount: leader.bid_amount,
            duration_days: leader.duration_days,
            remaining_secs,
        })
    }

    /// Return all bids placed for `slot_id` in submission order.
    pub fn get_spotlight_bids(env: Env, slot_id: u64) -> Vec<SpotlightBid> {
        read_slot_bids(&env, slot_id)
    }

    /// Return the current leading bid for `slot_id`, if any.
    pub fn get_slot_leader(env: Env, slot_id: u64) -> Option<SlotLeader> {
        read_slot_leader(&env, slot_id)
    }

    /// Return `true` if the slot has been finalised.
    pub fn is_slot_finalised(env: Env, slot_id: u64) -> bool {
        env.storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::SlotFinalised(slot_id))
            .unwrap_or(false)
    }
}
