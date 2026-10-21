//! Integration tests for the buy and sell revert paths (issue #571).
//!
//! The two most critical trading failure paths — buying with insufficient XLM and
//! selling more keys than a holder owns — are exercised here behind a single shared
//! setup helper so the revert-path assertion pattern stays consistent across buy and
//! sell:
//!
//! * **Insufficient buy** — a buy whose attached payment is one stroop below the
//!   current price must revert with [`ContractError::InsufficientPayment`] and leave
//!   supply and buyer holdings untouched.
//! * **Oversell** — a holder who owns exactly two keys must not be able to sell a
//!   third. The contract's `sell_key` settles a single key per call (there is no
//!   quantity argument), so a "sell quantity 3" by a two-key holder is exercised as
//!   three sequential sells: the first two drain the position and the third is the
//!   oversell, which must revert with [`ContractError::InsufficientBalance`] while
//!   mutating no state.

mod contract_test_env;

use contract_test_env::{
    capture_snapshot, compute_expected_buy_price, register_creator_keys, register_test_creator,
    set_key_price_for_tests, test_env_with_auths,
};
use creator_keys::{ContractError, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

/// Base key price shared by both revert-path tests (flat bonding curve => price is
/// constant across supply, so this is also the current price for every buy).
const BASE_PRICE: i128 = 100;

/// Shared setup helper: deploys the contract, sets a positive key price, and seeds a
/// single registered creator. Returns the client and the creator address.
///
/// Both revert-path tests build on this identical fixture so the buy and sell cases
/// start from the same deployed-and-seeded state.
fn setup(env: &Env) -> (CreatorKeysContractClient<'_>, Address) {
    let (client, _) = register_creator_keys(env);
    set_key_price_for_tests(env, &client, BASE_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    (client, creator)
}

/// Buying with an attached amount one stroop below the current price must revert with
/// the insufficient-funds error code and leave supply and buyer holdings unchanged.
#[test]
fn test_buy_reverts_on_insufficient_funds_without_state_change() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let buyer = Address::generate(&env);

    // Current price for the first key (flat curve => base price).
    let current_price = compute_expected_buy_price(0, BASE_PRICE);
    let insufficient_amount = current_price - 1; // one stroop short of the price

    let before = capture_snapshot(&client, &creator, &buyer);
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);

    // Invoke buy with the attached amount one stroop below the current price.
    let result = client.try_buy_key(&creator, &buyer, &insufficient_amount, &None);

    // Must revert with the insufficient-funds error code.
    assert_eq!(result, Err(Ok(ContractError::InsufficientPayment)));

    // Key supply and buyer holdings are unchanged after the failed buy.
    let after = capture_snapshot(&client, &creator, &buyer);
    before.assert_unchanged(&after);
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_key_balance(&creator, &buyer), 0);
}

/// Selling more keys than a holder owns must revert with the insufficient-balance
/// error code, and the failed oversell must leave holder balance and creator supply
/// unchanged.
#[test]
fn test_sell_reverts_on_oversell_without_state_change() {
    let env = test_env_with_auths();
    let (client, creator) = setup(&env);
    let holder = Address::generate(&env);

    // Set up a holder with exactly two keys via buy transactions.
    client.buy_key(&creator, &holder, &BASE_PRICE, &None);
    client.buy_key(&creator, &holder, &BASE_PRICE, &None);
    assert_eq!(client.get_key_balance(&creator, &holder), 2);
    assert_eq!(client.get_total_key_supply(&creator), 2);

    // Attempt to sell three keys. `sell_key` settles one key per call, so the first
    // two sells succeed and drain the holder's two keys...
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
    client.sell_key(&creator, &holder, &None);
    client.sell_key(&creator, &holder, &None);
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
    assert_eq!(client.get_total_key_supply(&creator), 0);

    // ...and the third sell is the oversell that exceeds the holder's original
    // holdings. Snapshot state immediately before it so the revert can be shown to
    // mutate nothing.
    let before_oversell = capture_snapshot(&client, &creator, &holder);

    let result = client.try_sell_key(&creator, &holder, &None);

    // Must revert with the insufficient-balance error code.
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));

    // Holder balance and creator supply are unchanged after the failed oversell.
    let after_oversell = capture_snapshot(&client, &creator, &holder);
    before_oversell.assert_unchanged(&after_oversell);
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
    assert_eq!(client.get_total_key_supply(&creator), 0);
}
