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
| `40` | `DisplayNameEmpty` | Creator display handle is blank (empty string or ASCII whitespace only) | Triggered in [`validate_creator_handle`](../creator-keys/src/lib.rs) before the length and character checks when the handle contains no non-whitespace bytes. |
| `97` | `UpgradeApprovalThresholdNotMet` | A timelocked upgrade was executed without enough distinct admin approvals | Triggered in [`apply_timelocked_upgrade`](../creator-keys/src/lib.rs) when the approval count is below `GLOBAL_PAUSE_THRESHOLD` (2). |
| `98` | `InvalidUpgradePayload` | A `TimelockChangeType::Upgrade` payload is not a 32-byte WASM hash | Triggered in [`decode_upgrade_payload`](../creator-keys/src/lib.rs) when the payload does not convert to `BytesN<32>`. |
| `99` | `ContractFrozen` | A timelocked upgrade was attempted while the protocol is frozen | Triggered in [`assert_upgrade_not_frozen`](../creator-keys/src/lib.rs) during a normal pause, or during a global pause with fewer than the required approvals. |
| `100` | `InvalidChangeType` | The supplied timelocked action is not of the change type the caller requires | Triggered in [`approve_upgrade`](../creator-keys/src/lib.rs) when the action is not a `TimelockChangeType::Upgrade`. |

**Backward compatibility for 97-100**: these four codes are appended after the
previously highest code (`96`, `TargetWeightsNotNormalized`). No existing variant
was renumbered, reordered, or reused, so previously deployed clients that decode
`ContractError` numerically keep working unchanged. Codes 47 and 48 remain
intentionally unassigned.

| Code | Variant | Description | Trigger Condition |
| :--- | :--- | :--- | :--- |
| `107` | `WalletAlertLimitReached` | The wallet already holds the maximum number of active price alerts | Triggered in [`price_alerts::register_alert`](../creator-keys/src/price_alerts.rs) when the wallet has `MAX_ACTIVE_ALERTS_PER_WALLET` (10) active alerts across all keys. Alerts that have fired or been cancelled do not count. |
| `108` | `KeyAlertLimitReached` | The key already has the maximum number of active price alerts | Triggered in [`price_alerts::register_alert`](../creator-keys/src/price_alerts.rs) when the key has `MAX_ACTIVE_ALERTS_PER_KEY` (16) active alerts from any wallets. |
| `109` | `AlertNotFound` | No active price alert with that id belongs to the wallet | Triggered in [`price_alerts::cancel_alert`](../creator-keys/src/price_alerts.rs) when the id never existed, belongs to another wallet, or the alert has already fired or been cancelled. |
| `110` | `AlreadyInitialized` | Protocol configuration has already been initialized | Triggered in [`initialize`](../creator-keys/src/lib.rs) when attempting to re-initialize an already-initialized contract. |
| `111` | `BundleSizeExceeded` | A bundle contains too many items | Triggered by `bundle::buy_bundle` when the number of items exceeds `MAX_BUNDLE_ITEMS`. |
| `112` | `BundleItemUnfilled` | A bundle item could not be filled completely | Triggered by `bundle::buy_bundle` when a requested item is partially filled. |
| `113` | `StateRestoring` | The creator's archived state is being restored | Triggered when buying keys for a creator in the restoring lifecycle state. |
| `114` | `InvalidLifecycleState` | The creator is in the wrong lifecycle state for the requested archive operation | Triggered by archive/restore lifecycle entrypoints when the creator state is incompatible. |
| `115` | `AnnouncementPageSizeExceeded` | The requested announcement page exceeds the supported maximum size | Triggered when announcement pagination requests more records than the supported maximum. |
| `116` | `AnnouncementRecordMissing` | An announcement index points to a record that is no longer available | Triggered when an announcement page references an absent record in stored pagination state. |
| `117` | `LockupActive` | The wallet's keys are still inside an investor lock-up window | Triggered by `swap`, `sell_key`, `batch_sell`, or `batch_sell_v2` before the stored `unlock_timestamp`; call `check_lockup` for remaining seconds. |
| `118` | `LockupNotFound` | No investor lock-up exists for the wallet and key | Triggered by `emergency_unlock` when no lock-up entry exists. |
**Backward compatibility for 107-118**: these twelve codes
are appended after the previously highest code (`106`, `CapTooLow`). Codes
`115` and `116` are reserved for announcement errors; investor lock-up errors
use `117` and `118`. No existing variant was renumbered,
reordered, or reused. `register_alert` reports an unregistered key with
`NotRegistered` (2) and a non-positive target price with `NotPositiveAmount`
(6).

