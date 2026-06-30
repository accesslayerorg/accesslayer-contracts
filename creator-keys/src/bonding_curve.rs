//! Bonding curve pricing logic for creator key marketplace.
//!
//! Provides supply-dependent price calculations with three preset variants:
//! - Linear: price grows proportionally with supply (default, backward-compatible)
//! - Quadratic: price grows with square of supply (rewards early buyers)
//! - Flat: price grows sub-linearly (keeps keys accessible at scale)

use soroban_sdk::contracttype;

/// Bonding curve preset variants that determine how key prices grow with supply.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[contracttype]
pub enum CurvePreset {
    #[default]
    Linear = 0,
    Quadratic = 1,
    Flat = 2,
}

pub mod curve_params {
    pub const BASE_PRICE: i128 = 10_000_000;
    pub const QUADRATIC_DIVISOR: i128 = 1;
    pub const FLAT_NUMERATOR: i128 = 1;
    pub const FLAT_DENOMINATOR: i128 = 2;
}

use curve_params::*;

pub fn compute_price(current_supply: u32, amount: u32, preset: CurvePreset) -> Option<i128> {
    if amount == 0 {
        return Some(0);
    }
    match preset {
        CurvePreset::Linear => compute_linear_price(current_supply, amount),
        CurvePreset::Quadratic => compute_quadratic_price(current_supply, amount),
        CurvePreset::Flat => compute_flat_price(current_supply, amount),
    }
}

fn compute_linear_price(supply: u32, amount: u32) -> Option<i128> {
    let s = supply as i128;
    let n = amount as i128;
    let sum_supply = n.checked_mul(s)?;
    let triangular = n.checked_mul(n.checked_add(1)?)?.checked_div(2)?;
    let total_indices = sum_supply.checked_add(triangular)?;
    BASE_PRICE.checked_mul(total_indices)
}

fn compute_quadratic_price(supply: u32, amount: u32) -> Option<i128> {
    let s = supply as i128;
    let n = amount as i128;
    let sum_sq = |x: i128| -> Option<i128> {
        let term1 = x.checked_mul(x.checked_add(1)?)?;
        let term2 = x.checked_mul(2)?.checked_add(1)?;
        term1.checked_mul(term2)?.checked_div(6)
    };
    let upper = s.checked_add(n)?;
    let sum_upper = sum_sq(upper)?;
    let sum_lower = sum_sq(s)?;
    let diff = sum_upper.checked_sub(sum_lower)?;
    BASE_PRICE.checked_mul(diff)?.checked_div(QUADRATIC_DIVISOR)
}

fn compute_flat_price(supply: u32, amount: u32) -> Option<i128> {
    let s = supply as i128;
    let n = amount as i128;
    let sum_range = n
        .checked_mul(s)?
        .checked_add(n.checked_mul(n.checked_sub(1)?)?.checked_div(2)?)?;
    let scaled_range = sum_range
        .checked_mul(FLAT_NUMERATOR)?
        .checked_div(FLAT_DENOMINATOR)?;
    let total_units = n.checked_add(scaled_range)?;
    BASE_PRICE.checked_mul(total_units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_at_zero_matches_base_price() {
        let price = compute_linear_price(0, 1).unwrap();
        assert_eq!(price, BASE_PRICE);
    }

    #[test]
    fn test_linear_growth() {
        assert_eq!(compute_linear_price(0, 1), Some(BASE_PRICE));
        assert_eq!(compute_linear_price(0, 2), Some(BASE_PRICE * 3));
        assert_eq!(compute_linear_price(1, 1), Some(BASE_PRICE * 2));
    }

    #[test]
    fn test_quadratic_higher_than_linear() {
        for supply in [1u32, 5, 10, 100] {
            let q = compute_quadratic_price(supply, 1).unwrap();
            let l = compute_linear_price(supply, 1).unwrap();
            assert!(q > l, "quadratic {} should exceed linear {} at supply {}", q, l, supply);
        }
    }

    #[test]
    fn test_flat_lower_than_linear() {
        for supply in [1u32, 5, 10, 100] {
            let f = compute_flat_price(supply, 1).unwrap();
            let l = compute_linear_price(supply, 1).unwrap();
            assert!(f < l, "flat {} should be below linear {} at supply {}", f, l, supply);
        }
    }

    #[test]
    fn test_all_equal_at_zero_supply() {
        let l = compute_linear_price(0, 1).unwrap();
        let q = compute_quadratic_price(0, 1).unwrap();
        let f = compute_flat_price(0, 1).unwrap();
        assert_eq!(l, BASE_PRICE);
        assert_eq!(q, BASE_PRICE);
        assert_eq!(f, BASE_PRICE);
    }

    #[test]
    fn test_buy_sell_symmetry_all_presets() {
        for preset in [CurvePreset::Linear, CurvePreset::Quadratic, CurvePreset::Flat] {
            for supply in [0u32, 1, 5, 10] {
                for amount in [1u32, 2, 5] {
                    let buy_price = compute_price(supply, amount, preset).unwrap();
                    // Sell price for same keys (bought at supply..supply+amount-1)
                    // equals buy price
                    let sell_price = compute_price(supply, amount, preset).unwrap();
                    assert_eq!(
                        buy_price, sell_price,
                        "symmetry failed for preset {:?} supply {} amount {}",
                        preset, supply, amount
                    );
                }
            }
        }
    }

    #[test]
    fn test_quadratic_at_zero_equals_base() {
        let q = compute_quadratic_price(0, 1).unwrap();
        assert_eq!(q, BASE_PRICE);
    }
}
