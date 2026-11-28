//! Upgrade rehearsal (Issue #1309): deploy the previous release, populate
//! state, upgrade to the current build and prove nothing was lost.
//!
//! `PREVIOUS_RELEASE` is the last released build, stored under
//! `creator-keys/tests/fixtures/releases/`. `CURRENT_BUILD` is refreshed by
//! `make wasm`. When storage types change, refresh the fixture from the last
//! tagged release and keep this test green.

use crate::{
    ContractError, CreatorKeysContractClient, RegisterCreatorParams, CURRENT_STORAGE_VERSION,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    vec, Address, BytesN, Env, String, Vec,
};

const PREVIOUS_RELEASE: &[u8] = include_bytes!("../tests/fixtures/releases/creator_keys_v1.wasm");
const CURRENT_BUILD: &[u8] = include_bytes!("../test_wasm/contract.wasm");

struct Rehearsal {
    env: Env,
    client: CreatorKeysContractClient<'static>,
    admin: Address,
    creator: Address,
    holder_a: Address,
    holder_b: Address,
    members: Vec<Address>,
}

fn deploy_previous_release() -> Rehearsal {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();
    env.ledger().with_mut(|l| l.timestamp = 1_000);

    let id = env.register(PREVIOUS_RELEASE, ());
    let client = CreatorKeysContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_treasury_address(&admin, &treasury);
    client.set_key_price(&admin, &1000i128);
    client.set_fee_config(&admin, &9000u32, &1000u32);
    client.set_protocol_fee_recipient(&admin, &treasury);
    let members = vec![
        &env,
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    client.set_global_pause_admins(&admin, &members);
    let members_kept = members.clone();

    let creator = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
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
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);
    client.buy_key(&creator, &holder_a, &1000i128, &None);
    client.buy_key(&creator, &holder_b, &1000i128, &None);
    client.buy_key(&creator, &holder_b, &1000i128, &None);
    Rehearsal {
        env,
        client,
        admin,
        creator,
        holder_a,
        holder_b,
        members: members_kept,
    }
}

fn upgrade_to_current(r: &Rehearsal) -> BytesN<32> {
    let target = r.env.deployer().upload_contract_wasm(CURRENT_BUILD);
    let members = r.members.clone();
    let id = r.client.upgrade(&r.admin, &target);
    r.client.approve_upgrade(&members.get(0).unwrap(), &id);
    r.client.approve_upgrade(&members.get(1).unwrap(), &id);
    let at = r.client.get_action(&id).unwrap().execution_not_before;
    r.env.ledger().with_mut(|l| l.timestamp = at);
    r.client.execute_action(&r.admin, &id);
    target
}

struct Snapshot {
    supply: u32,
    holders: u32,
    bal_a: u32,
    bal_b: u32,
    treasury: i128,
    claimable_a: i128,
    claimable_b: i128,
    fee: (u32, u32),
}

fn snapshot(r: &Rehearsal) -> Snapshot {
    let fee = r.client.get_protocol_fee_view();
    Snapshot {
        supply: r.client.get_total_key_supply(&r.creator),
        holders: r.client.get_creator_holder_count(&r.creator),
        bal_a: r.client.get_key_balance(&r.creator, &r.holder_a),
        bal_b: r.client.get_key_balance(&r.creator, &r.holder_b),
        treasury: r.client.get_treasury_balance(),
        claimable_a: r.client.get_claimable_dividend(&r.creator, &r.holder_a),
        claimable_b: r.client.get_claimable_dividend(&r.creator, &r.holder_b),
        fee: (fee.creator_bps, fee.protocol_bps),
    }
}

#[test]
fn state_is_identical_before_and_after_upgrade() {
    let r = deploy_previous_release();
    let before = snapshot(&r);
    assert_eq!(before.supply, 3);
    assert_eq!(before.holders, 2);
    assert_eq!(r.client.get_version(), 1);

    let target = upgrade_to_current(&r);
    assert_eq!(r.client.get_version(), 2);
    assert_eq!(r.client.get_logic_address(), Some(target));

    let after = snapshot(&r);
    assert_eq!(after.supply, before.supply);
    assert_eq!(after.holders, before.holders);
    assert_eq!(after.bal_a, before.bal_a);
    assert_eq!(after.bal_b, before.bal_b);
    assert_eq!(after.treasury, before.treasury);
    assert_eq!(after.claimable_a, before.claimable_a);
    assert_eq!(after.claimable_b, before.claimable_b);
    assert_eq!(after.fee, before.fee);
}

