use creator_keys::{
    events, AllowanceError, ContractError, CreatorKeysContract, CreatorKeysContractClient,
    RegisterCreatorParams, DEFAULT_FLASH_LOAN_GUARD_LEDGERS, MAX_FLASH_LOAN_GUARD_LEDGERS,
};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, String, Symbol, TryFromVal, TryIntoVal,
};

fn setup() -> (Env, CreatorKeysContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_treasury_address(&admin, &treasury);
    client.set_key_price(&admin, &100i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_protocol_fee_recipient(&admin, &treasury);
    (env, client, admin, treasury)
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient, creator: &Address) {
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

fn advance_ledgers(env: &Env, ledgers: u32) {
    let mut ledger = env.ledger().get();
    ledger.sequence_number += ledgers;
    env.ledger().set(ledger);
}

fn assert_guard_event(env: &Env, wallet: &Address, creator: &Address) {
    let mut found = false;
    for (_, topics, data) in env.events().all().iter() {
        let Some(topic) = topics.get(0) else {
            continue;
        };
        let name: Symbol = topic.try_into_val(env).unwrap();
        if name != events::FLASH_LOAN_BLOCKED_EVENT_NAME {
            continue;
        }
        let payload: events::FlashLoanBlockedEvent = data.try_into_val(env).unwrap();
        assert_eq!(payload.wallet, *wallet);
        assert_eq!(payload.key_id, *creator);
        assert_eq!(payload.ledger, env.ledger().sequence());
        found = true;
    }
    assert!(found, "blocked flash-loan operation must emit its event");
}

#[test]
fn same_ledger_sell_is_rejected_and_emits_event() {
    let (env, client, _, _) = setup();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000i128, &None);

    assert_eq!(
        client.try_sell_key(&creator, &buyer, &None),
        Err(Ok(ContractError::FlashLoanDetected))
    );
    assert_guard_event(&env, &buyer, &creator);
}

#[test]
fn next_ledger_sell_succeeds_with_default_window() {
    let (env, client, _, _) = setup();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let buyer = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000i128, &None);
    advance_ledgers(&env, 1);

    assert_eq!(client.sell_key(&creator, &buyer, &None), 0);
    assert_eq!(
        client.get_flash_loan_guard_ledgers(),
        DEFAULT_FLASH_LOAN_GUARD_LEDGERS
    );
}

#[test]
fn admin_window_applies_to_single_and_batch_buys() {
    let (env, client, admin, _) = setup();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let buyer = Address::generate(&env);
    client.set_flash_loan_guard_ledgers(&admin, &3u32);
    assert_eq!(client.get_flash_loan_guard_ledgers(), 3);

    client.buy_key(&creator, &buyer, &1000i128, &None);
    advance_ledgers(&env, 2);
    assert_eq!(
        client.try_sell_key(&creator, &buyer, &None),
        Err(Ok(ContractError::FlashLoanDetected))
    );
    assert_guard_event(&env, &buyer, &creator);
    advance_ledgers(&env, 1);
    assert_eq!(client.sell_key(&creator, &buyer, &None), 0);

    let batch_buyer = Address::generate(&env);
    client.batch_buy(
        &batch_buyer,
        &soroban_sdk::vec![&env, (creator.clone(), 1u32)],
    );
    assert_eq!(
        client.try_sell_key(&creator, &batch_buyer, &None),
        Err(Ok(ContractError::FlashLoanDetected))
    );
}

#[test]
fn delegated_transfer_is_blocked_inside_guard_window() {
    let (env, client, _, _) = setup();
    let creator = Address::generate(&env);
    register_creator(&env, &client, &creator);
    let buyer = Address::generate(&env);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.buy_key(&creator, &buyer, &1000i128, &None);
    client.approve(&buyer, &spender, &creator, &1u32);

    assert_eq!(
        client.try_transfer_from(&spender, &buyer, &recipient, &creator, &1u32),
        Err(Ok(AllowanceError::FlashLoanDetected))
    );
    assert_guard_event(&env, &buyer, &creator);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
    assert_eq!(client.get_key_balance(&creator, &recipient), 0);
}

#[test]
fn guard_configuration_rejects_invalid_windows() {
    let (_env, client, admin, _) = setup();
    assert_eq!(
        client.try_set_flash_loan_guard_ledgers(&admin, &0u32),
        Err(Ok(ContractError::NotPositiveAmount))
    );
    assert_eq!(
        client.try_set_flash_loan_guard_ledgers(&admin, &(MAX_FLASH_LOAN_GUARD_LEDGERS + 1)),
        Err(Ok(ContractError::LimitTooHigh))
    );
}
