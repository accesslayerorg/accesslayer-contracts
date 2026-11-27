# Supported Token Behaviors & Requirements

Access Layer contracts interact with external SEP-41 tokens across payment (`buy_with`, `sell_with`, `redeem_payment_asset`), escrow (`deposit_performance_bond`, `propose_swap`, `accept_swap`, `fund_stream`), claim (`claim_dividend`, `withdraw`, `claim_airdrop`), and distribution (`declare_dividend`, `create_airdrop`) flows.

To protect reserve accounting and user funds, only standard SEP-41 tokens that satisfy the requirements below are supported.

## Required Token Behaviors

1. **Exact transfer amounts (no fee-on-transfer)**
   - A `transfer(from, to, amount)` call must increase `balance(to)` by **exactly** `amount` and decrease `balance(from)` by **exactly** `amount`.
   - Every inbound transfer measures `balance(recipient)` before and after `transfer(...)`. If `balance_after - balance_before != expected_amount`, the contract reverts with `Error::UnsupportedToken`.
   - Fee-on-transfer tokens are rejected **before** any reserve, escrow, or supply accounting is updated.

2. **No silent no-op transfers**
   - `transfer` must actually move balances. Tokens that return `()` without updating `balance(to)` fail the post-transfer balance delta check and revert with `Error::UnsupportedToken`.

3. **No rebasing or external balance mutation**
   - Token balances held in contract custody must not change between transactions without an explicit contract transfer (no elastic supply / rebasing).
   - If a token rebases mid-transfer, the balance delta check rejects it with `Error::UnsupportedToken`.

4. **Deterministic non-panicking transfers for valid balances**
   - If a token panics during `transfer`, the Soroban host aborts and rolls back the entire invocation, leaving all contract storage and reserve accounting unchanged.

5. **Supported decimals (`1..=18`)**
   - Multi-asset payment configuration (`add_payment_asset`) normalizes token amounts to 7-decimal internal accounting (`INTERNAL_DECIMALS = 7`).
   - Only tokens with `1 <= decimals <= 18` are accepted at configuration time. Tokens with `0` decimals or `> 18` decimals are rejected with `ContractError::UnsupportedToken`.

## Admin Allowlist Requirement

Only tokens explicitly vetted and added to the admin allowlist can be configured for protocol use:

- **Creator Keys (`creator-keys`)**:
  - `allowlist_token(admin, token)` / `remove_allowlisted_token(admin, token)` / `is_token_allowlisted(token)`
  - `add_payment_asset` and `set_xlm_token_address` reject non-allowlisted tokens with `UnsupportedToken`.
- **Holder Rewards (`holder-rewards`)**:
  - `allowlist_token(admin, token)` / `remove_allowlisted_token(admin, token)` / `is_token_allowlisted(token)`
  - `create_airdrop` rejects non-allowlisted reward tokens with `ContractError::UnsupportedToken`.

## Reserve Accounting Invariant

For all supported tokens, the contract's tracked reserve/escrow accounting (`AssetReserve`, `BondStatus`, `Airdrop`, `KeyDividend`, `PayoutStream`) equals the contract's actual on-chain `TokenClient::balance(contract)` at every quiescent state.

## Testing Mocks

The `test-utils` crate (`test_utils::non_standard_tokens`) provides five mock token contracts for verifying these invariants:

- `FeeOnTransferToken` — deducts a configurable basis-point fee on every transfer.
- `RebasingToken` — scales balances via a mutable multiplier (including optional mid-transfer rebasing).
- `NoOpTransferToken` — returns from `transfer` without updating balances.
- `PanickingToken` — panics during `transfer`.
- `UnusualDecimalToken` — exposes configurable `decimals()` (e.g. `0`, `2`, `18`, `25`).
