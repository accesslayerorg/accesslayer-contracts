# Referral reward contract

`referral-contract` records an immutable direct referrer for each wallet and
walks that relationship up to three tiers. The wallet must authorize
`register_referral` and `on_swap_fee`.

`on_swap_fee(wallet, fee_amount)` treats `fee_amount` as the referral-fee
allocation (not the full swap amount). For each registered tier it accrues
40%, 20%, or 10% of that allocation to the referrer's balance, rounded down
to the smallest integer unit. The remaining allocation is not assigned by
this contract. `get_reward_balance(referrer)` reports the accumulated amount.

Each tier allocation emits a `ref_dist` event whose
`ReferralRewardDistributed` payload contains `tier`, `referrer`, and `amount`.
The event and accrued balance provide a stable settlement/indexing surface.
The current entrypoint does not accept an asset address, so it does not
transfer tokens; the caller's swap/settlement flow must handle payment in the
same units as `fee_amount`.

`get_referral_chain(wallet)` returns a Soroban vector ordered from direct
referrer outward, containing no more than three addresses. Self-referrals and
repeat registrations are rejected.
