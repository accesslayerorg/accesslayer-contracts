//! #1236: signed holder-balance attestations.

extern crate std;

use creator_keys::{
    balance_attestation::{AttestationError, AttesterRotatedEvent, ATTESTATION_TTL_LEDGERS},
    CreatorKeysContractClient,
};
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, IntoVal, Symbol,
};

mod contract_test_env;

use contract_test_env::{register_creator_keys, set_pricing_and_fees, test_env_with_auths};

const LEDGER: u32 = 1_000;
const BALANCE: i128 = 42;

struct Attester {
    signing_key: SigningKey,
    public_key: BytesN<32>,
}

fn attester(env: &Env, seed: u8) -> Attester {
    let signing_key = SigningKey::from_bytes(&[seed; 32]);
    let public_key = BytesN::from_array(env, &signing_key.verifying_key().to_bytes());
    Attester {
        signing_key,
        public_key,
    }
}

fn sign(env: &Env, attester: &Attester, payload: &Bytes) -> BytesN<64> {
    let msg: std::vec::Vec<u8> = payload.iter().collect();
    BytesN::from_array(env, &attester.signing_key.sign(&msg).to_bytes())
}

struct Setup<'a> {
    client: CreatorKeysContractClient<'a>,
    contract: Address,
    admin: Address,
    attester: Attester,
    holder: Address,
    key_id: Address,
}

fn setup(env: &Env) -> Setup<'_> {
    let (client, contract) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, 1_000, 9_000, 1_000);
    let attester = attester(env, 7);
    client.set_attester(&admin, &attester.public_key);
    env.ledger().with_mut(|l| l.sequence_number = LEDGER);
    Setup {
        client,
        contract,
        admin,
        attester,
        holder: Address::generate(env),
        key_id: Address::generate(env),
    }
}

fn signed(env: &Env, s: &Setup, signer: &Attester, balance: i128, ledger: u32) -> BytesN<64> {
    let payload = s
        .client
        .attestation_payload(&s.holder, &s.key_id, &balance, &ledger);
    sign(env, signer, &payload)
}

#[test]
fn valid_attestation_within_ttl_verifies() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let sig = signed(&env, &s, &s.attester, BALANCE, LEDGER);
    assert!(s
        .client
        .verify_attestation(&s.holder, &s.key_id, &BALANCE, &LEDGER, &sig));

    // Still valid on the last ledger of the TTL window.
    env.ledger()
        .with_mut(|l| l.sequence_number = LEDGER + ATTESTATION_TTL_LEDGERS);
    assert!(s
        .client
        .verify_attestation(&s.holder, &s.key_id, &BALANCE, &LEDGER, &sig));
}

#[test]
fn tampered_fields_fail_verification() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let sig = signed(&env, &s, &s.attester, BALANCE, LEDGER);
    let other = Address::generate(&env);

    assert!(s
        .client
        .try_verify_attestation(&s.holder, &s.key_id, &(BALANCE + 1), &LEDGER, &sig)
        .is_err());
    assert!(s
        .client
        .try_verify_attestation(&other, &s.key_id, &BALANCE, &LEDGER, &sig)
        .is_err());
    assert!(s
        .client
        .try_verify_attestation(&s.holder, &other, &BALANCE, &LEDGER, &sig)
        .is_err());
    assert!(s
        .client
        .try_verify_attestation(&s.holder, &s.key_id, &BALANCE, &(LEDGER - 1), &sig)
        .is_err());
}

#[test]
fn expired_attestation_fails() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let sig = signed(&env, &s, &s.attester, BALANCE, LEDGER);
    env.ledger()
        .with_mut(|l| l.sequence_number = LEDGER + ATTESTATION_TTL_LEDGERS + 1);
    assert_eq!(
        s.client
            .try_verify_attestation(&s.holder, &s.key_id, &BALANCE, &LEDGER, &sig),
        Err(Ok(AttestationError::AttestationExpired))
    );
}

#[test]
fn future_ledger_attestation_fails() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let sig = signed(&env, &s, &s.attester, BALANCE, LEDGER + 1);
    assert_eq!(
        s.client
            .try_verify_attestation(&s.holder, &s.key_id, &BALANCE, &(LEDGER + 1), &sig),
        Err(Ok(AttestationError::FutureLedger))
    );
}

