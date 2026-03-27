use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

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

    let result = client.try_set_key_price(&admin, &-1);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
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
