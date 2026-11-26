//! Regression checks that the public contract helpers retain their pre-extraction behavior.

use creator_keys::{compute_price, fee, math, ContractError, CurvePreset};

fn math_curve(preset: CurvePreset) -> math::CurveKind {
    match preset {
        CurvePreset::Linear => math::CurveKind::Linear,
        CurvePreset::Quadratic => math::CurveKind::Quadratic,
        CurvePreset::Flat => math::CurveKind::Flat,
    }
}

#[test]
fn curve_adapter_matches_pure_math_for_every_preset() {
    for preset in [
        CurvePreset::Flat,
        CurvePreset::Linear,
        CurvePreset::Quadratic,
    ] {
        for (base_price, slope, supply, amount) in [
            (1, 0, 0, 0),
            (1_000, 10, 0, 1),
            (1_000, 10, 20, 5),
            (999, 3, 1_000, 2),
        ] {
            let expected =
                math::compute_price(base_price, slope, supply, amount, math_curve(preset))
                    .ok_or(ContractError::Overflow);
            assert_eq!(
                compute_price(base_price, slope, supply, amount, preset),
                expected
            );
        }
    }
}

#[test]
fn fee_adapter_matches_pure_math_at_rounding_boundaries() {
    for (total, creator_bps, protocol_bps) in [
        (-100, 9_000, 1_000),
        (0, 9_000, 1_000),
        (1, 9_000, 1_000),
        (999, 9_000, 1_000),
        (1_001, 4_000, 4_000),
        (fee::MAX_SAFE_AMOUNT, 5_000, 5_000),
    ] {
        assert_eq!(
            fee::compute_fee_split(total, creator_bps, protocol_bps),
            math::compute_fee_split(total, creator_bps, protocol_bps)
        );
        assert_eq!(
            fee::checked_compute_fee_split(total, creator_bps, protocol_bps),
            math::checked_compute_fee_split(total, creator_bps, protocol_bps)
        );
    }

    for (amount, bps) in [
        (-1, 10_000),
        (0, 10_000),
        (1, 1_000),
        (999, 1_000),
        (fee::MAX_SAFE_AMOUNT, 10_000),
        (i128::MAX, 2),
    ] {
        assert_eq!(
            fee::apply_percentage_fee(amount, bps),
            math::apply_percentage_fee(amount, bps)
        );
    }
}

#[test]
fn quote_and_buyback_arithmetic_preserve_existing_results() {
    let (creator_fee, protocol_fee) =
        math::checked_compute_fee_split(999, 9_000, 1_000).expect("bounded split must succeed");
    let fees = math::checked_fee_sum(creator_fee, protocol_fee).unwrap();
    assert_eq!(math::checked_add_i128(999, fees), Some(1_998));
    assert_eq!(math::checked_sub_i128(999, fees), Some(0));

    assert_eq!(fee::compute_net_buyback_cost(1_000, 1_000), Some(900));
    assert_eq!(fee::compute_buyback_cost(1_000, 1_000), Some(1_100));
    assert_eq!(
        fee::compute_net_buyback_cost(1_000, 1_000),
        math::compute_net_buyback_cost(1_000, 1_000)
    );
    assert_eq!(
        fee::compute_buyback_cost(1_000, 1_000),
        math::compute_buyback_cost(1_000, 1_000)
    );
}
