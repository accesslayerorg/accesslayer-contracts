mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_update_creator_fee_recipient_success() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let new_recipient = Address::generate(&env);

    let result = client.try_update_creator_fee_recipient(&creator, &creator, &new_recipient);
    assert_eq!(result, Ok(Ok(())));

    let recipient = client.get_creator_fee_recipient(&creator);
    assert_eq!(
        recipient, new_recipient,
        "creator fee recipient should be updated"
    );
}

#[test]
fn test_update_creator_fee_recipient_rejects_non_current_recipient() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let not_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    let result =
        client.try_update_creator_fee_recipient(&creator, &not_recipient, &new_recipient);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));

    let current = client.get_creator_fee_recipient(&creator);
    assert_eq!(
        current, creator,
        "recipient should remain unchanged after unauthorized call"
    );
}

#[test]
fn test_update_creator_fee_recipient_rejects_zero_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let zero_str = String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero_addr = Address::from_string(&zero_str);

    let result = client.try_update_creator_fee_recipient(&creator, &creator, &zero_addr);
    assert_eq!(result, Err(Ok(ContractError::ZeroAddress)));

    let current = client.get_creator_fee_recipient(&creator);
    assert_eq!(
        current, creator,
        "recipient should remain unchanged after zero address rejection"
    );
}

#[test]
fn test_update_creator_fee_recipient_rejects_unregistered_creator() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let unregistered = Address::generate(&env);
    let caller = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    let result =
        client.try_update_creator_fee_recipient(&unregistered, &caller, &new_recipient);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_update_creator_fee_recipient_emits_event() {
    use soroban_sdk::testutils::Events;
    use soroban_sdk::TryIntoVal;

    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let new_recipient = Address::generate(&env);
    client.update_creator_fee_recipient(&creator, &creator, &new_recipient);

    let all_events = env.events().all();
    assert_eq!(all_events.len(), 1, "expected exactly one event");

    let (_contract_id, _topics, data): (
        Address,
        soroban_sdk::Vec<soroban_sdk::Val>,
        soroban_sdk::Val,
    ) = all_events.get(0).unwrap();

    let payload: creator_keys::events::CreatorFeeRecipientUpdated =
        data.try_into_val(&env).unwrap();
    assert_eq!(payload.creator, creator);
    assert_eq!(payload.old_recipient, creator);
    assert_eq!(payload.new_recipient, new_recipient);
}

#[test]
fn test_update_creator_fee_recipient_fee_routing_after_rotation() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &1000);
    client.set_fee_config(&admin, &9000_u32, &1000_u32);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000, &None);

    let balance_before = client.get_creator_fee_balance(&creator);

    let new_recipient = Address::generate(&env);
    client.update_creator_fee_recipient(&creator, &creator, &new_recipient);

    assert_eq!(
        client.get_creator_fee_recipient(&creator),
        new_recipient,
        "fee recipient should be rotated"
    );

    let buyer2 = Address::generate(&env);
    client.buy_key(&creator, &buyer2, &1000, &None);

    let balance_after = client.get_creator_fee_balance(&creator);
    assert!(
        balance_after > balance_before,
        "creator fees should continue accruing after rotation"
    );
}

#[test]
fn test_update_creator_fee_recipient_chained_rotation() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let second = Address::generate(&env);
    client.update_creator_fee_recipient(&creator, &creator, &second);
    assert_eq!(client.get_creator_fee_recipient(&creator), second);

    let third = Address::generate(&env);
    client.update_creator_fee_recipient(&creator, &second, &third);
    assert_eq!(client.get_creator_fee_recipient(&creator), third);

    let attempt = Address::generate(&env);
    let result = client.try_update_creator_fee_recipient(&creator, &creator, &attempt);
    assert_eq!(
        result,
        Err(Ok(ContractError::Unauthorized)),
        "original creator can no longer rotate after handing off"
    );
}
