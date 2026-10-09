//! Integration tests for configurable supply cap enforcement (#997).
//!
//! Covered acceptance criteria:
//! 1. A buy that would push post-trade supply past the cap is blocked.
//! 2. A partial fill that lands exactly on the cap succeeds.
//! 3. `get_supply_info` reports correct `(supply, cap, remaining)` at every
//!    supply level.
//! 4. `SupplyCapReached` is emitted exactly once, on the trade that fills the
//!    cap, and never again afterwards.
//! 5. A configured cap of `0` is treated as unlimited: buys keep growing
//!    supply past any small ceiling, and `get_supply_info` reports cap `0`
//!    with unbounded remaining.
//!
//! Caps are immutable once set, so each scenario registers its own creator
//! and then applies that key's cap via `set_supply_cap`.

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_pricing_and_fees, test_env_with_auths};
use creator_keys::events::{SupplyCapReachedEvent, SUPPLY_CAP_REACHED_EVENT_NAME};
use creator_keys::{
    ContractError, CreatorKeysContractClient, CurvePreset, KeyMetadata, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol, Vec,
};

const KEY_PRICE: i128 = 1_000;

fn metadata(env: &Env) -> KeyMetadata {
    KeyMetadata {
        name: String::from_str(env, "Capped Key"),
        symbol: String::from_str(env, "CAP"),
        description: String::from_str(env, "supply cap test key"),
        image_cid: String::from_str(env, "ipfs://avatar"),
    }
}

/// Deploy the contract with a positive key price; returns the client and the
/// protocol admin (required by `register_key`).
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _id) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, 9_000, 1_000);
    (client, admin)
}

/// Register a creator key and apply its hard supply cap. A `supply_cap` of
/// `0` leaves the key uncapped.
fn register_capped_key(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    admin: &Address,
    handle: &str,
    supply_cap: u32,
) -> Address {
    let creator = Address::generate(env);
    client.register_key(
        admin,
        &creator,
        &String::from_str(env, handle),
        &metadata(env),
        &CurvePreset::Linear,
        &0,
        &false,
    );
    if supply_cap > 0 {
        client.set_supply_cap(&creator, &supply_cap);
    }
    creator
}

/// Total payment the current quote asks for one key (price plus fees).
fn quote_at(client: &CreatorKeysContractClient<'_>, creator: &Address) -> i128 {
    client.get_buy_quote(creator).total_amount
}

/// Buy one key at the current quote, returning the post-buy supply.
fn buy_one(client: &CreatorKeysContractClient<'_>, creator: &Address, buyer: &Address) -> u32 {
    let quote = client.get_buy_quote(creator);
    client.buy_key(creator, buyer, &quote.total_amount, &None)
}

/// Collect every `SupplyCapReached` event from the current ledger's event log.
fn supply_cap_reached_events(env: &Env) -> Vec<SupplyCapReachedEvent> {
    let mut found = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(env);
        if name == SUPPLY_CAP_REACHED_EVENT_NAME {
            found.push_back(data.into_val(env));
        }
    }
    found
}

// ---------------------------------------------------------------------------
// Criterion 1: buys that would exceed the cap are blocked
// ---------------------------------------------------------------------------

#[test]
fn test_buy_blocked_when_post_trade_supply_would_exceed_cap() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = register_capped_key(&env, &client, &admin, "capped", 10);
    let buyer = Address::generate(&env);

    // Fill the key to its cap: every buy here has a post-trade supply of at
    // most 10, so all of them are allowed.
    for expected in 1..=10u32 {
        assert_eq!(buy_one(&client, &creator, &buyer), expected);
    }
    assert_eq!(client.get_supply(&creator), 10);

    // The next buy would push post-trade supply to 11 > cap 10, so it is
    // rejected with SupplyCapExceeded and no state may change.
    let result = client.try_buy_key(&creator, &buyer, &quote_at(&client, &creator), &None);
    assert_eq!(result, Err(Ok(ContractError::SupplyCapExceeded)));
    assert_eq!(client.get_supply(&creator), 10);
    assert_eq!(client.get_balance(&creator, &buyer), 10);
}

