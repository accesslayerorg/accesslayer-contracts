#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec};

#[cfg(test)]
mod test;

/// Name of the event emitted the first time a nonce is consumed for a wallet.
pub const NONCE_CONSUMED_EVENT_NAME: &str = "NonceConsumed";

/// Ledger threshold at which a consumed nonce is refreshed towards
/// [`NONCE_TTL_EXTEND_TO`]. Archival of a persistent entry keeps the stored
/// value, so replay protection survives; the refresh only avoids the cost of
/// restoring an archived entry on the hot path.
pub const NONCE_TTL_THRESHOLD: u32 = 100_000;

/// Ledger count to extend a consumed nonce to (~30 days at 5s ledgers).
pub const NONCE_TTL_EXTEND_TO: u32 = 518_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NonceRegistryError {
    /// The nonce has already been consumed for this wallet.
    NonceAlreadyUsed = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Marks `(wallet, nonce)` as consumed. Nonces carry no ordering, so each
    /// pair is tracked independently.
    UsedNonce(Address, u64),
}

/// Payload of the `NonceConsumed` event.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonceConsumed {
    pub wallet: Address,
    pub nonce: u64,
}

#[contract]
pub struct NonceRegistryContract;

fn used_nonce_key(wallet: &Address, nonce: u64) -> DataKey {
    DataKey::UsedNonce(wallet.clone(), nonce)
}

fn is_used(env: &Env, wallet: &Address, nonce: u64) -> bool {
    env.storage()
        .persistent()
        .has(&used_nonce_key(wallet, nonce))
}

fn store_used(env: &Env, wallet: &Address, nonce: u64) {
    let key = used_nonce_key(wallet, nonce);
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, NONCE_TTL_THRESHOLD, NONCE_TTL_EXTEND_TO);
}

fn emit_consumed(env: &Env, wallet: &Address, nonce: u64) {
    env.events().publish(
        (Symbol::new(env, NONCE_CONSUMED_EVENT_NAME), wallet.clone()),
        NonceConsumed {
            wallet: wallet.clone(),
            nonce,
        },
    );
}

#[contractimpl]
impl NonceRegistryContract {
    /// Marks `nonce` as consumed for `wallet`.
    ///
    /// Returns [`NonceRegistryError::NonceAlreadyUsed`] if the nonce was already
    /// consumed, and emits `NonceConsumed` otherwise. Deliberately does not
    /// require authorization: submission is performed by a relayer, and the
    /// signed message being verified is what authorizes the action. As a
    /// result a third party can burn a nonce, which only invalidates a
    /// meta-transaction that has not been submitted yet.
    pub fn consume_nonce(env: Env, wallet: Address, nonce: u64) -> Result<(), NonceRegistryError> {
        if is_used(&env, &wallet, nonce) {
            return Err(NonceRegistryError::NonceAlreadyUsed);
        }
        store_used(&env, &wallet, nonce);
        emit_consumed(&env, &wallet, nonce);
        Ok(())
    }

    /// Returns whether `nonce` has been consumed for `wallet`.
    pub fn is_nonce_used(env: Env, wallet: Address, nonce: u64) -> bool {
        is_used(&env, &wallet, nonce)
    }

    /// Pre-emptively burns every nonce in `nonces` for `wallet`.
    ///
    /// Only the wallet itself may invalidate its nonces. Nonces that are
    /// already consumed are skipped rather than rejected, so the call is
    /// idempotent; the returned value is the number of newly consumed nonces.
    /// `NonceConsumed` is emitted once per newly consumed nonce.
    pub fn bulk_invalidate(env: Env, wallet: Address, nonces: Vec<u64>) -> u32 {
        wallet.require_auth();

        let mut newly_consumed = 0u32;
        for nonce in nonces.iter() {
            if is_used(&env, &wallet, nonce) {
                continue;
            }
            store_used(&env, &wallet, nonce);
            emit_consumed(&env, &wallet, nonce);
            newly_consumed += 1;
        }
        newly_consumed
    }
}
