# Atomic batch swaps

`batch_swap(trader, items)` authorizes the trader once and executes 1–15
`BatchSwapItem` entries in order. Each entry contains `key_id`, `amount`,
`direction` and `max_slippage_bps`. The same trader owns every buy or sell.
Zero amount is rejected, as are tolerances above 10,000 basis points. A zero
tolerance retains the existing `swap` opt-out convention.

Each item checks the bonding curve's relative spot-price impact before its
trade. Exceeding any item's tolerance returns `SlippageExceeded`; the Soroban
transaction rolls back earlier items, fees and events. Supply caps must allow
a complete fill. Buys reuse `buy_keys_core`; sells reuse the same authorized
core as `batch_sell`, so their pause, blacklist, balance, freeze and lockup
guards remain in force. Buying and immediately selling the same position can
therefore still fail the flash-loan guard.

`get_batch_quote(items)` estimates each item in order without executing trades.
Virtual supplies account for repeated keys. It returns the same per-item
schema and summed fee used by a receipt. As with other quote views, execution
can fail a trading guard or differ if state changes before submission.

`BatchSwapOutput.output` follows `swap`: the gross payment on a buy and the net
proceeds on a sell. `fee` reports creator, protocol and trade-fee allocations,
plus configured royalty and buy LP allocations; these allocations are included
in existing accounting and must not be added to a buy's payment again. Launch
penalties remain governed by the existing sell path.

The `BatchSwapExecuted` receipt has `trader`, `items`, `total_fee` and `ledger`.
Its event topics are `batch_sw`, trader; the payload is the complete receipt.
No existing event, storage schema or numeric error is changed. Tests cover
mixed trades, quote agreement, repeated-key pricing, event fields, limits and
rollback when a later item fails slippage.