#[test]
fn test_second_buyer_cannot_mint_past_a_filled_cap() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = register_capped_key(&env, &client, &admin, "capped_two", 2);
    let first = Address::generate(&env);
    let second = Address::generate(&env);

    // The first buyer fills the key; the second is capped out.
    assert_eq!(buy_one(&client, &creator, &first), 1);
    assert_eq!(buy_one(&client, &creator, &first), 2);

    let result = client.try_buy_key(&creator, &second, &quote_at(&client, &creator), &None);
    assert_eq!(result, Err(Ok(ContractError::SupplyCapExceeded)));
    assert_eq!(client.get_supply(&creator), 2);
    assert_eq!(client.get_balance(&creator, &second), 0);
}

// ---------------------------------------------------------------------------
// Criterion 2: a partial fill that lands exactly on the cap succeeds
// ---------------------------------------------------------------------------

#[test]
fn test_partial_fill_exactly_reaching_cap_succeeds() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = register_capped_key(&env, &client, &admin, "partial", 10);
    let buyer = Address::generate(&env);

    for _ in 0..8 {
        buy_one(&client, &creator, &buyer);
    }

    // A batch buy of exactly 2 fills the key to its cap and must succeed.
    // `buy_keys` returns the resulting total supply, not the quantity filled.
    let quote = client.get_buy_quote(&creator);
    let new_supply = client.buy_keys(&creator, &buyer, &2, &(quote.total_amount * 10), &None);
    assert_eq!(new_supply, 10);
    assert_eq!(client.get_supply(&creator), 10);

    let info = client.get_supply_info(&creator);
    assert_eq!(info.supply, info.cap);
    assert_eq!(info.remaining, 0);
}

#[test]
fn test_single_buy_that_reaches_cap_exactly_succeeds() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = register_capped_key(&env, &client, &admin, "single_fill", 1);
    let buyer = Address::generate(&env);

    // A cap of 1 makes the very first buy an exact fill.
    let supply = buy_one(&client, &creator, &buyer);
    assert_eq!(supply, 1);

    let info = client.get_supply_info(&creator);
    assert_eq!(info.supply, 1);
    assert_eq!(info.cap, 1);
    assert_eq!(info.remaining, 0);
}

// ---------------------------------------------------------------------------
// Criterion 3: get_supply_info is correct at every supply level
// ---------------------------------------------------------------------------

#[test]
fn test_get_supply_info_tracks_supply_cap_and_remaining() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = register_capped_key(&env, &client, &admin, "info", 5);
    let buyer = Address::generate(&env);

    // Freshly deployed key: nothing minted yet.
    let info = client.get_supply_info(&creator);
    assert_eq!(info.supply, 0);
    assert_eq!(info.cap, 5);
    assert_eq!(info.remaining, 5);

    // Mid-curve values stay consistent after every buy.
    for expected_supply in 1..=5u32 {
        buy_one(&client, &creator, &buyer);
        let info = client.get_supply_info(&creator);
        assert_eq!(info.supply, expected_supply);
        assert_eq!(info.cap, 5);
        assert_eq!(info.remaining, 5 - expected_supply);
    }

    // At the cap the remaining count is exactly zero.
    let info = client.get_supply_info(&creator);
    assert_eq!(info.supply, 5);
    assert_eq!(info.remaining, 0);
}

// ---------------------------------------------------------------------------
// Criterion 4: SupplyCapReached is emitted exactly once when the cap is hit
// ---------------------------------------------------------------------------

#[test]
fn test_supply_cap_reached_emitted_exactly_once_on_exact_fill() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = register_capped_key(&env, &client, &admin, "event", 3);
    let buyer = Address::generate(&env);

    for _ in 0..3 {
        buy_one(&client, &creator, &buyer);
    }

    let events = supply_cap_reached_events(&env);
    assert_eq!(events.len(), 1, "the cap-filling trade must emit one event");
    let event = events.get(0).unwrap();
    assert_eq!(event.creator_id, creator);
    assert_eq!(event.new_supply, 3);
    assert_eq!(event.cap, 3);
    assert_eq!(event.ledger, env.ledger().sequence());
}

