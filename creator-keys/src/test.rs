use super::*;
use soroban_sdk::testutils::Events;
use soroban_sdk::TryIntoVal;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_read_key_balance_returns_registered_creator_supply() {
    let env = Env::default();
    let creator = Address::generate(&env);
    let contract_id = env.register(CreatorKeysContract, ());

    let profile = CreatorProfile {
        creator: creator.clone(),
        handle: String::from_str(&env, "alice"),
        supply: 7,
        holder_count: 3,
        fee_recipient: creator.clone(),
    };

    let supply = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::creator(&creator), &profile);

        read_key_balance(&env, &creator)
    });
    assert_eq!(supply, 7);
}

#[test]
fn test_read_key_balance_returns_zero_for_missing_creator() {
    let env = Env::default();
    let missing_creator = Address::generate(&env);
    let contract_id = env.register(CreatorKeysContract, ());

    let supply = env.as_contract(&contract_id, || read_key_balance(&env, &missing_creator));
    assert_eq!(supply, 0);
}

#[test]
fn test_get_fee_config_returns_stored_protocol_config() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let config = fee::FeeConfig {
        creator_bps: 9000,
        protocol_bps: 1000,
    };

    let stored = env.as_contract(&contract_id, || {
        env.storage().persistent().set(&DataKey::FeeConfig, &config);
        CreatorKeysContract::get_fee_config(env.clone()).unwrap()
    });
    assert_eq!(stored.creator_bps, 9000);
    assert_eq!(stored.protocol_bps, 1000);
}

#[test]
fn test_read_protocol_fee_config_returns_stored_config() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let config = fee::FeeConfig {
        creator_bps: 8200,
        protocol_bps: 1800,
    };

    let stored = env.as_contract(&contract_id, || {
        env.storage().persistent().set(&DataKey::FeeConfig, &config);
        read_protocol_fee_config(&env).unwrap()
    });

    assert_eq!(stored.creator_bps, 8200);
    assert_eq!(stored.protocol_bps, 1800);
}

#[test]
fn test_read_required_protocol_fee_config_returns_stored_config() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let config = fee::FeeConfig {
        creator_bps: 7600,
        protocol_bps: 2400,
    };

    let stored = env.as_contract(&contract_id, || {
        env.storage().persistent().set(&DataKey::FeeConfig, &config);
        read_required_protocol_fee_config(&env).unwrap()
    });

    assert_eq!(stored.creator_bps, 7600);
    assert_eq!(stored.protocol_bps, 2400);
}

#[test]
fn test_get_fee_config_reads_protocol_fee_bps() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let config = fee::FeeConfig {
        creator_bps: 7500,
        protocol_bps: 2500,
    };

    let stored = env.as_contract(&contract_id, || {
        env.storage().persistent().set(&DataKey::FeeConfig, &config);
        CreatorKeysContract::get_fee_config(env.clone()).unwrap()
    });
    assert_eq!(stored.protocol_bps, 2500);
}

#[test]
fn test_get_fee_config_persists_across_repeated_reads() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator_bps = 9500;
    let protocol_bps = 500;
    client.set_fee_config(&admin, &creator_bps, &protocol_bps);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    // Repeatedly read the fee config and verify stability
    for _ in 0..5 {
        let config = client.get_fee_config().unwrap();
        assert_eq!(config.creator_bps, creator_bps);
        assert_eq!(config.protocol_bps, protocol_bps);

        let creator_fee_bps = client.get_creator_fee_bps(&creator);
        assert_eq!(creator_fee_bps, creator_bps);

        let protocol_share = client.get_protocol_treasury_share_bps();
        assert_eq!(protocol_share, protocol_bps);
    }
}

#[test]
fn test_register_creator() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.handle, handle);
    assert_eq!(profile.creator, creator);
    assert_eq!(profile.supply, 0);
    assert_eq!(profile.holder_count, 0);
    assert_eq!(profile.fee_recipient, creator);
}

