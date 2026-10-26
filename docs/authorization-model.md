# Contract function authorization model

This document describes the caller restrictions for every public entrypoint in the `creator-keys` contract. Functions are grouped by access level to help contributors verify that access control is correct and complete.

For return value semantics of read-only methods, see [read-only-methods.md](./read-only-methods.md). For error codes, see [error-codes.md](./error-codes.md).

---

## Access levels

| Level | Description |
|---|---|
| **Admin** | Requires auth from the protocol admin address (`admin.require_auth()`). The admin address is stored in contract storage and can be changed via `set_protocol_admin`. |
| **Creator** | Requires auth from the creator address being registered (`creator.require_auth()`). |
| **Key holder** | Requires auth from the buyer or seller address (`buyer.require_auth()` / `seller.require_auth()`). |
| **Open** | No auth required. Read-only view methods that anyone can call. |

---

## Admin-only functions

These functions modify protocol-wide configuration and require authorization from the stored admin address.

### `set_fee_config(admin: Address, creator_bps: u32, protocol_bps: u32) -> Result<(), ContractError>`

Sets the global fee split between creators and the protocol.

- **Auth**: `admin.require_auth()`
- **Constraints**: `creator_bps + protocol_bps` must equal `10000`. `protocol_bps` must not exceed `5000` (`PROTOCOL_BPS_MAX`).
- **No-op**: Returns `Ok(())` without writing if the new config matches the stored config.

### `set_volume_brackets(admin: Address, brackets: Vec<VolumeBracket>) -> Result<(), ContractError>`

Replaces the per-key rolling-volume swap fee brackets without clearing recorded volume.

- **Auth**: `admin.require_auth()` and the stored protocol admin check.
- **Constraints**: First threshold must be zero; subsequent minimum volumes must be strictly increasing; fee values must be at most 10,000 bps; one to five brackets are accepted.
- **Behavior**: Applies immediately to fee queries and the next trade. A newly detected event is emitted by the next successful trade for that key.

---

### `set_key_price(admin: Address, price: i128) -> Result<(), ContractError>`

Sets the global key price used for all buy and sell operations.

- **Auth**: `admin.require_auth()`
- **Constraints**: `price` must be `> 0`.
- **No-op**: Returns `Ok(())` without writing if the new price matches the stored price.

---

### `set_treasury_address(admin: Address, treasury: Address)`

Sets the protocol treasury address used for fee routing.

- **Auth**: `admin.require_auth()`
- **No-op**: Returns without writing if the new address matches the stored address.

---

### `set_protocol_admin(admin: Address, new_admin: Address)`

Transfers the protocol admin role to a new address.

- **Auth**: `admin.require_auth()`
- **No-op**: Returns without writing if the new admin matches the stored admin.

### `set_milestones(admin: Address, key_id: Address, milestones: Vec<MilestoneTier>) -> Result<(), ContractError>`

Configures the graduated supply milestone tiers for a key (intended at key creation).

- **Auth**: `admin.require_auth()` — caller must be the stored protocol admin.
- **Constraints**: `key_id` must be a registered key; `milestones` must contain 1-5 tiers in strictly ascending supply order; each `fee_bps` must not exceed 10,000.
- **Behavior**: Stores the milestone config with no tier applied yet. Tiers are applied in ascending supply order as `check_and_apply_milestone` is invoked post-swap.

### `check_and_apply_milestone(key_id: Address) -> Result<(u32, u32), ContractError>`

Re-evaluates the active milestone tier for a key after a supply change (intended to be called post-swap).

- **Auth**: Open (no authorization required).
- **Behavior**: Applies the highest newly reached tier whose fee lowers the fee currently in effect. The fee only ever moves downward; a supply drop below a milestone never regresses to a higher fee.
- **Returns**: `(tier, fee_bps)` — the 1-based count of tiers reached and the fee currently in effect.
- **Emits**: `MilestoneReached` when a newly reached tier lowers the fee.

### `get_current_milestone(key_id: Address) -> Result<(u32, u32), ContractError>`

Read-only view returning the active milestone tier and fee for a key.

- **Auth**: Open (no authorization required).
- **Returns**: `(tier, fee_bps)` — tier is `0` when no milestone has been reached.

---

## Creator-only functions

### `register_creator(creator: Address, handle: String) -> Result<(), ContractError>`

Registers a new creator profile on-chain.

- **Auth**: `creator.require_auth()`
- **Constraints**: Handle must be 3-32 characters, lowercase alphanumeric or underscores.
- **Fails**: `AlreadyRegistered` if a profile already exists for `creator`.

### `buyback(creator: Address, caller: Address, amount: u32, payment: i128, max_total_cost: Option<i128>) -> Result<u32, ContractError>`

Creator-authorized buyback that burns keys from the creator's own held balance.

- **Auth**: `caller.require_auth()`
- **Constraints**:
  - `caller` must equal `creator`, otherwise `Unauthorized`
  - `amount` must be `> 0`
  - `payment` must be `> 0` and `>= get_buyback_quote(creator, amount)`
  - `amount` must not exceed the creator's total supply, otherwise `InsufficientSupply`
  - the creator wallet must already hold at least `amount` keys, otherwise `InsufficientBalance`