#[test]
fn test_supply_cap_reached_emitted_once_for_partial_fill_reaching_cap() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = register_capped_key(&env, &client, &admin, "event_partial", 10);
    let buyer = Address::generate(&env);

    for _ in 0..8 {
        buy_one(&client, &creator, &buyer);
    }
    assert!(supply_cap_reached_events(&env).is_empty());

    // The batch fill that lands on the cap emits exactly one event.
    let quote = client.get_buy_quote(&creator);
    client.buy_keys(&creator, &buyer, &2, &(quote.total_amount * 10), &None);

    let events = supply_cap_reached_events(&env);
    assert_eq!(events.len(), 1);
    assert_eq!(events.get(0).unwrap().new_supply, 10);
    assert_eq!(events.get(0).unwrap().cap, 10);
}

#[test]
fn test_supply_cap_reached_not_emitted_again_after_cap() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = register_capped_key(&env, &client, &admin, "event_once", 3);
    let buyer = Address::generate(&env);

    for _ in 0..3 {
        buy_one(&client, &creator, &buyer);
    }
    assert_eq!(supply_cap_reached_events(&env).len(), 1);

    // The event is emitted only by a successful mint, and every further buy
    // reverts with SupplyCapExceeded without moving supply — so no second
    // event can ever be published for this key.
    let result = client.try_buy_key(&creator, &buyer, &quote_at(&client, &creator), &None);
    assert_eq!(result, Err(Ok(ContractError::SupplyCapExceeded)));
    assert_eq!(client.get_supply(&creator), 3);
}

#[test]
fn test_uncapped_key_never_emits_supply_cap_reached() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = register_capped_key(&env, &client, &admin, "no_event", 0);
    let buyer = Address::generate(&env);

    for _ in 0..5 {
        buy_one(&client, &creator, &buyer);
    }

    assert!(supply_cap_reached_events(&env).is_empty());
}

// ---------------------------------------------------------------------------
// Criterion 5: a cap of 0 means unlimited supply
// ---------------------------------------------------------------------------

#[test]
fn test_zero_cap_allows_unlimited_supply_growth() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);
    let creator = register_capped_key(&env, &client, &admin, "uncapped", 0);
    let buyer = Address::generate(&env);

    // Grow well past a small number of keys — no ceiling applies.
    for expected in 1..=20u32 {
        let supply = buy_one(&client, &creator, &buyer);
        assert_eq!(supply, expected);
    }

    let info = client.get_supply_info(&creator);
    assert_eq!(info.supply, 20);
    assert_eq!(info.cap, 0);
    assert_eq!(info.remaining, u32::MAX);
}

// ---------------------------------------------------------------------------
// Configuration surface: the cap is stored in the key's own config
// ---------------------------------------------------------------------------

#[test]
fn test_cap_is_stored_in_key_config() {
    let env = test_env_with_auths();
    let (client, admin) = setup(&env);

    let capped = register_capped_key(&env, &client, &admin, "stored_cap", 7);
    assert_eq!(client.get_max_supply(&capped), Some(7));
    let info = client.get_supply_info(&capped);
    assert_eq!(info.cap, 7);
    assert_eq!(info.remaining, 7);

    // An uncapped key (cap 0) writes no cap storage at all.
    let uncapped = register_capped_key(&env, &client, &admin, "stored_none", 0);
    assert_eq!(client.get_max_supply(&uncapped), None);
}

#[test]
fn test_register_creator_stores_cap_at_registration() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);

    // `max_supply` is the optional deployment-time cap.
    let creator = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "deploy_cap"),
        },
        &None,
        &Some(5u32),
        &None,
        &None,
        &None,
        &None,
    );

    assert_eq!(client.get_max_supply(&creator), Some(5));
    let info = client.get_supply_info(&creator);
    assert_eq!(info.supply, 0);
    assert_eq!(info.cap, 5);
    assert_eq!(info.remaining, 5);
}

#[test]
fn test_register_creator_zero_cap_is_unlimited() {
    let env = test_env_with_auths();
    let (client, _admin) = setup(&env);

    let creator = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "zero_cap"),
        },
        &None,
        &Some(0),
        &None,
        &None,
        &None,
        &None,
    );

    // #997: cap 0 registers as unlimited rather than reverting.
    let info = client.get_supply_info(&creator);
    assert_eq!(info.cap, 0);
    assert_eq!(info.remaining, u32::MAX);
}
