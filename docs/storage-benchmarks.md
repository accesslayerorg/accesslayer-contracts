# Storage Read Benchmarks and Hot Paths

This document describes storage access patterns in the creator-keys contract and notes performance-sensitive paths for optimization candidates.

## Storage Reads by Hot Path

All storage reads use `env.storage().persistent()` to access the contract's persistent key-value store. The contract uses typed keys defined in [`constants::storage`](../creator-keys/src/lib.rs).

### Query Read Paths (Read-Only, No Auth Required)

These paths are called frequently by clients and indexers:

| Method | Key | Type | Count | Notes |
|--------|-----|------|-------|-------|
| `get_creator_details` | `DataKey::Creator(address)` | `CreatorProfile` | 1 | Caller: indexers, client profile views. Single lookup, no sub-reads. |
| `get_holder_key_count` | `DataKey::Creator(address)` + optional `DataKey::KeyBalance(creator, holder)` | `CreatorProfile` + optional `u32` | 1–2 | Conditional: 1 read if creator unregistered; 2 reads if registered and checking holder balance. |
| `get_creator_fee_config` | `DataKey::Creator(address)` + `DataKey::FeeConfig` | `CreatorProfile` + `FeeConfig` | 1–2 | Conditional: 1 read if creator unregistered; 2 reads if registered (requires fee config check). |
| `get_buy_quote` | `DataKey::KeyPrice` + `DataKey::Creator(creator)` + `DataKey::FeeConfig` | `i128` + `CreatorProfile` + `FeeConfig` | 3 | Hot path: called per quote preview before each buy. All three reads required. |
| `get_sell_quote` | `DataKey::KeyPrice` + `DataKey::Creator(creator)` + `DataKey::KeyBalance(creator, holder)` + `DataKey::FeeConfig` | `i128` + `CreatorProfile` + `u32` + `FeeConfig` | 4 | Hot path: called per quote preview before each sell. All four reads required. |

### State-Modifying Paths (Auth Required, Higher Latency Tolerance)

These paths are called during buy/sell transactions and creator setup:

| Method | Read Count | Reads | Notes |
|--------|-----------|-------|-------|
| `buy_key` | 3 | `DataKey::KeyPrice`, `DataKey::Creator(creator)`, `DataKey::KeyBalance(creator, buyer)` | Hot path for transactions. Reads key price and creator profile; reads (not writes) buyer balance to determine if holder count should increment. |
| `sell_key` | 1 | `DataKey::Creator(creator)` + implicit `DataKey::KeyBalance(creator, seller)` read in storage check | Hot path for transactions. Seller must hold keys. Requires two storage accesses (creator profile + holder balance) to verify balance and update both. |
| `register_creator` | 1 | `DataKey::Creator(creator)` (existence check only) | Infrequent. Single lookup to guard against double-registration. |
| `set_fee_config` | 1 | None (write-only) | Infrequent (admin-only). No reads; just validation and storage write. |
| `set_key_price` | 1 | None (write-only) | Infrequent (admin-only). No reads; just validation and storage write. |

## Performance Optimization Candidates

### Short-Term (No Schema Change)

1. **Cache key price in quote loops**: If a client previews multiple quotes in a single session, the key price is read from storage on every quote call. Consider caching this client-side or in a batched quote endpoint if the contract API is extended.

2. **Combine `get_holder_key_count` and `get_creator_fee_config` queries**: Both read the same `CreatorProfile` once. If used together in a client query, a new view method could return both in a single call. (Schema extension; see below.)

3. **Monitor holder count churn**: Every buy/sell that changes the holder count for a creator writes the entire `CreatorProfile`. The profile is small (address, handle string, supply u32, holder_count u32, fee_recipient address), so this is not a bottleneck today but scales linearly with profile size.

### Medium-Term (Schema Extensions)

1. **Separate holder count storage**: If `CreatorProfile` grows, consider storing `holder_count` in its own data key (e.g., `DataKey::HolderCount(creator)`) to avoid rewriting the entire profile on every buy/sell. Verify with gas profiling before implementing.

2. **Indexed key balance reads**: The contract uses a composite key `DataKey::KeyBalance(creator, holder)` for each holder balance. No bulk-read API exists. If analytics require scanning all holders for a creator, this requires off-chain polling or an indexer.

3. **Quote view batching**: Extend the contract with a method like `get_quote_batch(creators: Vec<Address>) -> Vec<QuoteResponse>` to reduce round-trip latency for clients that show multiple creator quotes at once.

## Reproducible Benchmarking

To measure storage read cost in your environment:

### Manual Profiling in Test

```rust
// In a test file, measure a single hot-path call:
let env = Env::default();
env.mock_all_auths();
let (client, contract_id) = register_creator_keys(&env);
set_pricing_and_fees(&env, &client, 1000, 9000, 1000);
let creator = register_test_creator(&env, &client, "bench_creator");

// Measure quote call
let start = std::time::Instant::now();
for _ in 0..100 {
    let _ = client.get_buy_quote(&creator);
}
let elapsed = start.elapsed();
println!("100 buy quotes: {:?} per call avg", elapsed / 100);
```

Run with `cargo test -- --nocapture bench_hot_path` to see console output.

### On Testnet

Use the Stellar RPC `estimateTransactionSize` or `simulateTransaction` to measure actual gas consumption:

```bash
# After deploying contract to testnet, run a real transaction:
soroban contract invoke ... --network testnet -- get_buy_quote --creator <address> | jq '.result'

# Check gas in the transaction envelope.
```

## Storage Layout Summary

```
Persistent Storage Keys:
- FeeConfig: Global protocol fee split (1 per contract)
- KeyPrice: Global key price (1 per contract)
- Creator(Address): Creator profile and supply (1 per creator)
- KeyBalance(creator, holder): Holder balance for creator (1 per holder per creator)
- TreasuryAddress, AdminAddress, ProtocolFeeRecipient: Global admin addresses (1 each)

No ephemeral or temporary storage is used.
```

## Notes for Contributors

- When adding new contract features, estimate storage reads and writes. Update this document if new hot paths are introduced.
- Use `env.storage().persistent()` for all data. Do not use ledger or volatile storage for contract state that must survive across invocations.
- Profile storage cost on testnet before mainnet deployment. Gas costs vary by network and may change with Soroban SDK updates.