#[test]
fn test_register_creator_persists_registration_metadata() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.creator, creator);
    assert_eq!(profile.handle, handle);
    assert_eq!(profile.supply, 0);
    assert_eq!(profile.holder_count, 0);
    assert_eq!(profile.fee_recipient, creator);
}

#[test]
fn test_duplicate_registration_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle);

    // Second registration should fail with AlreadyRegistered error
    let result = client.try_register_creator(&creator, &handle);
    assert_eq!(result, Err(Ok(ContractError::AlreadyRegistered)));
    assert_no_events(&env);
}

#[test]
fn test_buy_key_fails_if_not_registered() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    let result = client.try_buy_key(&creator, &buyer, &100);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
    assert_no_events(&env);
}

#[test]
fn test_buy_key_success() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let buyer = Address::generate(&env);
    let supply = client.buy_key(&creator, &buyer, &100);
    assert_eq!(supply, 1);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.supply, 1);
    assert_eq!(profile.holder_count, 1);
}

#[test]
fn test_get_creator_holder_count_counts_unique_holders() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let holder_one = Address::generate(&env);
    let holder_two = Address::generate(&env);

    client.buy_key(&creator, &holder_one, &100);
    client.buy_key(&creator, &holder_one, &100);
    client.buy_key(&creator, &holder_two, &100);

    let first_read = client.get_creator_holder_count(&creator);
    let second_read = client.get_creator_holder_count(&creator);

    assert_eq!(first_read, 2);
    assert_eq!(second_read, 2);
}

#[test]
fn test_get_creator_fails_if_not_registered() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);

    let result = client.try_get_creator(&creator);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_buy_key_insufficient_payment() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &100);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let buyer = Address::generate(&env);
    let result = client.try_buy_key(&creator, &buyer, &99);
    assert_eq!(result, Err(Ok(ContractError::InsufficientPayment)));
    assert_no_events(&env);
}

#[test]
fn test_set_key_price_invalid_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let result = client.try_set_key_price(&admin, &0);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
    assert_no_events(&env);

    let result = client.try_set_key_price(&admin, &-1);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
    assert_no_events(&env);
}

#[test]
fn test_get_key_balance_returns_zero_for_unregistered_creator() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);
    let wallet = Address::generate(&env);

    let balance = client.get_key_balance(&unregistered_creator, &wallet);
    assert_eq!(balance, 0);
}

#[test]
fn test_is_creator_registered_returns_false_for_unregistered() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);

    let is_registered = client.is_creator_registered(&unregistered_creator);
    assert!(!is_registered);
}

#[test]
fn test_get_total_key_supply_returns_zero_for_unregistered() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);

    let supply = client.get_total_key_supply(&unregistered_creator);
    assert_eq!(supply, 0);
}

#[test]
fn test_get_key_balance_returns_zero_for_unregistered_wallet() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let unregistered_wallet = Address::generate(&env);

    let balance = client.get_key_balance(&creator, &unregistered_wallet);
    assert_eq!(balance, 0);
}

#[test]
fn test_get_creator_fee_config_returns_defaults_for_unregistered() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);

    let fee_view = client.get_creator_fee_config(&unregistered_creator);
    assert!(!fee_view.is_registered);
    assert!(!fee_view.is_configured);
    assert_eq!(fee_view.creator_bps, 0);
    assert_eq!(fee_view.protocol_bps, 0);
}

#[test]
fn test_get_treasury_address_returns_none_initially() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let result = client.get_treasury_address();
    assert_eq!(result, None);
}

#[test]
fn test_get_treasury_address_returns_set_address() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_treasury_address(&admin, &treasury);

    let result = client.get_treasury_address();
    assert_eq!(result, Some(treasury));
}

