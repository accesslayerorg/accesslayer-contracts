//! Integration tests for trusted-forwarder nonce management (issue #858).
//!
//! Covers the `get_nonce` view, nonce enforcement and increment in
//! `forward_buy`, the signed-message layout, and nonce-entry TTL behaviour.
//! Buyers sign with real ed25519 keys so the contract's signature check runs
//! exactly as it would on-chain.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator};
use creator_keys::constants::storage;
use creator_keys::{ContractError, CreatorKeysContractClient, CREATOR_TTL_LEDGERS};
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::testutils::storage::Persistent;
use soroban_sdk::testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke};
use soroban_sdk::xdr::{FromXdr, ToXdr};
use soroban_sdk::{Address, Bytes, BytesN, Env, IntoVal, InvokeError};

const KEY_PRICE: i128 = 100;

/// Return type of `try_forward_buy`.
type ForwardResult =
    Result<Result<(), soroban_sdk::ConversionError>, Result<ContractError, InvokeError>>;

/// A buyer wallet: an ed25519 key pair and the Stellar account (`G...`)
/// address it controls.
struct Wallet {
    signing_key: SigningKey,
    public_key: BytesN<32>,
    address: Address,
}

fn wallet(env: &Env, seed: u8) -> Wallet {
    let signing_key = SigningKey::from_bytes(&[seed; 32]);
    let raw_public_key = signing_key.verifying_key().to_bytes();
    Wallet {
        public_key: BytesN::from_array(env, &raw_public_key),
        address: account_address(env, &raw_public_key),
        signing_key,
    }
}

/// XDR `ScVal::Address(ScAddress::Account(PublicKey::Ed25519(pk)))`.
fn account_address_xdr(env: &Env, raw_public_key: &[u8; 32]) -> Bytes {
    let mut xdr = Bytes::from_array(env, &[0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0]);
    xdr.extend_from_array(raw_public_key);
    xdr
}

fn account_address(env: &Env, raw_public_key: &[u8; 32]) -> Address {
    Address::from_xdr(env, &account_address_xdr(env, raw_public_key))
        .expect("well-formed account address XDR")
}

/// Builds the signed message byte-for-byte from the layout documented on
/// `forward_buy`, without going through the SDK tuple encoder, so the tests
/// pin the wire format that off-chain signers must reproduce.
fn signed_message(
    env: &Env,
    contract: &Address,
    creator: &Address,
    buyer: &Address,
    quantity: u32,
    nonce: u64,
) -> Bytes {
    // ScVal::Vec, Some, 5 elements
    let mut msg = Bytes::from_array(env, &[0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 5]);
    msg.append(&contract.clone().to_xdr(env));
    msg.append(&creator.clone().to_xdr(env));
    msg.append(&buyer.clone().to_xdr(env));
    // ScVal::U32
    msg.extend_from_array(&[0, 0, 0, 3]);
    msg.extend_from_array(&quantity.to_be_bytes());
    // ScVal::U64
    msg.extend_from_array(&[0, 0, 0, 5]);
    msg.extend_from_array(&nonce.to_be_bytes());
    msg
}

fn sign(env: &Env, signer: &Wallet, msg: &Bytes) -> BytesN<64> {
    let raw: std::vec::Vec<u8> = msg.iter().collect();
    BytesN::from_array(env, &signer.signing_key.sign(&raw).to_bytes())
}

struct Setup<'a> {
    env: &'a Env,
    client: CreatorKeysContractClient<'a>,
    contract_id: Address,
    creator: Address,
    forwarder: Address,
}

