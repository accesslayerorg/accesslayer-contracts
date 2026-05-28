# Creator State Storage TTL

This document explains how creator state is stored in Soroban, which entries are long-lived, and what TTL bump strategy operators should plan for in production.

For the broader storage-key schema and invariants, see [storage-key-invariants.md](./storage-key-invariants.md).

## Current Storage Classes

The `creator-keys` contract stores creator state only in Soroban persistent storage via `env.storage().persistent()`. It does not currently store any creator state in `temporary()` or `instance()` storage.

That matters because persistent entries can still expire if their TTL is not extended. Unlike temporary entries, expired persistent entries may be restored, but production integrations should treat expiry as an outage risk rather than a normal lifecycle event.

## Creator State Entry Types

| Entry | Storage key | Storage class | Long-lived? | Notes |
| --- | --- | --- | --- | --- |
| Creator profile | `DataKey::Creator(Address)` | Persistent | Yes | Stores registration metadata, supply, holder count, and fee recipient for a creator. |
| Holder balance | `DataKey::KeyBalance(Address, Address)` | Persistent | Yes | Stores the number of keys a specific holder owns for a specific creator. |

## Non-Creator Entries

The contract also stores global protocol configuration in persistent storage:

- `DataKey::FeeConfig`
- `DataKey::KeyPrice`
- `DataKey::TreasuryAddress`
- `DataKey::AdminAddress`
- `DataKey::ProtocolFeeRecipient`

These entries are important for contract operation, but they are not the creator-scoped state covered by this document.

## Persistent vs Temporary Summary

Creator state entries are currently classified as follows:

- Persistent: `DataKey::Creator(Address)`, `DataKey::KeyBalance(Address, Address)`
- Temporary: none
- Instance: none

This means there are no creator-state entries that automatically fall into a short-lived, disposable category. Every creator profile and holder balance is expected to remain available over long time horizons.

## Expected TTL Bump Strategy

Because creator state is long-lived, operators should plan to extend TTL before entries get close to expiry.

Recommended approach:

1. Periodically bump every active creator profile entry (`DataKey::Creator(Address)`).
2. Periodically bump holder balance entries (`DataKey::KeyBalance(Address, Address)`) for holders that should retain their positions.
3. Bump the contract instance and wasm code on their own maintenance cadence as part of the same operational runbook.
4. Do not rely on read-only contract calls to refresh TTL automatically; the current contract implementation does not call `extend_ttl` for creator entries.

In practice, the simplest safe policy is to run a scheduled maintenance job that enumerates known creators and live holder positions, then extends TTL well before the network threshold is reached.

## Entries Most at Risk of Expiry

The highest-risk creator-state entries are the ones that may become dormant for long periods:

- Creator profiles for inactive creators that are no longer trading
- Holder balance entries for long-term holders who stop interacting with the contract

These entries still represent valid on-chain state, so they should not be allowed to drift toward expiry just because the creator or holder is inactive.

## Operational Guidance

- Treat creator profiles as canonical state that should be preserved for the lifetime of the product.
- Treat non-zero holder balances as equally important long-lived state.
- If a future contract version introduces `temporary()` storage for derived or cache-like data, document it separately and keep it clearly distinct from canonical creator state.
- If a future contract version starts bumping TTL in-contract, update this document to describe which write paths extend which entries.

## Code References

- `DataKey` definitions: [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs)
- Creator profile reads and writes: [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs)
- Holder balance reads and writes: [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs)

## External References

- [Soroban storage types](https://docs.rs/soroban-sdk/latest/soroban_sdk/storage/struct.Storage.html)
- [Persistent storage TTL extension methods](https://docs.rs/soroban-sdk/latest/soroban_sdk/storage/struct.Persistent.html)
- [Stellar guide: choosing the right storage type](https://developers.stellar.org/docs/build/guides/storage/choosing-the-right-storage)
