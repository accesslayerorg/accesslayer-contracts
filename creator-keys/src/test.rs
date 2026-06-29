use super::*;
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::TryIntoVal;
use soroban_sdk::{Address, Env, String};

// --- Time-locked key allocation tests (#404) ---

#[test]
fn test_register_creator_with_locked_allocation() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    let locked = LockedAllocation {
        amount: 100,
        unlock_ledger: 1000,
        claimed: false,
    };

    client.register_creator(&creator, &handle, &Some(locked), &None, &None, &None);

    let stored = client.get_locked_allocation(&creator).unwrap();
    assert_eq!(stored.amount, 100);
    assert_eq!(stored.unlock_ledger, 1000);
    assert!(!stored.claimed);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.supply, 100);
}

#[test]
fn test_register_creator_locked_allocation_reverts_past_ledger() {
    let env = Env::default();
    env.mock_all_auths();
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 500;
    env.ledger().set(ledger_info.clone());
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    let locked = LockedAllocation {
        amount: 100,
        unlock_ledger: 400,
        claimed: false,
    };

    let result = client.try_register_creator(&creator, &handle, &Some(locked), &None, &None, &None);
    assert_eq!(result, Err(Ok(ContractError::AllocationLocked)));
}

#[test]
fn test_claim_locked_allocation_success() {
    let env = Env::default();
    env.mock_all_auths();
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info.clone());
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    let locked = LockedAllocation {
        amount: 50,
        unlock_ledger: 200,
        claimed: false,
    };

    client.register_creator(&creator, &handle, &Some(locked), &None, &None, &None);

    // Advance ledger past unlock
    ledger_info.sequence_number = 250;
    env.ledger().set(ledger_info);

    client.claim_locked_allocation(&creator);

    let stored = client.get_locked_allocation(&creator).unwrap();
    assert!(stored.claimed);

    let balance = client.get_key_balance(&creator, &creator);
    assert_eq!(balance, 50);
}

#[test]
fn test_claim_locked_allocation_reverts_early() {
    let env = Env::default();
    env.mock_all_auths();
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info.clone());
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    let locked = LockedAllocation {
        amount: 50,
        unlock_ledger: 200,
        claimed: false,
    };

    client.register_creator(&creator, &handle, &Some(locked), &None, &None, &None);

    // Try to claim before unlock
    let result = client.try_claim_locked_allocation(&creator);
    assert_eq!(result, Err(Ok(ContractError::AllocationLocked)));
}

#[test]
fn test_claim_locked_allocation_reverts_double_claim() {
    let env = Env::default();
    env.mock_all_auths();
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info.clone());
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    let locked = LockedAllocation {
        amount: 50,
        unlock_ledger: 200,
        claimed: false,
    };

    client.register_creator(&creator, &handle, &Some(locked), &None, &None, &None);

    // Advance ledger past unlock
    ledger_info.sequence_number = 250;
    env.ledger().set(ledger_info);

    client.claim_locked_allocation(&creator);

    // Try to claim again
    let result = client.try_claim_locked_allocation(&creator);
    assert_eq!(result, Err(Ok(ContractError::AlreadyClaimed)));
}

#[test]
fn test_get_locked_allocation_returns_none_when_not_set() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);

    let result = client.get_locked_allocation(&creator);
    assert_eq!(result, None);
}

#[test]
fn test_get_locked_allocation_returns_allocation_when_set() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");
    let locked = LockedAllocation {
        amount: 100,
        unlock_ledger: 1000,
        claimed: false,
    };

    client.register_creator(&creator, &handle, &Some(locked), &None, &None, &None);

    let result = client.get_locked_allocation(&creator).unwrap();
    assert_eq!(result.amount, 100);
    assert_eq!(result.unlock_ledger, 1000);
    assert!(!result.claimed);
}

// --- Transfer keys tests ---

#[test]
fn test_transfer_keys_basic() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None, &None);
    client.buy_key(&creator, &sender, &100i128, &None);
    client.buy_key(&creator, &sender, &100i128, &None);
    client.buy_key(&creator, &sender, &100i128, &None);

    client.transfer_keys(&creator, &sender, &recipient, &1);

    assert_eq!(client.get_key_balance(&creator, &sender), 2);
    assert_eq!(client.get_key_balance(&creator, &recipient), 1);
    assert_eq!(client.get_total_key_supply(&creator), 3);
}

