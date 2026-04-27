# Safely Extending Contract Error Codes

This guide applies to the primary on-chain error enum in `creator-keys/src/lib.rs`:

- `ContractError` is marked with `#[contracterror]`
- it uses explicit numeric discriminants starting at `1`
- existing values are currently assigned sequentially from `1` through `14`

## Naming

Choose new variants that match the existing style:

- Use `UpperCamelCase` variant names such as `AlreadyRegistered` and `ProtocolFeeExceedsCap`.
- Prefer names that describe the failing condition, not the implementation detail.
- Keep names specific to the contract domain so they remain readable in logs, tests, and client code.
- If a new error is only used by a single flow, name it for that flow rather than reusing a generic placeholder.

## Safety

Error numbers are part of the contract's external interface and must remain stable.

- Do not reorder existing variants.
- Do not change existing numeric discriminants.
- Do not reuse a retired number for a different meaning.
- Only append new variants with the next unused number.

Changing existing error numbers can break callers that compare on the serialized error code, decode transaction results, or rely on documented tables.

## Documentation

Whenever you add, rename, or repurpose an error variant, update the relevant documentation in the same change:

- `docs/error-codes.md`
- any test comments or tables that assert discriminant stability
- any client-facing documentation that lists contract error meanings

If the new error affects a specific flow, also update nearby tests so the expected variant and numeric value stay explicit.

## Example

If you need a new error, add it at the end of the enum and document it alongside the existing table entries:

```rust
pub enum ContractError {
    // existing variants...
    NewFailure = 15,
}
```

That approach keeps the serialized contract surface backward compatible for existing consumers.
