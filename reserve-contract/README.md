# Reserve contract

`ReserveContract` tracks a bonding-curve reserve independently for each key
address.

- `initialize(admin)` sets the account authorized to deploy reserves.
- `accrue_reserve(key_id, fee_amount)` records `floor(fee_amount / 20)`, i.e.
  5% of the swap fee, and emits `ReserveAccrued`.
- `deploy_reserve(key_id, amount)` requires the configured admin's
  authorization, subtracts the deployed amount, and emits `ReserveDeployed`.
  A call may deploy no more than half of that key's reserve balance at the
  start of the call.
- `get_reserve_balance(key_id)` returns the remaining recorded balance.

Reserve amounts use integer units, so fractional amounts are rounded down.
