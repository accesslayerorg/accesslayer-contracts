//! Property-based tests and contract integration tests for graduated bonding curves (#861).
//!
//! Validates:
//! 1. Monotonicity: P(N) >= P(N - 1) across arbitrary supplies and milestone configurations.
//! 2. Boundary Smoothness: No discontinuous drops across milestone thresholds.
//! 3. Total Cost Invariant: sum_{i=S}^{S+Q-1} P(i) >= Q * P(S).
//! 4. Contract Configuration: validation guards, storage, event publishing, and query_price integration.

mod contract_test_env;

use contract_test_env::{register_creator_keys, register_test_creator, test_env_with_auths};
use creator_keys::{
    compute_graduated_curve_price, graduated_exponent_for_supply, CurveConfigError,
};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec as SorobanVec};

/// Deterministic pseudo-random number generator for reproducible property tests.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xdeadbeef_cafebabe } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn gen_range(&mut self, min: u64, max: u64) -> u64 {
        if min >= max {
            return min;
        }
        min + (self.next_u64() % (max - min + 1))
    }
}

#[test]
fn test_graduated_curve_property_suite_10000_iterations() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    let seed: u64 = 0x861_0849_2026_0924;
    let mut rng = SimpleRng::new(seed);

    const ITERATIONS: usize = 10_000;

    for i in 0..ITERATIONS {
        let milestone_count = rng.gen_range(1, 5) as usize;
        let mut milestones_raw = std::vec::Vec::new();
        let mut prev_threshold = 0u32;

        for _ in 0..milestone_count {
            let step = rng.gen_range(5, 500) as u32;
            let threshold = prev_threshold + step;
            let exponent = rng.gen_range(1, 5) as u32;
            milestones_raw.push((threshold, exponent));
            prev_threshold = threshold;
        }

        let mut milestones: SorobanVec<(u32, u32)> = SorobanVec::new(&env);
        for m in milestones_raw.iter() {
            milestones.push_back(*m);
        }

        let base_price = rng.gen_range(1, 10_000_000) as i128;
        let slope = rng.gen_range(1, 1_000_000) as i128;

        // --- Property 1: Monotonicity ---
        for _ in 0..3 {
            let max_supply = (prev_threshold + 500) as u64;
            let s = rng.gen_range(1, max_supply) as u32;

            let p_curr = compute_graduated_curve_price(&milestones, base_price, slope, s);
            let p_prev = compute_graduated_curve_price(&milestones, base_price, slope, s - 1);

            match (p_curr, p_prev) {
                (Ok(curr), Ok(prev)) => {
                    assert!(
                        curr >= prev,
                        "Monotonicity violated at seed: {}, iter: {}, supply: {}, P(S)={} < P(S-1)={}",
                        seed,
                        i,
                        s,
                        curr,
                        prev
                    );
                }
                (Err(_), _) => {
                    // Overflow is acceptable at astronomical numbers, but should never occur within our test range
                }
                _ => {}
            }
        }

        // --- Property 2: Boundary Smoothness across Milestone Transitions ---
        for &(threshold, _) in milestones_raw.iter() {
            if threshold >= 1 {
                let p_before =
                    compute_graduated_curve_price(&milestones, base_price, slope, threshold - 1)
                        .unwrap_or(0);
                let p_at = compute_graduated_curve_price(&milestones, base_price, slope, threshold)
                    .unwrap_or(0);
                let p_after =
                    compute_graduated_curve_price(&milestones, base_price, slope, threshold + 1)
                        .unwrap_or(0);

                assert!(
                    p_at >= p_before,
                    "Discontinuity before threshold {} at seed: {}, iter: {}: P(T)={} < P(T-1)={}",
                    threshold,
                    seed,
                    i,
                    p_at,
                    p_before
                );
                assert!(
                    p_after >= p_at,
                    "Discontinuity after threshold {} at seed: {}, iter: {}: P(T+1)={} < P(T)={}",
                    threshold,
                    seed,
                    i,
                    p_after,
                    p_at
                );
            }
        }

        // --- Property 3: Total Cost Invariant sum_{k=0}^{Q-1} P(S+k) >= Q * P(S) ---
        let start_s = rng.gen_range(0, (prev_threshold + 100) as u64) as u32;
        let qty = rng.gen_range(1, 20) as u32;

        let p_start_res = compute_graduated_curve_price(&milestones, base_price, slope, start_s);
        if let Ok(p_start) = p_start_res {
            let mut total_cost: i128 = 0;
            let mut overflowed = false;

            for q in 0..qty {
                match compute_graduated_curve_price(&milestones, base_price, slope, start_s + q) {
                    Ok(p_step) => {
                        if let Some(next_total) = total_cost.checked_add(p_step) {
                            total_cost = next_total;
                        } else {
                            overflowed = true;
                            break;
                        }
                    }
                    Err(_) => {
                        overflowed = true;
                        break;
                    }
                }
            }

            if !overflowed {
                let min_expected = (qty as i128) * p_start;
                assert!(
                    total_cost >= min_expected,
                    "Total cost invariant violated at seed: {}, iter: {}: total_cost={} < Q * P(S)={}",
                    seed,
                    i,
                    total_cost,
                    min_expected
                );
            }
        }

        // --- Exponent Resolution Verification ---
        for &(threshold, exp) in milestones_raw.iter() {
            let active_exp = graduated_exponent_for_supply(&milestones, threshold);
            assert_eq!(
                active_exp, exp,
                "Exponent mismatch at threshold {} at seed: {}, iter: {}",
                threshold, seed, i
            );
        }
    }
}

