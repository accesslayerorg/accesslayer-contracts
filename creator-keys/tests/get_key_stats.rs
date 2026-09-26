//! Integration tests for `get_key_stats` — the aggregated read-only view that
//! returns all key-level fields in a single RPC call.
//!
//! ## Test inventory
//!
//! | # | Name | What it checks |
//! |---|------|----------------|
//! | 1 | `test_get_key_stats_required_fields` | All required scalar fields are returned correctly after setup |
//! | 2 | `test_get_key_stats_auction_fields_present` | Auction fields populated when auction is configured |
//! | 3 | `test_get_key_stats_auction_fields_absent` | Auction fields zero and `has_auction` false when no auction |
//! | 4 | `test_get_key_stats_ttl_bumped` | TTL on all read entries is bumped after the call |
//! | 5 | `test_get_key_stats_no_panic_minimal_state` | No panic when optional fields (cap, lockup, …) are absent |
//! | 6 | `test_get_key_stats_key_not_found` | Returns `NotRegistered` for an unknown key_id |

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::constants::storage;
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _, Ledger},
    Address, Env,
};

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

const KEY_PRICE: i128 = 1_000;

// ---------------------------------------------------------------------------
// Helper: build a minimal registered setup with pricing + fees.
// Returns (client, contract_id, admin, creator).
// ---------------------------------------------------------------------------

fn setup(
    env: &Env,
) -> (
    CreatorKeysContractClient<'_>,
    Address, // contract_id
    Address, // admin (owns the protocol admin role)
    Address, // creator
) {
    let (client, contract_id) = register_creator_keys(env);
    // Register admin first (must be done before set_fee_config).
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);
    client.set_fee_config(&admin, &9_000u32, &1_000u32);
    let creator = register_test_creator(env, &client, "alice");
    (client, contract_id, admin, creator)
}

// ---------------------------------------------------------------------------
// Test 1 — all required fields returned correctly
// ---------------------------------------------------------------------------

