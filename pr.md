## Summary

- Add `set_supply_cap` entrypoint so creators can configure a max supply after registration (#766)
- Add multisig pause mechanism with `propose_pause` / `approve_pause` requiring 2-of-3 admin approval (#761)
- Add linear vesting schedule with `create_vesting` / `claim_vested` for creator key allocations (#763)

Closes #766
Closes #761
Closes #763

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

## New error variants (appended to end of enum)
- `CapAlreadySet = 37`
- `MultisigAdminLimitExceeded = 38`
- `ProposalNotFound = 39`
- `AlreadyApproved = 40`
- `ApprovalThresholdNotMet = 41`
- `VestingNotFound = 42`
- `NothingToClaim = 43`
- `VestingNotStarted = 44`
