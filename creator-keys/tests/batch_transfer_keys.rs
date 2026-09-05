//! Integration tests for the `batch_transfer_keys` entrypoint.
//!
//! Each test covers a single observable behaviour so that a regression on any
//! one aspect produces a focused, descriptive failure.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, Vec};

const KEY_PRICE: i128 = 100;
const CREATOR_BPS: u32 = 9_000;
const PROTOCOL_BPS: u32 = 1_000;

// ---------------------------------------------------------------------------
// Happy-path: balances
// ---------------------------------------------------------------------------

#[test]
fn test_batch_transfer_sender_balance_decremented_by_total() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);

    for _ in 0..5 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }

    let transfers = Vec::from_array(&env, [(r1.clone(), 2u32), (r2.clone(), 1u32)]);
    client.batch_transfer_keys(&creator, &sender, &transfers);

    assert_eq!(
        client.get_key_balance(&creator, &sender),
        2,
        "sender balance must decrease by total transferred (5 - 3 = 2)"
    );
}

#[test]
fn test_batch_transfer_each_recipient_balance_incremented_correctly() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);
    let r3 = Address::generate(&env);

    for _ in 0..6 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }

    let transfers = Vec::from_array(
        &env,
        [(r1.clone(), 3u32), (r2.clone(), 2u32), (r3.clone(), 1u32)],
    );
    client.batch_transfer_keys(&creator, &sender, &transfers);

    assert_eq!(client.get_key_balance(&creator, &r1), 3, "r1 must hold 3");
    assert_eq!(client.get_key_balance(&creator, &r2), 2, "r2 must hold 2");
    assert_eq!(client.get_key_balance(&creator, &r3), 1, "r3 must hold 1");
}

#[test]
fn test_batch_transfer_accumulates_onto_existing_recipient_balance() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    for _ in 0..4 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }
    // Pre-seed recipient with 1 key from a different buyer.
    let pre_buyer = Address::generate(&env);
    client.buy_key(&creator, &pre_buyer, &KEY_PRICE, &None);
    client.transfer_keys(&creator, &pre_buyer, &recipient, &1);

    let transfers = Vec::from_array(&env, [(recipient.clone(), 2u32)]);
    client.batch_transfer_keys(&creator, &sender, &transfers);

    assert_eq!(
        client.get_key_balance(&creator, &recipient),
        3,
        "recipient balance must accumulate (1 existing + 2 transferred = 3)"
    );
}

// ---------------------------------------------------------------------------
// Total supply invariant
// ---------------------------------------------------------------------------

#[test]
fn test_batch_transfer_total_supply_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);

    for _ in 0..4 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }

    let supply_before = client.get_total_key_supply(&creator);
    let transfers = Vec::from_array(&env, [(r1.clone(), 1u32), (r2.clone(), 1u32)]);
    client.batch_transfer_keys(&creator, &sender, &transfers);
    let supply_after = client.get_total_key_supply(&creator);

    assert_eq!(
        supply_before, supply_after,
        "total supply must be unchanged after a batch transfer"
    );
}

// ---------------------------------------------------------------------------
// Holder count
// ---------------------------------------------------------------------------

#[test]
fn test_batch_transfer_holder_count_increments_for_new_recipients() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);

    for _ in 0..4 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }
    let holders_before = client.get_creator_holder_count(&creator);

    let transfers = Vec::from_array(&env, [(r1.clone(), 1u32), (r2.clone(), 1u32)]);
    client.batch_transfer_keys(&creator, &sender, &transfers);
    let holders_after = client.get_creator_holder_count(&creator);

    assert_eq!(
        holders_before + 2,
        holders_after,
        "holder count must increment for each new recipient (was {holders_before}, expected {})",
        holders_before + 2
    );
}

#[test]
fn test_batch_transfer_holder_count_decrements_when_sender_empties() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    let holders_before = client.get_creator_holder_count(&creator);

    let transfers = Vec::from_array(&env, [(recipient.clone(), 1u32)]);
    client.batch_transfer_keys(&creator, &sender, &transfers);
    let holders_after = client.get_creator_holder_count(&creator);

    // sender exits (−1), recipient enters (+1) → net zero change.
    assert_eq!(
        holders_before, holders_after,
        "holder count must be unchanged when sender empties and recipient is new"
    );
    assert_eq!(
        client.get_key_balance(&creator, &sender),
        0,
        "sender must have zero balance after transferring all keys"
    );
}

