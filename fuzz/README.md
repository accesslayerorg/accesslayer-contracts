# Stateful contract fuzzing

Install `cargo-fuzz` once with `cargo install cargo-fuzz`. Run both targets with
`make fuzz`; each target starts from the checked-in corpus and continues
coverage-guided fuzzing. To run one target, use
`cargo fuzz run trading fuzz/corpus/trading` or
`cargo fuzz run escrow fuzz/corpus/escrow`.

The trading target executes buys, sells, transfers, dividend distributions, and
claims against `creator-keys`. After every attempted operation it checks that
the reported supply equals the sum of the fuzz accounts' balances. The trading
contract's buy/sell entrypoints do not move a token-backed reserve, so reserve
solvency is checked for the escrow target instead.

The escrow target fuzzes the existing `KeySwapContract` proposal, fill, cancel,
and expiry lifecycle. It checks that each token's contract balance equals the
sum of outstanding offers, and that token balances across the accounts and
escrow are conserved after every operation. This contract does not currently
expose a dispute entrypoint; rejected third-party fill/cancel attempts are
adversarial checks, not a dispute workflow. The checked-in seeds model the
existing mixed-trade and atomic-swap integration scenarios.

When a run finds a crash, `cargo-fuzz` saves the reproducing input under
`fuzz/artifacts/<target>/`. Replay a saved input with, for example,
`cargo fuzz run trading fuzz/artifacts/trading/crash-<hash>`. Minimize it with
`cargo fuzz tmin trading fuzz/artifacts/trading/crash-<hash>`. Keep the minimized
input as a regression seed by copying it into
`fuzz/corpus/trading/` or `fuzz/corpus/escrow/`; subsequent fuzz runs replay
corpus inputs before mutation. The nightly workflow runs both targets for five
minutes and uploads the evolving corpora and crash artifacts, even if a target
finds a failure.