impl Setup<'_> {
    /// Signs `(contract, creator, buyer, quantity, nonce)` with `buyer`'s key.
    fn signature(&self, buyer: &Wallet, quantity: u32, nonce: u64) -> BytesN<64> {
        let msg = signed_message(
            self.env,
            &self.contract_id,
            &self.creator,
            &buyer.address,
            quantity,
            nonce,
        );
        sign(self.env, buyer, &msg)
    }

    fn try_forward(
        &self,
        buyer: &Wallet,
        quantity: u32,
        nonce: u64,
        signature: &BytesN<64>,
    ) -> ForwardResult {
        self.client.try_forward_buy(
            &self.creator,
            &buyer.address,
            &buyer.public_key,
            &quantity,
            &nonce,
            signature,
        )
    }

    /// Signs and submits a forwarded buy of one key with `nonce`.
    fn forward(&self, buyer: &Wallet, nonce: u64) -> ForwardResult {
        let signature = self.signature(buyer, 1, nonce);
        self.try_forward(buyer, 1, nonce, &signature)
    }

    fn nonce_ttl(&self, wallet: &Address) -> u32 {
        let key = storage::forwarder_nonce(wallet);
        self.env.as_contract(&self.contract_id, || {
            self.env.storage().persistent().get_ttl(&key)
        })
    }

    fn nonce_entry_exists(&self, wallet: &Address) -> bool {
        let key = storage::forwarder_nonce(wallet);
        self.env.as_contract(&self.contract_id, || {
            self.env.storage().persistent().has(&key)
        })
    }

    fn advance_ledgers(&self, ledgers: u32) {
        self.env
            .ledger()
            .with_mut(|li| li.sequence_number += ledgers);
    }
}

/// Registers the contract, a priced creator, an admin and a trusted forwarder.
/// Authorization is mocked for setup; tests that care about which auths are
/// required override it per call with `mock_auths`.
fn setup(env: &Env) -> Setup<'_> {
    env.mock_all_auths();
    let (client, contract_id) = register_creator_keys(env);
    // Keep the instance alive when tests advance the ledger to drain TTLs.
    env.deployer().extend_ttl(
        contract_id.clone(),
        CREATOR_TTL_LEDGERS,
        CREATOR_TTL_LEDGERS,
    );
    let admin = Address::generate(env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);
    let creator = register_test_creator(env, &client, "alice");
    let forwarder = Address::generate(env);
    client.set_trusted_forwarder(&admin, &forwarder);
    Setup {
        env,
        client,
        contract_id,
        creator,
        forwarder,
    }
}

// ── get_nonce ────────────────────────────────────────────────────────────────

#[test]
fn get_nonce_returns_zero_for_wallet_without_forwarded_history() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);

    assert_eq!(s.client.get_nonce(&buyer.address), 0);
    assert_eq!(s.client.get_nonce(&Address::generate(&env)), 0);
}

#[test]
fn get_nonce_never_panics_on_fresh_contract_or_missing_entry() {
    let env = Env::default();
    // Bare deployment: no admin, no price, no forwarder, no nonce entries.
    let (client, contract_id) = register_creator_keys(&env);
    let wallet_address = wallet(&env, 1).address;

    for _ in 0..3 {
        assert_eq!(client.try_get_nonce(&wallet_address), Ok(Ok(0)));
    }

    // Reading a missing nonce must not create an entry (and therefore has no
    // TTL to extend).
    let key = storage::forwarder_nonce(&wallet_address);
    let exists = env.as_contract(&contract_id, || env.storage().persistent().has(&key));
    assert!(!exists);
}

#[test]
fn get_nonce_requires_no_authorization() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    assert_eq!(s.forward(&buyer, 0), Ok(Ok(())));

    // No auths mocked for this call at all.
    let nonce = s.client.mock_auths(&[]).get_nonce(&buyer.address);
    assert_eq!(nonce, 1);
    assert!(env.auths().is_empty());
}

// ── nonce increment ──────────────────────────────────────────────────────────

#[test]
fn successful_forward_buy_increments_nonce() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);

    assert_eq!(s.forward(&buyer, 0), Ok(Ok(())));
    assert_eq!(s.client.get_nonce(&buyer.address), 1);

    assert_eq!(s.forward(&buyer, 1), Ok(Ok(())));
    assert_eq!(s.client.get_nonce(&buyer.address), 2);

    assert_eq!(s.client.get_key_balance(&s.creator, &buyer.address), 2);
}