// ---------------------------------------------------------------------------
// Error paths
// ---------------------------------------------------------------------------

#[test]
fn test_batch_transfer_exceeds_limit_reverts_with_batch_transfer_size_exceeded() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);

    for _ in 0..11 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }

    let mut transfers = Vec::new(&env);
    for _ in 0..11u32 {
        transfers.push_back((Address::generate(&env), 1u32));
    }

    let result = client.try_batch_transfer_keys(&creator, &sender, &transfers);
    assert_eq!(
        result,
        Err(Ok(ContractError::BatchTransferSizeExceeded)),
        "11 entries must revert with BatchTransferSizeExceeded"
    );
}

#[test]
fn test_batch_transfer_exactly_10_recipients_succeeds() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);

    for _ in 0..10 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }

    let mut transfers = Vec::new(&env);
    for _ in 0..10u32 {
        transfers.push_back((Address::generate(&env), 1u32));
    }

    client.batch_transfer_keys(&creator, &sender, &transfers);
    assert_eq!(
        client.get_key_balance(&creator, &sender),
        0,
        "sender must have zero balance after transferring all 10 keys"
    );
}

#[test]
fn test_batch_transfer_total_exceeds_balance_reverts_with_insufficient_balance() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);

    // Sender holds 3 keys; batch requests 4.
    for _ in 0..3 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }

    let transfers = Vec::from_array(&env, [(r1.clone(), 2u32), (r2.clone(), 2u32)]);
    let result = client.try_batch_transfer_keys(&creator, &sender, &transfers);
    assert_eq!(
        result,
        Err(Ok(ContractError::InsufficientBalance)),
        "total exceeding balance must revert with InsufficientBalance"
    );
}

#[test]
fn test_batch_transfer_total_exceeds_balance_state_unchanged() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);

    for _ in 0..3 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }
    let balance_before = client.get_key_balance(&creator, &sender);
    let supply_before = client.get_total_key_supply(&creator);

    let transfers = Vec::from_array(&env, [(r1.clone(), 2u32), (r2.clone(), 2u32)]);
    let _ = client.try_batch_transfer_keys(&creator, &sender, &transfers);

    assert_eq!(
        client.get_key_balance(&creator, &sender),
        balance_before,
        "sender balance must be unchanged after failed batch transfer"
    );
    assert_eq!(
        client.get_total_key_supply(&creator),
        supply_before,
        "total supply must be unchanged after failed batch transfer"
    );
}

#[test]
fn test_batch_transfer_self_recipient_reverts_with_invalid_recipient() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let other = Address::generate(&env);

    for _ in 0..3 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }

    // First recipient is valid, second is self.
    let transfers = Vec::from_array(&env, [(other.clone(), 1u32), (sender.clone(), 1u32)]);
    let result = client.try_batch_transfer_keys(&creator, &sender, &transfers);
    assert_eq!(
        result,
        Err(Ok(ContractError::InvalidRecipient)),
        "self-transfer inside a batch must revert with InvalidRecipient"
    );
}

#[test]
fn test_batch_transfer_zero_quantity_entry_reverts_with_zero_transfer_amount() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);
    let creator = register_test_creator(&env, &client, "alice");
    let sender = Address::generate(&env);
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);

    for _ in 0..3 {
        client.buy_key(&creator, &sender, &KEY_PRICE, &None);
    }

    let transfers = Vec::from_array(&env, [(r1.clone(), 1u32), (r2.clone(), 0u32)]);
    let result = client.try_batch_transfer_keys(&creator, &sender, &transfers);
    assert_eq!(
        result,
        Err(Ok(ContractError::ZeroTransferAmount)),
        "an entry with zero quantity must revert with ZeroTransferAmount"
    );
}

#[test]
fn test_batch_transfer_unregistered_creator_reverts() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, KEY_PRICE, CREATOR_BPS, PROTOCOL_BPS);

    let unregistered = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let transfers = Vec::from_array(&env, [(recipient.clone(), 1u32)]);
    let result = client.try_batch_transfer_keys(&unregistered, &sender, &transfers);
    assert_eq!(
        result,
        Err(Ok(ContractError::NotRegistered)),
        "unregistered creator must revert with NotRegistered"
    );
}
