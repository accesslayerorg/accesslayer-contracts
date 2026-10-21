# Creator Keys Lifecycle Operations Checklist

This checklist helps operators and integrators work with the lifecycle safeguards already exposed by the Creator Keys contract.

## Persistent state and TTL

- Treat creator profiles, holder balances, auction state, governance state, and deprecation state as persistent operational data.
- Monitor TTL health for inactive positions as well as actively traded keys.
- Use the contract's TTL refresh and normal write paths according to the contract's current configuration; do not assume a dormant position will remain available indefinitely without maintenance.

## Pre-launch auctions

- Publish auction configuration before accepting bids and validate the intended supply and price parameters.
- Verify auction state before closing or cancelling an auction.
- Confirm the transition to the bonding curve after the auction lifecycle completes.

## Governance and privileged actions

- Review the configured proposal deadline and quorum settings before starting a governance action.
- Record proposal inputs and expected outcomes in the operational change record.
- Treat administrative key-lifecycle operations as high-impact actions that require an independently reviewed release plan.

## Key deprecation and buyback operations

- Set a fixed redemption price only after calculating the required escrow for circulating supply.
- Confirm the deprecation state and available escrow before communicating redemption terms to holders.
- Give holders clear timing and support information before executing a deprecation-related operation.
- Preserve event and transaction records so the lifecycle can be audited after completion.

## Incident response

If lifecycle state is unexpected, pause user-facing automation, capture the relevant contract events and ledger references, and investigate before attempting a compensating transaction. Do not retry privileged lifecycle actions blindly.