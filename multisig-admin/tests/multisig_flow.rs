use multisig_admin::{
    ActionKind, ContractCallAction, MultisigAdminContract, MultisigAdminContractClient,
    MultisigError, ProposalAction, UpdateSignersAction,
};
use soroban_sdk::{
    contract, contractimpl, testutils::Address as _, vec, Address, Env, IntoVal, Symbol, Val, Vec,
};

#[contract]
struct PrivilegedTarget;

#[contractimpl]
impl PrivilegedTarget {
    pub fn update(env: Env, admin: Address, value: u32) {
        admin.require_auth();
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "value"), &value);
    }

    pub fn value(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, "value"))
            .unwrap_or(0)
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

fn call_action(env: &Env, target: &Address, multisig_id: &Address) -> ProposalAction {
    let args: Vec<Val> = vec![env, multisig_id.into_val(env), 42u32.into_val(env)];
    ProposalAction::ContractCall(ContractCallAction {
        kind: ActionKind::AclChange,
        target: target.clone(),
        function: Symbol::new(env, "update"),
        args,
    })
}

#[test]
fn unauthorized_signer_cannot_propose_or_sign() {
    let env = Env::default();
    let (signers, client) = setup(&env);
    let outsider = Address::generate(&env);
    let action = ProposalAction::UpdateSigners(UpdateSignersAction {
        signers: signers.clone(),
        threshold: 2,
    });
    assert_eq!(
        client.try_propose_action(&outsider, &action),
        Err(Ok(MultisigError::Unauthorized))
    );
    let proposal_id = client.propose_action(&signers.get(0).unwrap(), &action);
    assert_eq!(
        client.try_sign_proposal(&outsider, &proposal_id),
        Err(Ok(MultisigError::Unauthorized))
    );
}

#[test]
fn execution_waits_for_threshold_and_then_authorizes_multisig_call() {
    let env = Env::default();
    let (signers, client) = setup(&env);
    let target_id = env.register(PrivilegedTarget, ());
    let target = PrivilegedTargetClient::new(&env, &target_id);
    let multisig_id = client.address.clone();
    let action = call_action(&env, &target_id, &multisig_id);
    let proposal_id = client.propose_action(&signers.get(0).unwrap(), &action);

    assert_eq!(
        client.try_execute_proposal(&signers.get(2).unwrap(), &proposal_id),
        Err(Ok(MultisigError::ThresholdNotReached))
    );
    assert_eq!(target.value(), 0);

    client.sign_proposal(&signers.get(0).unwrap(), &proposal_id);
    assert_eq!(
        client.try_execute_proposal(&signers.get(2).unwrap(), &proposal_id),
        Err(Ok(MultisigError::ThresholdNotReached))
    );
    client.sign_proposal(&signers.get(1).unwrap(), &proposal_id);
    client.execute_proposal(&signers.get(2).unwrap(), &proposal_id);
    assert_eq!(target.value(), 42);
}

#[test]
fn rejection_vetoes_even_after_threshold_is_met() {
    let env = Env::default();
    let (signers, client) = setup(&env);
    let proposal_id = client.propose_action(
        &signers.get(0).unwrap(),
        &ProposalAction::UpdateSigners(UpdateSignersAction {
            signers: signers.clone(),
            threshold: 2,
        }),
    );
    client.sign_proposal(&signers.get(0).unwrap(), &proposal_id);
    client.sign_proposal(&signers.get(2).unwrap(), &proposal_id);
    client.reject_proposal(&signers.get(1).unwrap(), &proposal_id);
    assert_eq!(
        client.try_execute_proposal(&signers.get(0).unwrap(), &proposal_id),
        Err(Ok(MultisigError::ProposalRejected))
    );
}

#[test]
fn signer_set_updates_require_old_threshold_and_apply_after_execution() {
    let env = Env::default();
    let (signers, client) = setup(&env);
    let new_signer = Address::generate(&env);
    let mut next_signers = signers.clone();
    next_signers.set(0, new_signer.clone());
    let proposal_id = client.propose_action(
        &signers.get(0).unwrap(),
        &ProposalAction::UpdateSigners(UpdateSignersAction {
            signers: next_signers.clone(),
            threshold: 2,
        }),
    );
    client.sign_proposal(&signers.get(0).unwrap(), &proposal_id);
    assert_eq!(client.get_signers(), signers);
    client.sign_proposal(&signers.get(1).unwrap(), &proposal_id);
    client.execute_proposal(&signers.get(2).unwrap(), &proposal_id);
    assert_eq!(client.get_signers(), next_signers);
    assert_eq!(client.get_threshold(), 2);

    let action = ProposalAction::UpdateSigners(UpdateSignersAction {
        signers,
        threshold: 2,
    });
    assert_eq!(client.try_propose_action(&new_signer, &action), Ok(Ok(1)));
}