#[test]
fn test_transfer_keys_sender_zeroed_out() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None, &None);
    client.buy_key(&creator, &sender, &100i128, &None);

    client.transfer_keys(&creator, &sender, &recipient, &1);

    assert_eq!(client.get_key_balance(&creator, &sender), 0);
    assert_eq!(client.get_key_balance(&creator, &recipient), 1);
    assert_eq!(client.get_total_key_supply(&creator), 1);
}

#[test]
fn test_transfer_keys_new_recipient() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None, &None);
    client.buy_key(&creator, &sender, &100i128, &None);

    let supply_before = client.get_total_key_supply(&creator);
    client.transfer_keys(&creator, &sender, &recipient, &1);
    let supply_after = client.get_total_key_supply(&creator);

    assert_eq!(supply_before, supply_after, "supply must not change");
    assert_eq!(client.get_key_balance(&creator, &recipient), 1);
}

#[test]
fn test_transfer_keys_self_transfer_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None, &None);
    client.buy_key(&creator, &sender, &100i128, &None);

    let result = client.try_transfer_keys(&creator, &sender, &sender, &1);
    assert_eq!(result, Err(Ok(ContractError::SelfTransfer)));
}

#[test]
fn test_transfer_keys_zero_amount_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None, &None);
    client.buy_key(&creator, &sender, &100i128, &None);

    let result = client.try_transfer_keys(&creator, &sender, &recipient, &0);
    assert_eq!(result, Err(Ok(ContractError::ZeroTransferAmount)));
}

#[test]
fn test_transfer_keys_insufficient_balance_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.set_key_price(&admin, &100i128);
    client.register_creator(&creator, &String::from_str(&env, "alice"), &None, &None, &None, &None);
    client.buy_key(&creator, &sender, &100i128, &None);

    let result = client.try_transfer_keys(&creator, &sender, &recipient, &2);
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));
}

// --- Max supply cap tests (#394) ---

#[test]
fn test_register_creator_with_max_supply() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle, &None, &Some(1000), &None, &None);

    let cap = client.get_max_supply(&creator).unwrap();
    assert_eq!(cap, 1000);
}

#[test]
fn test_register_creator_max_supply_zero_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    let result = client.try_register_creator(&creator, &handle, &None, &Some(0), &None, &None);
    assert_eq!(result, Err(Ok(ContractError::NotPositiveAmount)));
}

#[test]
fn test_buy_exceeds_max_supply_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let admin = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle, &None, &Some(5), &None, &None);
    client.set_key_price(&admin, &100);
    client.set_fee_config(&admin, &9000, &1000);

    // Buy 5 keys to reach cap
    for _ in 0..5 {
        client.buy_key(&creator, &buyer, &100, &None);
    }

    // Try to buy one more - should revert
    let result = client.try_buy_key(&creator, &buyer, &100, &None);
    assert_eq!(result, Err(Ok(ContractError::SupplyCapExceeded)));
}

#[test]
fn test_buy_within_max_supply_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let admin = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle, &None, &Some(10), &None, &None);
    client.set_key_price(&admin, &100);
    client.set_fee_config(&admin, &9000, &1000);

    // Buy 5 keys (within cap)
    for _ in 0..5 {
        client.buy_key(&creator, &buyer, &100, &None);
    }

    let supply = client.get_creator_supply(&creator);
    assert_eq!(supply, 5);
}

#[test]
fn test_get_max_supply_returns_none_for_uncapped() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    let cap = client.get_max_supply(&creator);
    assert_eq!(cap, None);
}

// --- Protocol fee recipient rotation tests (#395) ---

#[test]
fn test_update_protocol_fee_recipient_success() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let old_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::ADMIN_ADDRESS, &admin);
        env.storage()
            .persistent()
            .set(&constants::storage::PROTOCOL_FEE_RECIPIENT, &old_recipient);
    });

    client.update_protocol_fee_recipient(&admin, &new_recipient);

    let stored = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get::<DataKey, Address>(&constants::storage::PROTOCOL_FEE_RECIPIENT)
            .unwrap()
    });
    assert_eq!(stored, new_recipient);
}

