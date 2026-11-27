# Storage Key Registry

Storage keys live in one place: [`common/src/keys.rs`](../common/src/keys.rs). Each
contract that uses the registry has a module there (`common::keys::<crate_name>`)
holding its key enum, and every variant records the storage class it is written to.
Contracts import their keys from that module, for example:

```rust
use common::keys::auto_invest_registry::DataKey;
```

## Why

Keys were defined ad hoc in each module, so two features could pick the same key
shape and overwrite each other's data, and a refactor could change a key's
encoding without anyone noticing. Keeping every key in one registry lets the tests
below catch both.

## How keys are encoded

A `#[contracttype]` enum key is stored as a vector holding the **variant name** as a
symbol followed by the variant's fields. The enum's name and the crate that defines
it are not part of the encoding. That is why the enums could move into `common`
without changing a single stored key, and it is also why renaming a variant, or
changing, adding or reordering its fields, changes the key and orphans existing data.

Raw keys such as `symbol_short!("admin")` or `(symbol_short!("bal"), holder)` encode
differently from enum variants and are not visible to the collision checks, so they
are not allowed outside the registry.

## Storage classes

Every variant is declared with `=> Instance`, `=> Persistent` or `=> Temporary`:

| Class | Use it for |
| --- | --- |
| `Instance` | Small, contract-wide config read on most calls (admin, global settings). Shares the instance TTL. |
| `Persistent` | Per-user or per-key records that must survive. Each entry has its own TTL and is archived, not deleted, when it expires. |
| `Temporary` | Data that may disappear (locks, short-lived nonces). Deleted when its TTL expires. |

The class is part of the golden file, so moving a key to another class is caught
the same way as an encoding change.

## Tests

`common/tests/storage_key_registry.rs` runs with `cargo test -p common`:

- **Collisions:** builds a sample of every variant of every registered enum,
  encodes it to XDR and fails if two keys of the same contract encode to the same
  bytes. It also fails if a variant has no sample.
- **Golden file:** compares every encoding and storage class with
  [`common/tests/storage_keys.golden`](../common/tests/storage_keys.golden). An
  existing line that changes or disappears fails the test.
- **Lint:** scans the non-test sources of every workspace member and fails on a
  storage call whose key is a raw tuple, `symbol_short!`, `Symbol::...` or a
  `SCREAMING_CASE` constant, and on a `#[contracttype]` enum named `*Key` that is
  declared outside the registry. Crates listed in `NOT_YET_MIGRATED` in that file
  are skipped, each with the reason.

## Adding a key

1. Add the variant at the **end** of the contract's enum in `common/src/keys.rs`,
   with a `///` doc saying what it stores and its storage class after `=>`.
2. Add a sample for it in `all_entries` in `common/tests/storage_key_registry.rs`.
3. Regenerate the golden file and check that only new lines were added:

   ```bash
   UPDATE_STORAGE_KEY_GOLDEN=1 cargo test -p common --test storage_key_registry
   git diff common/tests/storage_keys.golden
   ```

4. Run `cargo test -p common` and commit the golden file with the change.

## Rules

- Never rename, remove or change the fields of an existing variant. Add a new
  variant and migrate the data instead.
- Never edit an existing line of the golden file by hand. A diff that changes an
  existing line is a storage layout change and needs a migration plan in the PR.
- Use the registry enum for every read and write, including `extend_ttl`. Do not
  build keys from tuples or symbols.
- A new contract gets its own module in `common/src/keys.rs`, declared with
  `storage_keys!` and the crate name as the contract label.
- Registry enums use `#[contracttype(export = false)]`, so they do not appear in
  any contract's interface spec. Keys are internal and are never passed to or
  returned from entrypoints.

## Not migrated yet

These crates keep their keys locally for now; each is listed in `NOT_YET_MIGRATED`:

| Crate | Reason |
| --- | --- |
| `creator-keys` | Does not compile on `main`; migrate once it builds. |
| `creator-keys-factory`, `dividend-contract` | Depend on `creator-keys`. |
| `creator-penalty`, `storage-cleanup` | Do not compile on `main`. |
| `key-health-score`, `key-liquidity-depth`, `referral-contract` | Key fields use contract-local types (`Feature`, `OrderSide`, `ReferralTier`) that would have to move into `common` with them. |
| `test-utils` | Mock contracts used only by tests. |
