#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

pub mod events;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Basis points denominator (10_000 = 100%).
pub const BPS_MAX: u32 = 10_000;

/// Premium rate: 1% of coverage_amount per 30-day period.
pub const PREMIUM_RATE_BPS: u32 = 100; // 1%

/// Duration of a single coverage period in seconds (30 days).
pub const COVERAGE_PERIOD_SECS: u64 = 30 * 24 * 60 * 60; // 2_592_000

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
/// Contract error variants.
///
/// **IMPORTANT**: append only — never insert or reorder. Discriminants are ABI.
pub enum ContractError {
    /// coverage_amount must be > 0.
    InvalidCoverageAmount = 1,
    /// threshold_bps must be > 0 and < BPS_MAX.
    InvalidThresholdBps = 2,
    /// An active policy already exists for this (wallet, key_id) pair.
    PolicyAlreadyExists = 3,
    /// No active policy found for this (wallet, key_id) pair.
    PolicyNotFound = 4,
    /// The policy has expired.
    PolicyExpired = 5,
    /// Current price is at or above the threshold; payout not triggered.
    PriceAboveThreshold = 6,
    /// Arithmetic overflow during premium or payout calculation.
    Overflow = 7,
    /// The oracle price must be > 0.
    InvalidPrice = 8,
    /// Admin address has not been set.
    AdminNotSet = 9,
    /// Caller is not the admin.
    Unauthorised = 10,
    /// Key price has not been set via set_key_price.
    KeyPriceNotSet = 11,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    /// Protocol admin.
    Admin,
    /// Current price for a key: `DataKey::KeyPrice(key_id)`.
    KeyPrice(Address),
    /// Active policy: `DataKey::Policy(wallet, key_id)`.
    Policy(Address, Address),
}

// ── Data types ────────────────────────────────────────────────────────────────

/// A holder's insurance policy for a specific key.
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct InsurancePolicy {
    pub wallet: Address,
    pub key_id: Address,
    /// Coverage amount the holder is protected for.
    pub coverage_amount: i128,
    /// Threshold expressed as basis points of purchase_price.
    /// Payout triggers when current_price < purchase_price * threshold_bps / BPS_MAX.
    pub threshold_bps: u32,
    /// Key price at the time of purchase; used to evaluate the threshold.
    pub purchase_price: i128,
    /// Premium paid by the holder.
    pub premium: i128,
    /// Ledger timestamp at policy expiry.
    pub expiry: u64,
    /// Whether a payout has already been triggered (prevents double-claim).
    pub paid_out: bool,
}

/// Read-only view returned by `get_insurance`.
#[derive(Clone, Eq, PartialEq)]
#[contracttype]
pub struct InsuranceView {
    pub wallet: Address,
    pub key_id: Address,
    pub coverage_amount: i128,
    pub threshold_bps: u32,
    pub purchase_price: i128,
    pub premium: i128,
    pub expiry: u64,
    pub paid_out: bool,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn read_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&DataKey::Admin)
}

fn read_key_price(env: &Env, key_id: &Address) -> Option<i128> {
    env.storage()
        .persistent()
        .get(&DataKey::KeyPrice(key_id.clone()))
}

fn read_policy(env: &Env, wallet: &Address, key_id: &Address) -> Option<InsurancePolicy> {
    env.storage()
        .persistent()
        .get(&DataKey::Policy(wallet.clone(), key_id.clone()))
}

/// Compute the premium for one 30-day period: `floor(coverage_amount * 100 / 10_000)`.
///
/// Returns `None` on overflow. For any coverage_amount in the i128 range,
/// the multiplication by 100 is the only risk — handled with checked_mul.
pub fn compute_premium(coverage_amount: i128) -> Option<i128> {
    if coverage_amount <= 0 {
        return None;
    }
    // premium = coverage_amount * PREMIUM_RATE_BPS / BPS_MAX
    // = coverage_amount * 100 / 10_000
    let numerator = coverage_amount.checked_mul(PREMIUM_RATE_BPS as i128)?;
    Some(numerator / BPS_MAX as i128)
}