#[test]
fn test_update_protocol_fee_recipient_unauthorized_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let old_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::ADMIN_ADDRESS, &admin);
        env.storage()
            .persistent()
            .set(&constants::storage::PROTOCOL_FEE_RECIPIENT, &old_recipient);
    });

    let result = client.try_update_protocol_fee_recipient(&unauthorized, &new_recipient);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_update_protocol_fee_recipient_zero_address_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let old_recipient = Address::generate(&env);
    let zero_str = String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero_addr = Address::from_string(&zero_str);

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::ADMIN_ADDRESS, &admin);
        env.storage()
            .persistent()
            .set(&constants::storage::PROTOCOL_FEE_RECIPIENT, &old_recipient);
    });

    let result = client.try_update_protocol_fee_recipient(&admin, &zero_addr);
    assert_eq!(result, Err(Ok(ContractError::ZeroAddress)));
}

#[test]
fn test_update_creator_fee_recipient_success() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let new_recipient = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle, &None, &None, &None, &None);
    client.update_creator_fee_recipient(&creator, &new_recipient);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.fee_recipient, new_recipient);
}

#[test]
fn test_update_creator_fee_recipient_unauthorized_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let new_recipient = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    let result = client.try_update_creator_fee_recipient(&unauthorized, &new_recipient);
    // This should fail because unauthorized is not the current fee recipient
    assert!(result.is_err());
}

#[test]
fn test_update_creator_fee_recipient_reverts_when_current_recipient_is_not_authorized() {
    let env = Env::default();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let current_recipient = Address::generate(&env);
    let new_recipient = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    let profile = CreatorProfile {
        creator: creator.clone(),
        handle: handle.clone(),
        supply: 0,
        holder_count: 0,
        fee_recipient: current_recipient.clone(),
        registered_at: 0,
    };

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&constants::storage::creator(&creator), &profile);
    });

    let result = client.try_update_creator_fee_recipient(&creator, &new_recipient);
    assert!(result.is_err());
}

#[test]
fn test_sell_key_accepts_exact_min_proceeds_boundary() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let seller = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.set_key_price(&admin, &100);
    client.set_fee_config(&admin, &9000, &1000);
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    client.buy_key(&creator, &seller, &100, &None);
    client.buy_key(&creator, &seller, &100, &None);

    let quote = client.get_sell_quote(&creator, &seller).unwrap();
    let exact_result = client.try_sell_key(&creator, &seller, &Some(quote.total_amount));
    assert_eq!(exact_result, Ok(Ok(1)));

    let second_quote = client.get_sell_quote(&creator, &seller).unwrap();
    let slippage_result = client.try_sell_key(&creator, &seller, &Some(second_quote.total_amount + 1));
    assert_eq!(slippage_result, Err(Ok(ContractError::SlippageExceeded)));
}

#[test]
fn test_sell_extends_creator_ttl_after_successful_sell() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let seller = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.set_key_price(&admin, &100);
    client.set_fee_config(&admin, &9000, &1000);
    client.register_creator(&creator, &handle, &None, &None, &None, &None);
    client.buy_key(&creator, &seller, &100, &None);

    let creator_key = constants::storage::creator(&creator);
    let initial_profile: CreatorProfile = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&creator_key).unwrap()
    });
    assert_eq!(initial_profile.supply, 1);

    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);

    let result = client.try_sell_key(&creator, &seller, &Some(1));
    assert_eq!(result, Ok(Ok(0)));

    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = CREATOR_TTL_LEDGERS + 1;
    env.ledger().set(ledger_info);

    let has_profile = env.as_contract(&contract_id, || {
        env.storage().persistent().has(&creator_key)
    });
    assert!(has_profile);
}

