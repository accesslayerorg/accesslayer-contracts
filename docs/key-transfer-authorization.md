# Key Transfer Authorization Model

This document describes the authorization rules for the `transfer_keys` function: who can call it, how Soroban auth is enforced, and what side-effects (or lack thereof) callers should expect.

For the complete contract authorization model covering all entrypoints, see [authorization-model.md](./authorization-model.md). For storage key invariants, see [storage-key-invariants.md](./storage-key-invariants.md).

---

## Function Signature

```rust
pub fn transfer_keys(
    env: Env,
    creator: Address,
    from: Address,
    to: Address,
    amount: u32,
) -> Result<(), ContractError>
```

---

## Authorization Rule

**Only the current key holder (`from`) authorizes the transfer.** The recipient (`to`) does **not** need to approve or sign the transaction.

This is enforced by a single `require_auth` call on the sender's address:

```rust
from.require_auth();
```

There is no corresponding `to.require_auth()` — the recipient's approval is neither required nor requested. This is consistent with the existing `sell_key` pattern, where only the seller authorizes the sale.

### Why no recipient approval?

- **Key transfers are sender-initiated**: The sender decides which keys to transfer and to whom. Requiring recipient approval would introduce a two-step flow (sender initiates, recipient approves) that is unnecessary for the intended use cases (see [Use Cases](#use-cases) below).
- **Consistency with Soroban token transfers**: The standard Stellar token interface (`SorobanToken`) also authenticates only the sender for `transfer` operations. The recipient does not sign.
- **Simplicity**: Avoiding an off-chain approval dance reduces UX friction for both wallets and marketplaces.

---

## Soroban `require_auth` Enforcement

The Soroban host function [`Address::require_auth`](https://docs.rs/soroban-sdk/latest/soroban_sdk/struct.Address.html#method.require_auth) performs the following at the protocol level:

1. **Signature verification**: The transaction must include a valid signature from the `from` address.
2. **Authentication entry**: The host records an authentication entry for `from` so that the caller's identity is cryptographically proven.
3. **Replay protection**: Authenticated invocations are scoped to the current transaction and ledger sequence; they cannot be replayed in a different context.

In the contract, `from.require_auth()` is called at the top of `transfer_keys`, before any state mutation:

```rust
pub fn transfer_keys(
    env: Env,
    creator: Address,
    from: Address,
    to: Address,
    amount: u32,
) -> Result<(), ContractError> {
    from.require_auth();  // ✅ Sender authenticates
    // ... subsequent validation and state updates ...
}
```

If the signature is missing or invalid, the Soroban host raises an `AuthInvalidError` before any contract code executes beyond the auth check. The function will not proceed.

### Error on invalid auth

When `from.require_auth()` fails (e.g., an imposter address or missing signature), the Soroban runtime returns a non-recoverable authentication error. The contract exits early and no state is mutated.

---

## Fee Behavior

**`transfer_keys` does not charge any fee.** No fee split is computed, and no creator or protocol fee is accrued.

This is because:

- The function **does not interact with the bonding curve** (no price lookup, no supply adjustment).
- Keys are moved between holders at a 1:1 ratio (1 key decremented from sender, 1 key incremented for recipient). Total supply for the creator remains unchanged.
- The only storage modifications are:
  - Decrement `KeyBalance(creator, from)` by `amount`
  - Increment `KeyBalance(creator, to)` by `amount`

Since the fixed key price is never read and no fee math is performed, the operation has zero protocol-level cost beyond the base Soroban transaction fee (ledger entry updates).

### Contrast with buy/sell

| Operation | Bonding curve interaction | Fee charged |
|-----------|--------------------------|-------------|
| `buy_key` | Yes (price lookup, supply + 1) | Yes (fee split computed and accrued) |
| `sell_key` | Yes (price lookup, supply - 1) | Yes (protocol fee accrued via `accrue_sell_protocol_fee`) |
| `transfer_keys` | **No** | **No** |

---

## Self-Transfer Restriction

**`transfer_keys` rejects self-transfers** — calls where `from == to` — with `ContractError::SelfTransfer`.

```rust
if from == to {
    return Err(ContractError::SelfTransfer);
}
```

### Rationale

- A self-transfer would decrement and increment the same holder's balance, resulting in a no-op state change.
- Allowing it would waste ledger space and caller gas without providing any useful semantic.
- The dedicated `SelfTransfer` error code makes the rejection reason unambiguous for clients and indexers, rather than overloading a zero-address error.

### Other preconditions

Beyond the self-transfer guard, `transfer_keys` validates:

- **`from` must have sufficient balance**: The `from` address must hold at least `amount` keys for `creator`. If not, the function returns `ContractError::InsufficientBalance`.
- **`creator` must be registered**: The creator profile must exist in storage. If not, the function returns `ContractError::NotRegistered`.
- **`amount` must be positive**: `amount` must be `> 0`. If `amount == 0`, the function returns `ContractError::ZeroTransferAmount`.

---

## Storage Impact

The function modifies two storage entries:

| Key | Change |
|-----|--------|
| `KeyBalance(creator, from)` | Decremented by `amount` (removed if resulting balance is 0) |
| `KeyBalance(creator, to)` | Incremented by `amount` (created if absent) |

The following are **not** modified:

- `Creator(creator)` profile (supply, holder_count remain unchanged)
- `FeeConfig` / `KeyPrice` / `AdminAddress` / `TreasuryAddress`
- Creator or protocol fee balances

---

## Use Cases

| Scenario | Why sender-only auth is sufficient |
|----------|------------------------------------|
| **Wallet-to-wallet transfer** | Alice wants to send keys she owns to Bob's wallet. She signs the transaction. Bob does not need to be online or approve. |
| **Marketplace escrow** | A marketplace contract holds keys for users. The marketplace (as `from`) can transfer keys to a buyer without requiring the buyer to pre-approve — the buyer already committed via an off-chain order. |
| **Batch distribution** | An airdrop contract transfers keys to many recipients in one operation. Requiring each recipient to approve would make batch distribution impractical. |

---

## Event Emission

If `transfer_keys` emits an event, the recommended event shape should follow the existing topic conventions documented in [contract-event-conventions.md](./contract-event-conventions.md):

> Topics: `(Symbol("transfer"), creator, from)` — the creator is the primary entity, `from` is the sender.
> Data: `(to: Address, amount: u32)` — the recipient and quantity transferred.

This mirrors the `sell` event pattern (`(Symbol("sell"), creator, seller)` / `supply`) while placing the key transfer-specific fields (`to`, `amount`) in the data payload.

---

## Summary

| Aspect | Behavior |
|--------|----------|
| **Who authorizes?** | Only `from` (sender). No `to` (recipient) approval required. |
| **Auth mechanism** | `from.require_auth()` — Soroban signature verification. |
| **Fee?** | None. No bonding curve interaction. No fee math. |
| **Self-transfer?** | Rejected with `ContractError::SelfTransfer` when `from == to`. |
| **State changes** | `KeyBalance` decremented for `from`, incremented for `to`. Supply unchanged. |
| **Access level** | Key holder (`from`). |
