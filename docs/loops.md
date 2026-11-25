# Stored collection loops

This inventory is in progress for upstream issue #1311. The table records the work limits for the paths covered by the local changes.

| Contract and operation | Worst-case work per invocation |
| --- | --- |
| Launch auction, `settle_auction` | At most 32 bids sorted and 32 bids examined for allocation. Each insertion compares at most six bids. |
| Launch auction, `settle_auction_page` | At most eight bids across sorting and allocation. Ranking uses binary search. Each insertion may copy the sorted vector, and legacy auctions may contain more than 32 bids. |
| Launch auction, `claim_refund` | Constant number of storage accesses for one bidder. |
| Sealed launch auction, `reveal` | At most 32 bids scanned. New commitments are limited to 256 bytes. |
| Sealed launch auction, `reveal_at` | One bid inspected by submission index. The stored vector is still read and written as a single entry. |
| Sealed launch auction, `settle` | At most 32 bids examined. |
| Sealed launch auction, `settle_page` | At most 16 bids examined. The stored vector is still read as a single entry. |
| Creator keys, governance snapshot cleanup | At most 32 snapshot IDs scanned and one expired snapshot removed. Snapshot creation caps its holder list at `MAX_SNAPSHOT_HOLDERS`. Cleanup deletes up to two records per holder. |
| Creator keys, `distribute_revenue` | The raw registry length is checked against `MAX_SNAPSHOT_HOLDERS` before reading any holder balances. Share calculation and storage each process at most the same number of holders. |
| Creator keys, `distribute_cycle` | At most `MAX_SNAPSHOT_HOLDERS` input addresses and the same number of distinct recipients. Duplicate detection can scan the distinct list once per input. |
| Creator keys, `distribute_dividend_claimable` | At most `MAX_SNAPSHOT_HOLDERS` input addresses. |
| Holder rewards, `create_airdrop` | At most 100 registry entries scanned, including entries with zero balances. Claims already operate on one holder's records. |

Auction settlement starts at cursor zero. Each successful call returns a cursor for the next page. The contract rejects stale cursors and opens the bonding curve or publishes the settlement event only after the final page. Cleanup uses a cursor stored per creator. Call `prune_snapshot_page` with the creator and an existing upper snapshot ID to continue cleanup independently of snapshot creation.

The workspace-wide inventory, the lint regression test, and network resource measurements are still outstanding. In particular, the legacy auction pages continue to load vector entries whose serialized sizes depend on historical state.