#[test]
fn test_failed_sell_does_not_extend_creator_ttl() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let seller = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    client.set_key_price(&admin, &100);
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    let creator_key = constants::storage::creator(&creator);
    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);

    let result = client.try_sell_key(&creator, &seller, &Some(1));
    assert_eq!(result, Err(Ok(ContractError::InsufficientBalance)));

    let mut ledger_info = env.ledger().get();
    ledger_info.sequence_number = CREATOR_TTL_LEDGERS + 1;
    env.ledger().set(ledger_info);

    let has_profile = env.as_contract(&contract_id, || {
        env.storage().persistent().has(&creator_key)
    });
    assert!(!has_profile);
}

// --- TTL extension tests (#396) ---

#[test]
fn test_register_creator_without_optional_params_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let handle = String::from_str(&env, "alice");

    // Registration with None for both optional params should work (backwards compatible)
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    let profile = client.get_creator(&creator);
    assert_eq!(profile.supply, 0);
    assert_eq!(client.get_max_supply(&creator), None);
    assert_eq!(client.get_locked_allocation(&creator), None);
}

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
        registered_at: 0,
        curve_preset: crate::bonding_curve::CurvePreset::Linear, // ADD THIS
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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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

    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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

    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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

    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    // Second registration should fail with AlreadyRegistered error
    let result = client.try_register_creator(&creator, &handle, &None, &None, &None, &None);
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

    let result = client.try_buy_key(&creator, &buyer, &100, &None);
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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    let buyer = Address::generate(&env);
    let supply = client.buy_key(&creator, &buyer, &100, &None);
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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    let holder_one = Address::generate(&env);
    let holder_two = Address::generate(&env);

    client.buy_key(&creator, &holder_one, &100, &None);
    client.buy_key(&creator, &holder_one, &100, &None);
    client.buy_key(&creator, &holder_two, &100, &None);

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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    let buyer = Address::generate(&env);
    let result = client.try_buy_key(&creator, &buyer, &99, &None);
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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000, &None);

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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &500, &None);

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
    client.register_creator(&creator, &handle, &None, &None, &None, &None);

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

fn assert_no_events(env: &Env) {
    let all_events = env.events().all();
    assert_eq!(
        all_events.len(),
        0,
        "Expected no events to be emitted, but found: {:?}",
        all_events
    );
}

// --- Checked-Addition Helper Tests (#216) ---

#[test]
fn test_checked_accumulate_normal_addition() {
    // Verify that normal accumulations work correctly
    let result = fee::checked_accumulate(100, 50);
    assert_eq!(result, Ok(150), "normal accumulation should succeed");
}

#[test]
fn test_checked_accumulate_with_negative_delta() {
    // Verify that negative deltas (e.g., correction) are handled correctly
    let result = fee::checked_accumulate(100, -30);
    assert_eq!(
        result,
        Ok(70),
        "accumulation with negative delta should work"
    );
}

#[test]
fn test_checked_accumulate_zero_delta() {
    // Verify that adding zero is a no-op
    let result = fee::checked_accumulate(500, 0);
    assert_eq!(result, Ok(500), "adding zero should return same value");
}

#[test]
fn test_checked_accumulate_zero_to_positive() {
    // Verify that starting from zero works
    let result = fee::checked_accumulate(0, 123);
    assert_eq!(result, Ok(123), "accumulating into zero should work");
}

#[test]
fn test_checked_accumulate_large_values() {
    // Verify that large but valid values accumulate correctly
    let large_current = 1_000_000_000_000i128;
    let large_delta = 500_000_000_000i128;
    let result = fee::checked_accumulate(large_current, large_delta);
    assert_eq!(
        result,
        Ok(1_500_000_000_000),
        "large value accumulation should work"
    );
}

#[test]
fn test_checked_accumulate_detects_positive_overflow() {
    // Verify that overflow is detected when adding towards i128::MAX
    let near_max = i128::MAX - 100;
    let result = fee::checked_accumulate(near_max, 200);
    assert_eq!(
        result,
        Err(ContractError::Overflow),
        "should detect overflow"
    );
}

#[test]
fn test_checked_accumulate_detects_negative_overflow() {
    // Verify that underflow is detected when subtracting below i128::MIN
    let near_min = i128::MIN + 100;
    let result = fee::checked_accumulate(near_min, -200);
    assert_eq!(
        result,
        Err(ContractError::Overflow),
        "should detect underflow"
    );
}

