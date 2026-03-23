# Access Layer Contracts Issue Backlog

This backlog is organized into contributor-ready sections. Each section contains 10 scoped issue drafts that can be opened individually on GitHub.

## Section 1: Creator Registry and Storage

1. `contracts-registry-01` Add validation rules for creator handles during registration.
2. `contracts-registry-02` Add a duplicate-registration guard with explicit error handling.
3. `contracts-registry-03` Replace ad hoc panic strings with structured contract errors.
4. `contracts-registry-04` Add creator metadata versioning or storage-layout notes.
5. `contracts-registry-05` Emit richer registration events for downstream indexing.
6. `contracts-registry-06` Add tests for registration authorization and duplicate prevention.
7. `contracts-registry-07` Add helper accessors for supply and creator lookup responses.
8. `contracts-registry-08` Document storage keys and invariants for contributor onboarding.
9. `contracts-registry-09` Add contract comments that explain persistent storage choices.
10. `contracts-registry-10` Define a roadmap issue for optional creator profile expansion fields.

## Section 2: Trading, Fees, and Economics

1. `contracts-economics-01` Design and implement a sell-key function with supply updates.
2. `contracts-economics-02` Add payment handling for buy transactions.
3. `contracts-economics-03` Add creator fee and protocol fee split logic.
4. `contracts-economics-04` Add bonding-curve pricing helpers with deterministic tests.
5. `contracts-economics-05` Add guardrails for zero-supply and invalid trade states.
6. `contracts-economics-06` Emit buy and sell events with consistent payload structure.
7. `contracts-economics-07` Add slippage or quote-validation design notes for the client boundary.
8. `contracts-economics-08` Add tests for fee accounting and balance conservation.
9. `contracts-economics-09` Add error variants for trading and payment failures.
10. `contracts-economics-10` Review numeric types and overflow assumptions for pricing math.

## Section 3: Testing, Tooling, and Deployment

1. `contracts-tooling-01` Add a reusable test helper module for common contract setup.
2. `contracts-tooling-02` Add cargo aliases or scripts for formatter, clippy, and test workflows.
3. `contracts-tooling-03` Add deployment notes for Stellar testnet.
4. `contracts-tooling-04` Add a contract release checklist covering verification and deployment.
5. `contracts-tooling-05` Add one `good first issue` test-only task with clear acceptance criteria.
6. `contracts-tooling-06` Add CI documentation explaining what each contract check validates.
7. `contracts-tooling-07` Add a local developer guide for Soroban prerequisites.
8. `contracts-tooling-08` Add fixture-based tests for event assertions.
9. `contracts-tooling-09` Add a workflow for generating and storing deploy artifacts safely.
10. `contracts-tooling-10` Add architecture notes describing how client and server depend on contract outputs.