#[test]
fn pending_dividends_stay_claimable_after_upgrade() {
    let r = deploy_previous_release();
    let distributor = Address::generate(&r.env);
    r.client
        .distribute_dividend(&r.creator, &distributor, &300i128);
    let pending_a = r.client.get_claimable_dividend(&r.creator, &r.holder_a);
    let pending_b = r.client.get_claimable_dividend(&r.creator, &r.holder_b);
    assert!(pending_a > 0 && pending_b > 0);

    upgrade_to_current(&r);
    assert_eq!(
        r.client.get_claimable_dividend(&r.creator, &r.holder_a),
        pending_a
    );
    assert_eq!(
        r.client.get_claimable_dividend(&r.creator, &r.holder_b),
        pending_b
    );
    let paid_a = r.client.claim_dividend(&r.creator, &r.holder_a);
    let paid_b = r.client.claim_dividend(&r.creator, &r.holder_b);
    assert_eq!(paid_a, pending_a);
    assert_eq!(paid_b, pending_b);
    assert_eq!(r.client.get_claimable_dividend(&r.creator, &r.holder_a), 0);
}

#[test]
fn buy_and_sell_work_against_upgraded_state() {
    let r = deploy_previous_release();
    upgrade_to_current(&r);
    r.client.migrate(&r.admin);

    let newcomer = Address::generate(&r.env);
    r.client
        .buy_key(&r.creator, &newcomer, &1_000_000i128, &None);
    assert_eq!(r.client.get_total_key_supply(&r.creator), 4);
    assert_eq!(r.client.get_creator_holder_count(&r.creator), 3);

    // Same-ledger buy/sell is blocked by the flash-loan guard, so advance.
    r.env.ledger().with_mut(|l| l.sequence_number += 100);
    r.client.sell_key(&r.creator, &r.holder_b, &None);
    assert_eq!(r.client.get_key_balance(&r.creator, &r.holder_b), 1);
    assert_eq!(r.client.get_total_key_supply(&r.creator), 3);
    // Untouched holder is unaffected.
    assert_eq!(r.client.get_key_balance(&r.creator, &r.holder_a), 1);
}

#[test]
fn governance_actions_work_after_upgrade() {
    let r = deploy_previous_release();
    upgrade_to_current(&r);
    r.client.set_fee_config(&r.admin, &8000u32, &2000u32);
    let fee = r.client.get_protocol_fee_view();
    assert_eq!((fee.creator_bps, fee.protocol_bps), (8000, 2000));
    // A second timelocked upgrade (to the same build) still goes through.
    upgrade_to_current(&r);
    assert_eq!(r.client.get_version(), 3);
}

#[test]
fn migrate_is_admin_only_and_runs_once() {
    let r = deploy_previous_release();
    upgrade_to_current(&r);
    assert_eq!(r.client.get_storage_version(), 0);

    let stranger = Address::generate(&r.env);
    assert_eq!(
        r.client.try_migrate(&stranger).unwrap_err().unwrap(),
        ContractError::Unauthorized
    );
    assert_eq!(r.client.get_storage_version(), 0);

    let before = snapshot(&r);
    assert_eq!(r.client.migrate(&r.admin), CURRENT_STORAGE_VERSION);
    assert_eq!(r.client.get_storage_version(), CURRENT_STORAGE_VERSION);
    assert_eq!(
        r.client.try_migrate(&r.admin).unwrap_err().unwrap(),
        ContractError::AlreadyMigrated
    );
    // Migration does not touch existing balances.
    let after = snapshot(&r);
    assert_eq!(after.supply, before.supply);
    assert_eq!(after.bal_a, before.bal_a);
    assert_eq!(after.bal_b, before.bal_b);
}

/// Guards the guard: if the fixture and the current build were identical in
/// storage layout *and* the assertions above were vacuous, this would still
/// pass. It pins that the fixture really is an older build (no `migrate`).
#[test]
fn previous_release_fixture_predates_migrate() {
    let r = deploy_previous_release();
    assert!(r.client.try_migrate(&r.admin).is_err());
}
