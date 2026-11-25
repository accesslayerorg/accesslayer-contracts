# Reentrancy protection

Use `common::guard::ReentrancyGuard` at the start of every entrypoint that
changes balances, claims or distributes value, or calls another contract while
state is being updated. Keep the guard alive for the entire operation:

```rust
let _guard = ReentrancyGuard::enter(&env);
```

The guard stores its lock in temporary contract storage and rejects nested
entry with `common::guard::Error::Reentrant`. Its scope guard clears the lock
when the operation returns, including `Result::Err`; a contract trap reverts
the whole invocation and its storage changes. Do not call `exit` early or
perform an external call before entering the guard.

Guard shared internal implementations where possible, so public aliases share
one protected path. Protect external token transfers in both their funding and
claiming paths. Add a reentrant mock-contract test whenever a new external
call or value-moving entrypoint is introduced, and check that failed callbacks
leave balances and accounting unchanged.

# Authorization matrix

Every public entrypoint on `CreatorKeysContract` must be classified in
`creator-keys/tests/auth_matrix.rs`. A build-time check in
`creator-keys/build.rs` scans all `#[contractimpl] impl CreatorKeysContract`
blocks under `creator-keys/src/` and fails compilation if any public function
is missing from the matrix.

## Adding a new entrypoint to the matrix

1. **Classify the entrypoint in `creator-keys/tests/auth_matrix.rs`**:
   - If the function mutates contract state or emits state-changing events, add
     a `MatrixEntry` to `STATE_CHANGING_MATRIX` (between `// AUTH_MATRIX_BEGIN`
     and `// AUTH_MATRIX_END`) with its expected `AuthorizedRole`:
     - `AuthorizedRole::Admin` — requires protocol admin authorization.
     - `AuthorizedRole::Creator` — requires the key creator's authorization.
     - `AuthorizedRole::Holder` — requires the acting holder, buyer, seller,
       voter, or liquidity provider's authorization.
     - `AuthorizedRole::Governor` — requires an eligible protocol governance
       participant or governance contract.
     - `AuthorizedRole::Anyone { reason: "..." }` — intentionally permissionless
       (for example, timelock execution or post-deadline finalization). You
       **must** provide a concise, non-empty `reason` explaining why no
       signature is required.
   - If the function is strictly a read-only view, add its name to
     `READ_ONLY_ENTRYPOINTS`.

2. **Add an execution test case**:
   - For protected roles (`Admin`, `Creator`, `Holder`, `Governor`), add a
     `verify_protected_case(...)` call in the matching role test function. The
     harness automatically checks that:
     - Calling with no auth (`env.set_auths(&[])`) reverts.
     - Calling with an unauthorized signer (`MockAuth` for `stranger`) reverts.
     - Calling with the expected signer succeeds and `env.auths()` records the
       expected address, contract ID, function symbol, and arguments.
   - For `AuthorizedRole::Anyone`, add a `verify_permissionless_case(...)` call
     confirming the entrypoint succeeds with `env.set_auths(&[])` and leaves
     `env.auths()` empty.

3. **Verify locally**:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test -p creator-keys --test auth_matrix
   ```
