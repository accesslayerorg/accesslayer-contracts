# Minimum Viable Test Structure for New Contract Functions

Contributors adding new contract entrypoints should include a small, consistent set of tests before opening a pull request. This guide defines the minimum categories and shows example structures for read-only and state-changing functions.

For shared setup helpers, see [`creator-keys/tests/contract_test_env/`](../creator-keys/tests/contract_test_env/). For quote-specific coverage, see [deterministic-quote-tests.md](./deterministic-quote-tests.md). For read-only return semantics, see [read-only-methods.md](./read-only-methods.md).

## Minimum test categories

Every new contract function should have tests covering all categories that apply:

| Category | When required | What to verify |
|---|---|---|
| **Happy path** | Always | The function succeeds and returns or stores the expected value under normal preconditions. |
| **Error cases** | When the function can fail | Each documented error variant is returned for the input that triggers it. Use `try_*` client methods and assert on `ContractError`. |
| **State assertions** | Write functions; read functions that must not mutate storage | After a successful call, storage and derived views match expectations. For read-only entrypoints, confirm no storage changed (see [`capture_snapshot`](../creator-keys/tests/contract_test_env/mod.rs)). |

Optional but encouraged when relevant:

- **Authorization**: caller must be the expected admin or account (often covered by existing auth tests; add one if your entrypoint introduces a new auth rule).
- **Idempotency / overwrite**: setting the same value twice or updating an existing value behaves as documented.
- **Boundary inputs**: zero, max length, or config limits that the function explicitly validates.

## Test file placement

- **Integration tests** for public contract entrypoints: `creator-keys/tests/<feature>.rs`
- **Unit tests** for internal helpers: `creator-keys/src/test.rs` or the module's `#[cfg(test)]` block

Use `mod contract_test_env;` and the shared helpers instead of copying env setup boilerplate.

## Naming convention

Name tests so a reviewer can tell what behavior is covered without opening the function body:

- `test_<entrypoint>_<expected_outcome>_<condition>`
- Examples: `test_get_protocol_admin_returns_none_initially`, `test_buy_key_insufficient_payment_fails`

## Example: read-only function

Read-only entrypoints need at least a success case and each documented error path. When the function must not write storage, capture state before and after the call.

```rust
mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_get_key_symbol_success() {
    // Happy path: registered creator returns stored handle.
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "alice");

    let symbol = client.get_key_symbol(&creator);
    assert_eq!(symbol, soroban_sdk::String::from_str(&env, "alice"));
}

#[test]
fn test_get_key_symbol_fails_if_not_registered() {
    // Error case: unregistered creator returns NotRegistered.
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let creator = Address::generate(&env);

    let result = client.try_get_key_symbol(&creator);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}
```

For read-only calls that must not mutate contract state:

```rust
let before = contract_test_env::capture_snapshot(&client, &creator, &holder);
client.get_sell_quote(&creator, &holder); // or your read entrypoint
let after = contract_test_env::capture_snapshot(&client, &creator, &holder);
before.assert_unchanged(&after);
```

## Example: write function

State-changing entrypoints need a happy path with post-condition checks, plus tests for each validation or business-rule failure. Confirm that failed calls do not leave partial state behind when that matters.

```rust
mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_set_protocol_fee_recipient_accepts_valid_address() {
    // Happy path: valid input is stored and readable.
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);

    let result = client.try_set_protocol_fee_recipient(&admin, &recipient);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(client.get_protocol_fee_recipient(), Some(recipient));
}

#[test]
fn test_set_protocol_fee_recipient_rejects_zero_address() {
    // Error case: invalid input returns the documented error.
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = Address::generate(&env);
    let zero_str = String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero_addr = Address::from_string(&zero_str);

    let result = client.try_set_protocol_fee_recipient(&admin, &zero_addr);
    assert_eq!(result, Err(Ok(ContractError::ZeroAddress)));

    // State assertion: rejection must not persist partial updates.
    assert_eq!(client.get_protocol_fee_recipient(), None);
}
```

For operations that change balances or supply, assert the observable views your callers rely on (for example `get_key_balance`, `get_total_key_supply`, or `get_creator`) in the same test as the happy path.

## Checklist before opening a PR

- [ ] Happy-path test exists for the new entrypoint.
- [ ] Each new or reachable `ContractError` variant has a dedicated test (or is covered by an existing shared test file).
- [ ] Write paths assert stored state and views after success; failure paths assert no unintended state when applicable.
- [ ] Read-only paths assert no storage mutation when the contract guarantees it.
- [ ] Tests use `contract_test_env` helpers where possible and run under `cargo test --workspace`.

## Related docs

- [CONTRIBUTING.md](../CONTRIBUTING.md) — verification commands and contribution rules
- [error-codes.md](./error-codes.md) — error variants to cover in failure tests
- [read-only-methods.md](./read-only-methods.md) — expected return values and edge cases for `get_*` / `is_*` entrypoints