| Code | Variant | Description | Trigger Condition |
| :--- | :--- | :--- | :--- |
| `119` | `TransferProposalNotFound` | No ownership-transfer proposal has that id | Triggered in [`key_ownership_transfer::read_proposal`](../creator-keys/src/key_ownership_transfer.rs) when the proposal never existed or was already executed or cancelled. |
| `120` | `TransferAlreadyPending` | The key already has a live ownership-transfer proposal | Triggered in [`key_ownership_transfer::propose_transfer`](../creator-keys/src/key_ownership_transfer.rs) when a second proposal is raised while one is still inside its 48h window or already accepted. An expired, never-accepted proposal is replaced instead of rejected. |
| `121` | `TransferWindowExpired` | The proposal was not accepted within 48h | Triggered in [`key_ownership_transfer::accept_transfer`](../creator-keys/src/key_ownership_transfer.rs) when `now >= expires_at` (`proposed_at + 172_800`). |
| `122` | `TransferNotAccepted` | The proposal was executed before the new owner accepted | Triggered in [`key_ownership_transfer::execute_transfer`](../creator-keys/src/key_ownership_transfer.rs) when `accepted_at` is `None`; this is also what an expired, unaccepted proposal reports. |
| `123` | `TransferAlreadyAccepted` | The proposed new owner accepted the proposal twice | Triggered in [`key_ownership_transfer::accept_transfer`](../creator-keys/src/key_ownership_transfer.rs) when `accepted_at` is already set. |

**Backward compatibility for 119-123**: these codes are appended after the
previously highest code (`118`, `LockupNotFound`). No existing variant
was renumbered, reordered, or reused. The ownership-handover flow reuses existing
codes where they already describe the failure: `NotRegistered` (2) for an
unregistered key, `SelfTransfer` (26) for a handover to the wallet that already
manages the key, `Unauthorized` (18) for a caller that is not the current owner,
`TimelockNotElapsed` (74) for execution before the 24h timelock, and `Overflow`
(3) on timestamp arithmetic.

| Code | Variant | Description | Trigger Condition |
| :--- | :--- | :--- | :--- |
| `124` | `NoDeprecationProposed` | No notice-period deprecation has been proposed for the supplied key | Triggered when attempting to execute or query deprecation for a key without a proposed notice period. |
| `125` | `DeprecationAlreadyProposed` | A notice-period deprecation has already been proposed for the supplied key | Triggered when proposing key deprecation on a key that already has an active deprecation notice. |
| `126` | `DeprecationNoticePeriodActive` | Key deprecation execution was attempted before the notice period elapsed | Triggered in `execute_deprecation` when the current timestamp is earlier than the end of the notice period. |
| `127` | `InvalidNoticePeriod` | The notice period in days is invalid (zero or above protocol ceiling) | Triggered when proposing deprecation with an out-of-range notice period. |
| `128` | `DeprecationAlreadyExecuted` | The key's notice-period deprecation has already been executed | Triggered when attempting an operation on a key whose notice-period deprecation has already finalised. |

**Backward compatibility for 124-128**: these codes are appended after the
previously highest code (`123`, `TransferAlreadyAccepted`). No existing variant
was renumbered, reordered, or reused.

| Code | Variant | Description | Trigger Condition |
| :--- | :--- | :--- | :--- |
| `129` | `SelfDelegation` | Attempted vote delegation to self | Triggered by `delegate_votes` when `holder == delegate_wallet`. |
| `130` | `AlreadyDelegated` | Key already has an active vote delegation | Triggered by `delegate_votes` when the holder has already delegated voting weight for this key; revoke first to re-delegate. |
| `131` | `NoDelegation` | No active vote delegation found to revoke | Triggered by `revoke_delegation` when the caller has no active delegation for this key. |
| `132` | `CannotReDelegate` | Re-delegation or chained delegation is rejected | Triggered by `delegate_votes` when a delegate tries to delegate their weight or a delegator tries to receive delegation (delegation chain limited to 1 level). |
| `133` | `NotAHolder` | Caller holds zero keys for this creator | Triggered by `delegate_votes` when the caller has no liquid keys for the specified creator key. |