#[test]
fn nonces_are_tracked_per_wallet() {
    let env = Env::default();
    let s = setup(&env);
    let alice = wallet(&env, 1);
    let bob = wallet(&env, 2);

    assert_eq!(s.forward(&alice, 0), Ok(Ok(())));
    assert_eq!(s.forward(&alice, 1), Ok(Ok(())));

    assert_eq!(s.client.get_nonce(&alice.address), 2);
    assert_eq!(s.client.get_nonce(&bob.address), 0);
    assert!(!s.nonce_entry_exists(&bob.address));

    // Bob starts from his own nonce 0, unaffected by Alice's history.
    assert_eq!(s.forward(&bob, 0), Ok(Ok(())));
    assert_eq!(s.client.get_nonce(&bob.address), 1);
    assert_eq!(s.client.get_nonce(&alice.address), 2);
}

// ── NonceAlreadyUsed ─────────────────────────────────────────────────────────

#[test]
fn replayed_nonce_is_rejected_with_nonce_already_used() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);

    let signature = s.signature(&buyer, 1, 0);
    assert_eq!(s.try_forward(&buyer, 1, 0, &signature), Ok(Ok(())));

    // Resubmitting the exact same signed payload is a replay.
    assert_eq!(
        s.try_forward(&buyer, 1, 0, &signature),
        Err(Ok(ContractError::NonceAlreadyUsed))
    );
    assert_eq!(s.client.get_nonce(&buyer.address), 1);
    assert_eq!(s.client.get_key_balance(&s.creator, &buyer.address), 1);
}

#[test]
fn future_nonce_is_rejected_with_nonce_already_used() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);

    assert_eq!(
        s.forward(&buyer, 1),
        Err(Ok(ContractError::NonceAlreadyUsed))
    );
    assert_eq!(
        s.forward(&buyer, 5),
        Err(Ok(ContractError::NonceAlreadyUsed))
    );
    assert_eq!(
        s.forward(&buyer, u64::MAX),
        Err(Ok(ContractError::NonceAlreadyUsed))
    );
    assert_eq!(s.client.get_nonce(&buyer.address), 0);
    assert!(!s.nonce_entry_exists(&buyer.address));
}

#[test]
fn bad_signature_fails_before_nonce_check_and_leaves_nonce_untouched() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    let impostor = wallet(&env, 9);

    // Wrong signer AND a stale-looking nonce: the signature check must win.
    let msg = signed_message(&env, &s.contract_id, &s.creator, &buyer.address, 1, 7);
    let forged = sign(&env, &impostor, &msg);
    let result = s.try_forward(&buyer, 1, 7, &forged);

    // `ed25519_verify` traps the invocation: a host abort, not NonceAlreadyUsed.
    assert_eq!(result, Err(Err(InvokeError::Abort)));
    assert_eq!(s.client.get_nonce(&buyer.address), 0);
    assert!(!s.nonce_entry_exists(&buyer.address));
}

#[test]
fn failed_purchase_does_not_increment_nonce() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);

    // Valid signature and nonce, but the creator is not registered, so the
    // purchase fails after the nonce was checked and incremented.
    let unregistered = Address::generate(&env);
    let msg = signed_message(&env, &s.contract_id, &unregistered, &buyer.address, 1, 0);
    let signature = sign(&env, &buyer, &msg);
    let result = s.client.try_forward_buy(
        &unregistered,
        &buyer.address,
        &buyer.public_key,
        &1,
        &0,
        &signature,
    );

    assert_eq!(result, Err(Ok(ContractError::NotRegistered)));
    assert_eq!(s.client.get_nonce(&buyer.address), 0);
    assert!(!s.nonce_entry_exists(&buyer.address));

    // The same nonce is still usable for a purchase that succeeds.
    assert_eq!(s.forward(&buyer, 0), Ok(Ok(())));
    assert_eq!(s.client.get_nonce(&buyer.address), 1);
}

// ── signed message coverage ──────────────────────────────────────────────────

/// Signs a message where one field differs from the submitted call and
/// asserts the signature is rejected without touching the nonce.
fn assert_rejected_when_signed_over(s: &Setup<'_>, buyer: &Wallet, msg: &Bytes) {
    let signature = sign(s.env, buyer, msg);
    let result = s.try_forward(buyer, 1, 0, &signature);
    assert_eq!(result, Err(Err(InvokeError::Abort)));
    assert_eq!(s.client.get_nonce(&buyer.address), 0);
}