- **Returns**: The new total supply for the creator after the burn

### `set_reward_rate(key_id: Address, caller: Address, new_rate_bps: u32) -> Result<(), LpRewardError>`

Sets the per-pool reward rate (in basis points) for `key_id`'s LP pool.

- **Auth**: `caller.require_auth()`
- **Constraints**:
  - `key_id` must be a registered creator (`read_registered_creator_profile`), otherwise `LpRewardError::Unauthorized`
  - `new_rate_bps` must be within `MIN_REWARD_RATE_BPS..=MAX_REWARD_RATE_BPS` (`1..=5000`), otherwise `LpRewardError::RateOutOfBounds`
- **Semantics**: Prospective only; existing positions' `reward_debt` snapshots remain untouched.
- **Emits**: `RewardRateUpdated` (`lp_rate`).

---

## Key admin functions

These functions are authorized by an admin of the key itself: a member of the
key's multi-sig admin set (configured with `set_multisig_admins`) or the
protocol admin.

### `freeze(key_id: Address, reason: String, admin: Address) -> Result<(), ContractError>`

Freezes the key: buys, sells and transfers of that key are rejected with
`KeyFrozen` until the key is released. The action is appended to the audit log
returned by `get_freeze_log`.

- **Auth**: `admin.require_auth()`
- **Constraints**:
  - `admin` must be a member of the key's multi-sig admin set, or the protocol
    admin, otherwise `Unauthorized`
  - `key_id` must be a registered creator, otherwise `NotRegistered`
  - the key must not already be frozen, otherwise `KeyAlreadyFrozen`
- **Emits**: `KeyFrozen` (`key_frz`).

### `unfreeze(key_id: Address, admin: Address) -> Result<(), ContractError>`

Releases a frozen key, restoring buy, sell and transfer.

- **Auth**: `admin.require_auth()`
- **Constraints**: same authority check as `freeze`; the key must be frozen,
  otherwise `KeyNotFrozen`
- **Emits**: `KeyUnfrozen` (`key_unfrz`).

`freeze_key(admin, key_id, reason)` and `unfreeze_key(admin, key_id)` are
aliases of the two entrypoints above with the admin argument first.

### Read-only views

| Method | Returns |
|---|---|
| `get_freeze_log(key_id: Address)` | `Vec<FreezeLogEntry>` |
| `get_key_status(key_id: Address)` | `KeyStatus` |
| `is_key_frozen(key_id: Address)` | `bool` |

---

## Key holder functions

These functions require authorization from the buyer or seller, not the creator whose keys are being traded.

### `buy_key(creator: Address, buyer: Address, payment: i128) -> Result<u32, ContractError>`

Buys one key for `creator` on behalf of `buyer`.

- **Auth**: `buyer.require_auth()`
- **Constraints**: `payment` must be `> 0` and `>=` the stored key price. The creator must be registered.
- **Returns**: The new total supply for the creator after the purchase.

---

### `sell_key(creator: Address, seller: Address) -> Result<u32, ContractError>`

Sells one key held by `seller` for `creator`.

- **Auth**: `seller.require_auth()`
- **Fails**: `InsufficientBalance` if the seller holds zero keys for the creator.
- **Returns**: The new total supply for the creator after the sale.

---

### `transfer_keys(creator: Address, from: Address, to: Address, amount: u32) -> Result<(), ContractError>`

Transfers `amount` keys from `from` to `to` for `creator`. Does not interact with the bonding curve and charges no fee.

- **Auth**: `from.require_auth()` — only the sender authorizes. No recipient approval required.
- **Rejects**: Self-transfers (`from == to`) with `ContractError::SelfTransfer`, and zero-amount transfers with `ContractError::ZeroTransferAmount`.
- **Fails**: `InsufficientBalance` if `from` holds fewer than `amount` keys.
- **Fails**: `NotRegistered` if `creator` is not registered.
- **State changes**: `KeyBalance(creator, from)` decremented, `KeyBalance(creator, to)` incremented. Creator supply and holder_count are unchanged.
- See [key-transfer-authorization.md](./key-transfer-authorization.md) for full details.

---

### `delegate_votes(holder: Address, key_id: Address, delegate_wallet: Address) -> Result<(), ContractError>`

Assigns the governance voting weight of `key_id` held by `holder` to `delegate_wallet` (proxy voting rights).

- **Auth**: `holder.require_auth()` — only the key holder may delegate their voting weight.
- **Constraints**:
  - `holder` must currently hold at least 1 key of `key_id` (`ContractError::NotAHolder`).
  - `holder` cannot delegate to itself (`ContractError::SelfDelegation`).
  - `key_id` must not already be delegated by `holder`; re-delegations require revoking first (`ContractError::AlreadyDelegated`).
  - Re-delegation is rejected bidirectionally (`ContractError::CannotReDelegate`): a delegate currently receiving voting weight cannot delegate, and a delegating holder cannot receive delegations.