**Backward compatibility for 129-133**: these five codes are appended after the
previously highest code (`128`, `DeprecationAlreadyExecuted`). No existing variant
was renumbered, reordered, or reused. Vote delegation (Issue #1116) reuses
`PollError::Unauthorized` (32) in governance voting when a delegator attempts
to vote directly.

| Code | Variant | Description | Trigger Condition |
| :--- | :--- | :--- | :--- |
| `138` | `ParityDeviationExceeded` | The two sides of an atomic swap deviate in value by more than the caller's tolerance | Triggered in [`atomic_swap`](../creator-keys/src/lib.rs) when the two sides' values differ by more than `max_parity_deviation_bps` basis points of the larger side; a tolerance of `0` requires an exact match. |

**Backward compatibility for 138**: this code is appended after the previously
highest code (`137`, `InvalidPollOption`). No existing variant was renumbered,
reordered, or reused. The atomic-swap guard reuses existing codes where they
already describe the failure: `SelfTransfer` (26) when both legs name the same
key, `ZeroTransferAmount` (27) for a zero-quantity leg, and `Overflow` (3) on
value arithmetic.

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
| `29` | `QuorumNotReached` | Proposal close attempted when total voting weight participation is below the creator's configured quorum threshold | Triggered in [`close_poll`](../creator-keys/src/events.rs) when `total_voting_weight * 10_000 < circulating_supply * quorum_bps`. |
| `30` | `QuorumTooHigh` | Quorum threshold basis points exceeds maximum limit (`> 5000` / 50%) | Triggered in [`set_quorum_bps`](../creator-keys/src/lib.rs) when `quorum_bps > 5000`. |
| `31` | `QuorumTooLow` | Quorum threshold basis points is below minimum limit (`< 100` / 1%) | Triggered in [`set_quorum_bps`](../creator-keys/src/lib.rs) when `quorum_bps < 100`. |
| `32` | `Unauthorized` | Caller lacks required creator authorization | Triggered in [`set_quorum_bps`](../creator-keys/src/lib.rs) when caller is not the registered creator. |
| `33` | `AlreadyClosed` | Poll or proposal has already been closed | Triggered in [`close_poll`](../creator-keys/src/events.rs) or [`cast_vote`](../creator-keys/src/events.rs) when `poll.closed == true`. |

---

## `ReputationError` Reference (Creator Reputation)

Defined in [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs) as `pub enum ReputationError`.

Returned by [`get_reputation`](../creator-keys/src/lib.rs) and
[`apply_governance_violation`](../creator-keys/src/lib.rs).

| Code | Name | Description | Trigger Condition |
|:---:|---|---|---|
| `1` | `Overflow` | Arithmetic overflow while accumulating a reputation delta | Triggered when a score update would exceed `i128` bounds. |
| `2` | `NotRegistered` | The creator address is not registered | Triggered when reading or updating reputation for an unknown creator. |
| `3` | `Unauthorized` | The caller is not the protocol admin | Triggered in `apply_governance_violation` when the caller is not the admin. |
| `4` | `NotPositiveAmount` | The supplied violation penalty is not positive | Triggered in `apply_governance_violation` when `penalty <= 0`. |

The zero floor never surfaces as an error: a negative delta that would push the
score below zero saturates at zero instead of reverting.

---

## `AllowanceError` Reference (Key Transfer Allowances)

Defined in [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs) as `pub enum AllowanceError`.

Returned by [`approve`](../creator-keys/src/lib.rs) and
[`transfer_from`](../creator-keys/src/lib.rs).

| Code | Name | Description | Trigger Condition |
|:---:|---|---|---|
| `1` | `Overflow` | Arithmetic overflow while decrementing a balance or allowance | Triggered on `u32` balance or allowance arithmetic overflow. |
| `2` | `ZeroAmount` | The requested transfer amount was zero | Triggered in `transfer_from` when `amount == 0`. |
| `3` | `SelfTransfer` | The spender attempted to transfer to itself | Triggered in `transfer_from` when `from == to`. |
| `4` | `InsufficientBalance` | The owner's live (non-frozen) key balance is smaller than the amount | Triggered in `transfer_from` when the spendable balance is below `amount`. |
| `5` | `InsufficientAllowance` | The spender's approved allowance is smaller than the amount | Triggered in `transfer_from` when the stored allowance is below `amount`. |
| `6` | `NotRegistered` | The creator address is not registered | Triggered when `key_id` has no creator profile. |
| `7` | `ProtocolPaused` | The contract is paused | Triggered in `transfer_from` when the pause flag is set. |
| `8` | `FrozenPosition` | The sender's keys are frozen and cannot be transferred | Triggered when the live balance is reduced below `amount` by frozen keys. |
| `9` | `HoldingCapExceeded` | The recipient would exceed the creator's per-wallet holding cap | Triggered when the post-transfer recipient balance exceeds the cap. |
| `10` | `ZeroAddress` | The recipient address was the zero address | Triggered in `transfer_from` when `to` is the zero address. |
| `11` | `CooldownActive` | The sender is still inside the creator's post-buy cooldown window | Triggered in `transfer_from` when the sender's last buy is within the creator's `cooldown_ledgers`, mirroring `transfer_keys`. |

`transfer_from` enforces the same invariants as the direct `transfer_keys`
path, including the cooldown window, so a spender cannot route keys around a
creator's protections.

---

## `SellTaxError` Reference (Sell Tax / Buyback)

Defined in [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs) as `pub enum SellTaxError`.

Returned by [`set_sell_tax_bps`](../creator-keys/src/lib.rs).

| Code | Name | Description | Trigger Condition |
|:---:|---|---|---|
| `1` | `Unauthorized` | The caller is not the creator owning the key | Triggered when `caller != creator`. |
| `2` | `NotRegistered` | The creator address is not registered | Triggered when the creator has no profile. |
| `3` | `TaxExceedsMax` | The requested tax exceeds the protocol ceiling | Triggered when `tax_bps > MAX_SELL_TAX_BPS`. |
| `4` | `Overflow` | Arithmetic overflow while accruing the tax into the buyback pool | Triggered on `i128` buyback balance overflow. |

---

## `EscalationError` Reference (Poll Quorum Escalation)

Defined in [`creator-keys/src/lib.rs`](../creator-keys/src/lib.rs) as `pub enum EscalationError`.

Returned by [`set_escalation_config`](../creator-keys/src/lib.rs),
[`evaluate_poll_escalation`](../creator-keys/src/lib.rs), and
[`get_escalation_status`](../creator-keys/src/lib.rs).

| Code | Name | Description | Trigger Condition |
|:---:|---|---|---|
| `1` | `Unauthorized` | The caller is not the protocol admin | Triggered in `set_escalation_config`. |
| `2` | `PollNotFound` | The poll does not exist for the creator | Triggered when the poll record is missing. |
| `3` | `AlreadyClosed` | The poll has already been closed | Triggered in `evaluate_poll_escalation` when `poll.closed`. |
| `4` | `Overflow` | Arithmetic overflow while extending a deadline | Triggered when `expires_at + extension_ledgers` overflows `u32`. |
| `5` | `TooEarlyToEscalate` | The proposal is not close enough to its deadline to be evaluated | Triggered when the ledger is more than `ESCALATION_EVALUATION_WINDOW_LEDGERS` before the deadline. |
| `6` | `BelowEscalationThreshold` | Participation is not within the escalation threshold of quorum | Triggered when participation is below `threshold_bps` of the quorum requirement, or when quorum was never configured, or when the proposal has already reached quorum. |
| `7` | `MaxExtensionsReached` | The proposal already consumed its maximum number of extensions | Triggered when `extensions_used >= max_extensions`. |
| `8` | `EscalationDisabled` | Quorum escalation is disabled | Triggered when no config is set, or when `max_extensions == 0`. |
| `9` | `InvalidEscalationConfig` | The supplied escalation configuration is invalid | Triggered when a bound in `EscalationConfig` is violated. |
| `10` | `NotRegistered` | The creator address is not registered | Triggered when the creator has no profile. |

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
- `DisplayNameEmpty` (40) is checked ahead of (12) and (14): a handle that is empty or entirely ASCII whitespace reports this rather than `HandleTooShort` or `InvalidHandleCharacter`.

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
