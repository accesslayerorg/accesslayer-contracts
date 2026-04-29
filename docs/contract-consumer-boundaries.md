# Contract outputs: client, server, and compatibility

Access Layer’s **Soroban** contracts are the on-chain source of truth for registration, key supply, pricing, and fee configuration. The **client** (wallet, web app) and **server** (API, indexers) consume specific **read surfaces** and **event payloads** from those contracts. This page lists the main dependencies and the compatibility expectations when those outputs change.

For fee math and rounding, see [fee-assumptions.md](./fee-assumptions.md). For testnet build and deploy steps, see [stellar-testnet-deployment.md](./stellar-testnet-deployment.md). For static names of read-only entrypoints (useful to indexers and version checks), see the `constants::creator_reads` submodule in `creator-keys` source.

## What clients and servers depend on

### Versioning and schema detection

- **`get_protocol_state_version`**: Intentionally bumps when *externally visible* protocol semantics or stable read shapes change. Clients and indexers can gate UI or parsing on this value.
- **Wasm hash (deploy artifact)**: Off-chain systems that pin “which build is live” should track the **deployed wasm hash** and/or **contract id** in config (see [deploy-artifacts.md](./deploy-artifacts.md)).

### Read-only state and “views”

Stable structs returned by read-only methods are part of the integration contract:

- **`ProtocolFeeView` / `get_protocol_fee_view`**: Non-optional fee configuration snapshot (`is_configured` plus bps). Used when you must not branch on `Option` at the type level.
- **`CreatorDetailsView`**, **`CreatorFeeView`**, **`HolderKeyCountView`**: Registration and per-creator state without panicking on bad addresses.
- **Quotes** — `get_buy_quote` / `get_sell_quote` → `QuoteResponse` (`price`, `creator_fee`, `protocol_fee`, `total_amount`). **Buy** uses `total_amount = price + fees`; **sell** uses `total_amount = price - fees` (seller payout after fees, under current logic).
- **Constants exposed as entrypoints** — e.g. `get_key_decimals` — must stay aligned with any off-chain display or formatting.

### Events

State-changing entrypoints that emit events (for example `register_creator`, `buy_key`) are consumption boundaries for **indexers and analytics**: payload shape and event names are stable API for the server. Any change to event name, field order, or field meaning should be treated as a **breaking** change for indexers unless versioned (e.g. a new event name) or coordinated with consumers.

### Event schema compatibility checklist

For any event payload or event name change, contributors should follow this compatibility checklist:

- Do not rename existing event names or fields without creating a new versioned event. Renames are breaking for downstream indexers that decode by field position or name.
- Do not change field semantics, types, or encoded meaning without explicit coordination and a documented migration plan.
- Prefer adding new optional fields only when clients and indexers can safely ignore unknown fields.
- If an event schema change is required, consider a versioned event name and bump `get_protocol_state_version` when consumer parsing expectations change.
- Coordinate with downstream indexers before deploying any event payload change; treat event payloads as a stable public API.

For storage key details tied to creator registration metadata and ownership expectations, see [Storage Key Invariants](./storage-key-invariants.md).

## Compatibility expectations

| Change | Expectation |
|--------|-------------|
| **Bugfix** that does not change storage layout, event schema, or read return shapes | New wasm deploy; `PROTOCOL_STATE_VERSION` may stay the same; consumers already compatible. |
| **New optional read method** or **new field with defaulting behavior** in a versioned read path | Coordinate clients/indexers; prefer bumping `get_protocol_state_version` when the new surface is the recommended path. |
| **Fee basis-point rules or `QuoteResponse` meaning** (e.g. what `total_amount` includes) | Treat as **breaking** for UIs and pricing previews: bump `PROTOCOL_STATE_VERSION`, document in [fee-assumptions.md](./fee-assumptions.md), and release in lockstep with client/server updates. |
| **Storage or event format change** | Breaking for indexers and any client that decodes old events: explicit migration or backfill plan; never silent change. |

## Out of scope for this repository

**Application features** (email, social graphs, off-chain key custody UX) and **indexer business logic** live in their respective codebases. This repo documents only what the chain exposes. When in doubt, treat **contract method names, event names, and serialized struct fields** as the public API to other layers.

Contributors can validate read surfaces against current tests with `cargo test -p creator-keys` and the workspace checks in [CONTRIBUTING.md](../CONTRIBUTING.md).
