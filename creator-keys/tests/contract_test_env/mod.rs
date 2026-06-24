//! Shared setup helpers for `creator-keys` integration tests.
//!
//! Compose the small functions here instead of one monolithic setup so each test
//! can opt in only to what it needs (pricing without fees, fees, registered creators, etc.).
//!
//! For the minimum test categories and example structures when adding new entrypoints,
//! see `docs/minimum-viable-test-structure.md` in the repo root.
//!
//! Not every integration-test binary uses every helper; this crate is compiled once per
//! `tests/*.rs` target, so we allow dead code at module scope.
#![allow(dead_code)]

use creator_keys::{constants, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

/// Stable timestamp used by integration tests unless a test needs to override it.
pub const DEFAULT_TEST_TIMESTAMP: u64 = 1_700_000_000;

/// Sets ledger timestamp to a deterministic value for reproducible test snapshots.
pub fn set_test_timestamp(env: &Env, timestamp: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = timestamp;
    env.ledger().set(ledger);
}

/// Sets the ledger sequence number to a deterministic value.
///
/// Use this in tests that assert on `registered_at` so the expected value is
/// known at the call site rather than inferred from the default sequence.
pub fn set_ledger_sequence(env: &Env, sequence: u32) {
    let mut ledger = env.ledger().get();
    ledger.sequence_number = sequence;
    env.ledger().set(ledger);
}

/// Default [`Env`] for tests: enables mocked authorization for authed entrypoints.
pub fn test_env_with_auths() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

/// Register [`CreatorKeysContract`] and return a client and the contract id.
pub fn register_creator_keys<'a>(env: &'a Env) -> (CreatorKeysContractClient<'a>, Address) {
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &id);
    (client, id)
}

/// Admin sets a positive key price. Returns the admin address used.
pub fn set_key_price_for_tests(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    key_price: i128,
) -> Address {
    let admin = Address::generate(env);
    client.set_key_price(&admin, &key_price);
    admin
}

/// Set global fee split. Returns the admin address used.
pub fn set_protocol_fee_bps(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    creator_bps: u32,
    protocol_bps: u32,
) -> Address {
    let admin = Address::generate(env);
    client.set_fee_config(&admin, &creator_bps, &protocol_bps);
    admin
}

/// Set key price and fee config using the same admin (typical for quote and fee tests).
pub fn set_pricing_and_fees(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    key_price: i128,
    creator_bps: u32,
    protocol_bps: u32,
) -> Address {
    let admin = Address::generate(env);
    client.set_key_price(&admin, &key_price);
    client.set_fee_config(&admin, &creator_bps, &protocol_bps);
    admin
}

/// Register a new creator with the given display handle.
pub fn register_test_creator(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    handle: &str,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(&creator, &String::from_str(env, handle));
    creator
}

/// Register a new creator with a specific curve preset.
pub fn register_test_creator_with_preset(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    handle: &str,
    preset: creator_keys::CurvePreset,
) -> Address {
    let creator = Address::generate(env);
    client.register_creator(&creator, &String::from_str(env, handle), &Some(preset));
    creator
}

/// Standard creator basis points used by fixture helpers as the default fee split.
pub const DEFAULT_CREATOR_BPS: u32 = 9000;

/// Standard protocol basis points used by fixture helpers as the default fee split.
pub const DEFAULT_PROTOCOL_BPS: u32 = 1000;

/// Register a creator with a provided fee configuration and return the creator address.
///
/// This helper sets the global fee config and registers a new creator in one step,
/// reducing boilerplate in tests that need a registered creator with non-default fees.
/// For the standard 90/10 split, pass [`DEFAULT_CREATOR_BPS`] and [`DEFAULT_PROTOCOL_BPS`].
pub fn register_test_creator_with_fee_config(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    handle: &str,
    creator_bps: u32,
    protocol_bps: u32,
) -> Address {
    let admin = Address::generate(env);
    client.set_fee_config(&admin, &creator_bps, &protocol_bps);
    let creator = Address::generate(env);
    client.register_creator(&creator, &String::from_str(env, handle));
    creator
}

/// Write the persistent key price directly (bypassing `set_key_price`), for state edge cases.
pub fn set_stored_key_price(env: &Env, contract_id: &Address, price: i128) {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::KEY_PRICE, &price);
    });
}

/// Computes the expected buy price for a given supply value.
///
/// Current bonding curve formula:
/// price = base_price (fixed price model)
///
/// This helper ensures that test fixtures stay aligned with the contract's
/// pricing logic and makes magic numbers in assertions more descriptive.
/// Computes the expected buy price for a given supply value and curve preset.
pub fn compute_expected_buy_price(supply: u32, preset: creator_keys::CurvePreset) -> i128 {
    creator_keys::bonding_curve::compute_price(supply, 1, preset).unwrap()
}

