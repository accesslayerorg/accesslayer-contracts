#![cfg_attr(
    not(test),
    deny(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)
)]

use soroban_sdk::{Env, U256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MathError {
    DivisionByZero,
    Overflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoundingDirection {
    Down,
    Up,
}

pub fn checked_add(left: i128, right: i128) -> Result<i128, MathError> {
    left.checked_add(right).ok_or(MathError::Overflow)
}

pub fn checked_mul_div(
    env: &Env,
    left: i128,
    right: i128,
    divisor: i128,
    rounding: RoundingDirection,
) -> Result<i128, MathError> {
    if divisor == 0 {
        return Err(MathError::DivisionByZero);
    }

    let negative = (left < 0) ^ (right < 0) ^ (divisor < 0);
    let numerator =
        U256::from_u128(env, left.unsigned_abs()).mul(&U256::from_u128(env, right.unsigned_abs()));
    let denominator = U256::from_u128(env, divisor.unsigned_abs());
    let mut quotient = numerator.div(&denominator);
    let has_remainder = numerator.rem_euclid(&denominator) != U256::from_u32(env, 0);

    let round_away_from_zero = has_remainder
        && matches!(
            (negative, rounding),
            (false, RoundingDirection::Up) | (true, RoundingDirection::Down)
        );
    if round_away_from_zero {
        quotient = quotient.add(&U256::from_u32(env, 1));
    }

    let magnitude = quotient.to_u128().ok_or(MathError::Overflow)?;
    let min_magnitude = 1u128 << 127;
    if negative && magnitude == min_magnitude {
        return Ok(i128::MIN);
    }
    let magnitude = i128::try_from(magnitude).map_err(|_| MathError::Overflow)?;
    if negative {
        magnitude.checked_neg().ok_or(MathError::Overflow)
    } else {
        Ok(magnitude)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_mul_div_uses_wide_intermediate_for_maximum_values() {
        let env = Env::default();
        assert_eq!(
            checked_mul_div(
                &env,
                i128::MAX,
                i128::MAX,
                i128::MAX,
                RoundingDirection::Down
            ),
            Ok(i128::MAX)
        );
    }

    #[test]
    fn checked_mul_div_preserves_minimum_signed_value() {
        let env = Env::default();
        assert_eq!(
            checked_mul_div(
                &env,
                i128::MIN,
                i128::MAX,
                i128::MAX,
                RoundingDirection::Down
            ),
            Ok(i128::MIN)
        );
    }

    #[test]
    fn checked_mul_div_reports_result_overflow_and_zero_divisor() {
        let env = Env::default();
        assert_eq!(
            checked_mul_div(&env, i128::MAX, i128::MAX, 1, RoundingDirection::Down),
            Err(MathError::Overflow)
        );
        assert_eq!(
            checked_mul_div(&env, i128::MAX, 1, 0, RoundingDirection::Down),
            Err(MathError::DivisionByZero)
        );
    }

    #[test]
    fn checked_mul_div_rounds_in_requested_direction() {
        let env = Env::default();
        assert_eq!(
            checked_mul_div(&env, 5, 1, 2, RoundingDirection::Down),
            Ok(2)
        );
        assert_eq!(checked_mul_div(&env, 5, 1, 2, RoundingDirection::Up), Ok(3));
        assert_eq!(
            checked_mul_div(&env, -5, 1, 2, RoundingDirection::Down),
            Ok(-3)
        );
        assert_eq!(
            checked_mul_div(&env, -5, 1, 2, RoundingDirection::Up),
            Ok(-2)
        );
    }

    #[test]
    fn checked_add_reports_signed_overflow() {
        assert_eq!(checked_add(i128::MAX, 1), Err(MathError::Overflow));
        assert_eq!(checked_add(i128::MIN, -1), Err(MathError::Overflow));
        assert_eq!(checked_add(i128::MAX, 0), Ok(i128::MAX));
    }
}
