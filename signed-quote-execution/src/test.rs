use super::*;
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Events, Ledger},
    token, Address, Env, IntoVal, Symbol,
};

#[contract]
struct MockCurve;

#[contractimpl]
impl MockCurve {
    pub fn curve_price(_env: Env, _key_id: Address) -> i128 {
        100
    }
}

struct Setup<'a> {
    env: Env,
    client: SignedQuoteExecutionContractClient<'a>,
    admin: Address,
    quoter: Address,
    taker: Address,
    key_id: Address,
    key_token: Address,
    payment_token: Address,
    treasury: Address,
    royalty_recipient: Address,
    signer: SigningKey,
}

fn setup(max_notional: i128) -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.sequence_number = 10);

    let contract = env.register(SignedQuoteExecutionContract, ());
    let client = SignedQuoteExecutionContractClient::new(&env, &contract);
    let admin = Address::generate(&env);
    let quoter = Address::generate(&env);
    let taker = Address::generate(&env);
    let key_id = Address::generate(&env);
    let treasury = Address::generate(&env);
    let royalty_recipient = Address::generate(&env);
    let key_token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let payment_token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let curve = env.register(MockCurve, ());

    client.initialize(
        &admin,
        &payment_token,
        &key_token,
        &treasury,
        &royalty_recipient,
        &curve,
        &100,
        &200,
        &1_000,
        &100,
    );
    let signer = SigningKey::from_bytes(&[7; 32]);
    let public_key = BytesN::from_array(&env, &signer.verifying_key().to_bytes());
    client.approve_quoter_with_key(&admin, &quoter, &public_key, &max_notional);

    token::StellarAssetClient::new(&env, &key_token).mint(&quoter, &1_000);
    token::StellarAssetClient::new(&env, &key_token).mint(&taker, &100);
    token::StellarAssetClient::new(&env, &payment_token).mint(&quoter, &100_000);
    token::StellarAssetClient::new(&env, &payment_token).mint(&taker, &10_000);
    client.deposit_quote_inventory(&quoter, &key_id, &1_000, &100_000);

    Setup {
        env,
        client,
        admin,
        quoter,
        taker,
        key_id,
        key_token,
        payment_token,
        treasury,
        royalty_recipient,
        signer,
    }
}

fn quote(s: &Setup<'_>, nonce: u64, price: i128, quantity: i128, expiry: u32) -> PriceQuote {
    PriceQuote {
        quoter: s.quoter.clone(),
        key_id: s.key_id.clone(),
        side: SIDE_BUY,
        quantity,
        price,
        nonce,
        expiry,
    }
}

fn sign(s: &Setup<'_>, quote: &PriceQuote) -> BytesN<64> {
    let payload = s.client.quote_payload(quote);
    let message: std::vec::Vec<u8> = payload.iter().collect();
    BytesN::from_array(&s.env, &s.signer.sign(&message).to_bytes())
}

#[test]
fn valid_quote_settles_once_with_fees_and_royalty() {
    let s = setup(50_000);
    let q = quote(&s, 1, 100, 10, 20);
    let signature = sign(&s, &q);
    let event = s.client.execute_quote(&s.taker, &q, &signature);

    assert_eq!(event.notional, 1_000);
    assert_eq!(event.protocol_fee, 10);
    assert_eq!(event.royalty_fee, 20);

    let executed: Symbol = Symbol::new(&s.env, QUOTE_EXECUTED_EVENT);
    let mut found = false;
    for (_, topics, _) in s.env.events().all().iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&s.env);
        if name == executed {
            found = true;
        }
    }
    assert!(found);

    assert!(s.client.is_nonce_used(&s.quoter, &1));
    assert!(s
        .client
        .try_execute_quote(&s.taker, &q, &signature)
        .is_err());

    let keys = token::TokenClient::new(&s.env, &s.key_token);
    let payment = token::TokenClient::new(&s.env, &s.payment_token);
    assert_eq!(keys.balance(&s.taker), 110);
    assert_eq!(payment.balance(&s.treasury), 10);
    assert_eq!(payment.balance(&s.royalty_recipient), 20);
    assert_eq!(payment.balance(&s.quoter), 1_000);
}

#[test]
fn expired_replayed_and_tampered_quotes_are_rejected() {
    let s = setup(50_000);
    let q = quote(&s, 1, 100, 10, 20);
    let signature = sign(&s, &q);

    s.env.ledger().with_mut(|l| l.sequence_number = 21);
    assert!(s
        .client
        .try_execute_quote(&s.taker, &q, &signature)
        .is_err());

    s.env.ledger().with_mut(|l| l.sequence_number = 20);
    s.client.execute_quote(&s.taker, &q, &signature);
    assert!(s
        .client
        .try_execute_quote(&s.taker, &q, &signature)
        .is_err());

    let mut tampered = quote(&s, 2, 100, 10, 20);
    tampered.price = 101;
    let signature = sign(&s, &quote(&s, 2, 100, 10, 20));
    assert!(s
        .client
        .try_execute_quote(&s.taker, &tampered, &signature)
        .is_err());
}

#[test]
fn notional_limit_and_curve_deviation_are_enforced() {
    let s = setup(1_500);
    let q = quote(&s, 1, 100, 20, 20);
    let signature = sign(&s, &q);
    assert!(s
        .client
        .try_execute_quote(&s.taker, &q, &signature)
        .is_err());

    let s = setup(50_000);
    let q = quote(&s, 1, 200, 10, 20);
    let signature = sign(&s, &q);
    assert!(s
        .client
        .try_execute_quote(&s.taker, &q, &signature)
        .is_err());
}

#[test]
fn revoking_a_quoter_blocks_new_quotes() {
    let s = setup(50_000);
    s.client.revoke_quoter(&s.admin, &s.quoter);
    let q = quote(&s, 1, 100, 10, 20);
    let signature = sign(&s, &q);
    assert!(s
        .client
        .try_execute_quote(&s.taker, &q, &signature)
        .is_err());
}
