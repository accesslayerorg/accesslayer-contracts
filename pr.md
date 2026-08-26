## Summary

- Add `set_supply_cap` entrypoint so creators can configure a max supply after registration (#766)
- Add multisig pause mechanism with `propose_pause` / `approve_pause` requiring 2-of-3 admin approval (#761)
- Add linear vesting schedule with `create_vesting` / `claim_vested` for creator key allocations (#763)
- Add timelock for admin config changes with 48-hour delay (#768)
- Add snapshot voting weight capture for governance polls (#765)

Closes #766
Closes #761
Closes #763
Closes #768
Closes #765

## Changes

### #766 — Supply cap configuration
- Added `set_supply_cap(creator, cap)` callable only by the creator
- Panics with `CapAlreadySet` if a cap already exists or new cap is below current supply
- Emits `supply_cap_set` event with key_id and cap value

### #761 — Multi-sig pause/unpause
- Added `set_multisig_admins(creator, admins)` to configure up to 3 admin addresses
- Added `propose_pause(creator, caller)` callable by any admin to initiate a pause proposal
- Added `approve_pause(creator, caller)` callable by a second admin to execute the pause
- Pause executes automatically when 2-of-3 threshold is reached; proposals reset after execution
- Emits `pause_proposed` and `trading_paused` events

### #763 — Vesting schedule
- Added `create_vesting(creator, beneficiary, total_keys, vesting_period_ledgers)` admin function
- Added `claim_vested(creator, beneficiary)` callable by the beneficiary
- Computes vested amount as `total_keys * elapsed_ledgers / vesting_period_ledgers` (floored)
- Panics with `NothingToClaim` if no new keys have vested
- Emits `vesting_created` and `keys_claimed` events

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
