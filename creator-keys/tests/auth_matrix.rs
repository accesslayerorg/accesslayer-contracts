//! Table-driven authorization matrix tests for every public `CreatorKeysContract` entrypoint.
//!
//! Each public entrypoint is classified by its expected authorized role (`Admin`, `Creator`,
//! `Holder`, `Governor`, `Anyone { reason }`) or as a read-only view (`READ_ONLY_ENTRYPOINTS`).
//! `creator-keys/build.rs` extracts every `pub fn` from `#[contractimpl] impl CreatorKeysContract`
//! at build time and fails compilation if any public function is missing from this matrix.
//!
//! For protected entrypoints, the harness verifies:
//! 1. Calling with no auth (`env.set_auths(&[])`) reverts.
//! 2. Calling with the wrong signer (`MockAuth` for an unauthorized address) reverts.
//! 3. Calling with the expected signer succeeds and `env.auths()` records the exact expected
//!    signer address, contract id, entrypoint symbol, and arguments.
//!
//! For intentionally permissionless (`Anyone`) entrypoints, the harness verifies that calling
//! with `env.set_auths(&[])` succeeds and `env.auths().is_empty()`.

mod contract_test_env;

use contract_test_env::register_creator_keys;
use creator_keys::{
    acl_roles::Role as AclRole, co_creators::CoCreatorRole, content_gating::GateType,
    price_alerts::AlertDirection, protocol_governance::ProtocolParameter,
    CreatorKeysContractClient, EventParams, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{
        Address as _, AuthorizedFunction, AuthorizedInvocation, Ledger as _, MockAuth,
        MockAuthInvoke,
    },
    vec, Address, BytesN, Env, IntoVal, String, Symbol, Val, Vec,
};
use std::collections::BTreeSet;