#[test]
fn signed_message_layout_matches_documented_encoding() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);

    // The account address is encoded exactly as the doc comment describes.
    let raw_public_key = buyer.public_key.to_array();
    assert_eq!(
        buyer.address.clone().to_xdr(&env),
        account_address_xdr(&env, &raw_public_key)
    );
    // Contract addresses are `ScVal::Address(ScAddress::Contract(hash))`.
    let contract_xdr = s.contract_id.clone().to_xdr(&env);
    assert_eq!(contract_xdr.len(), 40);
    assert_eq!(
        contract_xdr.slice(0..8),
        Bytes::from_array(&env, &[0, 0, 0, 18, 0, 0, 0, 1])
    );

    // A signature over the hand-assembled bytes is accepted by the contract.
    assert_eq!(s.forward(&buyer, 0), Ok(Ok(())));
}

#[test]
fn signature_over_different_contract_address_is_rejected() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    let other_deployment = Address::generate(&env);
    let msg = signed_message(&env, &other_deployment, &s.creator, &buyer.address, 1, 0);
    assert_rejected_when_signed_over(&s, &buyer, &msg);
}

#[test]
fn signature_over_different_creator_is_rejected() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    let other_creator = register_test_creator(&env, &s.client, "bob");
    let msg = signed_message(&env, &s.contract_id, &other_creator, &buyer.address, 1, 0);
    assert_rejected_when_signed_over(&s, &buyer, &msg);
}

#[test]
fn signature_over_different_buyer_is_rejected() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    let other_buyer = wallet(&env, 2).address;
    let msg = signed_message(&env, &s.contract_id, &s.creator, &other_buyer, 1, 0);
    assert_rejected_when_signed_over(&s, &buyer, &msg);
}

#[test]
fn signature_over_different_quantity_is_rejected() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    let msg = signed_message(&env, &s.contract_id, &s.creator, &buyer.address, 2, 0);
    assert_rejected_when_signed_over(&s, &buyer, &msg);
}

#[test]
fn signature_over_different_nonce_is_rejected() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    let msg = signed_message(&env, &s.contract_id, &s.creator, &buyer.address, 1, 1);
    assert_rejected_when_signed_over(&s, &buyer, &msg);
}

// ── signing key bound to buyer ───────────────────────────────────────────────

#[test]
fn signature_from_key_other_than_buyers_is_rejected_with_invalid_signature() {
    let env = Env::default();
    let s = setup(&env);
    let victim = wallet(&env, 1);
    let impostor = wallet(&env, 9);

    // The impostor signs a well-formed message naming the victim as buyer and
    // supplies their own public key, so the ed25519 check alone would pass.
    let msg = signed_message(&env, &s.contract_id, &s.creator, &victim.address, 1, 0);
    let signature = sign(&env, &impostor, &msg);
    let result = s.client.try_forward_buy(
        &s.creator,
        &victim.address,
        &impostor.public_key,
        &1,
        &0,
        &signature,
    );

    assert_eq!(result, Err(Ok(ContractError::InvalidSignature)));
    assert_eq!(s.client.get_nonce(&victim.address), 0);
    assert_eq!(s.client.get_key_balance(&s.creator, &victim.address), 0);
}

#[test]
fn contract_address_buyer_is_rejected_with_invalid_signature() {
    let env = Env::default();
    let s = setup(&env);
    let signer = wallet(&env, 1);
    // `Address::generate` yields a contract (`C...`) address, which has no
    // ed25519 key of its own to sign with.
    let contract_buyer = Address::generate(&env);

    let msg = signed_message(&env, &s.contract_id, &s.creator, &contract_buyer, 1, 0);
    let signature = sign(&env, &signer, &msg);
    let result = s.client.try_forward_buy(
        &s.creator,
        &contract_buyer,
        &signer.public_key,
        &1,
        &0,
        &signature,
    );

    assert_eq!(result, Err(Ok(ContractError::InvalidSignature)));
    assert_eq!(s.client.get_nonce(&contract_buyer), 0);
}

// ── authorization ────────────────────────────────────────────────────────────