#[test]
fn rotation_invalidates_old_key_and_emits_event() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let old_sig = signed(&env, &s, &s.attester, BALANCE, LEDGER);
    let new_attester = attester(&env, 9);

    s.client.rotate_attester(&s.admin, &new_attester.public_key);

    let event = env
        .events()
        .all()
        .iter()
        .find(|(_, topics, _)| {
            let name: Symbol = topics.get(0).unwrap().into_val(&env);
            name == Symbol::new(&env, "attester_rotated")
        })
        .map(|(_, _, data)| -> AttesterRotatedEvent { data.into_val(&env) })
        .expect("attester_rotated event");
    assert_eq!(
        event,
        AttesterRotatedEvent {
            old_key: s.attester.public_key.clone(),
            new_key: new_attester.public_key.clone(),
        }
    );
    assert_eq!(
        s.client.get_attester(),
        Some(new_attester.public_key.clone())
    );

    assert!(s
        .client
        .try_verify_attestation(&s.holder, &s.key_id, &BALANCE, &LEDGER, &old_sig)
        .is_err());
    let new_sig = signed(&env, &s, &new_attester, BALANCE, LEDGER);
    assert!(s
        .client
        .verify_attestation(&s.holder, &s.key_id, &BALANCE, &LEDGER, &new_sig));
}

#[test]
fn only_admin_can_set_or_rotate() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let attacker = Address::generate(&env);
    let rogue = attester(&env, 3);
    assert_eq!(
        s.client.try_set_attester(&attacker, &rogue.public_key),
        Err(Ok(AttestationError::Unauthorized))
    );
    assert_eq!(
        s.client.try_rotate_attester(&attacker, &rogue.public_key),
        Err(Ok(AttestationError::Unauthorized))
    );
    assert_eq!(s.client.get_attester(), Some(s.attester.public_key.clone()));
}

#[test]
fn set_attester_is_one_shot_and_rotation_needs_a_new_key() {
    let env = test_env_with_auths();
    let s = setup(&env);
    let other = attester(&env, 5);
    assert_eq!(
        s.client.try_set_attester(&s.admin, &other.public_key),
        Err(Ok(AttestationError::AttesterAlreadySet))
    );
    assert_eq!(
        s.client
            .try_rotate_attester(&s.admin, &s.attester.public_key),
        Err(Ok(AttestationError::SameAttester))
    );
}

#[test]
fn verify_without_attester_and_rotate_without_attester_fail() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 9_000, 1_000);
    let key = attester(&env, 1);
    let holder = Address::generate(&env);
    let sig = BytesN::from_array(&env, &[0u8; 64]);
    assert_eq!(
        client.try_verify_attestation(&holder, &holder, &1, &0, &sig),
        Err(Ok(AttestationError::AttesterNotSet))
    );
    assert_eq!(
        client.try_rotate_attester(&admin, &key.public_key),
        Err(Ok(AttestationError::AttesterNotSet))
    );
}

#[test]
fn payload_binds_network_and_contract_and_does_not_replay_elsewhere() {
    let env = test_env_with_auths();
    let s = setup(&env);

    // Pin the documented layout so off-chain signers can reproduce it.
    let mut expected = Bytes::from_slice(&env, b"ACL_BALANCE_ATTESTATION_V1");
    expected.append(&env.ledger().network_id().into());
    expected.append(&s.contract.clone().to_xdr(&env));
    expected.append(&s.holder.clone().to_xdr(&env));
    expected.append(&s.key_id.clone().to_xdr(&env));
    expected.extend_from_array(&BALANCE.to_be_bytes());
    expected.extend_from_array(&LEDGER.to_be_bytes());
    assert_eq!(
        s.client
            .attestation_payload(&s.holder, &s.key_id, &BALANCE, &LEDGER),
        expected
    );

    // The same attester key trusted by a second deployment rejects the signature.
    let sig = signed(&env, &s, &s.attester, BALANCE, LEDGER);
    let (other_client, _) = register_creator_keys(&env);
    let other_admin = set_pricing_and_fees(&env, &other_client, 1_000, 9_000, 1_000);
    other_client.set_attester(&other_admin, &s.attester.public_key);
    assert!(other_client
        .try_verify_attestation(&s.holder, &s.key_id, &BALANCE, &LEDGER, &sig)
        .is_err());
}
