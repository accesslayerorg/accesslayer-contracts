# Creator Profile Optional Fields Roadmap

This roadmap item scopes optional creator profile field expansion without breaking current storage and read semantics.

## Candidate Optional Fields

These fields are explicitly optional and must not be required for registration:

- `display_name`: human-readable name distinct from handle.
- `bio`: short creator description.
- `avatar_url`: off-chain image URL reference.
- `social_links`: bounded list of external profile links.
- `website_url`: primary website link.

## Backward Compatibility Expectations

- Existing `CreatorProfile` storage entries remain readable without migration.
- Existing read methods (`get_creator`, `get_creator_details`, key/supply/fee reads) keep current return shapes unless a new versioned method is introduced.
- Optional fields should use additive storage patterns:
  - either a new optional map keyed by creator address, or
  - a versioned profile representation with explicit default handling.
- Missing optional data must resolve to deterministic defaults in view methods.
- Existing indexers should continue to decode current events and reads without schema breakage.

## Phased Contributor Plan

1. Design phase
   - Decide additive storage model (`DataKey::CreatorOptionalProfile` map vs versioned struct).
   - Define max sizes and allowed formats for each optional field.
2. Contract phase
   - Add write entrypoint(s) for optional profile updates with auth and validation.
   - Add read entrypoint(s) that return stable, non-panicking views.
3. Migration/compatibility phase
   - Confirm pre-existing creator records read correctly with no backfill required.
   - Add tests for missing optional fields and mixed old/new records.
4. Integration phase
   - Document consumer behavior for missing fields and default values.
   - Align event/indexer expectations for optional profile updates.

## Follow-Up Implementation Tasks

- Add `ContractError` variants for optional profile validation failures.
- Add storage key invariants for new optional profile storage paths.
- Add tests for bounds, invalid URL formats (if enforced), and auth constraints.
- Add read-only documentation for optional profile response semantics.