#[test]
fn forward_buy_succeeds_without_buyer_soroban_auth() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    let signature = s.signature(&buyer, 1, 0);

    // Only the forwarder's authorization is provided. If forward_buy reached
    // `buyer.require_auth()` this call would fail.
    s.client
        .mock_auths(&[MockAuth {
            address: &s.forwarder,
            invoke: &MockAuthInvoke {
                contract: &s.contract_id,
                fn_name: "forward_buy",
                args: (
                    s.creator.clone(),
                    buyer.address.clone(),
                    buyer.public_key.clone(),
                    1u32,
                    0u64,
                    signature.clone(),
                )
                    .into_val(&env),
                sub_invokes: &[],
            },
        }])
        .forward_buy(
            &s.creator,
            &buyer.address,
            &buyer.public_key,
            &1,
            &0,
            &signature,
        );

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, s.forwarder);
    assert_eq!(s.client.get_nonce(&buyer.address), 1);
    assert_eq!(s.client.get_key_balance(&s.creator, &buyer.address), 1);
}

#[test]
fn direct_buy_key_still_requires_buyer_auth() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);

    // No buyer authorization provided: buy_key must reject.
    let result =
        s.client
            .mock_auths(&[])
            .try_buy_key(&s.creator, &buyer.address, &KEY_PRICE, &None);
    assert_eq!(result, Err(Err(InvokeError::Abort)));
    assert_eq!(s.client.get_key_balance(&s.creator, &buyer.address), 0);

    // With buyer authorization the same call succeeds, and the buyer is the
    // address that was required to authorize.
    s.client
        .buy_key(&s.creator, &buyer.address, &KEY_PRICE, &None);
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, buyer.address);
}

#[test]
fn only_trusted_forwarder_can_submit() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    let signature = s.signature(&buyer, 1, 0);

    // Authorization from some other relayer is not the forwarder's auth.
    let relayer = Address::generate(&env);
    let result = s
        .client
        .mock_auths(&[MockAuth {
            address: &relayer,
            invoke: &MockAuthInvoke {
                contract: &s.contract_id,
                fn_name: "forward_buy",
                args: (
                    s.creator.clone(),
                    buyer.address.clone(),
                    buyer.public_key.clone(),
                    1u32,
                    0u64,
                    signature.clone(),
                )
                    .into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_forward_buy(
            &s.creator,
            &buyer.address,
            &buyer.public_key,
            &1,
            &0,
            &signature,
        );
    assert_eq!(result, Err(Err(InvokeError::Abort)));
    assert_eq!(s.client.get_nonce(&buyer.address), 0);
}

// ── TTL ──────────────────────────────────────────────────────────────────────

#[test]
fn nonce_ttl_is_extended_on_write() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);

    assert_eq!(s.forward(&buyer, 0), Ok(Ok(())));
    assert_eq!(s.nonce_ttl(&buyer.address), CREATOR_TTL_LEDGERS);

    // A later write re-extends to the full window.
    s.advance_ledgers(1_000);
    assert_eq!(s.nonce_ttl(&buyer.address), CREATOR_TTL_LEDGERS - 1_000);
    assert_eq!(s.forward(&buyer, 1), Ok(Ok(())));
    assert_eq!(s.nonce_ttl(&buyer.address), CREATOR_TTL_LEDGERS);
}

#[test]
fn nonce_ttl_is_extended_on_get_nonce_read() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    assert_eq!(s.forward(&buyer, 0), Ok(Ok(())));

    s.advance_ledgers(50_000);
    assert_eq!(s.nonce_ttl(&buyer.address), CREATOR_TTL_LEDGERS - 50_000);

    // A pure read, with no write in between, restores the full window.
    assert_eq!(s.client.get_nonce(&buyer.address), 1);
    assert_eq!(s.nonce_ttl(&buyer.address), CREATOR_TTL_LEDGERS);
}

#[test]
fn rejected_forward_buy_rolls_back_nonce_ttl_bump() {
    let env = Env::default();
    let s = setup(&env);
    let buyer = wallet(&env, 1);
    assert_eq!(s.forward(&buyer, 0), Ok(Ok(())));

    s.advance_ledgers(1_000);
    // A rejected forward_buy reverts every storage effect, TTL bumps included.
    assert_eq!(
        s.forward(&buyer, 0),
        Err(Ok(ContractError::NonceAlreadyUsed))
    );
    assert_eq!(s.nonce_ttl(&buyer.address), CREATOR_TTL_LEDGERS - 1_000);
}
