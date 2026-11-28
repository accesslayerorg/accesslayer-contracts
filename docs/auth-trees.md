# Cross-contract authorization trees

This document lists every cross-contract call in the workspace contracts and
the authorization tree a signer is expected to produce for it. A tree is the
nested set of `(contract, function, args)` invocations a user (or a contract,
via `authorize_as_current_contract`) authorizes. A signature for one tree
cannot be reused for an invocation that is not in it, so every tree below is
kept to the minimum the flow needs.

Tests live in:

- `creator-keys/tests/auth_trees.rs`
- `multisig-admin/tests/auth_trees.rs`

They use strict `env.mock_auths(...)` with the exact tree below (not
`mock_all_auths`), so any extra `require_auth` makes the call fail, and they
compare `env.auths()` against the documented tree so an unexpected invocation
or argument also fails the test.

Notation: `signer: A.f(args) -> [B.g(args)]` means the signer authorizes the
root call `A.f` with exactly one nested invocation `B.g`. `[]` means no nested
invocations. "Invoker auth" means the calling contract authorizes the call
implicitly because it is the direct caller (Soroban's invoker rule); it never
appears in a user's tree.

## Flow mapping for issue #1318

| Issue flow | What exists in this repository |
|---|---|
| Buy | `CreatorKeysContract::buy_key` (and its aliases), `swap(.., Buy, ..)`, `buy_with` (multi-asset). Payment is an internal `i128` accounting value; no token contract is called. |
| Sell | `CreatorKeysContract::sell_key`, `batch_sell`, `swap(.., Sell, ..)`. Proceeds are internal accounting; no token contract is called. |
| Lending | **No lending or borrowing flow exists.** The only related code is the flash-loan guard (`assert_flash_loan_guard`), which is a same-contract ledger check with no cross-contract call. Nothing to document or test. |
| Escrow | `KeySwapContract` P2P escrow (`propose_swap` / `accept_swap` / `cancel_swap`), the creator performance bond (`deposit_performance_bond`, slash, release), and the deprecated-key buyback escrow (internal accounting only, no cross-contract call). |

## creator-keys

### Buy / sell

| Entry point | Cross-contract calls | Expected user tree | Tested |
|---|---|---|---|
| `buy_key(creator, buyer, payment, max_price)` | circuit-breaker reads are not on this path | `buyer: keys.buy_key(creator, buyer, payment, max_price) -> []` | `buy_key_tree_is_single_root_with_no_sub_invocations`, `buy_key_rejects_signature_for_different_arguments` |
| `sell_key(creator, seller, min_proceeds)` | none | `seller: keys.sell_key(creator, seller, min_proceeds) -> []` | `sell_key_tree_is_single_root_with_no_sub_invocations` |
| `swap(creator, trader, amount, direction, max_slippage_bps)` | `breaker.is_suspended(creator)` (read, no auth); `breaker.record_swap(keys, creator, amount, impact_bps)` | `trader: keys.swap(creator, trader, amount, direction, max_slippage_bps) -> []` | `swap_tree_excludes_breaker_self_authorization` (Buy direction; Sell routes through `batch_sell` with the same single-root trader tree) |
| `buy_with(buyer, key_id, amount, token, max_pay)` | oracle `try_invoke_contract` price read (no auth) | `buyer: keys.buy_with(..) -> []` | Covered by the same single-root pattern; the oracle read requires no auth |

**Contract self-authorization (`record_breaker_swap`).** `swap` calls
`env.authorize_as_current_contract` with exactly one entry:

```
keys (self): breaker.record_swap(keys, creator, amount, impact_bps) -> []
```

The entry names the function and every argument and has no
`sub_invocations`, so the breaker cannot spend creator-keys' authority on any
further call. Tested by:

- `malicious_breaker_cannot_extend_contract_self_authorization`: a breaker
  that tries `token.transfer(keys, attacker, amount)` inside `record_swap`
  makes the swap fail and moves no funds.
- `malicious_breaker_cannot_reuse_trader_signature`: a breaker that asks for
  the trader's authorization inside `record_swap` makes the swap fail.

### Escrow: P2P key swap (`KeySwapContract`)

| Entry point | Expected user tree | Contract-authorized calls |
|---|---|---|
| `propose_swap(proposer, offered_key, offered_amount, requested_key, requested_amount, counterparty)` | `proposer: swap.propose_swap(..) -> [offered_key.transfer(proposer, swap, offered_amount)]` | none |
| `accept_swap(caller, proposal_id)` | `counterparty: swap.accept_swap(counterparty, id) -> [requested_key.transfer(counterparty, proposer, requested_amount)]` | `offered_key.transfer(swap, counterparty, offered_amount)` by invoker auth |
| `cancel_swap(caller, proposal_id)` | `proposer: swap.cancel_swap(proposer, id) -> []` | `offered_key.transfer(swap, proposer, offered_amount)` by invoker auth |

Tested by `p2p_swap_escrow_trees_are_minimal`,
`p2p_swap_cancel_tree_has_no_sub_invocations`,
`malicious_offered_token_cannot_reuse_proposer_signature` (a token whose
`transfer` also tries to pull a second token out of the proposer's wallet) and
`exact_auth_assertion_detects_unexpected_nested_invocation` (shows that the
same malicious token produces a tree that differs from the documented one, so
the exact-tree assertion catches it).

### Escrow: performance bond

| Entry point | Expected tree |
|---|---|
| `deposit_performance_bond(key_id, amount)` | `key_id: keys.deposit_performance_bond(key_id, amount) -> [xlm.transfer(key_id, keys, amount)]` |
| Governance slash (`slash_after_governance`) | `xlm.transfer(keys, treasury, slash)` by invoker auth; no user tree |
| `finalize_deprecation` / release | `xlm.transfer(keys, key_id, amount)` by invoker auth; no user tree |

Tested by `performance_bond_deposit_tree_is_minimal` and
`performance_bond_deposit_rejects_unbound_token_signature` (a bare
`xlm.transfer` signature that is not rooted at `deposit_performance_bond`
cannot fund the bond).

### Key transfer callbacks

| Entry point | Cross-contract call | Expected user tree |
|---|---|---|
| `transfer_and_call(creator, from, to, amount, data)` | `to.on_key_received(from, creator, amount, data)` when `to` is a contract | `from: keys.transfer_and_call(creator, from, to, amount, data) -> []` |

The receiver callback needs no authorization. Tested by
`transfer_and_call_tree_excludes_receiver_callback` and
`malicious_receiver_cannot_reuse_sender_signature` (a receiver that tries
`token.transfer(from, attacker, ..)` inside the callback makes the whole
transfer fail). Re-entering `transfer_keys` from the callback is separately
blocked by the callback guard (`tests/transfer_and_call.rs`).

### Other creator-keys cross-contract calls (no user sub-invocations)

| Call site | Call | Authorization |
|---|---|---|
| `KeyDividendContract::declare_dividend` | `token.transfer(creator, dividend, amount)` | `creator: declare_dividend(..) -> [token.transfer(creator, dividend, amount)]` |
| `KeyDividendContract::claim_dividend` / `reclaim_expired_dividend` | `token.transfer(dividend, wallet, ..)` | invoker auth; user tree is the root call only |
| `PayoutStreamContract::fund_stream` | `token.transfer(payer, stream, amount)` | `payer: fund_stream(..) -> [token.transfer(payer, stream, amount)]` |
| `PayoutStreamContract::withdraw` | `token.transfer(stream, creator, ..)` | invoker auth |
| `token_rescue::rescue` | `token.transfer(keys, to, amount)` | invoker auth after admin `require_auth` |
| `multi_buy` | `token.balance(buyer)` | read, no auth |
| `badge_access_control` | `badge.*` reads | read, no auth |
| `OracleGuardContract::get_price`, `multi_asset_payment` oracle reads | price reads | read, no auth |
| `proxy` | `invoke_contract(implementation, ..)` | forwards the call; no `authorize_as_current_contract` |

These follow the same single-root or root-plus-one-transfer shape as the
tested escrow flows. They are listed so new reviewers can check them; they do
not yet have dedicated tree assertions.

## multisig-admin

`execute_action(executor, proposal_id)` is the only other
`authorize_as_current_contract` call site. It authorizes exactly one entry:

```
multisig (self): proposal.target.proposal.function(proposal.args) -> []
```

| Expected executor tree | Tested |
|---|---|
| `executor: multisig.execute_action(executor, proposal_id) -> []` | `execute_action_executor_tree_is_exactly_execute_action` |

Negative tests: `self_authorization_does_not_cover_nested_calls_from_target`
(a target that forwards the multisig's authority to a second privileged
contract fails) and `target_cannot_reuse_executor_signature` (a target that
asks for the executor's authorization fails).

## Other workspace contracts

| Contract | Cross-contract call | Authorization |
|---|---|---|
| `creator-keys-factory::deploy_key` | `key.register_creator(params, ..)` on the freshly deployed key | `creator: factory.deploy_key(creator, salt, config) -> [key.register_creator(..)]` |
| `dividend-contract::snapshot_holders` | `keys.get_holder_registry(key_id)` | read, no auth |

Contracts outside the Cargo workspace (`holder-rewards`,
`protocol-fee-contract`, `rating-contract`) are not built by CI. Their calls
(`holder-rewards` airdrop funding `token.transfer(caller, rewards, total)` and
payouts by invoker auth; `protocol-fee-contract` `receive_fee` callback;
`rating-contract` balance read) follow the same patterns.

## Findings

The tests found no over-broad authorization: both `authorize_as_current_contract`
call sites already name a single function with its full argument list and no
sub-invocations, and every user tree above contains only the invocations the
flow needs. No contract code was changed.

## Checklist for new cross-contract calls

See the "Cross-contract calls" checklist in [CONTRIBUTING.md](../CONTRIBUTING.md#cross-contract-call-checklist).