include!(concat!(env!("OUT_DIR"), "/contract_entrypoints.rs"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthorizedRole {
    Admin,
    Creator,
    Holder,
    Governor,
    Anyone { reason: &'static str },
}

#[derive(Clone, Copy, Debug)]
struct MatrixEntry {
    name: &'static str,
    role: AuthorizedRole,
}

// AUTH_MATRIX_BEGIN
const STATE_CHANGING_MATRIX: &[MatrixEntry] = &[
    MatrixEntry { name: "accept_transfer", role: AuthorizedRole::Creator },
    MatrixEntry { name: "accrue_lp_trading_fee", role: AuthorizedRole::Creator },
    MatrixEntry { name: "accumulate_fees", role: AuthorizedRole::Admin },
    MatrixEntry { name: "add_approved_caller", role: AuthorizedRole::Admin },
    MatrixEntry { name: "add_co_creator", role: AuthorizedRole::Creator },
    MatrixEntry { name: "add_launch_window_whitelist", role: AuthorizedRole::Creator },
    MatrixEntry { name: "add_liquidity", role: AuthorizedRole::Holder },
    MatrixEntry { name: "add_liquidity_for", role: AuthorizedRole::Holder },
    MatrixEntry { name: "add_payment_asset", role: AuthorizedRole::Admin },
    MatrixEntry { name: "add_to_whitelist", role: AuthorizedRole::Creator },
    MatrixEntry { name: "airdrop_keys", role: AuthorizedRole::Creator },
    MatrixEntry { name: "apply_governance_violation", role: AuthorizedRole::Admin },
    MatrixEntry { name: "apply_trade_volume", role: AuthorizedRole::Anyone { reason: "Permissionless rolling-volume fee tier accumulator updated on trade activity." } },
    MatrixEntry { name: "approve", role: AuthorizedRole::Creator },
    MatrixEntry { name: "approve_co_creator_action", role: AuthorizedRole::Creator },
    MatrixEntry { name: "approve_pause", role: AuthorizedRole::Holder },
    MatrixEntry { name: "approve_upgrade", role: AuthorizedRole::Holder },
    MatrixEntry { name: "archive_creator", role: AuthorizedRole::Creator },
    MatrixEntry { name: "atomic_swap", role: AuthorizedRole::Holder },
    MatrixEntry { name: "batch_buy", role: AuthorizedRole::Holder },
    MatrixEntry { name: "batch_buy_v2", role: AuthorizedRole::Holder },
    MatrixEntry { name: "batch_claim_dividend", role: AuthorizedRole::Holder },
    MatrixEntry { name: "batch_sell", role: AuthorizedRole::Holder },
    MatrixEntry { name: "batch_sell_v2", role: AuthorizedRole::Holder },
    MatrixEntry { name: "batch_trade", role: AuthorizedRole::Holder },
    MatrixEntry { name: "batch_transfer_keys", role: AuthorizedRole::Holder },
    MatrixEntry { name: "begin_restore", role: AuthorizedRole::Creator },
    MatrixEntry { name: "blacklist_wallet", role: AuthorizedRole::Admin },
    MatrixEntry { name: "bundle_buy", role: AuthorizedRole::Holder },
    MatrixEntry { name: "burn", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy_key", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy_key_with_ref_and_slip", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy_key_with_referrer", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy_key_with_slippage", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy_keys", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy_keys_with_ref_and_slip", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy_keys_with_referrer", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy_keys_with_slippage", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy_with", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buy_with_slippage", role: AuthorizedRole::Holder },
    MatrixEntry { name: "buyback", role: AuthorizedRole::Creator },
    MatrixEntry { name: "cancel_action", role: AuthorizedRole::Admin },
    MatrixEntry { name: "cancel_alert", role: AuthorizedRole::Holder },
    MatrixEntry { name: "cancel_auction", role: AuthorizedRole::Creator },
    MatrixEntry { name: "cancel_config_change", role: AuthorizedRole::Admin },
    MatrixEntry { name: "cancel_curve_update", role: AuthorizedRole::Creator },
    MatrixEntry { name: "cancel_rescue", role: AuthorizedRole::Admin },
    MatrixEntry { name: "cancel_transfer", role: AuthorizedRole::Creator },
    MatrixEntry { name: "cast_delegated_vote", role: AuthorizedRole::Creator },
    MatrixEntry { name: "cast_poll_vote", role: AuthorizedRole::Holder },
    MatrixEntry { name: "cast_proposal_vote", role: AuthorizedRole::Governor },
    MatrixEntry { name: "cast_vote", role: AuthorizedRole::Holder },
    MatrixEntry { name: "cast_vote_with_snapshot", role: AuthorizedRole::Holder },
    MatrixEntry { name: "check_and_apply_milestone", role: AuthorizedRole::Anyone { reason: "Permissionless keeper hook that applies reached supply fee milestones post-swap." } },
    MatrixEntry { name: "claim_cycle", role: AuthorizedRole::Holder },
    MatrixEntry { name: "claim_dividend", role: AuthorizedRole::Holder },
    MatrixEntry { name: "claim_dividend_claimable", role: AuthorizedRole::Holder },
    MatrixEntry { name: "claim_locked_allocation", role: AuthorizedRole::Creator },
    MatrixEntry { name: "claim_lp_rewards", role: AuthorizedRole::Holder },
    MatrixEntry { name: "claim_referral_rewards", role: AuthorizedRole::Holder },
    MatrixEntry { name: "claim_revenue_share", role: AuthorizedRole::Holder },
    MatrixEntry { name: "claim_royalties", role: AuthorizedRole::Creator },
    MatrixEntry { name: "claim_royalties_as_co_creator", role: AuthorizedRole::Creator },
    MatrixEntry { name: "claim_secondary_proceeds", role: AuthorizedRole::Holder },
    MatrixEntry { name: "claim_stake_reward", role: AuthorizedRole::Holder },
    MatrixEntry { name: "claim_vault_rewards", role: AuthorizedRole::Holder },
    MatrixEntry { name: "claim_vested", role: AuthorizedRole::Holder },
    MatrixEntry { name: "claim_vested_cliff", role: AuthorizedRole::Holder },
    MatrixEntry { name: "close_event", role: AuthorizedRole::Creator },
    MatrixEntry { name: "close_poll", role: AuthorizedRole::Anyone { reason: "Anyone may finalize a governance poll once its voting deadline has passed." } },
    MatrixEntry { name: "close_proposal", role: AuthorizedRole::Anyone { reason: "Alias for close_poll; anyone may finalize once the voting deadline has passed." } },
    MatrixEntry { name: "close_snapshot_poll", role: AuthorizedRole::Anyone { reason: "Anyone may finalize a snapshot poll once its closing ledger has passed." } },
    MatrixEntry { name: "compact", role: AuthorizedRole::Admin },
    MatrixEntry { name: "compact_supply", role: AuthorizedRole::Admin },
    MatrixEntry { name: "complete_restore", role: AuthorizedRole::Creator },
    MatrixEntry { name: "configure_auction", role: AuthorizedRole::Creator },
    MatrixEntry { name: "configure_graduated_curve", role: AuthorizedRole::Creator },
    MatrixEntry { name: "configure_protocol_governance", role: AuthorizedRole::Admin },
    MatrixEntry { name: "create_event", role: AuthorizedRole::Creator },
    MatrixEntry { name: "create_lockup", role: AuthorizedRole::Holder },
    MatrixEntry { name: "create_poll", role: AuthorizedRole::Creator },
    MatrixEntry { name: "create_proposal", role: AuthorizedRole::Governor },
    MatrixEntry { name: "create_snapshot_poll", role: AuthorizedRole::Creator },
    MatrixEntry { name: "create_tier", role: AuthorizedRole::Creator },
    MatrixEntry { name: "create_vesting", role: AuthorizedRole::Creator },
    MatrixEntry { name: "create_vesting_cliff", role: AuthorizedRole::Creator },
    MatrixEntry { name: "declare_dividend", role: AuthorizedRole::Creator },
    MatrixEntry { name: "delegate", role: AuthorizedRole::Creator },
    MatrixEntry { name: "delegate_votes", role: AuthorizedRole::Holder },
    MatrixEntry { name: "deposit_performance_bond", role: AuthorizedRole::Creator },
    MatrixEntry { name: "deprecate", role: AuthorizedRole::Creator },
    MatrixEntry { name: "deprecate_key", role: AuthorizedRole::Creator },
    MatrixEntry { name: "disable_whitelist", role: AuthorizedRole::Creator },
    MatrixEntry { name: "distribute_cycle", role: AuthorizedRole::Admin },
    MatrixEntry { name: "distribute_dividend", role: AuthorizedRole::Creator },
    MatrixEntry { name: "distribute_dividend_claimable", role: AuthorizedRole::Creator },
    MatrixEntry { name: "distribute_protocol_revenue", role: AuthorizedRole::Admin },
    MatrixEntry { name: "distribute_revenue", role: AuthorizedRole::Creator },
    MatrixEntry { name: "distribute_vault_rewards", role: AuthorizedRole::Holder },
    MatrixEntry { name: "early_unstake", role: AuthorizedRole::Holder },
    MatrixEntry { name: "early_unstake_position", role: AuthorizedRole::Holder },
    MatrixEntry { name: "emergency_unlock", role: AuthorizedRole::Admin },
    MatrixEntry { name: "enable_whitelist", role: AuthorizedRole::Creator },
    MatrixEntry { name: "escape_redeem", role: AuthorizedRole::Holder },
    MatrixEntry { name: "evaluate_poll_escalation", role: AuthorizedRole::Anyone { reason: "Permissionless keeper hook that extends a poll when participation is below quorum near expiry." } },
    MatrixEntry { name: "execute_action", role: AuthorizedRole::Admin },
    MatrixEntry { name: "execute_config_change", role: AuthorizedRole::Admin },
    MatrixEntry { name: "execute_curve_update", role: AuthorizedRole::Anyone { reason: "Anyone may apply a queued curve parameter update once its 48-hour notice period has elapsed." } },
    MatrixEntry { name: "execute_deprecation", role: AuthorizedRole::Admin },
    MatrixEntry { name: "execute_merge", role: AuthorizedRole::Anyone { reason: "Anyone may execute an approved key merge proposal once its timelock has elapsed." } },
    MatrixEntry { name: "execute_proposal", role: AuthorizedRole::Anyone { reason: "Anyone may execute a passed protocol governance proposal once its timelock has elapsed." } },
    MatrixEntry { name: "execute_retirement", role: AuthorizedRole::Admin },
    MatrixEntry { name: "execute_split", role: AuthorizedRole::Anyone { reason: "Anyone may execute an approved key split proposal once its timelock has elapsed." } },
    MatrixEntry { name: "execute_transfer", role: AuthorizedRole::Anyone { reason: "Anyone may execute an accepted key ownership transfer once its timelock has elapsed." } },
    MatrixEntry { name: "exit_deprecating", role: AuthorizedRole::Creator },
    MatrixEntry { name: "finalize_deprecation", role: AuthorizedRole::Anyone { reason: "Anyone may finalize deprecation and release remaining bond once the notice window has elapsed." } },
    MatrixEntry { name: "finalize_merge", role: AuthorizedRole::Anyone { reason: "Anyone may finalize a creator-approved key merge once its timelock has elapsed." } },
    MatrixEntry { name: "finalize_migration_deprecation", role: AuthorizedRole::Anyone { reason: "Anyone may finalize key deprecation once the migration exit window has expired." } },
    MatrixEntry { name: "forward_buy", role: AuthorizedRole::Holder },
    MatrixEntry { name: "freeze", role: AuthorizedRole::Admin },
    MatrixEntry { name: "freeze_key", role: AuthorizedRole::Admin },
    MatrixEntry { name: "freeze_position", role: AuthorizedRole::Admin },
    MatrixEntry { name: "fund_escape_reserve", role: AuthorizedRole::Admin },
    MatrixEntry { name: "fund_retirement_buyback", role: AuthorizedRole::Admin },
    MatrixEntry { name: "get_price", role: AuthorizedRole::Holder },
    MatrixEntry { name: "get_twap_price", role: AuthorizedRole::Holder },
    MatrixEntry { name: "global_pause", role: AuthorizedRole::Holder },
    MatrixEntry { name: "global_resume", role: AuthorizedRole::Holder },
    MatrixEntry { name: "grant_role", role: AuthorizedRole::Creator },
    MatrixEntry { name: "initialise_key", role: AuthorizedRole::Creator },
    MatrixEntry { name: "initialize", role: AuthorizedRole::Admin },
    MatrixEntry { name: "merge", role: AuthorizedRole::Creator },
    MatrixEntry { name: "migrate_curve", role: AuthorizedRole::Admin },
    MatrixEntry { name: "multi_buy", role: AuthorizedRole::Holder },
    MatrixEntry { name: "pause", role: AuthorizedRole::Admin },
    MatrixEntry { name: "pause_all", role: AuthorizedRole::Admin },
    MatrixEntry { name: "pause_platform", role: AuthorizedRole::Admin },
    MatrixEntry { name: "pause_with_expiry", role: AuthorizedRole::Holder },
    MatrixEntry { name: "post_announcement", role: AuthorizedRole::Creator },
    MatrixEntry { name: "post_announcement_as_co_creator", role: AuthorizedRole::Creator },
    MatrixEntry { name: "propose_action", role: AuthorizedRole::Admin },
    MatrixEntry { name: "propose_config_change", role: AuthorizedRole::Admin },
    MatrixEntry { name: "propose_curve_update", role: AuthorizedRole::Creator },
    MatrixEntry { name: "propose_deprecation", role: AuthorizedRole::Admin },
    MatrixEntry { name: "propose_merge", role: AuthorizedRole::Creator },
    MatrixEntry { name: "propose_pause", role: AuthorizedRole::Holder },
    MatrixEntry { name: "propose_retirement", role: AuthorizedRole::Admin },
    MatrixEntry { name: "propose_split", role: AuthorizedRole::Creator },
    MatrixEntry { name: "propose_transfer", role: AuthorizedRole::Creator },
    MatrixEntry { name: "queue_platform_resume", role: AuthorizedRole::Admin },
    MatrixEntry { name: "queue_rescue", role: AuthorizedRole::Admin },
    MatrixEntry { name: "rate_key", role: AuthorizedRole::Holder },
    MatrixEntry { name: "redeem", role: AuthorizedRole::Holder },
    MatrixEntry { name: "redeem_event", role: AuthorizedRole::Holder },
    MatrixEntry { name: "redeem_payment_asset", role: AuthorizedRole::Admin },
    MatrixEntry { name: "refresh_ttl", role: AuthorizedRole::Admin },
    MatrixEntry { name: "register_alert", role: AuthorizedRole::Holder },
    MatrixEntry { name: "register_creator", role: AuthorizedRole::Creator },
    MatrixEntry { name: "register_key", role: AuthorizedRole::Admin },
    MatrixEntry { name: "register_referral", role: AuthorizedRole::Holder },
    MatrixEntry { name: "reinvest_dividend", role: AuthorizedRole::Creator },
    MatrixEntry { name: "remove_approved_caller", role: AuthorizedRole::Admin },
    MatrixEntry { name: "remove_co_creator", role: AuthorizedRole::Creator },
    MatrixEntry { name: "remove_co_creator_role", role: AuthorizedRole::Creator },
    MatrixEntry { name: "remove_from_blacklist", role: AuthorizedRole::Admin },
    MatrixEntry { name: "remove_from_whitelist", role: AuthorizedRole::Creator },
    MatrixEntry { name: "remove_launch_window_whitelist", role: AuthorizedRole::Creator },
    MatrixEntry { name: "remove_liquidity", role: AuthorizedRole::Holder },
    MatrixEntry { name: "remove_payment_asset", role: AuthorizedRole::Admin },
    MatrixEntry { name: "rescue", role: AuthorizedRole::Admin },
    MatrixEntry { name: "resume_all", role: AuthorizedRole::Admin },
    MatrixEntry { name: "resume_platform", role: AuthorizedRole::Admin },
    MatrixEntry { name: "revoke_delegate", role: AuthorizedRole::Creator },
    MatrixEntry { name: "revoke_delegation", role: AuthorizedRole::Holder },
    MatrixEntry { name: "revoke_role", role: AuthorizedRole::Creator },
    MatrixEntry { name: "rotate_attester", role: AuthorizedRole::Admin },
    MatrixEntry { name: "secondary_market_buy", role: AuthorizedRole::Holder },
    MatrixEntry { name: "self_freeze", role: AuthorizedRole::Admin },
    MatrixEntry { name: "self_unfreeze", role: AuthorizedRole::Admin },
    MatrixEntry { name: "sell", role: AuthorizedRole::Holder },
    MatrixEntry { name: "sell_key", role: AuthorizedRole::Holder },
    MatrixEntry { name: "sell_key_with_slippage", role: AuthorizedRole::Holder },
    MatrixEntry { name: "sell_with_slippage", role: AuthorizedRole::Holder },
    MatrixEntry { name: "send_tip", role: AuthorizedRole::Holder },
    MatrixEntry { name: "set_attester", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_buy_cooldown", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_buyback_pool_address", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_circuit_breaker", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_circuit_breaker_threshold", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_co_creator", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_co_creator_threshold", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_cooldown", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_curve_slope", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_curve_slope_milestones", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_discount_tiers", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_distribution_cycle_length", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_early_access_mode", role: AuthorizedRole::Holder },
    MatrixEntry { name: "set_early_exit_penalty", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_escalation_config", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_fee_config", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_fee_router", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_fee_tiers", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_feed_price", role: AuthorizedRole::Holder },
    MatrixEntry { name: "set_flash_loan_guard_ledgers", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_gate", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_global_deadline", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_global_pause_admins", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_governance_address", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_graduated_curve", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_holder_cap", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_holding_cap", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_holding_cap_bound", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_key_pause_override", role: AuthorizedRole::Holder },
    MatrixEntry { name: "set_key_price", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_launch_penalty", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_launch_window", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_launch_window_bypass_enabled", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_leaderboard_config", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_lockup_duration", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_lp_allocation_bps", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_lp_contract_address", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_max_buy_quantity", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_milestones", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_min_hold_for_access", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_min_performance_bond", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_multisig_admins", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_oracle_address", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_oracle_price", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_oracle_set", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_oracle_staleness_threshold", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_payment_asset_rate", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_payment_asset_staleness", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_price_ceiling", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_price_floor", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_price_retention", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_protocol_admin", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_protocol_fee", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_protocol_fee_recipient", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_quorum_bps", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_referral_fee_bps", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_rescue_threshold", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_retention_policy", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_reward_rate", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_royalty", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_royalty_by_role", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_royalty_rate", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_sell_tax_bps", role: AuthorizedRole::Holder },
    MatrixEntry { name: "set_snapshot_retention", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_spread_bps", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_staking_multiplier_tiers", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_supply_cap", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_supply_milestones", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_timelock_delay", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_trade_limit", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_treasury_address", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_trusted_forwarder", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_twap_buffer_size", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_volume_brackets", role: AuthorizedRole::Admin },
    MatrixEntry { name: "set_whitelist", role: AuthorizedRole::Creator },
    MatrixEntry { name: "set_xlm_token_address", role: AuthorizedRole::Admin },
    MatrixEntry { name: "stake_extend", role: AuthorizedRole::Holder },
    MatrixEntry { name: "stake_keys", role: AuthorizedRole::Holder },
    MatrixEntry { name: "stake_keys_locked", role: AuthorizedRole::Holder },
    MatrixEntry { name: "submit_rating", role: AuthorizedRole::Holder },
    MatrixEntry { name: "subscribe", role: AuthorizedRole::Holder },
    MatrixEntry { name: "subscribe_access", role: AuthorizedRole::Holder },
    MatrixEntry { name: "swap", role: AuthorizedRole::Holder },
    MatrixEntry { name: "take_leaderboard_snapshot", role: AuthorizedRole::Governor },
    MatrixEntry { name: "take_snapshot", role: AuthorizedRole::Admin },
    MatrixEntry { name: "take_snapshot_governance", role: AuthorizedRole::Governor },
    MatrixEntry { name: "topup_reward_pool", role: AuthorizedRole::Holder },
    MatrixEntry { name: "transfer_and_call", role: AuthorizedRole::Holder },
    MatrixEntry { name: "transfer_from", role: AuthorizedRole::Holder },
    MatrixEntry { name: "transfer_keys", role: AuthorizedRole::Holder },
    MatrixEntry { name: "unfreeze", role: AuthorizedRole::Admin },
    MatrixEntry { name: "unfreeze_key", role: AuthorizedRole::Admin },
    MatrixEntry { name: "unfreeze_position", role: AuthorizedRole::Admin },
    MatrixEntry { name: "unpause", role: AuthorizedRole::Admin },
    MatrixEntry { name: "unstake_keys", role: AuthorizedRole::Holder },
    MatrixEntry { name: "update_config", role: AuthorizedRole::Admin },
    MatrixEntry { name: "update_creator_fee_recipient", role: AuthorizedRole::Creator },
    MatrixEntry { name: "update_holder_cap", role: AuthorizedRole::Creator },
    MatrixEntry { name: "update_metadata", role: AuthorizedRole::Creator },
    MatrixEntry { name: "update_protocol_fee_recipient", role: AuthorizedRole::Admin },
    MatrixEntry { name: "update_whitelist", role: AuthorizedRole::Creator },
    MatrixEntry { name: "upgrade", role: AuthorizedRole::Admin },
    MatrixEntry { name: "vault_deposit", role: AuthorizedRole::Holder },
    MatrixEntry { name: "vault_withdraw", role: AuthorizedRole::Holder },
    MatrixEntry { name: "vote_merge", role: AuthorizedRole::Holder },
    MatrixEntry { name: "withdraw_treasury", role: AuthorizedRole::Admin },
];

const READ_ONLY_ENTRYPOINTS: &[&str] = &[
    "aggregate_price",
    "attestation_payload",
    "balance_at",
    "can_co_creator_perform",
    "ceiling_status",
    "check_access",
    "check_lockup",
    "check_schema_version",
    "compute_fees_for_payment",
    "current_curve_slope_milestone",
    "escape_enabled",
    "from_internal",
    "get_action",
    "get_active_fee_tier",
    "get_alerts",
    "get_allowance",
    "get_analytics",
    "get_announcements",
    "get_attester",
    "get_auction_config",
    "get_balance",
    "get_bid_ask_price",
    "get_bond_status",
    "get_bundle_quote",
    "get_buy_cooldown",
    "get_buy_quote",
    "get_buyback_pool_balance",
    "get_buyback_quote",
    "get_circuit_breaker",
    "get_claimable_dividend",
    "get_co_creator",
    "get_co_creator_approval_count",
    "get_co_creator_fee_balance",
    "get_co_creator_role",
    "get_co_creator_threshold",
    "get_co_creators",
    "get_cooldown_remaining",
    "get_cooldown_status",
    "get_created_at_ledger",
    "get_creator",
    "get_creator_details",
    "get_creator_fee_balance",
    "get_creator_fee_bps",
    "get_creator_fee_config",
    "get_creator_fee_recipient",
    "get_creator_holder_count",
    "get_creator_lifecycle",
    "get_creator_supply",
    "get_creator_treasury_share",
    "get_creators_batch",
    "get_cumulative_window_purchases",
    "get_current_fee",
    "get_current_fee_bps",
    "get_current_milestone",
    "get_curve_exponent",
    "get_curve_preset",
    "get_curve_slope",
    "get_delegate",
    "get_deprecation_proposal",
    "get_deprecation_status",
    "get_dynamic_fee_view",
    "get_early_exit_penalty_bps",
    "get_escalation_config",
    "get_escalation_status",
    "get_escape_reserve",
    "get_fee_config",
    "get_fee_discount",
    "get_fee_router",
    "get_flash_loan_guard_ledgers",
    "get_freeze_log",
    "get_gate_config",
    "get_global_deadline",
    "get_global_pause_admins",
    "get_global_trading_paused",
    "get_governance_address",
    "get_graduated_curve",
    "get_graduated_exponent",
    "get_holder_cap",
    "get_holder_count",
    "get_holder_key_count",
    "get_holder_registry",
    "get_holding_cap",
    "get_is_paused",
    "get_key_balance",
    "get_key_decimals",
    "get_key_market_stats",
    "get_key_metadata",
    "get_key_name",
    "get_key_owner",
    "get_key_rating",
    "get_key_rolling_volume",
    "get_key_stats",
    "get_key_status",
    "get_key_symbol",
    "get_launch_penalty_bps",
    "get_launch_window_cap",
    "get_leaderboard",
    "get_leaderboard_config",
    "get_leaderboard_ledgers",
    "get_liquid_balance",
    "get_locked_allocation",
    "get_lockup_duration",
    "get_logic_address",
    "get_lp_pool_rewards",
    "get_lp_position",
    "get_lp_rewards",
    "get_lp_total_liquidity",
    "get_max_buy_quantity",
    "get_max_supply",
    "get_merge_proposal",
    "get_merge_status",
    "get_metadata",
    "get_min_hold_for_access",
    "get_multi_buy_budget",
    "get_multisig_admins",
    "get_nonce",
    "get_oracle_address",
    "get_oracle_price",
    "get_pause_state",
    "get_payment_asset_rate",
    "get_payment_asset_reserve",
    "get_payment_base_reserve",
    "get_platform_pause",
    "get_platform_resume_eta",
    "get_poll_result",
    "get_poll_results",
    "get_position_frozen",
    "get_previous_wasm",
    "get_price_at_supply",
    "get_price_floor",
    "get_price_impact",
    "get_price_retention",
    "get_price_snapshot_count",
    "get_protocol_admin",
    "get_protocol_fee_bps",
    "get_protocol_fee_recipient",
    "get_protocol_fee_view",
    "get_protocol_proposal",
    "get_protocol_recipient_balance",
    "get_protocol_state_version",
    "get_protocol_status",
    "get_protocol_trade_fee",
    "get_protocol_treasury_share_bps",
    "get_quorum_bps",
    "get_referral_earnings",
    "get_referrer",
    "get_reputation",
    "get_rescue_threshold",
    "get_retention_policy",
    "get_retirement_pool_balance",
    "get_retirement_proposal",
    "get_revenue_pool",
    "get_revenue_share",
    "get_reward_pool_balance",
    "get_reward_rate",
    "get_royalty_config",
    "get_royalty_rate",
    "get_self_frozen_balance",
    "get_sell_quote",
    "get_sell_tax_bps",
    "get_snapshot_balance",
    "get_snapshot_meta",
    "get_snapshot_retention",
    "get_split_proposal",
    "get_spread_bps",
    "get_stake_unlock_ledger",
    "get_staked_balance",
    "get_staking_multiplier_tiers",
    "get_staking_position",
    "get_staking_rewards_pool",
    "get_staking_weight",
    "get_subscription",
    "get_supply",
    "get_supply_info",
    "get_supply_milestones",
    "get_tier",
    "get_tier_subscription",
    "get_timelock_delay",
    "get_timelock_proposal",
    "get_tip_history",
    "get_total_key_supply",
    "get_total_staked",
    "get_total_tips",
    "get_trade_limits",
    "get_transfer_proposal",
    "get_treasury_address",
    "get_treasury_balance",
    "get_trusted_forwarder",
    "get_twap",
    "get_twap_buffer_size",
    "get_twap_seconds",
    "get_twap_snapshots",
    "get_unclaimed_dividend",
    "get_unique_trader_count",
    "get_upgrade_approvals",
    "get_upgrade_target",
    "get_vault_pending_rewards",
    "get_vault_share",
    "get_vault_total_shares",
    "get_version",
    "get_vesting_info",
    "get_vesting_schedule",
    "get_volume",
    "get_volume_brackets",
    "get_vote_delegate",
    "get_vote_delegators",
    "get_vote_snapshot",
    "get_voting_weight",
    "get_wallet_rolling_volume",
    "get_wallet_whitelist_status",
    "get_weighted_reputation",
    "get_whitelist_status",
    "has_gated_access",
    "has_role",
    "has_traded",
    "is_approved_caller",
    "is_auction_pending",
    "is_creator_registered",
    "is_key_frozen",
    "is_key_paused",
    "is_paused",
    "is_protocol_config_initialized",
    "is_subscribed",
    "is_wallet_blacklisted",
    "is_whitelisted",
    "multi_buy_creator_supply",
    "oracle_report",
    "payment_assets",
    "query_price",
    "redeemed_count",
    "redemption_of",
    "rescuable",
    "simulate_buy",
    "simulate_sell",
    "supply_at",
    "to_internal",
    "tracked_balance",
    "verify_attestation",
];
// AUTH_MATRIX_END

// ── Matrix completeness & invariants ─────────────────────────────────────────

#[test]
fn test_matrix_covers_every_public_entrypoint() {
    let mut seen = BTreeSet::new();
    let mut duplicates = std::vec::Vec::new();

    for entry in STATE_CHANGING_MATRIX {
        if !seen.insert(entry.name) {
            duplicates.push(entry.name);
        }
        if let AuthorizedRole::Anyone { reason } = entry.role {
            assert!(
                !reason.trim().is_empty(),
                "permissionless entrypoint `{}` must document a non-empty reason",
                entry.name
            );
        }
    }

    for &name in READ_ONLY_ENTRYPOINTS {
        if !seen.insert(name) {
            duplicates.push(name);
        }
    }

    assert!(
        duplicates.is_empty(),
        "duplicate entrypoint(s) in auth matrix: {:?}",
        duplicates
    );

    let actual: BTreeSet<&str> = CONTRACT_ENTRYPOINTS.iter().copied().collect();
    let missing: std::vec::Vec<&str> = actual.difference(&seen).copied().collect();
    let stale: std::vec::Vec<&str> = seen.difference(&actual).copied().collect();

    assert!(
        missing.is_empty(),
        "public entrypoint(s) missing from auth matrix: {:?}",
        missing
    );
    assert!(
        stale.is_empty(),
        "stale entrypoint(s) in auth matrix not found on CreatorKeysContract: {:?}",
        stale
    );
}

// ── Shared fixture & assertion helpers ───────────────────────────────────────

struct MatrixFixture<'a> {
    env: &'a Env,
    contract_id: Address,
    client: CreatorKeysContractClient<'a>,
    admin: Address,
    creator: Address,
    holder: Address,
    governor: Address,
    stranger: Address,
}

fn setup_fixture(env: &Env) -> MatrixFixture<'_> {
    env.mock_all_auths();
    env.ledger().set_sequence_number(100);
    env.ledger().set_timestamp(10_000);

    let (client, contract_id) = register_creator_keys(env);
    let admin = Address::generate(env);
    let creator = Address::generate(env);
    let holder = Address::generate(env);
    let governor = Address::generate(env);
    let stranger = Address::generate(env);

    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &100i128);
    client.set_fee_config(&admin, &9_000u32, &1_000u32);
    client.set_treasury_address(&admin, &admin);

    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "matrix_creator"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Give holder and governor keys so holder/governance entrypoints have valid state.
    client.buy_keys(&creator, &holder, &5u32, &500i128, &None);
    client.buy_keys(&creator, &governor, &5u32, &500i128, &None);

    MatrixFixture {
        env,
        contract_id,
        client,
        admin,
        creator,
        holder,
        governor,
        stranger,
    }
}

fn assert_auth_recorded(
    env: &Env,
    contract_id: &Address,
    expected_signer: &Address,
    fn_name: &str,
    expected_args: &Vec<Val>,
) {
    let auths = env.auths();
    let expected_fn = AuthorizedFunction::Contract((
        contract_id.clone(),
        Symbol::new(env, fn_name),
        expected_args.clone(),
    ));
    let matched = auths
        .iter()
        .any(|(addr, inv): &(Address, AuthorizedInvocation)| {
            addr == expected_signer && inv.function == expected_fn
        });
    assert!(
        matched,
        "expected env.auths() to contain ({:?}, {:?}) for `{}`, got: {:?}",
        expected_signer, expected_fn, fn_name, auths
    );
}

/// Runs the 3-step authorization matrix check for a single protected entrypoint:
/// 1. No auth (`env.set_auths(&[])`) -> must revert.
/// 2. Wrong signer (`MockAuth` for `wrong_signer`) -> must revert.
/// 3. Correct signer (`MockAuth` for `expected_signer`) -> must succeed and match `env.auths()`.
fn verify_protected_case<F>(
    fn_name: &str,
    expected_role: AuthorizedRole,
    setup_and_args: impl Fn(&MatrixFixture<'_>) -> Vec<Val>,
    invoke: F,
) where
    F: Fn(&MatrixFixture<'_>) -> bool,
{
    assert!(
        STATE_CHANGING_MATRIX
            .iter()
            .any(|e| e.name == fn_name && e.role == expected_role),
        "`{fn_name}` must be listed in STATE_CHANGING_MATRIX with role {:?}",
        expected_role
    );

    // 1. No auth -> must revert
    {
        let env = Env::default();
        let fx = setup_fixture(&env);
        let _args = setup_and_args(&fx);
        env.set_auths(&[]);
        assert!(
            !invoke(&fx),
            "`{fn_name}` must revert when called with no auth (env.set_auths(&[]))"
        );
    }

    // 2. Wrong signer -> must revert
    {
        let env = Env::default();
        let fx = setup_fixture(&env);
        let args = setup_and_args(&fx);
        env.mock_auths(&[MockAuth {
            address: &fx.stranger,
            invoke: &MockAuthInvoke {
                contract: &fx.contract_id,
                fn_name,
                args,
                sub_invokes: &[],
            },
        }]);
        assert!(
            !invoke(&fx),
            "`{fn_name}` must revert when authorized only by wrong signer"
        );
    }

    // 3. Correct signer -> must succeed and record expected address & arguments in env.auths()
    {
        let env = Env::default();
        let fx = setup_fixture(&env);
        let args = setup_and_args(&fx);
        let expected_signer = match expected_role {
            AuthorizedRole::Admin => fx.admin.clone(),
            AuthorizedRole::Creator => fx.creator.clone(),
            AuthorizedRole::Holder => fx.holder.clone(),
            AuthorizedRole::Governor => fx.governor.clone(),
            AuthorizedRole::Anyone { .. } => {
                panic!("verify_protected_case called with AuthorizedRole::Anyone for `{fn_name}`")
            }
        };
        env.mock_auths(&[MockAuth {
            address: &expected_signer,
            invoke: &MockAuthInvoke {
                contract: &fx.contract_id,
                fn_name,
                args: args.clone(),
                sub_invokes: &[],
            },
        }]);
        assert!(
            invoke(&fx),
            "`{fn_name}` must succeed when authorized by expected signer {:?}",
            expected_role
        );
        assert_auth_recorded(&env, &fx.contract_id, &expected_signer, fn_name, &args);
    }
}

/// Verifies an intentionally permissionless (`Anyone`) entrypoint succeeds with
/// `env.set_auths(&[])` and requires no authorization in `env.auths()`.
fn verify_permissionless_case(
    fn_name: &str,
    setup_state: impl Fn(&MatrixFixture<'_>),
    invoke: impl Fn(&MatrixFixture<'_>) -> bool,
) {
    assert!(
        STATE_CHANGING_MATRIX
            .iter()
            .any(|e| e.name == fn_name && matches!(e.role, AuthorizedRole::Anyone { .. })),
        "`{fn_name}` must be listed in STATE_CHANGING_MATRIX with AuthorizedRole::Anyone"
    );

    let env = Env::default();
    let fx = setup_fixture(&env);
    setup_state(&fx);
    env.set_auths(&[]);
    assert!(
        invoke(&fx),
        "permissionless entrypoint `{fn_name}` must succeed with env.set_auths(&[])"
    );
    assert!(
        env.auths().is_empty(),
        "permissionless entrypoint `{fn_name}` must not record any auths, got: {:?}",
        env.auths()
    );
}

// ── Admin role matrix tests ──────────────────────────────────────────────────

#[test]
fn test_admin_role_auth_matrix() {
    // set_trusted_forwarder (regression: previously missing admin.require_auth())
    verify_protected_case(
        "set_trusted_forwarder",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), fx.stranger.clone()).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_trusted_forwarder(&fx.admin, &fx.stranger)
                .is_ok()
        },
    );

    // set_key_price
    verify_protected_case(
        "set_key_price",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), 250i128).into_val(fx.env),
        |fx| fx.client.try_set_key_price(&fx.admin, &250i128).is_ok(),
    );

    // set_fee_config
    verify_protected_case(
        "set_fee_config",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), 8_500u32, 1_500u32).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_fee_config(&fx.admin, &8_500u32, &1_500u32)
                .is_ok()
        },
    );

    // set_treasury_address
    verify_protected_case(
        "set_treasury_address",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), fx.stranger.clone()).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_treasury_address(&fx.admin, &fx.stranger)
                .is_ok()
        },
    );

    // set_protocol_admin
    verify_protected_case(
        "set_protocol_admin",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), fx.stranger.clone()).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_protocol_admin(&fx.admin, &fx.stranger)
                .is_ok()
        },
    );

    // pause
    verify_protected_case(
        "pause",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(),).into_val(fx.env),
        |fx| fx.client.try_pause(&fx.admin).is_ok(),
    );

    // unpause
    verify_protected_case(
        "unpause",
        AuthorizedRole::Admin,
        |fx| {
            fx.client.pause(&fx.admin);
            (fx.admin.clone(),).into_val(fx.env)
        },
        |fx| fx.client.try_unpause(&fx.admin).is_ok(),
    );

    // pause_all
    verify_protected_case(
        "pause_all",
        AuthorizedRole::Admin,
        |fx| {
            (
                fx.admin.clone(),
                String::from_str(fx.env, "maintenance"),
                3_600u64,
            )
                .into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_pause_all(
                    &fx.admin,
                    &String::from_str(fx.env, "maintenance"),
                    &3_600u64,
                )
                .is_ok()
        },
    );

    // set_governance_address
    verify_protected_case(
        "set_governance_address",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), fx.governor.clone()).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_governance_address(&fx.admin, &fx.governor)
                .is_ok()
        },
    );

    // set_min_performance_bond
    verify_protected_case(
        "set_min_performance_bond",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), 1_000i128).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_min_performance_bond(&fx.admin, &1_000i128)
                .is_ok()
        },
    );

    // set_rescue_threshold
    verify_protected_case(
        "set_rescue_threshold",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), fx.stranger.clone(), 5_000i128).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_rescue_threshold(&fx.admin, &fx.stranger, &5_000i128)
                .is_ok()
        },
    );

    // set_attester
    verify_protected_case(
        "set_attester",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), BytesN::from_array(fx.env, &[7u8; 32])).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_attester(&fx.admin, &BytesN::from_array(fx.env, &[7u8; 32]))
                .is_ok()
        },
    );

    // compact
    verify_protected_case(
        "compact",
        AuthorizedRole::Admin,
        |fx| {
            (
                fx.admin.clone(),
                fx.holder.clone(),
                fx.creator.clone(),
                50u32,
            )
                .into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_compact(&fx.admin, &fx.holder, &fx.creator, &50u32)
                .is_ok()
        },
    );

    // compact_supply
    verify_protected_case(
        "compact_supply",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), fx.creator.clone(), 50u32).into_val(fx.env),
        |fx| {
            fx.client
                .try_compact_supply(&fx.admin, &fx.creator, &50u32)
                .is_ok()
        },
    );

    // freeze
    verify_protected_case(
        "freeze",
        AuthorizedRole::Admin,
        |fx| {
            (
                fx.creator.clone(),
                String::from_str(fx.env, "audit"),
                fx.admin.clone(),
            )
                .into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_freeze(&fx.creator, &String::from_str(fx.env, "audit"), &fx.admin)
                .is_ok()
        },
    );

    // unfreeze
    verify_protected_case(
        "unfreeze",
        AuthorizedRole::Admin,
        |fx| {
            fx.client
                .freeze(&fx.creator, &String::from_str(fx.env, "audit"), &fx.admin);
            (fx.creator.clone(), fx.admin.clone()).into_val(fx.env)
        },
        |fx| fx.client.try_unfreeze(&fx.creator, &fx.admin).is_ok(),
    );

    // configure_protocol_governance
    verify_protected_case(
        "configure_protocol_governance",
        AuthorizedRole::Admin,
        |fx| (fx.admin.clone(), fx.creator.clone(), 1u32).into_val(fx.env),
        |fx| {
            fx.client
                .try_configure_protocol_governance(&fx.admin, &fx.creator, &1u32)
                .is_ok()
        },
    );
}