/// Number of stroops in one display unit.
///
/// Creator key amounts use 7 decimal places, so 10,000,000 stroops equals
/// 1.0000000 display unit.
pub const STROOPS_PER_DISPLAY_UNIT: i128 = 10_000_000;

/// Converts a raw stroop amount into whole display units.
pub fn stroops_to_display_units(stroops: i128) -> i128 {
    stroops / STROOPS_PER_DISPLAY_UNIT
}

/// Snapshot of observable contract state for a (creator, holder) pair.
///
/// Capture before and after a read-only call with [`capture_snapshot`], then call
/// [`assert_unchanged`] to confirm no storage mutation occurred.
#[derive(Debug, PartialEq)]
pub struct ContractStateSnapshot {
    pub supply: u32,
    pub holder_count: u32,
    pub key_balance: u32,
}

/// Capture a [`ContractStateSnapshot`] for the given creator and holder.
pub fn capture_snapshot(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    holder: &Address,
) -> ContractStateSnapshot {
    ContractStateSnapshot {
        supply: client.get_total_key_supply(creator),
        holder_count: client.get_creator_holder_count(creator),
        key_balance: client.get_key_balance(creator, holder),
    }
}

/// Performs `count` buys with payment amounts that increment from `starting_amount`.
///
/// Returns the resulting observable state for the creator and buyer after the sequence.
pub fn perform_incrementing_buys(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    buyer: &Address,
    count: u32,
    starting_amount: i128,
    amount_step: i128,
) -> ContractStateSnapshot {
    for buy_index in 0..count {
        let payment = starting_amount + i128::from(buy_index) * amount_step;
        client.buy_key(creator, buyer, &payment, &None);
    }

    capture_snapshot(client, creator, buyer)
}

impl ContractStateSnapshot {
    /// Asserts that `self` and `other` are identical, failing with a descriptive message if not.
    pub fn assert_unchanged(&self, after: &ContractStateSnapshot) {
        assert_eq!(
            self, after,
            "contract state changed during a read-only call: before={self:?}, after={after:?}"
        );
    }
}

/// Computes the expected (gross) sell price for a given supply value.
///
/// Current bonding curve formula:
/// price = base_price (fixed price model)
///
/// Mirrors [`compute_expected_buy_price`]: a sell quote's gross `price` equals the
/// base key price regardless of supply. The seller's net payout is then
/// `price - creator_fee - protocol_fee`, computed via the `fee` helpers, so this
/// returns the gross figure that `get_sell_quote().price` is asserted against.
/// Computes the expected (gross) sell price for a given supply value and curve preset.
pub fn compute_expected_sell_price(supply: u32, preset: creator_keys::CurvePreset) -> i128 {
    // Sell price at supply S is the buy price at supply S-1
    if supply == 0 {
        0
    } else {
        creator_keys::bonding_curve::compute_price(supply - 1, 1, preset).unwrap()
    }
}

/// Computes the expected protocol fee from a given price and bps value.
///
/// This helper makes fixture intent explicit and keeps tests aligned
/// when the fee config changes.
pub fn compute_expected_protocol_fee(price: i128, protocol_bps: u32) -> i128 {
    (price * protocol_bps as i128) / 10_000
}

/// Computes the expected creator fee from a given price and fee split.
///
/// Mirrors [`compute_expected_protocol_fee`] using the shared fee split helper
/// so buy fee-recipient balance assertions stay aligned with quote math.
pub fn compute_expected_creator_fee(price: i128, creator_bps: u32, protocol_bps: u32) -> i128 {
    creator_keys::fee::compute_fee_split(price, creator_bps, protocol_bps).0
}

/// Represents a trade operation (buy or sell) in a sequence.
#[derive(Debug, Clone, Copy)]
pub enum TradeOperation {
    /// A buy operation (increases balance by 1).
    Buy,
    /// A sell operation (decreases balance by 1).
    Sell,
}

/// Computes the expected key balance after a sequence of buy and sell operations.
///
/// Takes an initial balance and applies a sequence of trades, returning the
/// final expected balance. Each buy increases the balance by 1, each sell
/// decreases it by 1 (stopping at 0 if a sell would go negative).
///
/// This helper makes test fixtures clearer by replacing magic numbers with
/// explicit trade sequences and reduces maintenance burden when test logic changes.
/// Mirrors the actual contract balance tracking logic.
pub fn compute_expected_balance_after_trades(
    initial_balance: u32,
    trades: &[TradeOperation],
) -> u32 {
    let mut balance = initial_balance as i32;
    for trade in trades {
        match trade {
            TradeOperation::Buy => balance += 1,
            TradeOperation::Sell => balance = (balance - 1).max(0),
        }
    }
    balance as u32
}
