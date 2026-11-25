//! #1236: signed holder-balance attestations for off-chain gating.
//!
//! An off-chain attester signs [`attestation_payload`] with the ed25519 key the
//! admin registered via [`set_attester`]. Any third party can then check the
//! attestation with [`verify_attestation`] instead of trusting an RPC read.
//!
//! The payload binds the attestation to this contract and this network so it
//! cannot be replayed against another deployment, and it is only accepted for
//! [`ATTESTATION_TTL_LEDGERS`] ledgers after the attested ledger. Rotating the
//! attester key immediately invalidates everything the old key signed.
//!
//! Payload layout (all integers big-endian):
//!
//! ```text
//! "ACL_BALANCE_ATTESTATION_V1"
//! sha256(network passphrase)   32 bytes
//! xdr(contract address)
//! xdr(holder)
//! xdr(key_id)
//! balance                      i128, 16 bytes
//! ledger                       u32,   4 bytes
//! ```

use crate::{assert_is_admin, extend_key_ttl_to_full_window};
use soroban_sdk::{contracterror, contracttype, xdr::ToXdr, Address, Bytes, BytesN, Env, Symbol};

/// Ledgers after the attested ledger during which an attestation is accepted
/// (~1 hour at 5 seconds per ledger).
pub const ATTESTATION_TTL_LEDGERS: u32 = 720;

const DOMAIN_TAG: &[u8] = b"ACL_BALANCE_ATTESTATION_V1";

/// Errors raised by the balance attestation entrypoints.
///
/// Kept separate from [`crate::ContractError`] because Soroban caps
/// `#[contracterror]` enums at 50 variants.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AttestationError {
    /// The caller is not the protocol admin.
    Unauthorized = 1,
    /// No attester key has been configured.
    AttesterNotSet = 2,
    /// An attester key is already configured; use `rotate_attester`.
    AttesterAlreadySet = 3,
    /// The attestation is older than [`ATTESTATION_TTL_LEDGERS`].
    AttestationExpired = 4,
    /// The attested ledger is later than the current ledger.
    FutureLedger = 5,
    /// The new attester key equals the current one.
    SameAttester = 6,
}

#[derive(Clone)]
#[contracttype]
pub enum AttestationKey {
    /// The ed25519 public key allowed to sign attestations.
    Attester,
}

/// Emitted with topic `("attester_set",)` when the first attester is configured.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AttesterSetEvent {
    pub public_key: BytesN<32>,
}

/// Emitted with topic `("attester_rotated",)` when the attester key changes.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AttesterRotatedEvent {
    pub old_key: BytesN<32>,
    pub new_key: BytesN<32>,
}

fn require_admin(env: &Env, admin: &Address) -> Result<(), AttestationError> {
    admin.require_auth();
    assert_is_admin(env, admin).map_err(|_| AttestationError::Unauthorized)
}

fn save_attester(env: &Env, public_key: &BytesN<32>) {
    let key = AttestationKey::Attester;
    env.storage().persistent().set(&key, public_key);
    extend_key_ttl_to_full_window(env, &key);
}

/// The currently trusted attester key, if any.
pub fn attester(env: &Env) -> Option<BytesN<32>> {
    env.storage().persistent().get(&AttestationKey::Attester)
}

pub fn set_attester(
    env: &Env,
    admin: &Address,
    public_key: &BytesN<32>,
) -> Result<(), AttestationError> {
    require_admin(env, admin)?;
    if attester(env).is_some() {
        return Err(AttestationError::AttesterAlreadySet);
    }
    save_attester(env, public_key);
    env.events().publish(
        (Symbol::new(env, "attester_set"),),
        AttesterSetEvent {
            public_key: public_key.clone(),
        },
    );
    Ok(())
}

/// Replaces the attester key. Attestations signed by the old key stop verifying
/// immediately, because verification only ever checks the current key.
pub fn rotate_attester(
    env: &Env,
    admin: &Address,
    new_key: &BytesN<32>,
) -> Result<(), AttestationError> {
    require_admin(env, admin)?;
    let old_key = attester(env).ok_or(AttestationError::AttesterNotSet)?;
    if old_key == *new_key {
        return Err(AttestationError::SameAttester);
    }
    save_attester(env, new_key);
    env.events().publish(
        (Symbol::new(env, "attester_rotated"),),
        AttesterRotatedEvent {
            old_key,
            new_key: new_key.clone(),
        },
    );
    Ok(())
}

/// The canonical bytes the attester signs (raw ed25519, no pre-hashing).
pub fn attestation_payload(
    env: &Env,
    holder: &Address,
    key_id: &Address,
    balance: i128,
    ledger: u32,
) -> Bytes {
    let mut payload = Bytes::from_slice(env, DOMAIN_TAG);
    payload.append(&env.ledger().network_id().into());
    payload.append(&env.current_contract_address().to_xdr(env));
    payload.append(&holder.clone().to_xdr(env));
    payload.append(&key_id.clone().to_xdr(env));
    payload.extend_from_array(&balance.to_be_bytes());
    payload.extend_from_array(&ledger.to_be_bytes());
    payload
}

/// Checks freshness, then the signature against the current attester key.
///
/// Returns `Ok(true)` for a valid attestation. Freshness and configuration
/// failures return an [`AttestationError`]. A bad signature (tampered fields,
/// wrong or rotated-out key) makes the host's ed25519 check abort the
/// invocation, so callers should treat any failed call as "not verified".
pub fn verify_attestation(
    env: &Env,
    holder: &Address,
    key_id: &Address,
    balance: i128,
    ledger: u32,
    signature: &BytesN<64>,
) -> Result<bool, AttestationError> {
    let public_key = attester(env).ok_or(AttestationError::AttesterNotSet)?;
    let current = env.ledger().sequence();
    if ledger > current {
        return Err(AttestationError::FutureLedger);
    }
    if current - ledger > ATTESTATION_TTL_LEDGERS {
        return Err(AttestationError::AttestationExpired);
    }
    let payload = attestation_payload(env, holder, key_id, balance, ledger);
    env.crypto()
        .ed25519_verify(&public_key, &payload, signature);
    Ok(true)
}
