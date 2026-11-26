mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
    DEFAULT_CREATOR_BPS, DEFAULT_PROTOCOL_BPS,
};
use creator_keys::{CreatorKeysContractClient, TimelockChangeType};
use soroban_sdk::{testutils::Address as _, Address, Bytes, Env};

const BUDGETS: &str = include_str!("../../budgets.toml");
const KEY_PRICE: i128 = 100;
const DISTRIBUTION_AMOUNT: i128 = 10_000;

struct StateSizes {
    holders: u32,
    keys_per_holder: u32,
    governance_actions: u32,
    distributions: u32,
}

fn integer(section: &str, key: &str) -> u64 {
    let section_name = format!("[{section}]");
    let mut in_section = false;
    for line in BUDGETS.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_section = line == section_name;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((left, right)) = line.split_once('=') {
            let left = left.trim().trim_matches(|c| c == '"' || c == '\'');
            if left == key {
                return right
                    .trim()
                    .replace('_', "")
                    .parse()
                    .unwrap_or_else(|_| panic!("invalid integer for {key} in {section_name}"));
            }
        }
    }
    panic!("missing {key} in {section_name}");
}

fn state_sizes() -> StateSizes {
    StateSizes {
        holders: integer("resources.state", "holders") as u32,
        keys_per_holder: integer("resources.state", "keys_per_holder") as u32,
        governance_actions: integer("resources.state", "governance_actions") as u32,
        distributions: integer("resources.state", "distributions") as u32,
    }
}

fn seed_state(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
) -> Vec<Address> {
    let state = state_sizes();
    let mut holders = Vec::new();
    for _ in 0..state.holders {
        let holder = Address::generate(env);
        for _ in 0..state.keys_per_holder {
            client.buy_key(creator, &holder, &KEY_PRICE, &None);
        }
        holders.push(holder);
    }
    holders
}

fn measure<F>(env: &Env, entrypoint: &str, invocation: F)
where
    F: FnOnce(),
{
    let cpu_budget = integer(&format!("resources.limits.{entrypoint}"), "cpu_insns");
    let mem_budget = integer(&format!("resources.limits.{entrypoint}"), "mem_bytes");
    env.cost_estimate().budget().reset_unlimited();
    invocation();
    let budget = env.cost_estimate().budget();
    let cpu = budget.cpu_instruction_cost();
    let mem = budget.memory_bytes_cost();
    println!("RESOURCE_USAGE\t{entrypoint}\t{cpu}\t{mem}\t{cpu_budget}\t{mem_budget}");
    assert!(
        cpu <= cpu_budget,
        "{entrypoint} cpu budget exceeded: {cpu} > {cpu_budget}"
    );
    assert!(
        mem <= mem_budget,
        "{entrypoint} memory budget exceeded: {mem} > {mem_budget}"
    );
}

#[test]
fn buy_resource_benchmark() {
    let env = test_env_with_auths();
    env.cost_estimate().budget().reset_unlimited();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        KEY_PRICE,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "bench_creator");
    let _ = seed_state(&env, &client, &creator);
    let buyer = Address::generate(&env);

    measure(&env, "buy", || {
        client.buy_key(&creator, &buyer, &KEY_PRICE, &None);
    });
}

#[test]
fn sell_resource_benchmark() {
    let env = test_env_with_auths();
    env.cost_estimate().budget().reset_unlimited();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        KEY_PRICE,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "bench_creator");
    let holders = seed_state(&env, &client, &creator);
    let seller = holders.first().unwrap().clone();

    measure(&env, "sell", || {
        client.sell_key(&creator, &seller, &None);
    });
}

#[test]
fn claim_resource_benchmark() {
    let env = test_env_with_auths();
    env.cost_estimate().budget().reset_unlimited();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(
        &env,
        &client,
        KEY_PRICE,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "bench_creator");
    let holders = seed_state(&env, &client, &creator);
    let distributor = Address::generate(&env);
    for _ in 0..state_sizes().distributions {
        client.distribute_dividend(&creator, &distributor, &DISTRIBUTION_AMOUNT);
    }
    let claimant = holders.first().unwrap().clone();

    measure(&env, "claim", || {
        client.claim_dividend(&creator, &claimant);
    });
}

#[test]
fn governance_resource_benchmark() {
    let env = test_env_with_auths();
    env.cost_estimate().budget().reset_unlimited();
    let (client, _) = register_creator_keys(&env);
    let admin = set_pricing_and_fees(
        &env,
        &client,
        KEY_PRICE,
        DEFAULT_CREATOR_BPS,
        DEFAULT_PROTOCOL_BPS,
    );
    let creator = register_test_creator(&env, &client, "bench_creator");
    let _ = seed_state(&env, &client, &creator);
    let payload = Bytes::new(&env);
    for _ in 0..state_sizes().governance_actions {
        client.propose_action(&admin, &TimelockChangeType::Fee, &payload);
    }

    measure(&env, "governance", || {
        client.propose_action(&admin, &TimelockChangeType::Fee, &payload);
    });
}
