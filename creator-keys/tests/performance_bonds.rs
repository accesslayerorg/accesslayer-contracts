use creator_keys::{
    constants,
    performance_bonds::BondSlashedEvent,
    protocol_governance::{GovernanceKey, ProtocolParameter, ProtocolProposal},
};
use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype,
    testutils::{Address as _, Events, Ledger},
    token, Address, Env, IntoVal, String, Symbol,
};

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};

#[allow(dead_code)]
#[contractclient(name = "GovernanceReentryClient")]
trait GovernanceReentry {
    fn execute_proposal(env: Env, proposal_id: u32) -> Result<(), creator_keys::ContractError>;
}

#[derive(Clone)]
#[contracttype]
enum ReentrantTokenKey {
    Balance(Address),
    CallbackTarget,
    CallbackProposal,
    CallbackAttempted,
}

#[contract]
struct ReentrantToken;

#[contractimpl]
impl ReentrantToken {
    pub fn set_balance(env: Env, owner: Address, amount: i128) {
        env.storage()
            .persistent()
            .set(&ReentrantTokenKey::Balance(owner), &amount);
    }

    pub fn configure_callback(env: Env, target: Address, proposal_id: u32) {
        env.storage()
            .persistent()
            .set(&ReentrantTokenKey::CallbackTarget, &target);
        env.storage()
            .persistent()
            .set(&ReentrantTokenKey::CallbackProposal, &proposal_id);
        env.storage()
            .persistent()
            .set(&ReentrantTokenKey::CallbackAttempted, &false);
    }

    pub fn balance(env: Env, owner: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&ReentrantTokenKey::Balance(owner))
            .unwrap_or(0)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let from_key = ReentrantTokenKey::Balance(from);
        let to_key = ReentrantTokenKey::Balance(to);
        let from_balance: i128 = env.storage().persistent().get(&from_key).unwrap_or(0);
        let to_balance: i128 = env.storage().persistent().get(&to_key).unwrap_or(0);
        let remaining = from_balance.checked_sub(amount).unwrap();
        let updated = to_balance.checked_add(amount).unwrap();
        env.storage().persistent().set(&from_key, &remaining);
        env.storage().persistent().set(&to_key, &updated);

        if let Some(target) = env
            .storage()
            .persistent()
            .get::<_, Address>(&ReentrantTokenKey::CallbackTarget)
        {
            let attempted_key = ReentrantTokenKey::CallbackAttempted;
            let attempted: bool = env
                .storage()
                .persistent()
                .get(&attempted_key)
                .unwrap_or(false);
            if !attempted {
                env.storage().persistent().set(&attempted_key, &true);
                let proposal_id = env
                    .storage()
                    .persistent()
                    .get(&ReentrantTokenKey::CallbackProposal)
                    .unwrap();
                GovernanceReentryClient::new(&env, &target).execute_proposal(&proposal_id);
            }
        }
    }
}