#[test]
fn test_graduated_curve_validation_and_storage() {
    let env = test_env_with_auths();
    let (client, _contract_id) = register_creator_keys(&env);
    let creator = register_test_creator(&env, &client, "creator");
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);

    // 1. Empty milestones rejected
    let empty_milestones: SorobanVec<(u32, u32)> = SorobanVec::new(&env);
    let res = client.try_set_graduated_curve(&creator, &empty_milestones);
    assert_eq!(
        res.err().unwrap().unwrap(),
        CurveConfigError::InvalidMilestoneCount
    );

    // 2. More than 5 milestones rejected
    let mut too_many: SorobanVec<(u32, u32)> = SorobanVec::new(&env);
    for k in 1..=6 {
        too_many.push_back((k * 100, 2));
    }
    let res = client.try_set_graduated_curve(&creator, &too_many);
    assert_eq!(
        res.err().unwrap().unwrap(),
        CurveConfigError::InvalidMilestoneCount
    );

    // 3. Non-ascending thresholds rejected
    let mut non_ascending: SorobanVec<(u32, u32)> = SorobanVec::new(&env);
    non_ascending.push_back((500, 1));
    non_ascending.push_back((300, 2));
    let res = client.try_set_graduated_curve(&creator, &non_ascending);
    assert_eq!(
        res.err().unwrap().unwrap(),
        CurveConfigError::ThresholdNotAscending
    );

    // 4. Invalid exponent rejected (0 or > 5)
    let mut bad_exponent: SorobanVec<(u32, u32)> = SorobanVec::new(&env);
    bad_exponent.push_back((100, 0));
    let res = client.try_set_graduated_curve(&creator, &bad_exponent);
    assert_eq!(
        res.err().unwrap().unwrap(),
        CurveConfigError::InvalidExponent
    );

    let mut bad_exponent_high: SorobanVec<(u32, u32)> = SorobanVec::new(&env);
    bad_exponent_high.push_back((100, 6));
    let res = client.try_set_graduated_curve(&creator, &bad_exponent_high);
    assert_eq!(
        res.err().unwrap().unwrap(),
        CurveConfigError::InvalidExponent
    );

    // 5. Unregistered creator rejected
    let unregistered = Address::generate(&env);
    let mut valid_milestones: SorobanVec<(u32, u32)> = SorobanVec::new(&env);
    valid_milestones.push_back((100, 1));
    valid_milestones.push_back((500, 2));
    let res = client.try_set_graduated_curve(&unregistered, &valid_milestones);
    assert_eq!(res.err().unwrap().unwrap(), CurveConfigError::NotRegistered);

    // 6. Valid configuration succeeds and persists
    client.set_graduated_curve(&creator, &valid_milestones);
    let stored = client.get_graduated_curve(&creator);
    assert!(stored.is_some());
    assert_eq!(stored.unwrap(), valid_milestones);

    // Exponent view query
    assert_eq!(client.get_graduated_exponent(&creator, &50), Some(1));
    assert_eq!(client.get_graduated_exponent(&creator, &100), Some(1));
    assert_eq!(client.get_graduated_exponent(&creator, &101), Some(2));
    assert_eq!(client.get_graduated_exponent(&creator, &500), Some(2));
    assert_eq!(client.get_graduated_exponent(&creator, &1000), Some(2));

    // Pricing integration test
    client.set_key_price(&admin, &1_000);
    client.set_curve_slope(&admin, &10);

    let p0 = client.query_price(&creator, &0);
    assert_eq!(p0, 1_000);

    let p50 = client.query_price(&creator, &50);
    // delta = 50, exp = 1 => 1000 + 10 * 50 = 1500
    assert_eq!(p50, 1_500);

    let p100 = client.query_price(&creator, &100);
    // delta = 100, exp = 1 => 1000 + 10 * 100 = 2000
    assert_eq!(p100, 2_000);

    let p102 = client.query_price(&creator, &102);
    // base at 100 is 2000. delta = 2, exp = 2 => 2^2 = 4. 2000 + 10 * 4 = 2040
    assert_eq!(p102, 2_040);
}
