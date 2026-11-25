use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

struct Setup<'a> {
    env: Env,
    client: PresaleReservationsContractClient<'a>,
    key_id: Address,
    key_token: Address,
    payment_token: Address,
    treasury: Address,
    royalty_recipient: Address,
}

fn setup(max_reservations: u32, max_per_wallet: u32) -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.sequence_number = 1);

    let contract = env.register(PresaleReservationsContract, ());
    let client = PresaleReservationsContractClient::new(&env, &contract);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let key_id = Address::generate(&env);
    let treasury = Address::generate(&env);
    let royalty_recipient = Address::generate(&env);
    let key_token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let payment_token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();

    client.initialize(
        &admin,
        &payment_token,
        &key_token,
        &treasury,
        &royalty_recipient,
    );
    client.open_reservations(&creator, &key_id, &max_reservations, &50, &10);
    client.configure_launch(
        &creator,
        &key_id,
        &100,
        &max_per_wallet,
        &20,
        &100,
        &100,
        &200,
    );
    token::StellarAssetClient::new(&env, &key_token).mint(&creator, &100);
    client.fund_keys(&creator, &key_id, &100);

    Setup {
        env,
        client,
        key_id,
        key_token,
        payment_token,
        treasury,
        royalty_recipient,
    }
}

fn fund_buyer(s: &Setup<'_>, buyer: &Address, amount: i128) {
    token::StellarAssetClient::new(&s.env, &s.payment_token).mint(buyer, &amount);
}

#[test]
fn reservations_enforce_total_capacity_per_wallet_cap_and_duplicates() {
    let s = setup(10, 3);
    let buyers = [
        Address::generate(&s.env),
        Address::generate(&s.env),
        Address::generate(&s.env),
        Address::generate(&s.env),
    ];
    for buyer in &buyers {
        fund_buyer(&s, buyer, 1_000);
    }

    assert_eq!(s.client.reserve(&buyers[0], &s.key_id, &3), 150);
    assert!(s.client.try_reserve(&buyers[0], &s.key_id, &1).is_err());
    assert!(s.client.try_reserve(&buyers[1], &s.key_id, &4).is_err());
    s.client.reserve(&buyers[1], &s.key_id, &3);
    s.client.reserve(&buyers[2], &s.key_id, &3);
    assert!(s.client.try_reserve(&buyers[3], &s.key_id, &2).is_err());

    assert_eq!(s.client.get_sale(&s.key_id).unwrap().total_reserved, 9);
    assert_eq!(
        s.client
            .get_reservation(&buyers[0], &s.key_id)
            .unwrap()
            .deposit,
        150
    );
}

#[test]
fn cancellation_before_deadline_refunds_deposit_minus_fee() {
    let s = setup(10, 3);
    let buyer = Address::generate(&s.env);
    fund_buyer(&s, &buyer, 1_000);
    s.client.reserve(&buyer, &s.key_id, &2);

    assert_eq!(s.client.cancel_reservation(&buyer, &s.key_id), 99);
    assert_eq!(
        token::TokenClient::new(&s.env, &s.payment_token).balance(&buyer),
        999
    );
    assert_eq!(
        token::TokenClient::new(&s.env, &s.payment_token).balance(&s.treasury),
        1
    );
    assert!(s.client.get_reservation(&buyer, &s.key_id).is_none());

    s.env.ledger().with_mut(|l| l.sequence_number = 10);
    assert!(s.client.try_cancel_reservation(&buyer, &s.key_id).is_err());
}

#[test]
fn conversion_credits_deposit_and_priority_closes_on_public_open() {
    let s = setup(10, 3);
    let buyer = Address::generate(&s.env);
    fund_buyer(&s, &buyer, 1_000);
    s.client.reserve(&buyer, &s.key_id, &2);

    assert!(s.client.try_convert(&buyer, &s.key_id).is_err());
    s.env.ledger().with_mut(|l| l.sequence_number = 10);
    assert!(s.client.try_public_buy(&buyer, &s.key_id, &1).is_err());
    assert_eq!(s.client.convert(&buyer, &s.key_id), 106);

    let keys = token::TokenClient::new(&s.env, &s.key_token);
    let payment = token::TokenClient::new(&s.env, &s.payment_token);
    assert_eq!(keys.balance(&buyer), 2);
    assert_eq!(payment.balance(&buyer), 794);
    assert_eq!(payment.balance(&s.treasury), 2);
    assert_eq!(payment.balance(&s.royalty_recipient), 4);
    assert!(s.client.get_reservation(&buyer, &s.key_id).is_none());
}

#[test]
fn unconverted_deposits_are_refunded_after_the_window() {
    let s = setup(10, 3);
    let buyer = Address::generate(&s.env);
    fund_buyer(&s, &buyer, 1_000);
    s.client.reserve(&buyer, &s.key_id, &3);

    s.env.ledger().with_mut(|l| l.sequence_number = 20);
    assert_eq!(s.client.refund_unconverted(&buyer, &s.key_id), 150);
    assert_eq!(
        token::TokenClient::new(&s.env, &s.payment_token).balance(&buyer),
        1_000
    );
    assert_eq!(s.client.get_sale(&s.key_id).unwrap().total_reserved, 0);
}
