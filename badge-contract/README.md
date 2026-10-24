# Key NFT Badge Contract (`badge-contract`)

## Summary

The `badge-contract` is a dedicated Soroban smart contract for minting achievement NFT badges to wallets that reach key platform engagement milestones. Badges serve as permanent on-chain proof of user engagement across trading, governance, and holding activities.

## Scope & Functionality

### Supported Badge Types

| Badge Type | Variant | Discriminant | Description |
|---|---|---|---|
| `FIRST_BUYER` | `BadgeType::FirstBuyer` | `1` | Awarded on the wallet's first creator key purchase. |
| `WHALE_HOLDER` | `BadgeType::WhaleHolder` | `2` | Awarded upon crossing significant holding thresholds. |
| `GOVERNANCE_VOTER` | `BadgeType::GovernanceVoter` | `3` | Awarded for participation in governance snapshot voting. |
| `LONG_TERM_HOLDER` | `BadgeType::LongTermHolder` | `4` | Awarded for maintaining holdings across epoch duration thresholds. |
| `TOP_TRADER` | `BadgeType::TopTrader` | `5` | Awarded for high-volume trading and market activity. |

### Public Methods

- `init(admin: Address) -> Result<(), BadgeError>`:
  Initializes the badge contract with an administrator.
- `set_admin(new_admin: Address) -> Result<(), BadgeError>`:
  Transfers contract admin rights (requires current admin authorization).
- `set_authorised_contract(contract: Address) -> Result<(), BadgeError>`:
  Sets or updates the primary authorized contract (admin-only).
- `add_authorised_contract(contract: Address) -> Result<(), BadgeError>`:
  Adds an authorised contract address capable of minting badges (admin-only).
- `remove_authorised_contract(contract: Address) -> Result<(), BadgeError>`:
  Revokes minting authorization from a contract address (admin-only).
- `is_authorised(contract: Address) -> bool`:
  Returns whether `contract` is currently authorized to mint badges.
- `mint_badge(wallet: Address, badge_type: BadgeType) -> Result<(), BadgeError>`:
  Mints an achievement badge to `wallet`. Restricted to authorised contracts only. Enforces that each badge type can only be earned once per wallet. Emits `BadgeMinted`.
- `mint_badge_by(caller: Address, wallet: Address, badge_type: BadgeType) -> Result<(), BadgeError>`:
  Mints a badge with an explicit caller address check (`caller.require_auth()` + authorization check).
- `get_badges(wallet: Address) -> Vec<BadgeRecord>`:
  Returns the complete list of earned badges and their mint timestamps for `wallet`.
- `has_badge(wallet: Address, badge_type: BadgeType) -> bool`:
  Returns `true` if `wallet` has earned `badge_type`.
- `get_badge_timestamp(wallet: Address, badge_type: BadgeType) -> Option<u64>`:
  Returns the mint timestamp if earned, `None` otherwise.
- `get_wallet_badge_count(wallet: Address) -> u32`:
  Returns the total number of badges earned by `wallet`.
- `get_total_badges() -> u64`:
  Returns the global total count of badges minted across all wallets.

### Events

- **`BadgeMinted`**:
  - Topics: `(Symbol::new("BadgeMinted"), wallet: Address)`
  - Data: `BadgeMinted { wallet: Address, badge_type: BadgeType }`
  - Emitted whenever a badge is minted.

### Error Codes

| Error Code | Variant | Value | Description |
|---|---|---|---|
| `AlreadyInitialized` | `BadgeError::AlreadyInitialized` | `1` | Contract has already been initialized. |
| `NotInitialized` | `BadgeError::NotInitialized` | `2` | Contract has not been initialized. |
| `Unauthorized` | `BadgeError::Unauthorized` | `3` | Caller lacks authorization. |
| `BadgeAlreadyEarned` | `BadgeError::BadgeAlreadyEarned` | `4` | Wallet has already earned this badge type. |
| `ContractAlreadyAuthorized` | `BadgeError::ContractAlreadyAuthorized` | `5` | Contract is already in the authorized list. |
| `ContractNotAuthorized` | `BadgeError::ContractNotAuthorized` | `6` | Contract was not found in the authorized list. |
| `ArithmeticOverflow` | `BadgeError::ArithmeticOverflow` | `7` | Numeric overflow during counter increment. |
| `NoAuthorisedContract` | `BadgeError::NoAuthorisedContract` | `8` | No authorized contract has been configured. |

### Storage & TTL Management

Persistent entries (`DataKey::Badge` and `DataKey::WalletBadges`) and instance entries are extended on each interaction using threshold `100,000` ledgers and target `518,400` ledgers (~30 days).