// ── Creator role matrix tests ────────────────────────────────────────────────

#[test]
fn test_creator_role_auth_matrix() {
    // accrue_lp_trading_fee (regression: previously missing key_id.require_auth())
    verify_protected_case(
        "accrue_lp_trading_fee",
        AuthorizedRole::Creator,
        |fx| {
            fx.client
                .add_liquidity(&fx.creator, &fx.holder, &10_000i128);
            (fx.creator.clone(), 500i128).into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_accrue_lp_trading_fee(&fx.creator, &500i128)
                .is_ok()
        },
    );

    // set_gate
    verify_protected_case(
        "set_gate",
        AuthorizedRole::Creator,
        |fx| (fx.creator.clone(), GateType::Threshold, 2u32).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_gate(&fx.creator, &GateType::Threshold, &2u32)
                .is_ok()
        },
    );

    // grant_role
    verify_protected_case(
        "grant_role",
        AuthorizedRole::Creator,
        |fx| (fx.creator.clone(), fx.holder.clone(), AclRole::Moderator).into_val(fx.env),
        |fx| {
            fx.client
                .try_grant_role(&fx.creator, &fx.holder, &AclRole::Moderator)
                .is_ok()
        },
    );

    // revoke_role
    verify_protected_case(
        "revoke_role",
        AuthorizedRole::Creator,
        |fx| {
            fx.client
                .grant_role(&fx.creator, &fx.holder, &AclRole::Moderator);
            (fx.creator.clone(), fx.holder.clone(), AclRole::Moderator).into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_revoke_role(&fx.creator, &fx.holder, &AclRole::Moderator)
                .is_ok()
        },
    );

    // set_reward_rate
    verify_protected_case(
        "set_reward_rate",
        AuthorizedRole::Creator,
        |fx| (fx.creator.clone(), fx.creator.clone(), 500u32).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_reward_rate(&fx.creator, &fx.creator, &500u32)
                .is_ok()
        },
    );

    // create_event
    verify_protected_case(
        "create_event",
        AuthorizedRole::Creator,
        |fx| {
            let params = EventParams {
                creator: fx.creator.clone(),
                key_id: fx.creator.clone(),
                event_id: 1u32,
                min_balance: 1u32,
                max_redemptions: 10u32,
                start_ledger: 90u32,
                end_ledger: 200u32,
                lock_on_redeem: false,
            };
            (params,).into_val(fx.env)
        },
        |fx| {
            let params = EventParams {
                creator: fx.creator.clone(),
                key_id: fx.creator.clone(),
                event_id: 1u32,
                min_balance: 1u32,
                max_redemptions: 10u32,
                start_ledger: 90u32,
                end_ledger: 200u32,
                lock_on_redeem: false,
            };
            fx.client.try_create_event(&params).is_ok()
        },
    );

    // close_event
    verify_protected_case(
        "close_event",
        AuthorizedRole::Creator,
        |fx| {
            fx.client.create_event(&EventParams {
                creator: fx.creator.clone(),
                key_id: fx.creator.clone(),
                event_id: 7u32,
                min_balance: 1u32,
                max_redemptions: 10u32,
                start_ledger: 90u32,
                end_ledger: 200u32,
                lock_on_redeem: false,
            });
            (fx.creator.clone(), fx.creator.clone(), 7u32).into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_close_event(&fx.creator, &fx.creator, &7u32)
                .is_ok()
        },
    );

    // create_poll
    verify_protected_case(
        "create_poll",
        AuthorizedRole::Creator,
        |fx| {
            let options = vec![
                fx.env,
                String::from_str(fx.env, "Yes"),
                String::from_str(fx.env, "No"),
            ];
            (
                fx.creator.clone(),
                String::from_str(fx.env, "Upgrade?"),
                options,
                100u32,
            )
                .into_val(fx.env)
        },
        |fx| {
            let options = vec![
                fx.env,
                String::from_str(fx.env, "Yes"),
                String::from_str(fx.env, "No"),
            ];
            fx.client
                .try_create_poll(
                    &fx.creator,
                    &String::from_str(fx.env, "Upgrade?"),
                    &options,
                    &100u32,
                )
                .is_ok()
        },
    );

    // add_co_creator
    verify_protected_case(
        "add_co_creator",
        AuthorizedRole::Creator,
        |fx| {
            (
                fx.creator.clone(),
                fx.creator.clone(),
                fx.holder.clone(),
                CoCreatorRole::Content,
            )
                .into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_add_co_creator(
                    &fx.creator,
                    &fx.creator,
                    &fx.holder,
                    &CoCreatorRole::Content,
                )
                .is_ok()
        },
    );

    // set_co_creator_threshold
    verify_protected_case(
        "set_co_creator_threshold",
        AuthorizedRole::Creator,
        |fx| {
            fx.client.add_co_creator(
                &fx.creator,
                &fx.creator,
                &fx.holder,
                &CoCreatorRole::Content,
            );
            (fx.creator.clone(), fx.creator.clone(), 1u32).into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_set_co_creator_threshold(&fx.creator, &fx.creator, &1u32)
                .is_ok()
        },
    );

    // propose_transfer
    verify_protected_case(
        "propose_transfer",
        AuthorizedRole::Creator,
        |fx| (fx.creator.clone(), fx.holder.clone()).into_val(fx.env),
        |fx| {
            fx.client
                .try_propose_transfer(&fx.creator, &fx.holder)
                .is_ok()
        },
    );

    // set_supply_cap
    verify_protected_case(
        "set_supply_cap",
        AuthorizedRole::Creator,
        |fx| (fx.creator.clone(), 100u32).into_val(fx.env),
        |fx| fx.client.try_set_supply_cap(&fx.creator, &100u32).is_ok(),
    );

    // set_royalty
    verify_protected_case(
        "set_royalty",
        AuthorizedRole::Creator,
        |fx| (fx.creator.clone(), 200u32, 200u32).into_val(fx.env),
        |fx| {
            fx.client
                .try_set_royalty(&fx.creator, &200u32, &200u32)
                .is_ok()
        },
    );
}