#[test]
fn test_get_treasury_address_persists_across_reads() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_treasury_address(&admin, &treasury);

    let first_read = client.get_treasury_address();
    let second_read = client.get_treasury_address();
    assert_eq!(first_read, second_read);
    assert_eq!(first_read, Some(treasury));
}

#[test]
fn test_get_treasury_address_returns_updated_value_after_admin_update() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury_old = Address::generate(&env);
    let treasury_new = Address::generate(&env);

    client.set_treasury_address(&admin, &treasury_old);
    assert_eq!(client.get_treasury_address(), Some(treasury_old.clone()));

    client.set_treasury_address(&admin, &treasury_new);
    let result = client.get_treasury_address();

    assert_eq!(result, Some(treasury_new));
    assert_ne!(result, Some(treasury_old));
}

#[test]
fn test_get_buy_quote_success() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &1000);
    client.set_fee_config(&admin, &9000, &1000); // 90/10 split

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let quote = client.get_buy_quote(&creator);
    assert_eq!(quote.price, 1000);
    assert_eq!(quote.creator_fee, 900);
    assert_eq!(quote.protocol_fee, 100);
    assert_eq!(quote.total_amount, 2000); // 1000 + 900 + 100
}

#[test]
fn test_get_sell_quote_success() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &1000);
    client.set_fee_config(&admin, &9000, &1000); // 90/10 split

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000);

    let quote = client.get_sell_quote(&creator, &buyer);
    assert_eq!(quote.price, 1000);
    assert_eq!(quote.creator_fee, 900);
    assert_eq!(quote.protocol_fee, 100);
    assert_eq!(quote.total_amount, 0); // 1000 - 900 - 100
}

#[test]
fn test_get_sell_quote_fails_if_insufficient_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &1000);
    client.set_fee_config(&admin, &9000, &1000);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let holder = Address::generate(&env); // Zero balance
    let result = client.try_get_sell_quote(&creator, &holder);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

#[test]
fn test_get_quote_fails_if_not_registered() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &1000);

    let creator = Address::generate(&env); // Not registered
    let result = client.try_get_buy_quote(&creator);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_get_quote_fails_if_fee_not_set() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &1000);
    // Fee config NOT set

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let result = client.try_get_buy_quote(&creator);
    assert_eq!(result, Err(Ok(ContractError::FeeConfigNotSet)));
}

#[test]
fn test_get_buy_quote_fails_if_not_registered() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &1000);

    let unregistered_creator = Address::generate(&env);
    let result = client.try_get_buy_quote(&unregistered_creator);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_get_creator_fee_recipient_success() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let recipient = client.get_creator_fee_recipient(&creator);
    assert_eq!(recipient, creator);
}

#[test]
fn test_get_creator_fee_recipient_fails_if_not_registered() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let unregistered_creator = Address::generate(&env);
    let result = client.try_get_creator_fee_recipient(&unregistered_creator);
    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
}

#[test]
fn test_quote_overflow_guards() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    // Set a massive price that will cause overflow when fees are added
    let max_price = i128::MAX - 1;
    client.set_key_price(&admin, &max_price);
    client.set_fee_config(&admin, &9000, &1000); // 90/10 split

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    // Buy quote: price + fees (will overflow)
    let result = client.try_get_buy_quote(&creator);
    assert_eq!(result, Err(Ok(ContractError::Overflow)));

    // Sell quote: price - fees (won't overflow if price is large, but let's test sub overflow)
    // Actually price - fees is safe if price > fees.
    // To test subtraction overflow, we need fees > price.
    // Price must be positive per contract constraint.
}

#[test]
fn test_get_protocol_admin_returns_none_initially() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let result = client.get_protocol_admin();
    assert_eq!(result, None);
}

#[test]
fn test_get_protocol_admin_returns_set_address() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &new_admin);

    let result = client.get_protocol_admin();
    assert_eq!(result, Some(new_admin));
}

