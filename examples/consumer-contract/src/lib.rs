#![no_std]
//! Example integrator contract for the AccessLayer creator-keys protocol.
//!
//! It shows the three things most integrations need, against the deployed
//! `creator-keys` contract and nothing else:
//!
//! 1. **Gate a function on key balance**: [`ConsumerContract::enter`] only
//!    succeeds for a caller holding at least `min_balance` keys of the
//!    configured creator, read live through `get_key_balance`.
//! 2. **Read the current price**: [`ConsumerContract::current_quote`] calls
//!    `get_buy_quote` and turns the protocol's errors into this contract's
//!    own [`ConsumerError`] codes.
//! 3. **Accept key transfers**: [`ConsumerContract::on_key_received`]
//!    implements the [`KeyReceiver`] callback that `transfer_and_call` invokes,
//!    accepting only the configured key.
//!
//! The walkthrough is in `docs/integration.md`; the tests in `src/test.rs` run
//! every path against the real `creator-keys` contract in CI.
//!
//! The protocol is reached through a small hand-written interface
//! ([`CreatorKeysInterface`]) instead of a dependency on the `creator-keys`
//! crate, so this contract's Wasm only contains the calls it makes.

use common::{KeyReceiver, KEY_RECEIVED_ACK};
use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, panic_with_error,
    symbol_short, Address, Bytes, Env, Symbol,
};

#[cfg(test)]
mod test;

/// Symbol returned by [`ConsumerContract::on_key_received`] to refuse a
/// transfer. Anything other than [`KEY_RECEIVED_ACK`] makes `transfer_and_call`
/// revert the whole transfer with `TransferNotAccepted`.
pub const REJECT: Symbol = symbol_short!("reject");

/// Event topic published by [`ConsumerContract::enter`]: `(ACCESS_EVENT, caller)`
/// with the caller's key balance as data.
pub const ACCESS_EVENT: Symbol = symbol_short!("access");

/// Event topic published when a transfer is accepted: `(RECEIVED_EVENT, from)`
/// with the transferred amount as data.
pub const RECEIVED_EVENT: Symbol = symbol_short!("received");

/// Ledgers per day at a 5 second close time, used for TTL extension.
const DAY_IN_LEDGERS: u32 = 17_280;
/// Extend this contract's storage to roughly 30 days whenever it drops below
/// roughly 7 days.
const TTL_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
const TTL_EXTEND_TO: u32 = 30 * DAY_IN_LEDGERS;

/// Wire-compatible mirror of `creator_keys::QuoteResponse`.
///
/// A `#[contracttype]` struct is encoded as a map keyed by field name, so a
/// struct with the same field names and types decodes the protocol's value
/// without importing the protocol crate.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuoteResponse {
    /// Key price before fees, in the payment asset's smallest unit.
    pub price: i128,
    /// Creator share of the fee.
    pub creator_fee: i128,
    /// Protocol share of the fee.
    pub protocol_fee: i128,
    /// What a buyer pays in total: `price + creator_fee + protocol_fee`.
    pub total_amount: i128,
}

/// The `creator_keys::ContractError` codes this example handles explicitly.
///
/// Error codes are stable across protocol upgrades (new variants are only
/// appended), so mirroring the ones you branch on is safe. Codes not listed
/// here still reach the caller as an `InvokeError::Contract(code)`.
#[contracterror(export = false)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ProtocolError {
    /// The creator key is not registered.
    NotRegistered = 2,
    /// The protocol admin has not set a base key price yet.
    KeyPriceNotSet = 5,
}

/// The slice of the `creator-keys` public interface this contract calls.
///
/// `contractclient` generates `CreatorKeysClient`, including the
/// `try_*` variants that return the callee's error instead of trapping.
#[contractclient(name = "CreatorKeysClient")]
pub trait CreatorKeysInterface {
    /// Keys `wallet` holds for `creator`; `0` when it never held any.
    fn get_key_balance(env: Env, creator: Address, wallet: Address) -> u32;
    /// Price and fee breakdown for buying the next key of `creator`.
    fn get_buy_quote(env: Env, creator: Address) -> Result<QuoteResponse, ProtocolError>;
}

/// Errors returned by this example contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ConsumerError {
    /// Constructor arguments were rejected (zero `min_balance`, or the keys
    /// contract and the key id are the same address).
    InvalidConfig = 1,
    /// The caller holds fewer keys than `min_balance`.
    InsufficientKeys = 2,
    /// The protocol has no quote for this key: it is not registered or no
    /// base price is set.
    QuoteUnavailable = 3,
    /// Any other failure while calling the protocol.
    ProtocolCallFailed = 4,
}

/// Immutable configuration written by the constructor.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// Address of the deployed `creator-keys` contract.
    pub keys_contract: Address,
    /// The creator key this integration is built around.
    pub key_id: Address,
    /// Minimum key balance required by [`ConsumerContract::enter`].
    pub min_balance: u32,
}

/// The last transfer accepted through [`ConsumerContract::on_key_received`].
///
/// Informational only: anyone can call `on_key_received` directly with made-up
/// arguments, so a receipt is never proof that keys arrived. Use
/// [`ConsumerContract::held_keys`] for that.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    /// Address the keys were transferred from.
    pub from: Address,
    /// Number of keys transferred.
    pub amount: u32,
    /// Caller-supplied payload (at most 256 bytes, enforced by the protocol).
    pub data: Bytes,
    /// Ledger sequence at which the callback ran.
    pub ledger: u32,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
    LastReceipt,
    Entries(Address),
}

#[contract]
pub struct ConsumerContract;

