# TypeScript bindings

Client and server code that hand-writes contract call types drifts from the
deployed interface and fails at runtime. This repository publishes generated
bindings instead: typed clients, typed error enums, typed event decoders, and a
manifest that records the WASM hash every binding was generated from.

Package: [`bindings/`](../bindings) (`@accesslayer/contracts-bindings`).

## What is generated

| Output | Source | Purpose |
| --- | --- | --- |
| `bindings/src/contracts/<contract>.ts` | `stellar contract bindings typescript` | Typed client, entrypoint parameter/return types, contract enums |
| `bindings/src/errors.ts` | contract spec (`udt_error_enum_v0`) | Error-code → variant registry |
| `bindings/src/events.ts` | Rust event definitions + spec payload types | Typed event decoders |
| `bindings/src/index.ts` | the contract list | Package barrel (one namespace per contract) |
| `bindings/manifest.json` | generation run | Package version, per-contract `wasmSha256`, interface hash, entrypoint/event counts, skipped contracts |

`bindings/src/decode.ts` is the only hand-written runtime file: it converts
RPC/XDR event shapes into plain values.

## Regenerate

Requirements: `stellar` CLI (v22+), Rust with the `wasm32v1-none` target, and
Node.js 18+.

```bash
./scripts/generate-bindings.sh   # or: make bindings
```

Useful flags:

| Flag | Effect |
| --- | --- |
| `--contracts a,b` | Only regenerate the listed contracts |
| `--wasm-dir DIR` | Reuse prebuilt `<crate>.wasm` files instead of building |
| `--no-build` | Reuse the existing `target/wasm32v1-none/release` build |
| `--strict` | Exit non-zero when a contract cannot be built or bound |

The script builds each contract with `cargo build --release --target
wasm32v1-none -p <crate>`, generates the client with `stellar contract bindings
typescript`, captures the interface with `stellar contract info interface`, and
then runs [`scripts/postprocess-bindings.mjs`](../scripts/postprocess-bindings.mjs)
to write the committed files.

The output is deterministic (sorted, no timestamps, fixed headers), so an
unchanged interface produces a byte-identical tree.

### A contract that does not compile

A contract whose WASM cannot be built is recorded in `manifest.json` under
`skipped`, with the reason, and the rest of the package is still regenerated.
Run with `--strict` to make that a hard failure. A skipped contract has **no**
bindings, so consumers must check the manifest before relying on one.

## Events

The contracts publish events through `env.events().publish(...)` and document the
shape on the payload type:

```rust
/// Event shape:
/// - topics: `(FEE_ADJUSTED_EVENT_NAME, key_id)`
/// - data: `FeeAdjustedEvent`
#[contracttype]
pub struct FeeAdjustedEvent { ... }
```

The post-processor combines those doc blocks with the publish call sites to
recover each event's topic symbol, and types the payload from the contract spec.
Events whose payload type cannot be resolved statically are typed `unknown`;
their name and topics still decode. See
[contract-event-conventions.md](./contract-event-conventions.md) for the naming
and topic conventions themselves.

## Example

[`examples/typescript-buy-and-decode`](../examples/typescript-buy-and-decode)
places a buy-side order through the bindings and decodes the emitted event
against a local network. It doubles as a smoke test for the generated client and
event decoder.

## CI

[`.github/workflows/bindings.yml`](../.github/workflows/bindings.yml) regenerates
the package on every push and pull request, rebuilds it with `tsc`, and fails if
`bindings/` differs from what is committed. A contract interface change without
a regenerated commit therefore cannot merge.

## Publishing a release

Tag the release `bindings-v<version>` (the tag must match
`bindings/package.json`). The workflow re-runs generation, verifies the tree is
clean, and publishes to npm with provenance. The published `manifest.json`
records the matching `wasmSha256` for every contract, so consumers can confirm
the bindings match the deployed WASM.

Required configuration:

* repository secret `NPM_TOKEN` with publish rights for the
  `@accesslayer/contracts-bindings` scope,
* an `@accesslayer` npm org that owns the package name.

## How consumers pin versions

1. Depend on a fixed version (`"@accesslayer/contracts-bindings": "0.1.0"`, not a
   range) so a contract interface change never arrives unreviewed.
2. Verify `manifest.json`'s `wasmSha256` for each contract you call against the
   deployed contract hash (`stellar contract info hash --wasm`,
   or your deployment record — see [deploy-artifacts.md](./deploy-artifacts.md)).
3. Commit the lockfile change together with the review of the generated diff.
4. Bump deliberately, reading the diff for the namespaces you use. A contract
   missing from `manifest.json` is either skipped (see `skipped`) or not part of
   the workspace.

## Limitations

* Contracts that do not compile cannot be bound (they appear under `skipped`).
* Event payload types are resolved from the spec and the Rust server side; an
  event that publishes a tuple of locally-typed values is decoded but left
  `unknown`.
* `stellar contract bindings typescript` emits parameter destructuring verbatim;
  a contract parameter named after a JS reserved word (for example `new`) would
  not parse. The post-processor replaces those parameter patterns with a plain
  object parameter, keeping property names (and the wire format) unchanged.
