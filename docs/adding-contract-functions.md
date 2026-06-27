# Adding a New Contract Function

This guide documents the expected patterns when adding new functionality
to the `creator-keys` contract.

## Module Placement

Place new functions in the module that matches their category:

| Category | Module | Example |
|----------|--------|---------|
| Trade logic | `creator-keys/src/lib.rs` — main contract impl | `buy_key`, `sell_key` |
| Configuration | `creator-keys/src/lib.rs` — admin section | `set_key_price`, `update_fee_bps` |
| View / read-only | `creator-keys/src/lib.rs` — view section | `get_total_key_supply`, `get_key_balance` |
| Admin / governance | `creator-keys/src/lib.rs` — admin section | `pause`, `unpause` |

## Authorization Pattern

Use `require_auth` to enforce who can call a function.

```rust
// Require the caller to be the creator themselves
pub fn update_creator_fee_recipient(
    env: Env,
    creator_id: Address,
    new_recipient: Address,
) -> Result<(), ContractError> {
    // The current fee recipient must authorize this call
    let current_recipient = get_fee_recipient(&env, &creator_id)?;
    current_recipient.require_auth();
    // ... function body
}
```

Rules:
- **Creator actions**: require `creator_id.require_auth()`
- **Fee recipient rotation**: require current recipient auth, not creator
- **Admin/pause functions**: require the stored admin address auth
- **Read-only views**: no auth required

## Event Emission Pattern

Define a new event struct and emit it at the end of the function:

```rust
// 1. Define the event struct in src/events.rs
#[contracttype]
#[derive(Debug, Clone, PartialEq)]
pub struct MyNewEvent {
    pub creator_id: Address,
    pub amount: i128,
}

// 2. Emit in the contract function
pub fn my_function(env: Env, creator_id: Address, amount: i128) {
    // ... function logic ...
    env.events().publish(
        (symbol_short!("MY_EVT"), creator_id.clone()),
        MyNewEvent { creator_id, amount },
    );
}
```

## CI Requirements

All PRs must pass the following checks before merge:

| Check | Command | What it enforces |
|-------|---------|------------------|
| Format | `cargo fmt --check` | Consistent code style |
| Lint | `cargo clippy -- -D warnings` | No warnings allowed |
| Tests | `cargo test` | All unit and regression tests pass |
| License | File license header check | BSD-3-Clause on all `.rs` files |

Run locally before pushing:
```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
