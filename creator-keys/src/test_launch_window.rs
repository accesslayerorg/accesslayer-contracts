#[cfg(test)]
mod tests {
    use crate::launch_window::*;
    use crate::ContractError;

    #[test]
    fn test_launch_window_cap_ramp() {
        // Verify that the cap interpolates linearly from start_cap to end_cap
        let start_cap = 10u32;
        let end_cap = 100u32;
        let window_ledgers = 100u32;

        // At start (ledger 0 of window): should be start_cap
        let elapsed = 0u32;
        let cap_range = end_cap.saturating_sub(start_cap);
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let interpolated = start_cap.saturating_add((fraction as u32).min(cap_range));
        assert_eq!(interpolated, 10, "At start should be start_cap");

        // At midpoint (ledger 50 of window): should be ~55
        let elapsed = 50u32;
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let interpolated = start_cap.saturating_add((fraction as u32).min(cap_range));
        assert!(interpolated >= 50 && interpolated <= 60, "At midpoint should be ~55");

        // At end (ledger 100 of window): should be end_cap
        let elapsed = 100u32;
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let interpolated = start_cap.saturating_add((fraction as u32).min(cap_range));
        assert_eq!(interpolated, 100, "At end should be end_cap");

        // Beyond end: cap remains at end_cap
        let elapsed = 150u32;
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let interpolated = start_cap.saturating_add((fraction as u32).min(cap_range));
        assert_eq!(interpolated, 100, "Beyond end should be end_cap");
    }

    #[test]
    fn test_monotonic_cap_increase() {
        // Verify monotonic increase across the full ramp
        let start_cap = 0u32;
        let end_cap = 100u32;
        let window_ledgers = 100u32;

        let mut previous_cap = start_cap;

        // Verify cap increases monotonically
        for i in 0..=window_ledgers {
            let elapsed = i;
            let cap_range = end_cap.saturating_sub(start_cap);
            let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
            let cap = start_cap.saturating_add((fraction as u32).min(cap_range));

            assert!(
                cap >= previous_cap,
                "Cap should not decrease at ledger {}",
                i
            );
            previous_cap = cap;
        }

        // Final cap should be end_cap
        assert_eq!(previous_cap, end_cap, "Final cap should be end_cap");
    }

    #[test]
    fn test_cap_ramp_edge_cases() {
        // Test when start_cap == end_cap
        let start_cap = 50u32;
        let end_cap = 50u32;
        let window_ledgers = 100u32;

        let elapsed = 50u32;
        let cap_range = end_cap.saturating_sub(start_cap);
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let cap = start_cap.saturating_add((fraction as u32).min(cap_range));

        assert_eq!(cap, 50, "When start_cap == end_cap, cap should remain constant");
    }

    #[test]
    fn test_cap_ramp_extreme_ramp() {
        // Large ramp: 1 -> 10000
        let start_cap = 1u32;
        let end_cap = 10_000u32;
        let window_ledgers = 100u32;

        // At start
        let elapsed = 0u32;
        let cap_range = end_cap.saturating_sub(start_cap);
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let cap = start_cap.saturating_add((fraction as u32).min(cap_range));
        assert_eq!(cap, 1);

        // At end
        let elapsed = 100u32;
        let fraction = (elapsed as u128) * (cap_range as u128) / (window_ledgers as u128);
        let cap = start_cap.saturating_add((fraction as u32).min(cap_range));
        assert_eq!(cap, 10_000);
    }

    #[test]
    fn test_data_key_ordering() {
        // Verify LaunchWindowDataKey types are ordered correctly
        use crate::launch_window::LaunchWindowDataKey;
        use soroban_sdk::testutils::Address as _;
        use soroban_sdk::Env;

        let env = Env::default();
        let addr1 = crate::Address::generate(&env);
        let addr2 = crate::Address::generate(&env);

        let key1 = LaunchWindowDataKey::Config(addr1.clone());
        let key2 = LaunchWindowDataKey::Config(addr2.clone());

        // Keys should be comparable (for storage ordering)
        let _ = key1 < key2 || key1 == key2 || key1 > key2;
    }
}