#[test]
fn deposit_governance_slash_and_deprecation_release_move_xlm() {
    let env = test_env_with_auths();
    let (client, contract) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 9_000, 1_000);
    let treasury = Address::generate(&env);
    client.set_treasury_address(&admin, &treasury);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.set_xlm_token_address(&admin, &token);
    client.set_min_performance_bond(&admin, &1_000);

    let creator = register_test_creator(&env, &client, "bondedcreator");
    token::StellarAssetClient::new(&env, &token).mint(&creator, &10_000);
    client.deposit_performance_bond(&creator, &10_000);
    assert_eq!(
        client.get_bond_status(&creator),
        creator_keys::performance_bonds::BondStatus {
            deposited: 10_000,
            slashed: 0,
            releasable: 10_000,
        }
    );

    let proposal = ProtocolProposal {
        id: 0,
        proposer: creator.clone(),
        key_id: creator.clone(),
        parameter: ProtocolParameter::BondSlash,
        new_value: 2_000,
        description: String::from_str(&env, "conduct violation"),
        snapshot_supply: 100,
        voting_ends_at: 1,
        executable_at: 1,
        votes_for: 100,
        votes_against: 0,
        executed: false,
    };
    env.as_contract(&contract, || {
        env.storage()
            .persistent()
            .set(&GovernanceKey::ProtocolGovProposal(0), &proposal);
    });
    env.ledger().with_mut(|ledger| ledger.timestamp = 2);
    client.execute_proposal(&0);

    let mut slash_event_found = false;
    let event_log = env.events().all();
    for (_, topics, data) in event_log.iter() {
        let name: Symbol = topics.get(0).unwrap().into_val(&env);
        if name == Symbol::new(&env, "BondSlashed") {
            let event: BondSlashedEvent = data.into_val(&env);
            assert_eq!(event.key_id, creator);
            assert_eq!(event.reason, String::from_str(&env, "conduct violation"));
            assert_eq!(event.slash_bps, 2_000);
            assert_eq!(event.slashed_amount, 2_000);
            slash_event_found = true;
        }
    }
    assert!(slash_event_found);
    let token_client = token::TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&treasury), 2_000);
    assert_eq!(
        client.get_bond_status(&creator),
        creator_keys::performance_bonds::BondStatus {
            deposited: 10_000,
            slashed: 2_000,
            releasable: 8_000,
        }
    );

    env.as_contract(&contract, || {
        env.storage()
            .persistent()
            .set(&constants::storage::deprecated_key(&creator), &0i128);
    });
    assert_eq!(client.finalize_deprecation(&creator), 8_000);
    assert_eq!(token_client.balance(&creator), 8_000);
    assert_eq!(token_client.balance(&contract), 0);
}

#[test]
fn token_callback_cannot_reenter_governance_and_transfer_rolls_back() {
    let env = test_env_with_auths();
    let (client, contract) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 9_000, 1_000);
    let treasury = Address::generate(&env);
    client.set_treasury_address(&admin, &treasury);

    let token = env.register(ReentrantToken, ());
    client.set_xlm_token_address(&admin, &token);
    client.set_min_performance_bond(&admin, &1_000);

    let creator = register_test_creator(&env, &client, "reentrantbond");
    let token_client = ReentrantTokenClient::new(&env, &token);
    token_client.set_balance(&creator, &10_000);
    client.deposit_performance_bond(&creator, &10_000);

    let proposal = ProtocolProposal {
        id: 0,
        proposer: creator.clone(),
        key_id: creator.clone(),
        parameter: ProtocolParameter::BondSlash,
        new_value: 2_000,
        description: String::from_str(&env, "reentrant transfer"),
        snapshot_supply: 100,
        voting_ends_at: 1,
        executable_at: 1,
        votes_for: 100,
        votes_against: 0,
        executed: false,
    };
    env.as_contract(&contract, || {
        env.storage()
            .persistent()
            .set(&GovernanceKey::ProtocolGovProposal(0), &proposal);
    });
    env.ledger().with_mut(|ledger| ledger.timestamp = 2);
    token_client.configure_callback(&contract, &0);

    assert!(client.try_execute_proposal(&0).is_err());
    assert_eq!(token_client.balance(&contract), 10_000);
    assert_eq!(token_client.balance(&treasury), 0);
    assert_eq!(
        client.get_bond_status(&creator),
        creator_keys::performance_bonds::BondStatus {
            deposited: 10_000,
            slashed: 0,
            releasable: 10_000,
        }
    );
    assert!(!client.get_protocol_proposal(&0).executed);
}

#[test]
fn configured_minimum_blocks_trades_until_bond_is_escrowed() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(&env, &client, 1_000, 9_000, 1_000);
    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.set_xlm_token_address(&admin, &token);
    client.set_min_performance_bond(&admin, &1_000);
    let creator = register_test_creator(&env, &client, "unbondedcreator");
    let buyer = Address::generate(&env);
    assert_eq!(
        client.try_buy(&creator, &buyer, &1_000, &None),
        Err(Ok(creator_keys::ContractError::PerformanceBondRequired))
    );
}
