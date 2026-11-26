#![no_std]

#[path = "../../creator-keys/src/math.rs"]
pub mod math;

#[cfg(kani)]
mod proofs {
    use super::math::{self, CurveKind};

    const MAX_PROOF_BASE_PRICE: u32 = 1_000_000;
    const MAX_PROOF_SLOPE: u16 = 10_000;
    const MAX_PROOF_SUPPLY: u16 = 10_000;
    const MAX_PROOF_POWER_BASE: u16 = 1_000;
    const MAX_PROOF_EXPONENT: u8 = 5;

    fn any_bps() -> u32 {
        let bps = u32::from(kani::any::<u16>());
        kani::assume(bps <= math::BPS_MAX);
        bps
    }

    fn any_fee_split() -> (u32, u32) {
        let creator_bps = any_bps();
        let protocol_bps = any_bps();
        kani::assume(creator_bps + protocol_bps <= math::BPS_MAX);
        (creator_bps, protocol_bps)
    }

    fn any_curve() -> CurveKind {
        let selector = kani::any::<u8>();
        kani::assume(selector <= 2);
        match selector {
            0 => CurveKind::Flat,
            1 => CurveKind::Linear,
            _ => CurveKind::Quadratic,
        }
    }

    fn any_batch_quantity() -> u32 {
        1 + u32::from(kani::any::<u8>() & 3)
    }

    fn any_curve_inputs() -> (i128, i128, u32, CurveKind) {
        // Clear bits outside the documented range before widening to i128.
        let base_price = kani::any::<u32>() & 0xF_FFFF;
        kani::assume(base_price > 0 && base_price <= MAX_PROOF_BASE_PRICE);

        let slope = kani::any::<u16>() & 0x3FFF;
        kani::assume(slope <= MAX_PROOF_SLOPE);

        let supply = kani::any::<u16>() & 0x3FFF;
        kani::assume(supply <= MAX_PROOF_SUPPLY);

        (
            i128::from(base_price),
            i128::from(slope),
            u32::from(supply),
            any_curve(),
        )
    }

    fn any_curve_comparison_inputs() -> (i128, i128, u32, CurveKind) {
        // Comparing two symbolic prices multiplies symbolic values on both
        // sides of the assertion. Keep that domain separate from safety.
        let base_price = 1 + i128::from(kani::any::<u8>() & 63);
        let slope = i128::from(kani::any::<u8>() & 7);
        let supply = u32::from(kani::any::<u8>() & 15);
        kani::assume(slope <= 4);
        kani::assume(supply <= 8);
        (base_price, slope, supply, any_curve())
    }

    #[kani::proof]
    fn percentage_fee_is_bounded_and_exact() {
        let amount = i128::from(kani::any::<u32>());
        let bps = any_bps();

        let fee = math::apply_percentage_fee(amount, bps).unwrap();
        let expected = (amount * i128::from(bps)) / i128::from(math::BPS_MAX);

        assert_eq!(fee, expected);
        assert!(fee >= 0);
        assert!(fee <= amount);
    }

    #[kani::proof]
    fn full_fee_split_conserves_amount() {
        let amount = i128::from(kani::any::<u32>());
        let protocol_bps = any_bps();
        let creator_bps = math::BPS_MAX - protocol_bps;

        let (creator, protocol) =
            math::checked_compute_fee_split(amount, creator_bps, protocol_bps).unwrap();

        assert!(creator >= 0 && creator <= amount);
        assert!(protocol >= 0 && protocol <= amount);
        assert_eq!(math::checked_fee_sum(creator, protocol), Some(amount));
    }

    #[kani::proof]
    fn partial_fee_split_never_overallocates() {
        let amount = i128::from(kani::any::<u32>());
        let (creator_bps, protocol_bps) = any_fee_split();

        let (creator, protocol) =
            math::checked_compute_fee_split(amount, creator_bps, protocol_bps).unwrap();
        let allocated = math::checked_fee_sum(creator, protocol).unwrap();

        assert!(creator >= 0 && protocol >= 0);
        assert!(creator <= amount && protocol <= amount);
        assert!(allocated <= amount);
    }

    #[kani::proof]
    fn quote_totals_preserve_accounting() {
        let price = i128::from(kani::any::<u32>());
        let (creator_bps, protocol_bps) = any_fee_split();
        let (creator_fee, protocol_fee) =
            math::checked_compute_fee_split(price, creator_bps, protocol_bps).unwrap();
        let fees = math::checked_fee_sum(creator_fee, protocol_fee).unwrap();

        let buy_total = math::checked_add_i128(price, fees).unwrap();
        let sell_total = math::checked_sub_i128(price, fees).unwrap();

        assert_eq!(buy_total, price + fees);
        assert_eq!(sell_total + fees, price);
        assert!(buy_total >= price);
        assert!(sell_total >= 0 && sell_total <= price);
    }

