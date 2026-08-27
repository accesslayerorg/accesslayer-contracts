## Summary

- Add timelock for admin config changes with 48-hour delay (#768)
- Add snapshot voting weight capture for governance polls (#765)

Closes #768
Closes #765

## Changes

### #768 — Time-locked admin config changes
- Added `propose_config_change(admin, change_type, payload)` to record a proposal with 48-hour delay
- Added `execute_config_change(admin, proposal_id)` that panics with `AllocationLocked` if called too early
- Added `cancel_config_change(admin, proposal_id)` to cancel pending proposals
- Supports change types: `UpdateFee`, `UpdateCurveExponent`, `UpdateTreasury`
- Emits `config_change_proposed`, `config_change_executed`, and `config_change_cancelled` events

### #765 — Snapshot voting weight capture
- Added `cast_vote_with_snapshot(creator_id, voter, poll_id, option_index)` that uses snapshot balance
- Snapshot captured lazily on first vote from the holder's live balance at vote time
- Prevents post-proposal key purchases from influencing vote weight
- Added `get_vote_snapshot` view function

### Already implemented (no changes needed)
- #767 — `distribute_dividend` already exists
- #769 — `transfer_keys` already exists
