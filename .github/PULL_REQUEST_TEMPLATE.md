## Summary

- 

## Testing

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`

## Checklist

- [ ] Linked issue or backlog item
- [ ] Added or updated `creator-keys` unit/integration tests for every changed contract behavior, including failure paths for new or reachable `ContractError` variants
- [ ] Ran `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`, or explained exactly why a command was not run
- [ ] Reviewed persistent storage changes against `docs/storage-key-invariants.md`; any storage layout change includes a migration/backward-compatibility note
- [ ] Confirmed event names, topic order, payload field order, and field meanings remain compatible with `docs/contract-event-conventions.md`, or documented the breaking change and versioning plan
- [ ] Updated docs for any changed public contract interface, read-only method, event schema, storage behavior, fee logic, or deployment workflow
- [ ] Scope stays limited to one contract concern and does not include unrelated formatting, lockfile, generated artifact, or dependency changes
