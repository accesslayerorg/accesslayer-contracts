//! Authorization-tree tests for `execute_action` (issue #1318).
//!
//! `execute_action` is the only place `multisig-admin` signs on its own behalf:
//! it calls `env.authorize_as_current_contract` with exactly one
//! `SubContractInvocation` (the approved `target.function(args)`) and no
//! nested sub-invocations. These tests pin that tree down:
//!
//! - the executor's signature covers `execute_action(executor, proposal_id)`
//!   and nothing else;
//! - the approved target call succeeds with the multisig as its authorizer;
//! - a malicious target cannot use the multisig's self-authorization for a
//!   further nested call, and cannot reuse the executor's signature.
//!
//! See `docs/auth-trees.md` for the full inventory.

use multisig_admin::{
    ActionKind, ContractCallAction, MultisigAdminContract, MultisigAdminContractClient,
    ProposalAction,
};
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation, MockAuth, MockAuthInvoke},
    vec, Address, Env, IntoVal, Symbol, Val, Vec,
};

#[contracttype]
enum TargetKey {
    Value,
    Inner,
}

/// Honest privileged target: requires the multisig's authorization.
#[contract]
struct PrivilegedTarget;

#[contractimpl]
impl PrivilegedTarget {
    pub fn update(env: Env, admin: Address, value: u32) {
        admin.require_auth();
        env.storage().instance().set(&TargetKey::Value, &value);
    }

    pub fn value(env: Env) -> u32 {
        env.storage().instance().get(&TargetKey::Value).unwrap_or(0)
    }
}

/// Malicious target: after its own `admin.require_auth()`, it tries to spend
/// more authorization than the multisig granted.
#[contract]
struct MaliciousTarget;

#[contractimpl]
impl MaliciousTarget {
    pub fn set_inner(env: Env, inner: Address) {
        env.storage().instance().set(&TargetKey::Inner, &inner);
    }

    /// Forwards the multisig's authority to a second privileged contract.
    pub fn escalate(env: Env, admin: Address, value: u32) {
        admin.require_auth();
        let inner: Address = env.storage().instance().get(&TargetKey::Inner).unwrap();
        PrivilegedTargetClient::new(&env, &inner).update(&admin, &value);
    }

    /// Tries to reuse the executor's signature, which only covers
    /// `execute_action`.
    pub fn steal_executor(_env: Env, admin: Address, executor: Address) {
        admin.require_auth();
        executor.require_auth();
    }
}

fn setup(env: &Env) -> (Vec<Address>, MultisigAdminContractClient<'_>) {
    env.mock_all_auths();
    let signers = vec![
        env,
        Address::generate(env),
        Address::generate(env),
        Address::generate(env),
    ];
    let contract_id = env.register(MultisigAdminContract, ());
    let client = MultisigAdminContractClient::new(env, &contract_id);
    client.initialize(&signers.get(0).unwrap(), &signers, &2);
    (signers, client)
}

fn approved_proposal(
    env: &Env,
    client: &MultisigAdminContractClient<'_>,
    signers: &Vec<Address>,
    target: &Address,
    function: &str,
    args: Vec<Val>,
) -> u32 {
    let action = ProposalAction::ContractCall(ContractCallAction {
        kind: ActionKind::AclChange,
        target: target.clone(),
        function: Symbol::new(env, function),
        args,
    });
    let id = client.propose_action(&signers.get(0).unwrap(), &action);
    client.sign_action(&signers.get(0).unwrap(), &id);
    client.sign_action(&signers.get(1).unwrap(), &id);
    id
}

/// Switch from `mock_all_auths` to strict mode: only the executor's
/// signature for `execute_action(executor, proposal_id)` exists.
fn mock_executor_only(
    env: &Env,
    client: &MultisigAdminContractClient<'_>,
    executor: &Address,
    id: u32,
) {
    env.mock_auths(&[MockAuth {
        address: executor,
        invoke: &MockAuthInvoke {
            contract: &client.address,
            fn_name: "execute_action",
            args: (executor.clone(), id).into_val(env),
            sub_invokes: &[],
        },
    }]);
}

#[test]
fn execute_action_executor_tree_is_exactly_execute_action() {
    let env = Env::default();
    let (signers, client) = setup(&env);
    let target_id = env.register(PrivilegedTarget, ());
    let target = PrivilegedTargetClient::new(&env, &target_id);
    let executor = signers.get(2).unwrap();

    // `env.auths()` reports the most recent top-level invocation, so it is
    // read before any other client call.
    let args: Vec<Val> = vec![&env, client.address.into_val(&env), 7u32.into_val(&env)];
    let first = approved_proposal(&env, &client, &signers, &target_id, "update", args);
    client.execute_action(&executor, &first);

    // The executor authorized exactly one root invocation with no nested
    // calls. The multisig's own authorization of `target.update` comes from
    // `authorize_as_current_contract`, not from any signer, so it must not
    // appear here.
    assert_eq!(
        env.auths(),
        std::vec![(
            executor.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "execute_action"),
                    (executor.clone(), first).into_val(&env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
    assert_eq!(target.value(), 7);

    // Strict mode: that single-root signature alone is enough.
    let args: Vec<Val> = vec![&env, client.address.into_val(&env), 8u32.into_val(&env)];
    let second = approved_proposal(&env, &client, &signers, &target_id, "update", args);
    mock_executor_only(&env, &client, &executor, second);
    client.execute_action(&executor, &second);
    assert_eq!(target.value(), 8);
}

#[test]
fn self_authorization_does_not_cover_nested_calls_from_target() {
    let env = Env::default();
    let (signers, client) = setup(&env);
    let inner_id = env.register(PrivilegedTarget, ());
    let inner = PrivilegedTargetClient::new(&env, &inner_id);
    let malicious_id = env.register(MaliciousTarget, ());
    MaliciousTargetClient::new(&env, &malicious_id).set_inner(&inner_id);

    let args: Vec<Val> = vec![&env, client.address.into_val(&env), 9u32.into_val(&env)];
    let id = approved_proposal(&env, &client, &signers, &malicious_id, "escalate", args);
    let executor = signers.get(2).unwrap();

    mock_executor_only(&env, &client, &executor, id);
    // The approved call is `malicious.escalate(multisig, 9)`; the nested
    // `inner.update(multisig, 9)` was never part of the self-authorized tree.
    assert!(client.try_execute_action(&executor, &id).is_err());
    assert_eq!(inner.value(), 0);
    assert!(!client.get_proposal(&id).executed);
}

#[test]
fn target_cannot_reuse_executor_signature() {
    let env = Env::default();
    let (signers, client) = setup(&env);
    let malicious_id = env.register(MaliciousTarget, ());
    let executor = signers.get(2).unwrap();
    let args: Vec<Val> = vec![&env, client.address.into_val(&env), executor.into_val(&env)];
    let id = approved_proposal(
        &env,
        &client,
        &signers,
        &malicious_id,
        "steal_executor",
        args,
    );

    mock_executor_only(&env, &client, &executor, id);
    assert!(client.try_execute_action(&executor, &id).is_err());
    assert!(!client.get_proposal(&id).executed);
}