#[contractimpl]
impl ConsumerContract {
    /// Stores the protocol address, the key to integrate with and the minimum
    /// balance for [`Self::enter`].
    ///
    /// # Panics
    /// * [`ConsumerError::InvalidConfig`] if `min_balance` is `0` or
    ///   `keys_contract == key_id`.
    pub fn __constructor(env: Env, keys_contract: Address, key_id: Address, min_balance: u32) {
        if min_balance == 0 || keys_contract == key_id {
            panic_with_error!(&env, ConsumerError::InvalidConfig);
        }
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                keys_contract,
                key_id,
                min_balance,
            },
        );
        extend_instance_ttl(&env);
    }

    /// Returns the stored configuration.
    pub fn config(env: Env) -> Config {
        read_config(&env)
    }

    /// A key-gated action: succeeds only when `caller` holds at least
    /// `min_balance` keys of the configured creator.
    ///
    /// The balance is read from the protocol on every call, so selling or
    /// transferring keys removes access immediately. Returns the caller's
    /// current balance and records one entry for them.
    ///
    /// # Auth
    /// Requires `caller` authorization.
    ///
    /// # Errors
    /// * [`ConsumerError::InsufficientKeys`] if the balance is below
    ///   `min_balance`.
    pub fn enter(env: Env, caller: Address) -> Result<u32, ConsumerError> {
        caller.require_auth();
        let config = read_config(&env);
        let balance = CreatorKeysClient::new(&env, &config.keys_contract)
            .get_key_balance(&config.key_id, &caller);
        if balance < config.min_balance {
            return Err(ConsumerError::InsufficientKeys);
        }

        let key = DataKey::Entries(caller.clone());
        let entries: u32 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&key, &entries.saturating_add(1));
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        extend_instance_ttl(&env);

        env.events().publish((ACCESS_EVENT, caller), balance);
        Ok(balance)
    }

    /// Read-only: whether `user` currently passes the [`Self::enter`] gate.
    pub fn has_access(env: Env, user: Address) -> bool {
        let config = read_config(&env);
        CreatorKeysClient::new(&env, &config.keys_contract).get_key_balance(&config.key_id, &user)
            >= config.min_balance
    }

    /// Read-only: how many times `user` has passed [`Self::enter`].
    pub fn entries(env: Env, user: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Entries(user))
            .unwrap_or(0)
    }

    /// Read-only: the protocol's current buy quote for the configured key.
    ///
    /// Uses the generated `try_get_buy_quote` so a protocol error comes back as
    /// a value instead of aborting this contract.
    ///
    /// # Errors
    /// * [`ConsumerError::QuoteUnavailable`] if the key is not registered or
    ///   no base price is set.
    /// * [`ConsumerError::ProtocolCallFailed`] for any other failure.
    pub fn current_quote(env: Env) -> Result<QuoteResponse, ConsumerError> {
        let config = read_config(&env);
        match CreatorKeysClient::new(&env, &config.keys_contract).try_get_buy_quote(&config.key_id)
        {
            Ok(Ok(quote)) => Ok(quote),
            Err(Ok(ProtocolError::NotRegistered | ProtocolError::KeyPriceNotSet)) => {
                Err(ConsumerError::QuoteUnavailable)
            }
            _ => Err(ConsumerError::ProtocolCallFailed),
        }
    }

    /// Read-only: keys of the configured creator held by this contract, read
    /// from the protocol. This, not [`Self::last_receipt`], is the trustworthy
    /// record of what the contract owns.
    pub fn held_keys(env: Env) -> u32 {
        let config = read_config(&env);
        CreatorKeysClient::new(&env, &config.keys_contract)
            .get_key_balance(&config.key_id, &env.current_contract_address())
    }

    /// Read-only: the last transfer accepted by [`Self::on_key_received`].
    pub fn last_receipt(env: Env) -> Option<Receipt> {
        env.storage().instance().get(&DataKey::LastReceipt)
    }

    /// Receiver callback invoked by `creator-keys` `transfer_and_call` after the
    /// keys have already moved to this contract. See the [`KeyReceiver`] impl
    /// below for the rules it applies.
    pub fn on_key_received(
        env: Env,
        from: Address,
        key_id: Address,
        amount: u32,
        data: Bytes,
    ) -> Symbol {
        <Self as KeyReceiver>::on_key_received(env, from, key_id, amount, data)
    }
}

impl KeyReceiver for ConsumerContract {
    /// Accepts transfers of the configured key and rejects everything else.
    ///
    /// Returning [`REJECT`] makes the protocol revert the transfer, so the
    /// sender keeps their keys. Two rules for writing a receiver:
    ///
    /// * Do not call back into `creator-keys` here. It is still executing
    ///   `transfer_and_call`, and Soroban rejects re-entry into a contract that
    ///   is already on the call stack, which would revert the transfer.
    /// * Treat the arguments as untrusted. Anyone can invoke this function
    ///   directly, so grant nothing of value from them alone; read balances
    ///   with `get_key_balance` in a separate call instead.
    fn on_key_received(
        env: Env,
        from: Address,
        key_id: Address,
        amount: u32,
        data: Bytes,
    ) -> Symbol {
        let config = read_config(&env);
        if key_id != config.key_id || amount == 0 {
            return REJECT;
        }

        env.storage().instance().set(
            &DataKey::LastReceipt,
            &Receipt {
                from: from.clone(),
                amount,
                data,
                ledger: env.ledger().sequence(),
            },
        );
        extend_instance_ttl(&env);
        env.events().publish((RECEIVED_EVENT, from), amount);
        KEY_RECEIVED_ACK
    }
}

fn read_config(env: &Env) -> Config {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .unwrap_or_else(|| panic_with_error!(env, ConsumerError::InvalidConfig))
}

fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
}
