# Contract Error Codes

This document maps all smart contract error codes defined in the `creator-keys` contract to their numeric discriminants, variant names, human-readable descriptions, and exact source code trigger conditions.

Error codes in Soroban contracts are defined using the `#[contracterror]` attribute on `u32` enums. The numeric values are part of the contract's fixed ABI and are returned directly to callers during transaction simulation and execution.

---

## `ContractError` Reference

Defined in [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs#L50-L83) as `pub enum ContractError`.

| Code | Name | Description | Trigger Condition |
|:---:|---|---|---|
| `1` | `AlreadyRegistered` | Creator address is already registered in contract storage | Triggered in [`register_creator`](../creator-keys/src/lib.rs#L1327) when profile already exists for `creator`. |
| `2` | `NotRegistered` | Creator profile does not exist for the specified address | Triggered in [`read_registered_creator_profile`](../creator-keys/src/lib.rs#L705) when looking up an unregistered creator address across trade, quote, dividend, and management entrypoints. |
| `3` | `Overflow` | Integer arithmetic would exceed storage or type bounds (`u32::MAX` or `i128::MAX`) | Triggered in [`checked_accumulate`](../creator-keys/src/lib.rs#L280), [`increment_creator_supply`](../creator-keys/src/lib.rs#L753), [`increment_key_balance`](../creator-keys/src/lib.rs#L790), [`calculate_buy_quote_fees`](../creator-keys/src/lib.rs#L916), or [`compute_buy_price_for_amount`](../creator-keys/src/lib.rs#L1046) on integer overflow. |
| `4` | `InsufficientPayment` | Payment supplied is less than the required price plus fees | Triggered in [`buy_key`](../creator-keys/src/lib.rs#L1473), [`buy_keys`](../creator-keys/src/lib.rs#L1645), or [`buy_keys_for`](../creator-keys/src/lib.rs#L1780) when provided payment `< total_amount`. |
| `5` | `KeyPriceNotSet` | Pricing or trading attempted before setting a key price | Triggered in [`read_key_price`](../creator-keys/src/lib.rs#L1020) when key price storage is empty for `creator`. |
| `6` | `NotPositiveAmount` | Amount or payment argument is zero or negative | Triggered in [`set_key_price`](../creator-keys/src/lib.rs#L1038), [`register_creator`](../creator-keys/src/lib.rs#L1339), [`buy_key`](../creator-keys/src/lib.rs#L1457), [`airdrop_keys`](../creator-keys/src/lib.rs#L1733), or [`buyback`](../creator-keys/src/lib.rs#L2453) when `amount <= 0` or `payment <= 0`. |
| `7` | `FeeConfigNotSet` | Trade or quote attempted before initializing global protocol fees | Triggered in [`read_protocol_fee_config`](../creator-keys/src/lib.rs#L900) when protocol fee configuration has not been set by an admin. |
| `8` | `InvalidFeeConfig` | Fee basis points sum is invalid (`creator_bps + protocol_bps != 10000`) | Triggered in [`assert_valid_fee_bps`](../creator-keys/src/lib.rs#L126-L129) when basis points do not sum to `10_000` (100%). |
| `9` | `InsufficientBalance` | Seller, sender, or buyback address does not hold enough keys | Triggered in [`sell_key`](../creator-keys/src/lib.rs#L1551), [`sell_keys`](../creator-keys/src/lib.rs#L1654), [`buyback`](../creator-keys/src/lib.rs#L2485), or [`transfer_keys`](../creator-keys/src/lib.rs#L2860) when `balance < requested_amount`. |
| `10` | `SellUnderflow` | Net sell payout subtraction resulted in a negative proceeds value | Triggered in [`calculate_sell_quote_fees`](../creator-keys/src/lib.rs#L971) or [`calculate_quote_response`](../creator-keys/src/lib.rs#L1134) when total fees exceed gross key price. |
| `11` | `ProtocolFeeExceedsCap` | Protocol fee share exceeds the maximum cap (`protocol_bps > 5000`) | Triggered in [`assert_valid_fee_bps`](../creator-keys/src/lib.rs#L132) when `protocol_bps > 5000` (50%). |
| `12` | `HandleTooShort` | Creator handle string length is below minimum bound (`< 3` chars) | Triggered in [`validate_handle`](../creator-keys/src/lib.rs#L832) when `handle.len() < 3`. |
| `13` | `HandleTooLong` | Creator handle string length exceeds maximum bound (`> 32` chars) | Triggered in [`validate_handle`](../creator-keys/src/lib.rs#L835) when `handle.len() > 32`. |
| `14` | `InvalidHandleCharacter` | Handle contains invalid characters (allowed: `a-z`, `0-9`, `_`) | Triggered in [`validate_handle`](../creator-keys/src/lib.rs#L844) when handle contains disallowed characters. |
| `15` | `ZeroAddress` | Target address is the Stellar zero address | Triggered in [`require_non_zero_address`](../creator-keys/src/lib.rs#L894) when configuring target addresses. |
| `16` | `SlippageExceeded` | Execution cost or proceeds violated caller-specified min/max bounds | Triggered in [`buy_key`](../creator-keys/src/lib.rs#L949), [`sell_key`](../creator-keys/src/lib.rs#L961), or [`buy_keys`](../creator-keys/src/lib.rs#L982) when execution price violates slippage parameters. |
| `17` | `ProtocolPaused` | State-changing transaction attempted while contract is paused | Triggered in [`require_not_paused`](../creator-keys/src/lib.rs#L859) when emergency pause mode is enabled. |
| `18` | `Unauthorized` | Caller lacks required authorization (admin, creator, or caller match) | Triggered in [`require_admin`](../creator-keys/src/lib.rs#L869), [`require_creator`](../creator-keys/src/lib.rs#L871), [`buy_keys_for`](../creator-keys/src/lib.rs#L1621), or [`airdrop_keys`](../creator-keys/src/lib.rs#L1727). |
| `19` | `NoDividendClaimable` | Holder has no accumulated dividend balance to claim | Triggered in [`claim_dividend`](../creator-keys/src/lib.rs#L2567) when claimable dividend is `0`. |
| `20` | `ZeroDistributionAmount` | Dividend distribution attempted with an amount of `0` | Triggered in [`distribute_dividend`](../creator-keys/src/lib.rs#L2517) when `amount == 0`. |
| `21` | `NoKeyHolders` | Dividend distribution attempted for creator with zero key holders | Triggered in [`distribute_dividend`](../creator-keys/src/lib.rs#L2523) when holder count or total supply is `0`. |
| `22` | `AllocationLocked` | Creator locked allocation claimed before lockup ledger sequence | Triggered in [`register_creator`](../creator-keys/src/lib.rs#L1336) or [`claim_locked_allocation`](../creator-keys/src/lib.rs#L2668) when current ledger `< lockup_ledger`. |
| `23` | `AlreadyClaimed` | Creator locked allocation was already claimed previously | Triggered in [`claim_locked_allocation`](../creator-keys/src/lib.rs#L2663) when locked allocation flag is true. |
| `24` | `SupplyCapExceeded` | Action would cause total key supply to exceed the creator supply cap | Triggered in [`register_creator`](../creator-keys/src/lib.rs#L1369), [`buy_key`](../creator-keys/src/lib.rs#L1483), or [`airdrop_keys`](../creator-keys/src/lib.rs#L1759) when `supply + amount > supply_cap`. |
| `25` | `InsufficientSupply` | Buyback quantity requested exceeds current circulating total supply | Triggered in [`get_buyback_quote`](../creator-keys/src/lib.rs#L2455) or [`buyback`](../creator-keys/src/lib.rs#L1648) when `amount > total_supply`. |
| `26` | `SelfTransfer` | Attempted key transfer to sender's own address (`from == to`) | Triggered in [`transfer_keys`](../creator-keys/src/lib.rs#L2846) when `from == to`. |
| `27` | `ZeroTransferAmount` | Attempted key transfer with an amount of `0` | Triggered in [`transfer_keys`](../creator-keys/src/lib.rs#L2843) when `amount == 0`. |
| `28` | `InsufficientTreasuryBalance` | Requested withdrawal exceeds protocol or creator treasury balance | Triggered in [`withdraw_protocol_treasury`](../creator-keys/src/lib.rs#L2198) or [`withdraw_creator_treasury`](../creator-keys/src/lib.rs#L2223) when `withdrawal > treasury_balance`. |
| `29` | `BatchClaimExceedsLimit` | Batch dividend claim request exceeds max batch size limit | Triggered in [`batch_claim_dividends`](../creator-keys/src/lib.rs#L2601) when `creators.len() > MAX_BATCH_CLAIM_LIMIT`. |
| `30` | `InvalidCoCreatorShare` | Co-creator revenue split share bps exceeds bounds (`> 10000`) | Triggered in [`validate_co_creator_config`](../creator-keys/src/lib.rs#L768) when `share_bps > 10000`. |
| `31` | `WhitelistOnly` | Buyer address is not in creator whitelist during whitelist window | Triggered in [`check_whitelist`](../creator-keys/src/lib.rs#L683) when whitelist is active and buyer is not allowed. |
| `32` | `WhitelistTooLarge` | Whitelist configuration address count exceeds maximum limit | Triggered in [`validate_whitelist_config`](../creator-keys/src/lib.rs#L637) when address count `> MAX_WHITELIST_SIZE`. |
| `33` | `AirdropRecipientLimitExceeded` | Airdrop recipient list length exceeds max limit per transaction | Triggered in [`airdrop_keys`](../creator-keys/src/lib.rs#L1730) when `recipients.len() > MAX_AIRDROP_RECIPIENT_LIMIT`. |

---

## `PollError` Reference (Governance Polls)

Defined in [`creator-keys/src/events.rs`](../creator-keys/src/events.rs#L366-L376) as `pub enum PollError`.

| Code | Name | Description | Trigger Condition |
|:---:|---|---|---|
| `20` | `NotRegistered` | Poll creation attempted for an unregistered creator address | Triggered in [`create_poll`](../creator-keys/src/events.rs#L464) when creator profile lookup fails. |
| `21` | `Overflow` | Poll counter or vote accumulation integer overflowed | Triggered in [`create_poll`](../creator-keys/src/events.rs#L480) or [`vote_poll`](../creator-keys/src/events.rs#L547) on arithmetic overflow. |
| `22` | `InvalidOptionCount` | Poll options list length is invalid (`< 2` or `> MAX_OPTIONS`) | Triggered in [`validate_poll_options`](../creator-keys/src/events.rs#L435) when options count is out of range. |
| `23` | `QuestionTooLong` | Poll question string exceeds maximum character/byte length | Triggered in [`create_poll`](../creator-keys/src/events.rs#L467) when question string length is too long. |
| `24` | `OptionTooLong` | Poll option text string exceeds maximum character/byte length | Triggered in [`validate_poll_options`](../creator-keys/src/events.rs#L442) when any option text string is too long. |
| `25` | `PollNotFound` | Requested poll ID does not exist for the specified creator | Triggered in [`read_poll`](../creator-keys/src/events.rs#L425) when poll record is missing. |
| `26` | `PollExpired` | Voting attempted on a poll after its expiration timestamp | Triggered in [`vote_poll`](../creator-keys/src/events.rs#L523) when `current_ledger_time > expires_at`. |
| `27` | `NotAHolder` | Voter does not hold any keys for the poll creator (`balance == 0`) | Triggered in [`vote_poll`](../creator-keys/src/events.rs#L532) when voter key balance is zero. |
| `28` | `InvalidOption` | Selected option index is out of bounds for the target poll | Triggered in [`vote_poll`](../creator-keys/src/events.rs#L526) when `option_index >= options.len()`. |

---

## Read-Only Quote Error Identifiers

Read-only quote functions defined in [`creator-keys/src/quote_view_errors.rs`](../creator-keys/src/quote_view_errors.rs) return string error identifiers for off-chain calculation paths:

| String Constant | Value | Context |
|---|---|---|
| `ERR_NOT_REGISTERED` | `"not_registered"` | Creator address is not registered |
| `ERR_FEE_CONFIG_NOT_SET` | `"fee_config_not_set"` | Protocol fee configuration has not been set |
| `ERR_OVERFLOW` | `"overflow"` | Quote math overflowed `i128` bounds |
| `ERR_SELL_UNDERFLOW` | `"sell_underflow"` | Sell quote fee subtraction underflowed proceeds |
| `ERR_ZERO_CLAIMABLE` | `"zero_claimable"` | Dividend claimable quote amount is zero |
| `ERR_NO_HOLDERS` | `"no_holders"` | Creator has no key holders |
| `ERR_DIVIDEND_AMOUNT_ZERO` | `"dividend_amount_zero"` | Dividend distribution quote amount is zero |

---

## Soroban Simulation Error Example

When invoking a contract method via standard Soroban RPC (`simulateTransaction`), contract reverts with a `ContractError` return a JSON response containing `Error(Contract, #<code>)`.

Below is a representative example of a `simulateTransaction` RPC response when `buy_key` reverts due to `SlippageExceeded` (`ContractError` Code 16):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "error": "HostError: Error(Contract, #16)",
    "transactionData": "AAAAAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA...",
    "events": [],
    "minResourceFee": "1000",
    "results": [
      {
        "auth": [],
        "xdr": "AAAAAAGAAAAD"
      }
    ]
  }
}
```

### Parsing Error Codes in Client Applications

In JavaScript/TypeScript using `@stellar/stellar-sdk` or Soroban Client libraries, contract error codes are decoded from simulation or submission results:

```typescript
try {
  const simResult = await server.simulateTransaction(tx);
  if (simResult.error) {
    // Extract numeric code from "Error(Contract, #16)" pattern
    const match = simResult.error.match(/Error\(Contract, #(\d+)\)/);
    if (match) {
      const errorCode = parseInt(match[1], 10);
      console.log(`Contract reverted with error code: ${errorCode}`);
      // Mapping to ContractError enum: 16 -> SlippageExceeded
    }
  }
} catch (err) {
  // Handle network or RPC errors
}
```

---

## Integration Notes & Best Practices

### Authorization and Registration
- `AlreadyRegistered` (Code 1) guards against re-registering an existing creator. Off-chain apps should call `is_registered(creator)` or `get_creator(creator)` prior to registration.
- `NotRegistered` (Code 2) applies to trades, quotes, and management. Callers must register creators prior to key trading.
- `HandleTooShort` (12), `HandleTooLong` (13), and `InvalidHandleCharacter` (14) are deterministic handle validation checks. Validate handles client-side (`/^[a-z0-9_]{3,32}$/`) before submission.

### Fees and Pricing
- `FeeConfigNotSet` (7) and `KeyPriceNotSet` (5) are initialization gates. Detect these and inform users that pricing/fees are not yet configured.
- `InvalidFeeConfig` (8) requires `creator_bps + protocol_bps == 10000`.
- `ProtocolFeeExceedsCap` (11) enforces `protocol_bps <= 5000` (50% max protocol share).

### Trading & Slippage
- `InsufficientPayment` (4) applies to buys when payment is less than total price + fees.
- `InsufficientBalance` (9) applies to sells, transfers, and buybacks when caller balance is insufficient.
- `SlippageExceeded` (16) indicates market movement between quote generation and execution. Refresh quotes and retry with wider slippage bounds.

### Governance Polls
- Poll errors use numeric codes 20–28 under `PollError`.
- `PollExpired` (26) occurs when voting on a poll past its ledger expiration.
- `NotAHolder` (27) requires non-zero creator key ownership to vote.

---

## Version Stability & ABI Rules

1. **Numeric Stability**: Error codes are fixed contract ABI discriminants.
2. **Safe Extension**: New variants MUST be appended at the end of the `ContractError` enum with the next unused numeric value.
3. **No Reordering**: Never insert error variants mid-enum, reorder existing variants, or re-assign retired numbers.
