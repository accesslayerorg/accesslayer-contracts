mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::ContractError;
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_update_protocol_fee_recipient_success() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let old_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_protocol_fee_recipient(&admin, &old_recipient);

    let result = client.try_update_protocol_fee_recipient(&admin, &new_recipient);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(new_recipient),
        "protocol fee recipient should be updated to new address"
    );
}

#[test]
fn test_update_protocol_fee_recipient_rejects_non_admin() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let not_admin = Address::generate(&env);
    let old_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_protocol_fee_recipient(&admin, &old_recipient);

    let result = client.try_update_protocol_fee_recipient(&not_admin, &new_recipient);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(old_recipient),
        "recipient should remain unchanged after unauthorized call"
    );
}

#[test]
fn test_update_protocol_fee_recipient_rejects_zero_address() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let old_recipient = Address::generate(&env);
    let zero_str = String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero_addr = Address::from_string(&zero_str);

    client.set_protocol_admin(&admin, &admin);
    client.set_protocol_fee_recipient(&admin, &old_recipient);

    let result = client.try_update_protocol_fee_recipient(&admin, &zero_addr);
    assert_eq!(result, Err(Ok(ContractError::ZeroAddress)));

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(old_recipient),
        "recipient should remain unchanged after zero address rejection"
    );
}

#[test]
fn test_update_protocol_fee_recipient_rejects_when_no_admin_set() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let caller = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    let result = client.try_update_protocol_fee_recipient(&caller, &new_recipient);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_update_protocol_fee_recipient_rejects_when_no_recipient_set() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);

    let result = client.try_update_protocol_fee_recipient(&admin, &new_recipient);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_update_protocol_fee_recipient_emits_event() {
    use soroban_sdk::testutils::Events;
    use soroban_sdk::TryIntoVal;

    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let old_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_protocol_fee_recipient(&admin, &old_recipient);

    client.update_protocol_fee_recipient(&admin, &new_recipient);

    let all_events = env.events().all();
    assert_eq!(all_events.len(), 1, "expected exactly one event");

    let (_contract_id, _topics, data): (
        Address,
        soroban_sdk::Vec<soroban_sdk::Val>,
        soroban_sdk::Val,
    ) = all_events.get(0).unwrap();

    let payload: creator_keys::events::ProtocolFeeRecipientUpdated =
        data.try_into_val(&env).unwrap();
    assert_eq!(payload.old_recipient, old_recipient);
    assert_eq!(payload.new_recipient, new_recipient);
}

#[test]
fn test_update_protocol_fee_recipient_fee_routing_after_rotation() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let admin = Address::generate(&env);
    let old_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_protocol_fee_recipient(&admin, &old_recipient);
    client.set_key_price(&admin, &1000);
    client.set_fee_config(&admin, &9000_u32, &1000_u32);

    let creator = Address::generate(&env);
    let handle = soroban_sdk::String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000, &None);

    let balance_before = client.get_protocol_recipient_balance();

    client.update_protocol_fee_recipient(&admin, &new_recipient);

    assert_eq!(
        client.get_protocol_fee_recipient(),
        Some(new_recipient),
        "recipient should be rotated"
    );

    let buyer2 = Address::generate(&env);
    client.buy_key(&creator, &buyer2, &1000, &None);

    let balance_after = client.get_protocol_recipient_balance();
    assert!(
        balance_after > balance_before,
        "fees should continue accruing after rotation"
    );
}
