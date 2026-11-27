#![no_std]

use soroban_sdk::{contractclient, symbol_short, token::TokenClient, Address, Bytes, Env, Symbol};

pub mod guard;

/// Error returned when a token transfer delivers a different amount than expected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedTokenError;

/// Transfers `expected_amount` of `token` from `from` to `to`, measuring the
/// recipient's balance before and after the transfer.
///
/// Returns `Ok(actual_received)` when `actual_received == expected_amount`, or
/// `Err(UnsupportedTokenError)` when the received amount differs (e.g.
/// fee-on-transfer, no-op transfer, or mid-transfer rebase).
pub fn transfer_exact(
    env: &Env,
    token: &Address,
    from: &Address,
    to: &Address,
    expected_amount: i128,
) -> Result<i128, UnsupportedTokenError> {
    let client = TokenClient::new(env, token);
    let balance_before = client.balance(to);
    client.transfer(from, to, &expected_amount);
    let balance_after = client.balance(to);
    let received = balance_after
        .checked_sub(balance_before)
        .ok_or(UnsupportedTokenError)?;
    if received != expected_amount {
        return Err(UnsupportedTokenError);
    }
    Ok(received)
}

/// Acknowledgement required for a key transfer callback to succeed.
pub const KEY_RECEIVED_ACK: Symbol = symbol_short!("key_ack");

/// Callback interface for contracts that accept transferred creator keys.
#[contractclient(name = "KeyReceiverClient")]
pub trait KeyReceiver {
    fn on_key_received(
        env: Env,
        from: Address,
        key_id: Address,
        amount: u32,
        data: Bytes,
    ) -> Symbol;
}
