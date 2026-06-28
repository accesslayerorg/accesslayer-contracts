# Peer-to-Peer Transfer: Supply Cap and Dividend Claimable Balance Interaction

This document explains how `transfer_keys` interacts with two other features: the creator supply cap and the dividend claimable balance. Both interactions are non-obvious because they involve state that is tracked per holder but computed differently depending on which flow (buy, transfer, or distribute) initiated the change.

For the authorization model and fee behavior of `transfer_keys`, see [key-transfer-authorization.md](./key-transfer-authorization.md). For dividend accumulator mechanics, see [dividend-distribution-fee-behavior.md](./dividend-distribution-fee-behavior.md).

---

## Supply Cap Interaction with `transfer_keys`

**The supply cap does not apply to `transfer_keys`.** The cap is enforced only in `buy_key`.

### Why

The supply cap limits how many keys can be minted through the bonding curve. `transfer_keys` does not mint or burn keys — it moves existing keys between wallets. The total supply for the creator is invariant across a transfer:

```
before: sender_balance = N, recipient_balance = M, total_supply = N + M + rest
after:  sender_balance = N - amount, recipient_balance = M + amount, total_supply = N + M + rest
```

No supply cap check is performed in `transfer_keys` because no supply change occurs.

### Consequence

A creator whose supply has reached the cap can still transfer keys between wallets. Clients and front-ends must not block transfer attempts based on the supply cap being reached. Only new buy attempts are subject to the cap.

---

## Dividend Claimable Balance: Before and After Transfer

**A transfer after a distribution does not change either wallet's claimable amount for that distribution.**

### How it works

When `transfer_keys` executes, it calls `settle_holder_dividends` for both the sender and the recipient **before** updating their balances. Settlement computes each wallet's earned dividend based on their current balance and freezes it in the pending storage slot:

```
earned = current_balance * (accumulator - checkpoint)
new_pending = old_pending + earned
checkpoint = current_accumulator
```

After settlement, the balances change. But since settlement already captured the earned amount at the old balance, the transfer has no effect on either wallet's claimable dividend from distributions that occurred before the transfer.

### Consequence

If a dividend was distributed before a transfer:
- The **sender's** claimable for that distribution is locked in at their pre-transfer balance and is unaffected by the transfer.
- The **recipient's** claimable for that distribution is locked in at their pre-transfer balance and is unaffected by receiving transferred keys.

Calling `get_claimable_dividend` for either wallet immediately after a transfer returns the same value it would have returned immediately before the transfer.

---

## Future Distribution Behavior After a Transfer

**Distributions that occur after a transfer use the post-transfer balances.**

After the transfer completes, both wallets' checkpoints are updated to the current accumulator. The next time a dividend is distributed, the per-key accumulator increases and each holder earns proportionally to their new balance:

```
earned = post_transfer_balance * (new_accumulator - checkpoint_at_transfer)
```

This means:
- The **recipient** earns a larger share of future distributions because their balance increased.
- The **sender** earns a smaller share of future distributions because their balance decreased.
- The effect is proportional and immediate: it applies to the very next distribution after the transfer.

---

## Pending Claimable Dividend Is Not Transferred with Keys

**Transferred keys do not carry any pending claimable dividend from the sender to the recipient.**

When keys move from sender to recipient, only the `KeyBalance` storage entries change. The dividend state — `HolderDividendPending` and `HolderDividendCheckpoint` — belongs to each wallet independently and is not moved or split.

Specifically:
- The sender's pending claimable balance is retained by the sender in full.
- The recipient's pending claimable balance is unaffected by the transfer.
- Each holder must call `claim_dividend` from their own wallet to withdraw their earned amount.
- There is no mechanism to transfer claimable dividend ownership alongside key ownership.

This is intentional: dividend entitlements are personal to the wallet that held the keys at the time of each distribution.

---

## Summary

| Aspect | Behavior |
|--------|----------|
| **Supply cap check during transfer** | Not performed. Cap applies only to `buy_key`. |
| **Claimable balance for past distributions** | Unchanged by transfer. Settled at pre-transfer balance before any balance update. |
| **Future distributions after transfer** | Computed at post-transfer balance. Recipient share increases; sender share decreases. |
| **Pending claimable transferred with keys** | No. Each holder retains and claims their own earned dividends independently. |