/// After a buy the stats view must reflect the updated supply and holder count,
/// the correct current price (bonding curve at supply 1), trading not paused,
/// and sensible defaults for optional numeric fields.
#[test]
fn test_get_key_stats_required_fields() {
    let env = test_env_with_auths();
    let (client, _contract_id, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    // One buy so supply and holder_count become non-zero.
    client.buy_key(&creator, &buyer, &KEY_PRICE, &None);

    let stats = client.get_key_stats(&creator);

    // supply and holder_count reflect the buy
    assert_eq!(
        stats.circulating_supply, 1,
        "circulating_supply should be 1 after one buy"
    );
    assert_eq!(
        stats.holder_count, 1,
        "holder_count should be 1 after one unique buyer"
    );

    // current_price must be positive (base_price + curve component at supply 1)
    assert!(
        stats.current_price > 0,
        "current_price must be positive when key price is set: got {}",
        stats.current_price
    );

    // trading is not paused
    assert!(
        !stats.trading_paused,
        "trading_paused should be false when not paused"
    );

    // circuit breaker default is 30 (value returned even when key absent)
    assert_eq!(
        stats.circuit_breaker_threshold_bps, 30,
        "circuit_breaker_threshold_bps should default to 30"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — auction fields present when auction is configured
// ---------------------------------------------------------------------------

#[test]
fn test_get_key_stats_auction_fields_present() {
    let env = test_env_with_auths();
    let (client, _contract_id, _admin, creator) = setup(&env);

    let auction_price: i128 = 500;
    let auction_supply: u32 = 20;

    // Configure a pre-launch auction before any buy.
    client.configure_auction(&creator, &creator, &auction_price, &auction_supply);

    let stats = client.get_key_stats(&creator);

    assert!(
        stats.has_auction,
        "has_auction should be true when auction is configured"
    );
    assert_eq!(
        stats.auction_price, auction_price,
        "auction_price should match configured value"
    );
    assert_eq!(
        stats.auction_supply, auction_supply,
        "auction_supply should match configured value"
    );
    assert_eq!(
        stats.auction_sold, 0,
        "auction_sold should be 0 before any auction buys"
    );

    // current_price during auction should equal the fixed auction price
    assert_eq!(
        stats.current_price, auction_price,
        "current_price should equal auction_price while supply < auction_supply"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — auction fields absent when no auction configured
// ---------------------------------------------------------------------------

#[test]
fn test_get_key_stats_auction_fields_absent() {
    let env = test_env_with_auths();
    let (client, _contract_id, _admin, creator) = setup(&env);

    let stats = client.get_key_stats(&creator);

    assert!(
        !stats.has_auction,
        "has_auction should be false when no auction is configured"
    );
    assert_eq!(
        stats.auction_price, 0,
        "auction_price should be 0 when no auction"
    );
    assert_eq!(
        stats.auction_supply, 0,
        "auction_supply should be 0 when no auction"
    );
    assert_eq!(
        stats.auction_sold, 0,
        "auction_sold should be 0 when no auction"
    );
}

// ---------------------------------------------------------------------------
// Test 4 — TTL bumped on all read entries
// ---------------------------------------------------------------------------

/// After draining the key price and creator profile entries near expiry, a call
/// to `get_key_stats` must bump each entry's TTL to at least
/// [`creator_keys::TTL_MIN_EXTENSION_LEDGERS`].
#[test]
fn test_get_key_stats_ttl_bumped() {
    let env = test_env_with_auths();
    let (client, contract_id, _admin, creator) = setup(&env);

    // Extend the contract instance/code lifetime so far-future ledger advances
    // do not archive the code itself.
    env.deployer().extend_ttl(
        contract_id.clone(),
        creator_keys::CREATOR_TTL_LEDGERS,
        creator_keys::CREATOR_TTL_LEDGERS,
    );

    // Drain entries to near expiry by jumping forward.
    let drain = creator_keys::CREATOR_TTL_LEDGERS - 100;
    {
        let mut info = env.ledger().get();
        info.sequence_number += drain;
        env.ledger().set(info);
    }

    let ttl_min = creator_keys::TTL_MIN_EXTENSION_LEDGERS;

    // Helper closure to read TTL inside the contract scope.
    let ttl = |key: &creator_keys::DataKey| {
        env.as_contract(&contract_id, || env.storage().persistent().get_ttl(key))
    };

    let price_ttl_before = ttl(&storage::KEY_PRICE);
    let creator_ttl_before = ttl(&storage::creator(&creator));

    assert!(
        price_ttl_before < ttl_min,
        "precondition: KEY_PRICE ttl should be below minimum before bump"
    );
    assert!(
        creator_ttl_before < ttl_min,
        "precondition: creator profile ttl should be below minimum before bump"
    );

    // The get_key_stats call should bump TTL on all read entries.
    client.get_key_stats(&creator);

    let price_ttl_after = ttl(&storage::KEY_PRICE);
    let creator_ttl_after = ttl(&storage::creator(&creator));

    assert!(
        price_ttl_after >= ttl_min,
        "KEY_PRICE TTL should be >= TTL_MIN_EXTENSION_LEDGERS after get_key_stats: got {price_ttl_after}"
    );
    assert!(
        creator_ttl_after >= ttl_min,
        "creator profile TTL should be >= TTL_MIN_EXTENSION_LEDGERS after get_key_stats: got {creator_ttl_after}"
    );
}

// ---------------------------------------------------------------------------
// Test 5 — no panic when all optional per-key fields are absent
// ---------------------------------------------------------------------------

/// A freshly registered creator with pricing and fees set, but none of the
/// optional settings (supply cap, holder cap, lockup, launch penalty, cooldown,
/// max buy quantity) configured. `get_key_stats` must return successfully with
/// zero for all optional numeric fields.
#[test]
fn test_get_key_stats_no_panic_minimal_state() {
    let env = test_env_with_auths();
    let (client, _contract_id, _admin, creator) = setup(&env);

    // Must not panic; all optional fields default to 0.
    let stats = client.get_key_stats(&creator);

    assert_eq!(stats.supply_cap, 0, "supply_cap should be 0 when uncapped");
    assert_eq!(
        stats.holder_cap_bps, 0,
        "holder_cap_bps should be 0 when uncapped"
    );
    assert_eq!(
        stats.lockup_duration_seconds, 0,
        "lockup_duration_seconds should be 0 when not configured"
    );
    assert_eq!(
        stats.launch_penalty_bps, 0,
        "launch_penalty_bps should be 0 when not configured"
    );
    assert_eq!(
        stats.buy_cooldown_ledgers, 0,
        "buy_cooldown_ledgers should be 0 when not configured"
    );
    assert_eq!(
        stats.max_buy_quantity, 0,
        "max_buy_quantity should be 0 when not configured"
    );
    assert!(!stats.has_auction, "has_auction should be false by default");
    assert!(
        !stats.trading_paused,
        "trading_paused should be false by default"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — KeyNotFound for unknown key_id
// ---------------------------------------------------------------------------

/// Calling `get_key_stats` with an address that was never registered must
/// return `Err(ContractError::NotRegistered)` — the 404-equivalent for callers.
#[test]
fn test_get_key_stats_key_not_found() {
    let env = test_env_with_auths();
    // Register the contract but don't register any creator.
    let (client, _) = register_creator_keys(&env);
    // Set up pricing so the contract is otherwise functional.
    set_key_price_for_tests(&env, &client, KEY_PRICE);

    let unknown = Address::generate(&env);

    let result = client.try_get_key_stats(&unknown);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "get_key_stats must return NotRegistered for an unknown key_id"
    );
}

// ---------------------------------------------------------------------------
// Test 7 — optional fields reflected when configured
// ---------------------------------------------------------------------------

/// When optional per-key settings are explicitly configured, `get_key_stats`
/// must surface them correctly rather than returning zero.
#[test]
fn test_get_key_stats_optional_fields_when_set() {
    let env = test_env_with_auths();
    let (client, _contract_id, admin, creator) = setup(&env);

    // Supply cap
    client.set_supply_cap(&creator, &500u32);

    // Holder cap — set_holder_cap takes `Option<u32>`; pass `Some(1_000)` to set explicitly.
    client.set_holder_cap(&creator, &Some(1_000u32));

    // Lockup duration (admin-only)
    client.set_lockup_duration(&admin, &86_400u64);

    // Buy cooldown — set_buy_cooldown(creator, cooldown_ledgers)
    client.set_buy_cooldown(&creator, &100u32);

    // Max buy quantity — set_max_buy_quantity(creator, max_qty)
    client.set_max_buy_quantity(&creator, &50u32);

    let stats = client.get_key_stats(&creator);

    assert_eq!(
        stats.supply_cap, 500,
        "supply_cap should reflect configured value"
    );
    assert_eq!(
        stats.holder_cap_bps, 1_000,
        "holder_cap_bps should reflect configured value"
    );
    assert_eq!(
        stats.lockup_duration_seconds, 86_400,
        "lockup_duration_seconds should reflect configured value"
    );
    assert_eq!(
        stats.buy_cooldown_ledgers, 100,
        "buy_cooldown_ledgers should reflect configured value"
    );
    assert_eq!(
        stats.max_buy_quantity, 50,
        "max_buy_quantity should reflect configured value"
    );
}

// ---------------------------------------------------------------------------
// Test 8 — trading_paused reflects global pause
// ---------------------------------------------------------------------------

/// When the protocol admin activates the global pause, `trading_paused` in the
/// stats view must flip to `true`.
#[test]
fn test_get_key_stats_trading_paused_reflects_pause() {
    let env = test_env_with_auths();
    let (client, _contract_id, admin, creator) = setup(&env);

    // Before pause: not paused
    let stats = client.get_key_stats(&creator);
    assert!(
        !stats.trading_paused,
        "trading_paused should be false before pause"
    );

    // Activate pause
    client.pause(&admin);

    // After pause: paused — get_key_stats is a read-only view and must succeed even while paused
    let stats = client.get_key_stats(&creator);
    assert!(
        stats.trading_paused,
        "trading_paused should be true after pause"
    );

    // Deactivate pause
    client.unpause(&admin);

    // After unpause: not paused again
    let stats = client.get_key_stats(&creator);
    assert!(
        !stats.trading_paused,
        "trading_paused should be false after unpause"
    );
}

// ---------------------------------------------------------------------------
// Test 9 — auction_sold reflects purchases made during auction phase
// ---------------------------------------------------------------------------

#[test]
fn test_get_key_stats_auction_sold_increments_on_purchase() {
    let env = test_env_with_auths();
    let (client, _contract_id, _admin, creator) = setup(&env);
    let buyer = Address::generate(&env);

    let auction_price: i128 = 500;
    let auction_supply: u32 = 10;

    client.configure_auction(&creator, &creator, &auction_price, &auction_supply);

    // Buy two keys through the auction.
    client.buy_key(&creator, &buyer, &auction_price, &None);
    client.buy_key(&creator, &buyer, &auction_price, &None);

    let stats = client.get_key_stats(&creator);

    assert!(
        stats.has_auction,
        "has_auction should still be true while auction is not exhausted"
    );
    assert_eq!(
        stats.auction_sold, 2,
        "auction_sold should reflect purchases: got {}",
        stats.auction_sold
    );
    assert_eq!(
        stats.circulating_supply, 2,
        "circulating_supply should equal keys bought through auction"
    );
}