// ── Holder role matrix tests ─────────────────────────────────────────────────

#[test]
fn test_holder_role_auth_matrix() {
    // redeem_event (regression: previously missing holder.require_auth())
    verify_protected_case(
        "redeem_event",
        AuthorizedRole::Holder,
        |fx| {
            fx.client.create_event(&EventParams {
                creator: fx.creator.clone(),
                key_id: fx.creator.clone(),
                event_id: 42u32,
                min_balance: 1u32,
                max_redemptions: 10u32,
                start_ledger: 90u32,
                end_ledger: 200u32,
                lock_on_redeem: true,
            });
            (fx.holder.clone(), fx.creator.clone(), 42u32).into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_redeem_event(&fx.holder, &fx.creator, &42u32)
                .is_ok()
        },
    );

    // batch_trade with empty orders (regression: previously emitted event without trader.require_auth())
    verify_protected_case(
        "batch_trade",
        AuthorizedRole::Holder,
        |fx| {
            let orders: Vec<(Address, bool, i128)> = Vec::new(fx.env);
            (fx.holder.clone(), orders).into_val(fx.env)
        },
        |fx| {
            let orders: Vec<(Address, bool, i128)> = Vec::new(fx.env);
            fx.client.try_batch_trade(&fx.holder, &orders).is_ok()
        },
    );

    // buy_key
    verify_protected_case(
        "buy_key",
        AuthorizedRole::Holder,
        |fx| (fx.creator.clone(), fx.holder.clone(), 100i128, None::<i128>).into_val(fx.env),
        |fx| {
            fx.client
                .try_buy_key(&fx.creator, &fx.holder, &100i128, &None)
                .is_ok()
        },
    );

    // sell_key
    verify_protected_case(
        "sell_key",
        AuthorizedRole::Holder,
        |fx| {
            // Advance ledger so same-ledger flash-loan guard does not block sell.
            fx.env.ledger().set_sequence_number(105);
            (fx.creator.clone(), fx.holder.clone(), None::<i128>).into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_sell_key(&fx.creator, &fx.holder, &None)
                .is_ok()
        },
    );

    // transfer_keys
    verify_protected_case(
        "transfer_keys",
        AuthorizedRole::Holder,
        |fx| {
            (
                fx.creator.clone(),
                fx.holder.clone(),
                fx.stranger.clone(),
                1u32,
            )
                .into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_transfer_keys(&fx.creator, &fx.holder, &fx.stranger, &1u32)
                .is_ok()
        },
    );

    // add_liquidity
    verify_protected_case(
        "add_liquidity",
        AuthorizedRole::Holder,
        |fx| (fx.creator.clone(), fx.holder.clone(), 1_000i128).into_val(fx.env),
        |fx| {
            fx.client
                .try_add_liquidity(&fx.creator, &fx.holder, &1_000i128)
                .is_ok()
        },
    );

    // remove_liquidity
    verify_protected_case(
        "remove_liquidity",
        AuthorizedRole::Holder,
        |fx| {
            let lp_id = fx.client.add_liquidity(&fx.creator, &fx.holder, &1_000i128);
            (lp_id,).into_val(fx.env)
        },
        |fx| fx.client.try_remove_liquidity(&1u64).is_ok(),
    );

    // register_alert
    verify_protected_case(
        "register_alert",
        AuthorizedRole::Holder,
        |fx| {
            (
                fx.creator.clone(),
                fx.holder.clone(),
                AlertDirection::Above,
                200i128,
            )
                .into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_register_alert(&fx.creator, &fx.holder, &AlertDirection::Above, &200i128)
                .is_ok()
        },
    );

    // delegate_votes
    verify_protected_case(
        "delegate_votes",
        AuthorizedRole::Holder,
        |fx| (fx.holder.clone(), fx.creator.clone(), fx.stranger.clone()).into_val(fx.env),
        |fx| {
            fx.client
                .try_delegate_votes(&fx.holder, &fx.creator, &fx.stranger)
                .is_ok()
        },
    );

    // revoke_delegation
    verify_protected_case(
        "revoke_delegation",
        AuthorizedRole::Holder,
        |fx| {
            fx.client
                .delegate_votes(&fx.holder, &fx.creator, &fx.stranger);
            (fx.holder.clone(), fx.creator.clone()).into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_revoke_delegation(&fx.holder, &fx.creator)
                .is_ok()
        },
    );

    // send_tip
    verify_protected_case(
        "send_tip",
        AuthorizedRole::Holder,
        |fx| {
            (
                fx.creator.clone(),
                fx.holder.clone(),
                50i128,
                None::<String>,
            )
                .into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_send_tip(&fx.creator, &fx.holder, &50i128, &None)
                .is_ok()
        },
    );
}

