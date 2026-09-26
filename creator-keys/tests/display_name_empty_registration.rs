//! Unit tests for the blank display-name guard on creator registration (#742).
//!
//! A registration whose display handle is empty — or nothing but whitespace, which
//! is the same thing from a user's point of view — is rejected with
//! [`ContractError::DisplayNameEmpty`]. The guard runs ahead of the length and
//! character rules so the caller gets the precise reason back, and it runs before
//! any storage write so a rejected registration leaves no trace.
//!
//! `empty_handle_registration_regression.rs` covers the empty-string case as a
//! regression; this file pins the guard's *ordering* against the other handle
//! rules and the no-partial-state invariant.

mod contract_test_env;

use contract_test_env::{register_creator_keys, test_env_with_auths};
use creator_keys::{ContractError, CreatorKeysContractClient, HANDLE_LEN_MIN};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

/// Register a fresh address with `handle`, returning that address and whether
/// the call succeeded (`Ok`) or the contract error it failed with (`Err`).
fn register_with_handle(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    handle: &str,
) -> (Address, Result<(), ContractError>) {
    let creator = Address::generate(env);
    let outcome = match client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    ) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => panic!("register_creator returned an undecodable success value"),
        Err(Ok(error)) => Err(error),
        Err(Err(invoke_error)) => {
            panic!("register_creator failed with a non-contract error: {invoke_error:?}")
        }
    };
    (creator, outcome)
}

/// A rejected registration must leave nothing behind: no profile, no derived
/// view state.
fn assert_no_creator_state(client: &CreatorKeysContractClient<'_>, creator: &Address) {
    assert!(
        !client.is_creator_registered(creator),
        "no creator profile should exist after a rejected registration"
    );
    assert_eq!(
        client.get_creator_holder_count(creator),
        0,
        "no holder count should be written after a rejected registration"
    );
    assert_eq!(
        client.get_total_key_supply(creator),
        0,
        "no supply should be written after a rejected registration"
    );
}

#[test]
fn empty_display_name_is_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let (creator, result) = register_with_handle(&env, &client, "");
    assert_eq!(
        result,
        Err(ContractError::DisplayNameEmpty),
        "an empty display name should be rejected as blank"
    );
    assert_no_creator_state(&client, &creator);
}

#[test]
fn single_whitespace_display_name_is_rejected() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let (creator, result) = register_with_handle(&env, &client, " ");
    assert_eq!(
        result,
        Err(ContractError::DisplayNameEmpty),
        "a single space is a blank display name, not a short one"
    );
    assert_no_creator_state(&client, &creator);
}

/// Whitespace-only handles long enough to clear the minimum length still fail as
/// blank rather than falling through to the character check — this is what makes
/// the guard's position in the validator observable.
#[test]
fn whitespace_only_display_name_is_blank_not_an_invalid_character() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    for handle in ["   ", "\t\t\t\t", " \t \t "] {
        let (creator, result) = register_with_handle(&env, &client, handle);
        assert_eq!(
            result,
            Err(ContractError::DisplayNameEmpty),
            "whitespace-only handle {handle:?} should be rejected as blank, \
             not as an invalid character"
        );
        assert_no_creator_state(&client, &creator);
    }
}

/// The blank guard must not swallow the other handle rules: a short but
/// non-blank handle still reports `HandleTooShort`.
#[test]
fn short_non_blank_display_name_still_reports_too_short() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let short = "a".repeat((HANDLE_LEN_MIN - 1) as usize);
    let (creator, result) = register_with_handle(&env, &client, &short);
    assert_eq!(
        result,
        Err(ContractError::HandleTooShort),
        "a non-blank handle below the minimum length is still HandleTooShort"
    );
    assert_no_creator_state(&client, &creator);
}

/// Likewise a handle containing a disallowed character among real content is an
/// `InvalidHandleCharacter`, not a blank name.
#[test]
fn non_blank_display_name_with_bad_characters_still_reports_invalid_character() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let (creator, result) = register_with_handle(&env, &client, "alice bob");
    assert_eq!(
        result,
        Err(ContractError::InvalidHandleCharacter),
        "whitespace mixed with real content is an invalid character, not a blank name"
    );
    assert_no_creator_state(&client, &creator);
}

#[test]
fn valid_display_name_registers_successfully() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let (creator, result) = register_with_handle(&env, &client, "alice");
    assert_eq!(
        result,
        Ok(()),
        "a valid non-empty display name should register"
    );
    assert!(
        client.is_creator_registered(&creator),
        "the creator profile should exist after a successful registration"
    );
    assert_eq!(
        client.get_creator(&creator).handle,
        String::from_str(&env, "alice"),
        "the stored handle should match the registered one"
    );
}

/// A blank name rejected first must not block a later valid registration by the
/// same address — the guard writes nothing, so the address is still free.
#[test]
fn rejected_blank_registration_leaves_the_address_registrable() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let creator = Address::generate(&env);
    let blank = client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "   "),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    assert_eq!(blank, Err(Ok(ContractError::DisplayNameEmpty)));

    let retry = client.try_register_creator(
        &creator_keys::RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    assert_eq!(
        retry,
        Ok(Ok(())),
        "the address should still be registrable after a rejected blank handle"
    );
    assert!(client.is_creator_registered(&creator));
}
