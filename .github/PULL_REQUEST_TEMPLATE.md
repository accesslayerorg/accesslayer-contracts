## Summary

- 

## Testing

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`

## Security review

<!--
Required when this PR changes a path in .github/sensitive-paths.yml
(authorization, fees, reserves and fund movement, upgrades, storage, or the
review process itself). The "Security checklist" CI job fails while any box
below is unticked. If an item does not apply, tick it and write N/A with the
reason. PRs that touch no sensitive path can leave this section as it is.
See docs/review.md.
-->

- [ ] Authorization: every new or changed entrypoint calls `require_auth` on the right address, admin/role checks are unchanged or reviewed, and no check moved after a state change
- [ ] Arithmetic: amounts, fees and shares use checked or saturating math, rounding favours the protocol, and overflow/underflow and zero-value paths are tested
- [ ] Fund movement: token transfers follow checks-effects-interactions, balances and reserves stay conserved, and recipients cannot be redirected
- [ ] Storage: keys, TTL extension and layout changes are reviewed against `docs/storage-key-invariants.md`, with a migration note for any layout change
- [ ] Upgrades: upgrade, proxy, pause and factory paths keep their admin/timelock gating, and the WASM fixture is refreshed when contract code changes
- [ ] Events: emitted events stay compatible with `docs/contract-event-conventions.md`, and security-relevant actions emit an event

## Checklist

- [ ] Linked issue or backlog item
- [ ] Added or updated `creator-keys` unit/integration tests for every changed contract behavior, including failure paths for new or reachable `ContractError` variants
- [ ] Ran `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`, or explained exactly why a command was not run
- [ ] Reviewed persistent storage changes against `docs/storage-key-invariants.md`; any storage layout change includes a migration/backward-compatibility note
- [ ] Confirmed event names, topic order, payload field order, and field meanings remain compatible with `docs/contract-event-conventions.md`, or documented the breaking change and versioning plan
- [ ] Updated docs for any changed public contract interface, read-only method, event schema, storage behavior, fee logic, or deployment workflow
- [ ] Scope stays limited to one contract concern and does not include unrelated formatting, lockfile, generated artifact, or dependency changes
