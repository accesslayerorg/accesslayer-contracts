# Protocol parameter governance

The protocol admin calls `configure_protocol_governance(admin, key_id,
minimum_balance)` once to select the creator key whose holders constitute the
electorate. The minimum balance must be positive. Configuration cannot be
replaced while proposals are outstanding or used by a proposer to select a
smaller electorate. This path is separate from the older proposal contract in
`acl_dividend_twap_gov`.

`create_proposal(proposer, parameter, new_value, description)` requires the
proposer's authorization and at least the configured balance. `parameter` is
`ProtocolParameter::KeyPrice` (positive value) or `CurveSlope` (nonnegative).
Descriptions must contain 1–1024 bytes. The proposal snapshots total supply
and the balances in the key's holder registry, up to 1000 recorded holders.
Supply includes locked allocations; only recorded holder balances can vote.

The existing poll `cast_vote` API is preserved.
`cast_proposal_vote(voter, proposal_id, support)` requires the voter's authorization and
uses their snapshot balance, never a caller-supplied weight or a later balance.
Each address votes once. Transferring keys after creation does not move the
proposal's voting power. Voting closes exactly 72 hours after creation.

`execute_proposal(proposal_id)` is permissionless after a further 24-hour delay.
At least 10% of snapshot supply must participate, rounded up, and yes votes
must strictly exceed no votes. An executed proposal cannot run twice. Failed
quorum or approval returns `Unauthorized`; early execution returns
`TimelockNotElapsed`. The successful call updates the selected protocol
storage entry atomically with the executed marker. Other existing admin
configuration paths retain their authorization rules.

`get_protocol_proposal` returns the parameter, value, description, snapshot
supply, deadlines, tallies and executed marker.

## Storage and events

New persistent `GovernanceKey` entries store configuration, the next ID,
proposals, snapshot weights and vote receipts. No existing storage variant or
error code is changed. Entries receive the standard full TTL window. Restore
archived governance entries before resuming governance; do not treat archived
configuration as a fresh deployment.

All variants use a `ProtocolGov` prefix: a Rust enum's type name is not part
of the encoded storage key. Regression tests compare the XDR encoding against
the older proposal and leaderboard keys and verify that leaderboard
configuration survives governance operations.

Events use the following topic and payload schemas:

| Meaning | Topics | Data |
| --- | --- | --- |
| ProposalCreated | `gov_new`, proposer | `ProtocolProposal` |
| VoteCast | `gov_vote`, proposal ID, voter | support, snapshot weight |
| ProposalExecuted | `gov_exec`, proposal ID | executed `ProtocolProposal` |

Regression coverage checks the eligibility threshold, frozen voting weights,
duplicate votes, exact 10% quorum, the 72h plus 24h boundary, all three event
types, parameter persistence and replay rejection.