// ── Governor role matrix tests ───────────────────────────────────────────────

#[test]
fn test_governor_role_auth_matrix() {
    // create_proposal
    verify_protected_case(
        "create_proposal",
        AuthorizedRole::Governor,
        |fx| {
            fx.client
                .configure_protocol_governance(&fx.admin, &fx.creator, &1u32);
            (
                fx.governor.clone(),
                ProtocolParameter::KeyPrice,
                200i128,
                String::from_str(fx.env, "raise price"),
            )
                .into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_create_proposal(
                    &fx.governor,
                    &ProtocolParameter::KeyPrice,
                    &200i128,
                    &String::from_str(fx.env, "raise price"),
                )
                .is_ok()
        },
    );

    // cast_proposal_vote
    verify_protected_case(
        "cast_proposal_vote",
        AuthorizedRole::Governor,
        |fx| {
            fx.client
                .configure_protocol_governance(&fx.admin, &fx.creator, &1u32);
            let proposal_id = fx.client.create_proposal(
                &fx.governor,
                &ProtocolParameter::KeyPrice,
                &200i128,
                &String::from_str(fx.env, "raise price"),
            );
            assert_eq!(proposal_id, 0u32);
            // Advance ledger by 1 so the snapshot checkpoint at creation ledger is queryable.
            fx.env.ledger().set_sequence_number(101);
            (fx.governor.clone(), proposal_id, true).into_val(fx.env)
        },
        |fx| {
            fx.client
                .try_cast_proposal_vote(&fx.governor, &0u32, &true)
                .is_ok()
        },
    );
}