    #[kani::proof]
    fn buyback_costs_are_ordered() {
        let gross = i128::from(kani::any::<u32>());
        let protocol_bps = any_bps();

        let net = math::compute_net_buyback_cost(gross, protocol_bps).unwrap();
        let total = math::compute_buyback_cost(gross, protocol_bps).unwrap();

        assert!(net >= 0);
        assert!(net <= gross);
        assert!(gross <= total);
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn curve_batch_cost_is_safe_within_bounds() {
        let (base_price, slope, supply, curve) = any_curve_inputs();
        let quantity = any_batch_quantity();
        let total = math::compute_price(base_price, slope, supply, quantity, curve).unwrap();

        assert!(total > 0);
    }

    #[kani::proof]
    #[kani::unwind(3)]
    fn curve_spot_price_is_monotone() {
        let (base_price, slope, supply, curve) = any_curve_comparison_inputs();
        let current = math::compute_price(base_price, slope, supply, 1, curve).unwrap();
        let next = math::compute_price(base_price, slope, supply + 1, 1, curve).unwrap();

        assert!(next >= current);
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn curve_batch_cost_is_not_below_initial_spot() {
        let (base_price, slope, supply, curve) = any_curve_comparison_inputs();
        let quantity = any_batch_quantity();
        let initial_spot = math::compute_price(base_price, slope, supply, 1, curve).unwrap();
        let total = math::compute_price(base_price, slope, supply, quantity, curve).unwrap();
        let minimum = initial_spot.checked_mul(i128::from(quantity)).unwrap();

        assert!(total >= minimum);
    }

    #[kani::proof]
    #[kani::unwind(4)]
    fn buy_cost_covers_round_trip_sell_proceeds() {
        let (base_price, slope, supply, curve) = any_curve_comparison_inputs();
        let quantity = 1 + u32::from(kani::any::<bool>());
        let resulting_supply = supply.checked_add(quantity).unwrap();

        let gross_buy = math::compute_price(base_price, slope, supply, quantity, curve).unwrap();
        let sell_start = resulting_supply.checked_sub(quantity).unwrap();
        let gross_sell =
            math::compute_price(base_price, slope, sell_start, quantity, curve).unwrap();
        assert_eq!(gross_buy, gross_sell);

        // No truncation is allowed: Kani checks the conversion succeeds.
        let fee_base = i128::from(u16::try_from(gross_sell).unwrap());
        let (creator_bps, protocol_bps) = any_fee_split();
        let (creator_fee, protocol_fee) =
            math::checked_compute_fee_split(fee_base, creator_bps, protocol_bps).unwrap();
        let fees = math::checked_fee_sum(creator_fee, protocol_fee).unwrap();
        let buy_cost = math::checked_add_i128(gross_buy, fees).unwrap();
        let sell_proceeds = math::checked_sub_i128(gross_sell, fees).unwrap();

        assert!(sell_proceeds <= gross_sell);
        assert!(gross_sell <= buy_cost);
        assert!(sell_proceeds <= buy_cost);
    }

    #[kani::proof]
    #[kani::unwind(7)]
    fn checked_power_is_safe_within_bound() {
        let base = kani::any::<u16>();
        kani::assume(base <= MAX_PROOF_POWER_BASE);
        let exponent = kani::any::<u8>();
        kani::assume(exponent <= MAX_PROOF_EXPONENT);

        let result = math::checked_pow_i128(i128::from(base), u32::from(exponent)).unwrap();
        assert!(result >= 0);
        if exponent == 0 {
            assert_eq!(result, 1);
        }
    }

    /// This non-vacuity check intentionally gives the creator the full amount
    /// before also adding a protocol fee. Kani must find an over-allocation.
    #[kani::proof]
    #[kani::should_panic]
    fn broken_fee_split_has_a_counterexample() {
        let amount = i128::from(kani::any::<u16>());
        kani::assume(amount > 0);
        let protocol_bps = any_bps();
        let protocol = math::apply_percentage_fee(amount, protocol_bps).unwrap();
        kani::assume(protocol > 0);

        let broken_creator = amount;
        let allocated = broken_creator.checked_add(protocol).unwrap();
        assert!(allocated <= amount);
    }
}