#[test]
fn test_checked_accumulate_boundary_max_add_zero() {
    // Edge case: adding zero at MAX value should succeed
    let result = fee::checked_accumulate(i128::MAX, 0);
    assert_eq!(
        result,
        Ok(i128::MAX),
        "MAX + 0 should succeed and return MAX"
    );
}

#[test]
fn test_checked_accumulate_boundary_max_add_one() {
    // Edge case: adding one to MAX should overflow
    let result = fee::checked_accumulate(i128::MAX, 1);
    assert_eq!(
        result,
        Err(ContractError::Overflow),
        "MAX + 1 should overflow"
    );
}

#[test]
fn test_checked_accumulate_boundary_min_subtract_one() {
    // Edge case: subtracting one from MIN should underflow
    let result = fee::checked_accumulate(i128::MIN, -1);
    assert_eq!(
        result,
        Err(ContractError::Overflow),
        "MIN - 1 should underflow"
    );
}

#[test]
fn test_checked_accumulate_dividend_distribution_scenario() {
    // Simulate a realistic dividend distribution scenario with multiple accumulations
    let mut accumulator = 0i128;

    // Simulate several dividend distributions
    let distributions = [100i128, 250, -50, 300];

    for dist in distributions {
        accumulator = fee::checked_accumulate(accumulator, dist).expect("distribution failed");
    }

    // Total should be 100 + 250 - 50 + 300 = 600
    assert_eq!(
        accumulator, 600,
        "accumulated dividend should match sum of distributions"
    );
}

// --- Fee Accounting Balance-Conservation Tests (#144) ---

#[test]
fn test_fee_split_conservation_90_10_split() {
    // Verify that a 90% creator / 10% protocol split conserves all value
    let total = 1000i128;
    let (creator_fee, protocol_fee) = fee::compute_fee_split(total, 9000, 1000);

    // Assertion 1: Both fees are non-negative
    assert!(creator_fee >= 0, "creator_fee must be non-negative");
    assert!(protocol_fee >= 0, "protocol_fee must be non-negative");

    // Assertion 2: Fees sum exactly to the total (no value lost or created)
    assert_eq!(
        creator_fee + protocol_fee,
        total,
        "creator_fee ({}) + protocol_fee ({}) must equal total ({})",
        creator_fee,
        protocol_fee,
        total
    );

    // Assertion 3: Creator gets ~90%, protocol gets ~10%
    assert!(
        creator_fee >= total * 9 / 10,
        "creator_fee must be at least 90% of total"
    );
    assert!(
        protocol_fee <= total / 10 + 1,
        "protocol_fee must be at most 10% of total (plus rounding)"
    );
}

#[test]
fn test_fee_split_conservation_50_50_split() {
    // Verify that a 50% / 50% split conserves all value
    let total = 10000i128;
    let (creator_fee, protocol_fee) = fee::compute_fee_split(total, 5000, 5000);

    assert_eq!(
        creator_fee + protocol_fee,
        total,
        "50/50 split must conserve total value: {} + {} != {}",
        creator_fee,
        protocol_fee,
        total
    );

    // Due to rounding, creator gets the remainder
    assert!(creator_fee >= total / 2, "creator_fee must be at least 50%");
    assert!(
        protocol_fee <= total / 2,
        "protocol_fee must be at most 50%"
    );
}

#[test]
fn test_fee_split_conservation_100_creator_zero_protocol() {
    // Edge case: all fees go to creator, none to protocol
    let total = 500i128;
    let (creator_fee, protocol_fee) = fee::compute_fee_split(total, 10000, 0);

    assert_eq!(creator_fee, total, "creator must receive all value");
    assert_eq!(protocol_fee, 0, "protocol must receive zero");
    assert_eq!(creator_fee + protocol_fee, total, "total must be conserved");
}

#[test]
fn test_fee_split_conservation_boundary_price_one() {
    // Edge case: very low trade value (price = 1)
    let total = 1i128;
    let (creator_fee, protocol_fee) = fee::compute_fee_split(total, 9000, 1000);

    assert!(creator_fee >= 0, "creator_fee must be non-negative");
    assert!(protocol_fee >= 0, "protocol_fee must be non-negative");
    assert_eq!(
        creator_fee + protocol_fee,
        total,
        "even at price=1, fees must sum to total: {} + {} != {}",
        creator_fee,
        protocol_fee,
        total
    );
}