#[test]
fn test_get_protocol_admin_persists_across_reads() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &new_admin);

    let first_read = client.get_protocol_admin();
    let second_read = client.get_protocol_admin();
    assert_eq!(first_read, second_read);
    assert_eq!(first_read, Some(new_admin));
}

// ---------------------------------------------------------------------------
// Regression tests: fee component ordering in emitted event payloads
//
// These tests lock down the stable field order of event payloads that carry
// fee-adjacent data. They are intentionally narrow: they assert ordering and
// presence of specific fields, not unrelated payload changes.
// ---------------------------------------------------------------------------

/// Regression: `CreatorRegisteredEvent` field order must remain
/// `(creator, handle, supply, holder_count)`.
///
/// The `REGISTER_EVENT_DATA_FIELDS` constant documents the intended order.
/// This test confirms the emitted payload matches that declaration so that
/// downstream indexers relying on positional field access do not silently break.
#[test]
fn test_register_event_field_order_is_stable() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    client.register_creator(&creator, &handle);

    let all_events = env.events().all();
    assert_eq!(
        all_events.len(),
        1,
        "expected exactly one event after registration"
    );

    let (_contract_id, topics, data): (
        Address,
        soroban_sdk::Vec<soroban_sdk::Val>,
        soroban_sdk::Val,
    ) = all_events.get(0).unwrap();

    // Topic[0] must be the event name symbol, Topic[1] must be the creator address.
    // This mirrors TOPIC_EVENT_NAME_INDEX=0 and TOPIC_CREATOR_INDEX=1.
    let event_name: soroban_sdk::Symbol = topics
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .unwrap()
        .try_into_val(&env)
        .unwrap();
    let topic_creator: Address = topics
        .get(events::TOPIC_CREATOR_INDEX)
        .unwrap()
        .try_into_val(&env)
        .unwrap();
    assert_eq!(event_name, events::REGISTER_EVENT_NAME);
    assert_eq!(topic_creator, creator);

    // Data must deserialise as CreatorRegisteredEvent with fields in declared order.
    let payload: events::CreatorRegisteredEvent = data.try_into_val(&env).unwrap();
    assert_eq!(payload.creator, creator, "field[0] creator mismatch");
    assert_eq!(payload.handle, handle, "field[1] handle mismatch");
    assert_eq!(payload.supply, 0, "field[2] supply mismatch");
    assert_eq!(payload.holder_count, 0, "field[3] holder_count mismatch");

    // Confirm the constant declaration matches the struct field order.
    assert_eq!(events::REGISTER_EVENT_DATA_FIELDS[0], "creator");
    assert_eq!(events::REGISTER_EVENT_DATA_FIELDS[1], "handle");
    assert_eq!(events::REGISTER_EVENT_DATA_FIELDS[2], "supply");
    assert_eq!(events::REGISTER_EVENT_DATA_FIELDS[3], "holder_count");
}

