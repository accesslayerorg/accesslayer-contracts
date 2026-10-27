#![no_std]

//! Session nonce contract for replay protection on signed actions (Issue #1036).
//!
//! Each wallet has a sequential nonce counter starting at `0`. An action
//! handler validates a signed off-chain action by calling [`consume_nonce`]
//! with the nonce embedded in the signed payload. The nonce must equal the
//! wallet's current counter; it is invalidated immediately by incrementing
//! the counter, so replaying the same signed action fails with
//! [`SessionNonceError::NonceAlreadyUsed`].
//!
//! Soroban executes transactions serially and the read-check-increment below
//! happens inside a single invocation, so two submissions carrying the same
//! nonce can never both succeed: the second one observes the incremented
//! counter and is rejected.
//!
//! [`consume_nonce`]: SessionNonceContract::consume_nonce

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

#[cfg(test)]
mod test;

/// Name of the event emitted when a signed action's nonce is consumed.
pub const ACTION_EXECUTED_EVENT_NAME: &str = "ActionExecuted";

/// Ledger threshold at which a wallet's nonce entry is refreshed towards
/// [`NONCE_TTL_EXTEND_TO`].
pub const NONCE_TTL_THRESHOLD: u32 = 100_000;

/// Ledger count to extend a wallet's nonce entry to (~30 days at 5s ledgers).
pub const NONCE_TTL_EXTEND_TO: u32 = 518_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SessionNonceError {
    /// The supplied nonce is not the wallet's current nonce: it was already
    /// consumed (replay) or is ahead of the counter (out of order).
    NonceAlreadyUsed = 1,
    /// The wallet's nonce counter cannot be incremented further.
    NonceOverflow = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Next unused nonce for a wallet. Absent means `0`.
    Nonce(Address),
}

/// Payload of the `ActionExecuted` event.
///
/// Event shape:
/// - topics: `(Symbol("ActionExecuted"), wallet)`
/// - data: `ActionExecuted`
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionExecuted {
    /// Wallet whose nonce was consumed.
    pub wallet: Address,
    /// The nonce that was consumed by this action.
    pub nonce: u64,
}

#[contract]
pub struct SessionNonceContract;

fn read_nonce(env: &Env, wallet: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::Nonce(wallet.clone()))
        .unwrap_or(0)
}

#[contractimpl]
impl SessionNonceContract {
    /// Returns the current (next unused) nonce for `wallet`. Starts at `0`.
    pub fn get_nonce(env: Env, wallet: Address) -> u64 {
        read_nonce(&env, &wallet)
    }

    /// Validates and consumes `nonce` for `wallet`.
    ///
    /// `nonce` must equal [`get_nonce`](Self::get_nonce); on success the
    /// stored nonce is incremented, `ActionExecuted` is emitted with the
    /// consumed nonce, and the new current nonce is returned. Any other value
    /// is rejected with [`SessionNonceError::NonceAlreadyUsed`] and leaves
    /// state untouched. Requires the wallet's authorization so a third party
    /// cannot advance (and thereby invalidate) another wallet's nonce.
    pub fn consume_nonce(env: Env, wallet: Address, nonce: u64) -> Result<u64, SessionNonceError> {
        wallet.require_auth();

        let current = read_nonce(&env, &wallet);
        if nonce != current {
            return Err(SessionNonceError::NonceAlreadyUsed);
        }
        let next = current
            .checked_add(1)
            .ok_or(SessionNonceError::NonceOverflow)?;

        let key = DataKey::Nonce(wallet.clone());
        env.storage().persistent().set(&key, &next);
        env.storage()
            .persistent()
            .extend_ttl(&key, NONCE_TTL_THRESHOLD, NONCE_TTL_EXTEND_TO);

        env.events().publish(
            (
                Symbol::new(&env, ACTION_EXECUTED_EVENT_NAME),
                wallet.clone(),
            ),
            ActionExecuted { wallet, nonce },
        );
        Ok(next)
    }
}