// ── Permissionless (Anyone) matrix tests ─────────────────────────────────────

#[test]
fn test_permissionless_anyone_auth_matrix() {
    // check_and_apply_milestone
    verify_permissionless_case(
        "check_and_apply_milestone",
        |_fx| {},
        |fx| fx.client.try_check_and_apply_milestone(&fx.creator).is_ok(),
    );

    // apply_trade_volume
    verify_permissionless_case(
        "apply_trade_volume",
        |_fx| {},
        |fx| fx.client.try_apply_trade_volume(&500i128).is_ok(),
    );

    // close_poll (permissionless once voting window expires)
    verify_permissionless_case(
        "close_poll",
        |fx| {
            let options = vec![
                fx.env,
                String::from_str(fx.env, "A"),
                String::from_str(fx.env, "B"),
            ];
            fx.client.create_poll(
                &fx.creator,
                &String::from_str(fx.env, "Pick one"),
                &options,
                &10u32,
            );
            fx.env.ledger().set_sequence_number(200);
        },
        |fx| fx.client.try_close_poll(&fx.creator, &1u32).is_ok(),
    );

    // execute_transfer (permissionless once accepted and timelocked)
    verify_permissionless_case(
        "execute_transfer",
        |fx| {
            let proposal_id = fx.client.propose_transfer(&fx.creator, &fx.holder);
            assert_eq!(proposal_id, 0u32);
            let executable_at = fx.client.accept_transfer(&proposal_id);
            fx.env.ledger().set_timestamp(executable_at + 1);
        },
        |fx| fx.client.try_execute_transfer(&0u32).is_ok(),
    );
}
