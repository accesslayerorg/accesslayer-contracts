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
