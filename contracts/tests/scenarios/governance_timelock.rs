use super::harness::{scenario_seed, Harness};
use creator_keys::{ContractError, TimelockChangeType};
use soroban_sdk::Bytes;

const UPGRADE_WASM: &[u8] = include_bytes!("../../../creator-keys/test_wasm/contract.wasm");
const TIMELOCK_DELAY: u64 = 86_400;

#[test]
fn governance_timelock_upgrade() {
    let mut h = Harness::new(scenario_seed("governance_timelock_upgrade"));

    h.step("configure timelock delay", |h| {
        h.keys
            .set_timelock_delay(&h.admin, &TIMELOCK_DELAY)
            .unwrap();
    });

    let before_version = h.step("record current version", |h| h.keys.get_version());
    let action_id = h.step("propose timelocked upgrade", |h| {
        let wasm_hash = h.env.deployer().upload_contract_wasm(UPGRADE_WASM);
        let mut raw = [0u8; 32];
        for (index, byte) in wasm_hash.iter().enumerate() {
            raw[index] = byte;
        }
        let payload = Bytes::from_array(h.env, &raw);
        h.keys
            .propose_action(&h.admin, &TimelockChangeType::Upgrade, &payload)
            .unwrap()
    });

    h.step("execute before timelock fails", |h| {
        h.assert_contract_error(
            h.keys.try_execute_action(&h.admin, &action_id),
            ContractError::TimelockNotElapsed,
        );
    });

    h.advance_time(TIMELOCK_DELAY + 1);
    h.step("execute without ratification fails", |h| {
        h.assert_contract_error(
            h.keys.try_execute_action(&h.admin, &action_id),
            ContractError::UpgradeApprovalThresholdNotMet,
        );
    });

    let first_approver = h.multisig_id.clone();
    let second_approver = h.governors[0].clone();
    h.step("ratify upgrade through multisig", |h| {
        h.ratify_upgrade(action_id, &first_approver);
        h.ratify_upgrade(action_id, &second_approver);
        assert_eq!(h.keys.get_upgrade_approvals(action_id), 2);
    });

    h.step("execute ratified upgrade", |h| {
        h.enforce_protocol_invariants = false;
        h.keys.execute_action(&h.admin, &action_id).unwrap();
        assert_ne!(h.keys.get_version(), before_version);
    });

    h.print_resources();
}
