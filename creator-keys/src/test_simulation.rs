//! Harness entry points that exercise the economic simulation in CI.
//!
//! Iteration count and seed are configurable via `SIM_SEQUENCES` and `SIM_SEED`
//! so the nightly job can run a larger sweep than the pull request run.

#![cfg(test)]

use contracts_tests_simulation::{Harness, HarnessConfig, SimulationReport, DEFAULT_SEED};

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn config_from_env(fee_bps: i128) -> HarnessConfig {
    HarnessConfig {
        seed: env_u64("SIM_SEED", DEFAULT_SEED),
        sequences: env_u64("SIM_SEQUENCES", 64) as u32,
        fee_bps,
        ..HarnessConfig::default()
    }
}

fn report_path() -> std::path::PathBuf {
    std::env::var("SIM_REPORT")
        .map(Into::into)
        .unwrap_or_else(|_| std::path::PathBuf::from("target/simulation/report.md"))
}

#[test]
fn simulation_holds_invariants_at_default_fee() {
    let cfg = config_from_env(30);
    println!("simulation seed = {}", cfg.seed);
    let harness = Harness::new(cfg.clone());
    let outcomes = harness.run_all();
    let report = SimulationReport::build(cfg.seed, &outcomes);
    report.write_to(&report_path()).expect("write simulation report");
    assert!(report.passed(), "simulation violations:\n{}", report.render());
}

#[test]
fn seeded_run_is_reproducible() {
    let cfg = HarnessConfig {
        sequences: 8,
        ..config_from_env(30)
    };
    let a = Harness::new(cfg.clone()).run_all();
    let b = Harness::new(cfg).run_all();
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.seed, y.seed);
        assert_eq!(x.actions, y.actions);
        assert_eq!(x.reserve_after, y.reserve_after);
    }
}

#[test]
fn weakened_fee_fails_suite() {
    let cfg = HarnessConfig {
        sequences: 32,
        fee_bps: 0,
        ..config_from_env(0)
    };
    let outcomes = Harness::new(cfg.clone()).run_all();
    let report = SimulationReport::build(cfg.seed, &outcomes);
    assert!(
        !report.passed(),
        "expected zero-fee configuration to produce violations"
    );
}