- **State changes**: Stores `VoteDelegationKey::Delegate(key_id, holder) -> delegate_wallet` and appends `holder` to `VoteDelegationKey::Delegators(key_id, delegate_wallet)`.
- **Emits**: `VotingDelegated` (`vote_del`).

---

### `revoke_delegation(holder: Address, key_id: Address) -> Result<(), ContractError>`

Revokes an existing proxy voting delegation for `key_id`, returning voting weight to `holder`.

- **Auth**: `holder.require_auth()` — only the delegating holder may revoke their delegation.
- **Constraints**:
  - An active delegation must exist for `(key_id, holder)` (`ContractError::NoDelegation`).
- **State changes**: Removes `VoteDelegationKey::Delegate(key_id, holder)` and removes `holder` from `VoteDelegationKey::Delegators(key_id, previous_delegate)`.
- **Emits**: `DelegationRevoked` (`v_del_rev`).

---

## Open (read-only) functions

These functions require no authorization. Anyone can call them. They do not mutate contract state.

### Quote methods

| Method | Returns |
|---|---|
| `get_buy_quote(creator: Address)` | `Result<QuoteResponse, ContractError>` |
| `get_buyback_quote(creator: Address, amount: u32)` | `Result<i128, ContractError>` |
| `get_sell_quote(creator: Address, holder: Address)` | `Result<QuoteResponse, ContractError>` |
| `compute_fees_for_payment(total: i128)` | `Result<(i128, i128), ContractError>` |

### Creator profile methods

| Method | Returns |
|---|---|
| `get_creator(creator: Address)` | `Result<CreatorProfile, ContractError>` |
| `get_creator_details(creator: Address)` | `CreatorDetailsView` |
| `is_creator_registered(creator: Address)` | `bool` |
| `get_creator_fee_recipient(creator: Address)` | `Result<Address, ContractError>` |

### Supply and balance methods

| Method | Returns |
|---|---|
| `get_key_balance(creator: Address, wallet: Address)` | `u32` |
| `get_holder_key_count(creator: Address, holder: Address)` | `HolderKeyCountView` |
| `get_total_key_supply(creator: Address)` | `u32` |
| `get_creator_supply(creator: Address)` | `Result<u32, ContractError>` |
| `get_creator_holder_count(creator: Address)` | `u32` |

### Key metadata methods

| Method | Returns |
|---|---|
| `get_key_name(creator: Address)` | `Result<String, ContractError>` |
| `get_key_symbol(creator: Address)` | `Result<String, ContractError>` |
| `get_key_decimals()` | `u32` |

### Fee configuration methods

| Method | Returns |
|---|---|
| `get_fee_config()` | `Option<FeeConfig>` |
| `get_creator_fee_config(creator: Address)` | `CreatorFeeView` |
| `get_creator_fee_bps(creator: Address)` | `Result<u32, ContractError>` |
| `get_creator_treasury_share(creator: Address)` | `Result<u32, ContractError>` |
| `get_protocol_treasury_share_bps()` | `Result<u32, ContractError>` |
| `get_protocol_fee_view()` | `ProtocolFeeView` |
| `is_protocol_config_initialized()` | `bool` |

### Protocol-level methods

| Method | Returns |
|---|---|
| `get_protocol_state_version()` | `u32` |
| `get_treasury_address()` | `Option<Address>` |
| `get_protocol_admin()` | `Option<Address>` |
| `get_protocol_fee_recipient()` | `Option<Address>` |

### Governance and vote delegation methods

| Method | Returns |
|---|---|
| `get_voting_weight(wallet: Address, key_id: Address)` | `u32` |
| `get_vote_delegate(holder: Address, key_id: Address)` | `Option<Address>` |
| `get_vote_delegators(delegate: Address, key_id: Address)` | `Vec<Address>` |

---

## Summary table

| Function | Access level | Mutates state |
|---|---|---|
| `register_creator` | Creator | Yes |
| `buyback` | Creator | Yes |
| `set_reward_rate` | Creator | Yes |
| `freeze` | Key multi-sig admin / protocol admin | Yes |
| `unfreeze` | Key multi-sig admin / protocol admin | Yes |
| `buy_key` | Key holder (buyer) | Yes |
| `sell_key` | Key holder (seller) | Yes |
| `transfer_keys` | Key holder (sender / `from`) | Yes |
| `delegate_votes` | Key holder (`holder`) | Yes |
| `revoke_delegation` | Key holder (`holder`) | Yes |
| `set_fee_config` | Admin | Yes |
| `set_volume_brackets` | Admin | Yes |
| `set_key_price` | Admin | Yes |
| `set_treasury_address` | Admin | Yes |
| `set_protocol_admin` | Admin | Yes |
| `set_milestones` | Admin | Yes |
| `check_and_apply_milestone` | Open | Yes |
| `get_current_milestone` | Open | No |
| `get_voting_weight` | Open | No |
| `get_vote_delegate` | Open | No |
| `get_vote_delegators` | Open | No |
| `get_freeze_log` | Open | No |
| `get_key_status` | Open | No |
| `is_key_frozen` | Open | No |
| All `get_*` / `is_*` methods | Open | No |
| `compute_fees_for_payment` | Open | No |