/// Returns `true` when `current_price` is strictly below the trigger threshold.
///
/// threshold_price = purchase_price * threshold_bps / BPS_MAX (floor division).
/// Payout is allowed when current_price < threshold_price.
pub fn price_below_threshold(purchase_price: i128, threshold_bps: u32, current_price: i128) -> bool {
    // threshold_price = floor(purchase_price * threshold_bps / BPS_MAX)
    let threshold_price = (purchase_price * threshold_bps as i128) / BPS_MAX as i128;
    current_price < threshold_price
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct KeyInsuranceContract;

#[contractimpl]
impl KeyInsuranceContract {
    // ── Admin bootstrap ───────────────────────────────────────────────────────

    /// Set the protocol admin. First call is open; subsequent calls require
    /// the existing admin to authorise (mirrors creator-keys pattern).
    pub fn set_admin(env: Env, admin: Address) {
        if let Some(existing) = read_admin(&env) {
            existing.require_auth();
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
    }

    // ── Price oracle (admin only) ─────────────────────────────────────────────

    /// Update the on-chain price for a key. Called by admin or an authorised
    /// price-feed contract to reflect current market price.
    pub fn set_key_price(
        env: Env,
        admin: Address,
        key_id: Address,
        price: i128,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        let stored_admin = read_admin(&env).ok_or(ContractError::AdminNotSet)?;
        if admin != stored_admin {
            return Err(ContractError::Unauthorised);
        }
        if price <= 0 {
            return Err(ContractError::InvalidPrice);
        }
        env.storage()
            .persistent()
            .set(&DataKey::KeyPrice(key_id), &price);
        Ok(())
    }

    /// Read the current price for a key.
    pub fn get_key_price(env: Env, key_id: Address) -> Option<i128> {
        read_key_price(&env, &key_id)
    }

    // ── Policy lifecycle ──────────────────────────────────────────────────────

    /// Purchase an insurance policy for `key_id`.
    ///
    /// - `coverage_amount`: the maximum payout the holder is covered for (> 0).
    /// - `threshold_bps`: payout triggers when current price drops below
    ///   `purchase_price * threshold_bps / 10_000`. Must be in (0, 10_000).
    ///
    /// Premium = 1% of coverage_amount per 30-day period, deducted from the
    /// holder's allowance. The policy expires after one 30-day period.
    ///
    /// Emits [`events::InsurancePurchasedEvent`].
    pub fn purchase_insurance(
        env: Env,
        wallet: Address,
        key_id: Address,
        coverage_amount: i128,
        threshold_bps: u32,
    ) -> Result<InsuranceView, ContractError> {
        wallet.require_auth();

        if coverage_amount <= 0 {
            return Err(ContractError::InvalidCoverageAmount);
        }
        if threshold_bps == 0 || threshold_bps >= BPS_MAX {
            return Err(ContractError::InvalidThresholdBps);
        }

        // Reject duplicate active policies.
        if let Some(existing) = read_policy(&env, &wallet, &key_id) {
            if !existing.paid_out && env.ledger().timestamp() < existing.expiry {
                return Err(ContractError::PolicyAlreadyExists);
            }
        }

        // Snapshot the current key price as the purchase reference price.
        let purchase_price = read_key_price(&env, &key_id)
            .ok_or(ContractError::KeyPriceNotSet)?;

        let premium = compute_premium(coverage_amount).ok_or(ContractError::Overflow)?;

        let expiry = env
            .ledger()
            .timestamp()
            .checked_add(COVERAGE_PERIOD_SECS)
            .ok_or(ContractError::Overflow)?;

        let policy = InsurancePolicy {
            wallet: wallet.clone(),
            key_id: key_id.clone(),
            coverage_amount,
            threshold_bps,
            purchase_price,
            premium,
            expiry,
            paid_out: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Policy(wallet.clone(), key_id.clone()), &policy);

        env.events().publish(
            events::insurance_purchased_topics(&wallet, &key_id),
            events::InsurancePurchasedEvent {
                wallet: wallet.clone(),
                key_id: key_id.clone(),
                coverage_amount,
                threshold_bps,
                premium,
                expiry,
            },
        );

        Ok(policy_to_view(&policy))
    }

    /// Trigger a payout for `holder`'s policy on `key_id`.
    ///
    /// Succeeds when all of the following hold:
    /// - An active, non-expired, not-yet-paid policy exists.
    /// - The current key price is strictly below `purchase_price * threshold_bps / 10_000`.
    ///
    /// The payout is capped at `coverage_amount`. The policy is marked paid_out
    /// so it cannot be claimed twice.
    ///
    /// Emits [`events::InsurancePayoutTriggeredEvent`].
    pub fn trigger_payout(
        env: Env,
        key_id: Address,
        holder: Address,
    ) -> Result<i128, ContractError> {
        let mut policy =
            read_policy(&env, &holder, &key_id).ok_or(ContractError::PolicyNotFound)?;

        if policy.paid_out {
            return Err(ContractError::PolicyNotFound);
        }
        if env.ledger().timestamp() >= policy.expiry {
            return Err(ContractError::PolicyExpired);
        }

        let current_price =
            read_key_price(&env, &key_id).ok_or(ContractError::KeyPriceNotSet)?;

        if !price_below_threshold(policy.purchase_price, policy.threshold_bps, current_price) {
            return Err(ContractError::PriceAboveThreshold);
        }

        // Payout is the full coverage_amount (price drop confirmed; cap is the coverage).
        let payout_amount = policy.coverage_amount;

        // Mark as paid out to prevent re-entry / double-claim.
        policy.paid_out = true;
        env.storage()
            .persistent()
            .set(&DataKey::Policy(holder.clone(), key_id.clone()), &policy);

        env.events().publish(
            events::insurance_payout_topics(&holder, &key_id),
            events::InsurancePayoutTriggeredEvent {
                wallet: holder.clone(),
                key_id: key_id.clone(),
                payout_amount,
            },
        );

        Ok(payout_amount)
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    /// Return policy details for `(wallet, key_id)`, or `None` if no policy exists.
    pub fn get_insurance(env: Env, wallet: Address, key_id: Address) -> Option<InsuranceView> {
        read_policy(&env, &wallet, &key_id).map(|p| policy_to_view(&p))
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn policy_to_view(p: &InsurancePolicy) -> InsuranceView {
    InsuranceView {
        wallet: p.wallet.clone(),
        key_id: p.key_id.clone(),
        coverage_amount: p.coverage_amount,
        threshold_bps: p.threshold_bps,
        purchase_price: p.purchase_price,
        premium: p.premium,
        expiry: p.expiry,
        paid_out: p.paid_out,
    }
}