#[test]
fn test_fee_split_conservation_boundary_price_two() {
    // Edge case: low trade value (price = 2)
    let total = 2i128;
    let (creator_fee, protocol_fee) = fee::compute_fee_split(total, 9000, 1000);

    assert_eq!(
        creator_fee + protocol_fee,
        total,
        "at price=2, fees must sum to total"
    );
}

#[test]
fn test_fee_split_conservation_boundary_odd_price_999() {
    // Edge case: odd total that creates rounding scenario
    let total = 999i128;
    let (creator_fee, protocol_fee) = fee::compute_fee_split(total, 9000, 1000);

    assert_eq!(
        creator_fee + protocol_fee,
        total,
        "odd price 999 must still conserve all value: {} + {} != {}",
        creator_fee,
        protocol_fee,
        total
    );

    // The remainder (from 999 * 1000 / 10000 = 99.9 -> 99) goes to creator
    assert!(creator_fee > protocol_fee, "remainder should go to creator");
}

#[test]
fn test_fee_split_conservation_large_amount() {
    // Test with large amounts to ensure no overflow issues affect conservation
    let total = 100_000_000_000i128;
    let (creator_fee, protocol_fee) = fee::compute_fee_split(total, 8000, 2000);

    assert_eq!(
        creator_fee + protocol_fee,
        total,
        "large amount conservation check failed"
    );

    // Verify proportions are roughly correct
    assert!(
        creator_fee >= total * 4 / 5,
        "creator should get ~80% of large amounts"
    );
    assert!(
        protocol_fee <= total / 5 + 1,
        "protocol should get ~20% of large amounts"
    );
}

#[test]
fn test_fee_split_conservation_deterministic_assertions() {
    // Deterministic test: verify specific known conversions are conserved
    let test_cases = [
        (100, 9000, 1000),  // 90/10 split: 90 / 10
        (1000, 5000, 5000), // 50/50 split: 500 / 500
        (2000, 8000, 2000), // 80/20 split: 1600 / 400
        (333, 7000, 3000),  // 70/30 split: ~233 / ~100
    ];

    for (total, creator_bps, protocol_bps) in test_cases {
        let (creator_fee, protocol_fee) = fee::compute_fee_split(total, creator_bps, protocol_bps);
        assert_eq!(
            creator_fee + protocol_fee,
            total,
            "failed for total={}, creator_bps={}, protocol_bps={}",
            total,
            creator_bps,
            protocol_bps
        );
    }
}

#[test]
fn test_checked_fee_sum_conservation() {
    // Verify that checked_fee_sum helper maintains value conservation
    let creator_fee = 450i128;
    let protocol_fee = 50i128;

    let sum = fee::checked_fee_sum(creator_fee, protocol_fee);
    assert_eq!(
        sum,
        Some(500),
        "fee sum must equal creator_fee + protocol_fee"
    );
}

#[test]
fn test_checked_fee_sum_overflow_behavior() {
    // Verify that overflow is detected (returns None) rather than wrapping
    let creator_fee = i128::MAX;
    let protocol_fee = 1i128;

    let sum = fee::checked_fee_sum(creator_fee, protocol_fee);
    assert_eq!(sum, None, "fee sum must detect overflow and return None");
}

#[test]
fn test_fee_split_conservation_across_multiple_fee_configs() {
    // Verify conservation property holds for multiple protocol fee configurations
    let total = 5000i128;
    let fee_configs = [
        (10000, 0),   // 100% creator
        (9500, 500),  // 95% creator, 5% protocol
        (9000, 1000), // 90% creator, 10% protocol
        (8200, 1800), // 82% creator, 18% protocol
        (5000, 5000), // 50% creator, 50% protocol
        (0, 10000),   // 100% protocol (edge case)
    ];

    for (creator_bps, protocol_bps) in fee_configs {
        let (creator_fee, protocol_fee) = fee::compute_fee_split(total, creator_bps, protocol_bps);
        assert_eq!(
            creator_fee + protocol_fee,
            total,
            "conservation check failed for config ({}, {})",
            creator_bps,
            protocol_bps
        );
    }
}

