use creator_keys::{
    constants,
    performance_bonds::BondSlashedEvent,
    protocol_governance::{GovernanceKey, ProtocolParameter, ProtocolProposal},
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token, Address, IntoVal, String, Symbol,
};

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};

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
