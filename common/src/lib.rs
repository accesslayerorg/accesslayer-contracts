#![no_std]

use soroban_sdk::{contractclient, symbol_short, Address, Bytes, Env, Symbol};

pub mod guard;

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