#[test]
fn test_fee_split_conservation_across_price_range() {
    // Verify conservation for a range of prices to ensure no systematic drift
    let prices = [1, 2, 10, 99, 100, 101, 500, 999, 1000, 10_000];

    for price in prices {
        let (creator_fee, protocol_fee) = fee::compute_fee_split(price, 9000, 1000);
        assert_eq!(
            creator_fee + protocol_fee,
            price,
            "conservation failed for price={}",
            price
        );
    }
}

#[test]
fn test_zero_net_boundary_seller_gets_zero_proceeds() {
    // Edge case: when fees equal or exceed price, seller should get zero (not negative)
    // This is a boundary condition that must be handled without allowing negative proceeds
    let price = 10i128;
    let (creator_fee, protocol_fee) = fee::compute_fee_split(price, 1000, 9000);

    // Total fees
    let total_fees = creator_fee + protocol_fee;
    assert_eq!(total_fees, price, "fees must sum to price");

    // Seller's net proceeds would be price - fees = 0
    let net_proceeds = price - total_fees;
    assert!(net_proceeds >= 0, "proceeds must not be negative");
    assert_eq!(
        net_proceeds, 0,
        "with extreme split, proceeds should be zero"
    );
}

// --- Compute Buyback Cost Helper Tests (#426) ---

#[test]
fn test_compute_buyback_cost_zero_fee_bps() {
    let gross_price = 1000i128;
    let result = fee::compute_buyback_cost(gross_price, 0);
    assert_eq!(
        result,
        Some(1000),
        "zero fee bps should return gross price unchanged"
    );
}

#[test]
fn test_compute_buyback_cost_standard_fee_bps() {
    let gross_price = 1000i128;
    let result = fee::compute_buyback_cost(gross_price, 1000);
    assert_eq!(
        result,
        Some(1100),
        "1000 bps (10%) on 1000 should yield 1100"
    );
}

#[test]
fn test_compute_buyback_cost_maximum_fee_bps() {
    let gross_price = 1000i128;
    let result = fee::compute_buyback_cost(gross_price, 5000);
    assert_eq!(
        result,
        Some(1500),
        "5000 bps (50%) on 1000 should yield 1500"
    );
}

// --- Compute Net Buyback Cost Helper Tests (#426) ---

#[test]
fn test_compute_net_buyback_cost_zero_fee_bps() {
    let gross_price = 1000i128;
    let result = fee::compute_net_buyback_cost(gross_price, 0);
    assert_eq!(
        result,
        Some(1000),
        "zero fee bps should return gross price unchanged"
    );
}

#[test]
fn test_compute_net_buyback_cost_standard_fee_bps() {
    let gross_price = 1000i128;
    let result = fee::compute_net_buyback_cost(gross_price, 1000);
    assert_eq!(
        result,
        Some(900),
        "1000 bps (10%) on 1000 should yield 900 net"
    );
}

#[test]
fn test_compute_net_buyback_cost_maximum_fee_bps() {
    let gross_price = 1000i128;
    let result = fee::compute_net_buyback_cost(gross_price, 5000);
    assert_eq!(
        result,
        Some(500),
        "5000 bps (50%) on 1000 should yield 500 net"
    );
}

#[test]
fn test_compute_net_buyback_cost_matches_inverse_of_compute_buyback_cost() {
    let base_price = 1500i128;
    let protocol_fee_bps = 1000; // 10%
    let net = fee::compute_net_buyback_cost(base_price, protocol_fee_bps);
    let total = fee::compute_buyback_cost(base_price, protocol_fee_bps);

    assert_eq!(net, Some(1350));
    assert_eq!(total, Some(1650));
    assert_eq!(
        net.unwrap() + total.unwrap() - base_price,
        base_price,
        "net + total - gross should equal gross"
    );
}

#[test]
fn test_compute_net_buyback_cost_zero_gross_price() {
    let result = fee::compute_net_buyback_cost(0, 1000);
    assert_eq!(result, Some(0), "zero gross price should return zero");
}
