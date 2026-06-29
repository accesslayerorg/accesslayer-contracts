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

---

## Summary table

| Function | Access level | Mutates state |
|---|---|---|
| `register_creator` | Creator | Yes |
| `buyback` | Creator | Yes |
| `buy_key` | Key holder (buyer) | Yes |
| `sell_key` | Key holder (seller) | Yes |
| `transfer_keys` | Key holder (sender / `from`) | Yes |
| `set_fee_config` | Admin | Yes |
| `set_key_price` | Admin | Yes |
| `set_treasury_address` | Admin | Yes |
| `set_protocol_admin` | Admin | Yes |
| All `get_*` / `is_*` methods | Open | No |
| `compute_fees_for_payment` | Open | No |