/// Regression: buy event topic order must remain `(BUY_EVENT_NAME, creator, buyer)`.
///
/// The `BUY_EVENT_DATA_FIELDS` constant documents the intended tuple order
/// `(supply, payment)`. This test confirms both topic and data ordering so that
/// indexers relying on positional topic access do not silently break.
#[test]
fn test_buy_event_topic_and_data_order_is_stable() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_key_price(&admin, &500);
    client.set_fee_config(&admin, &9000, &1000);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "bob");
    client.register_creator(&creator, &handle);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &500);

    let all_events = env.events().all();
    // Each client call is a separate invocation; env.events().all() returns events
    // from the most recent call only. After buy_key, exactly one buy event is present.
    assert_eq!(all_events.len(), 1, "expected exactly one buy event");

    // The buy event is the only one emitted in this invocation.
    let (_contract_id, topics, data): (
        Address,
        soroban_sdk::Vec<soroban_sdk::Val>,
        soroban_sdk::Val,
    ) = all_events.get(all_events.len() - 1).unwrap();

    // Topic[0] = event name, Topic[1] = creator, Topic[2] = buyer
    let event_name: soroban_sdk::Symbol = topics
        .get(events::TOPIC_EVENT_NAME_INDEX)
        .unwrap()
        .try_into_val(&env)
        .unwrap();
    let topic_creator: Address = topics
        .get(events::TOPIC_CREATOR_INDEX)
        .unwrap()
        .try_into_val(&env)
        .unwrap();
    let topic_buyer: Address = topics
        .get(events::TOPIC_BUYER_INDEX)
        .unwrap()
        .try_into_val(&env)
        .unwrap();
    assert_eq!(
        event_name,
        events::BUY_EVENT_NAME,
        "topic[0] event name mismatch"
    );
    assert_eq!(topic_creator, creator, "topic[1] creator mismatch");
    assert_eq!(topic_buyer, buyer, "topic[2] buyer mismatch");

    // Data tuple must be (supply: u32, payment: i128) — supply first, payment second.
    let (supply, payment): (u32, i128) = data.try_into_val(&env).unwrap();
    assert_eq!(supply, 1, "data[0] supply mismatch");
    assert_eq!(payment, 500, "data[1] payment mismatch");

    // Confirm the constant declaration matches the tuple field order.
    assert_eq!(events::BUY_EVENT_DATA_FIELDS[0], "supply");
    assert_eq!(events::BUY_EVENT_DATA_FIELDS[1], "payment");
}

/// Regression: `CreatorRegisteredEvent` initial fee-adjacent fields (`supply`,
/// `holder_count`) must be zero at registration time and must not be reordered
/// relative to identity fields (`creator`, `handle`).
///
/// This guards against a refactor accidentally swapping the numeric tail fields
/// with the address/string head fields, which would silently corrupt indexer reads.
#[test]
fn test_register_event_fee_adjacent_fields_are_zero_and_ordered_after_identity_fields() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "carol");
    client.register_creator(&creator, &handle);

    let all_events = env.events().all();
    let (_contract_id, _topics, data): (
        Address,
        soroban_sdk::Vec<soroban_sdk::Val>,
        soroban_sdk::Val,
    ) = all_events.get(0).unwrap();

    let payload: events::CreatorRegisteredEvent = data.try_into_val(&env).unwrap();

    // Identity fields come first (indices 0 and 1 in REGISTER_EVENT_DATA_FIELDS).
    assert_eq!(
        payload.creator, creator,
        "identity field 'creator' must be first"
    );
    assert_eq!(
        payload.handle, handle,
        "identity field 'handle' must be second"
    );

    // Fee-adjacent numeric fields come after identity fields (indices 2 and 3).
    // Both must be zero at registration time — no supply or holders yet.
    assert_eq!(
        payload.supply, 0,
        "fee-adjacent field 'supply' must be zero at registration"
    );
    assert_eq!(
        payload.holder_count, 0,
        "fee-adjacent field 'holder_count' must be zero at registration"
    );

    // Cross-check: the constant array must place numeric fields after identity fields.
    let identity_fields = &events::REGISTER_EVENT_DATA_FIELDS[..2];
    let numeric_fields = &events::REGISTER_EVENT_DATA_FIELDS[2..];
    assert_eq!(identity_fields, &["creator", "handle"]);
    assert_eq!(
        numeric_fields,
        &["supply", "holder_count", "creator_bps", "protocol_bps"]
    );
}

/// Asserts that no events were emitted during the most recent contract call.
///
/// In the Soroban test environment, `env.events().all()` returns events from
/// the most recent top-level invocation only. This helper confirms that the
/// event log for the last call is empty, typically used to verify that failed
/// transactions did not leave side-effect artifacts in the event stream.
fn assert_no_events(env: &Env) {
    let all_events = env.events().all();
    assert_eq!(
        all_events.len(),
        0,
        "Expected no events to be emitted, but found: {:?}",
        all_events
    );
}
