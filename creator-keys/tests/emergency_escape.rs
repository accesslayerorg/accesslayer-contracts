//! Integration tests for prolonged-pause emergency reserve redemption.

mod contract_test_env;

use contract_test_env::{set_pricing_and_fees, test_env_with_auths};
use creator_keys::{
    ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String,
};

fn setup() -> (
    Env,
    CreatorKeysContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = test_env_with_auths();
    // Keep contract entries alive across the ESCAPE_DELAY_LEDGERS advance.
    env.ledger().with_mut(|l| {
        l.min_persistent_entry_ttl = 200_000;
        l.min_temp_entry_ttl = 200_000;
        l.max_entry_ttl = 200_000;
    });
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = set_pricing_and_fees(&env, &client, 100, 9000, 1000);
    let creator = Address::generate(&env);
    let holder = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "escape"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.buy_key(&creator, &holder, &100i128, &None);
    (env, client, admin, creator, holder)
}

#[test]
fn escape_redeem_rejects_before_delay() {
    let (_env, client, admin, creator, holder) = setup();
    client.fund_escape_reserve(&admin, &creator, &1_000i128);
    client.pause(&admin);

    assert!(!client.escape_enabled());
    assert_eq!(
        client.try_escape_redeem(&holder, &creator, &1u32),
        Err(Ok(ContractError::EscapeNotEnabled))
    );
}

#[test]
fn escape_redeem_pays_pro_rata_and_burns_supply() {
    let (env, client, admin, creator, holder) = setup();
    client.fund_escape_reserve(&admin, &creator, &1_000i128);

    let holder_two = Address::generate(&env);
    client.buy_key(&creator, &holder_two, &100i128, &None);
    client.pause(&admin);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += creator_keys::ESCAPE_DELAY_LEDGERS;
    env.ledger().set(ledger);
    assert!(client.escape_enabled());

    let payout = client.escape_redeem(&holder, &creator, &1u32);
    assert_eq!(payout, 500);
    assert_eq!(client.get_key_balance(&creator, &holder), 0);
    assert_eq!(client.get_creator_supply(&creator), 1);
}

#[test]
fn unpause_before_delay_clears_escape_timer() {
    let (env, client, admin, creator, holder) = setup();
    client.pause(&admin);

    let mut ledger = env.ledger().get();
    ledger.sequence_number += creator_keys::ESCAPE_DELAY_LEDGERS - 1;
    env.ledger().set(ledger);
    client.unpause(&admin);

    assert!(!client.escape_enabled());
    env.ledger()
        .with_mut(|l| l.sequence_number += creator_keys::ESCAPE_DELAY_LEDGERS);
    assert!(!client.escape_enabled());
    assert_eq!(
        client.try_escape_redeem(&holder, &creator, &1u32),
        Err(Ok(ContractError::EscapeNotEnabled))
    );
}

#[test]
fn last_holder_rounding_pays_remaining_reserve_exactly() {
    let (env, client, admin, creator, holder) = setup();
    client.fund_escape_reserve(&admin, &creator, &1_001i128);
    client.pause(&admin);

    env.ledger().with_mut(|l| {
        l.sequence_number += creator_keys::ESCAPE_DELAY_LEDGERS;
    });

    let payout = client.escape_redeem(&holder, &creator, &1u32);
    assert_eq!(payout, 1_001);
    assert_eq!(client.get_escape_reserve(&creator), 0);
    assert_eq!(client.get_creator_supply(&creator), 0);
}

#[test]
fn normal_trading_is_rejected_while_escape_paused() {
    let (env, client, admin, creator, holder) = setup();
    client.pause(&admin);
    env.ledger().with_mut(|l| {
        l.sequence_number += creator_keys::ESCAPE_DELAY_LEDGERS;
    });

    assert!(client.escape_enabled());
    assert_eq!(
        client.try_buy_key(&creator, &holder, &100i128, &None),
        Err(Ok(ContractError::ProtocolPaused))
    );
    assert_eq!(
        client.try_sell_key(&creator, &holder, &None),
        Err(Ok(ContractError::ProtocolPaused))
    );
}
