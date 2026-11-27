//! Soroban-independent bonding-curve and fee arithmetic.
//!
//! Keep this module limited to `core` types so the same production code can be
//! compiled directly by the Kani proof crate without a Soroban environment.

/// Basis points per 100%.
pub const BPS_MAX: u32 = 10_000;

/// Largest amount that can be multiplied by any valid basis-point value.
pub const MAX_SAFE_AMOUNT: i128 = i128::MAX / BPS_MAX as i128;

/// Pure representation of the contract's curve presets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveKind {
    Linear,
    Quadratic,
    Flat,
}

/// Prices `amount` consecutive keys beginning at `supply`.
pub fn compute_price(
    base_price: i128,
    slope: i128,
    supply: u32,
    amount: u32,
    curve: CurveKind,
) -> Option<i128> {
    let mut total = 0i128;

    for offset in 0..amount {
        let step_supply = supply.checked_add(offset)?;
        let component = match curve {
            CurveKind::Flat => 0,
            CurveKind::Linear => slope.checked_mul(i128::from(step_supply))?,
            CurveKind::Quadratic => {
                let step_supply = i128::from(step_supply);
                let supply_squared = step_supply.checked_mul(step_supply)?;
                slope.checked_mul(supply_squared)?
            }
        };
        let key_price = base_price.checked_add(component)?;
        total = total.checked_add(key_price)?;
    }

    Some(total)
}

/// Checked integer exponentiation used by graduated curves.
pub fn checked_pow_i128(base: i128, exponent: u32) -> Option<i128> {
    let mut result = 1i128;
    let mut remaining = exponent;
    while remaining > 0 {
        result = result.checked_mul(base)?;
        remaining -= 1;
    }
    Some(result)
}

/// Computes `(creator_amount, protocol_amount)` using the contract's rounding rules.
pub fn compute_fee_split(total: i128, creator_bps: u32, protocol_bps: u32) -> (i128, i128) {
    if total <= 0 {
        return (0, 0);
    }
    let protocol_amount = (total * i128::from(protocol_bps)) / i128::from(BPS_MAX);
    let creator_amount = if creator_bps.saturating_add(protocol_bps) == BPS_MAX {
        total - protocol_amount
    } else {
        (total * i128::from(creator_bps)) / i128::from(BPS_MAX)
    };
    (creator_amount, protocol_amount)
}

/// Applies a basis-point percentage, returning `None` on overflow.
pub fn apply_percentage_fee(amount: i128, bps: u32) -> Option<i128> {
    if amount <= 0 {
        return Some(0);
    }
    checked_div_i128(amount.checked_mul(i128::from(bps))?, i128::from(BPS_MAX))
}

/// Computes a gross price after deducting its protocol fee.
pub fn compute_net_buyback_cost(gross_price: i128, protocol_fee_bps: u32) -> Option<i128> {
    let protocol_fee = apply_percentage_fee(gross_price, protocol_fee_bps)?;
    gross_price.checked_sub(protocol_fee)
}

/// Computes a gross price after adding its protocol fee.
pub fn compute_buyback_cost(gross_price: i128, protocol_fee_bps: u32) -> Option<i128> {
    let protocol_fee = apply_percentage_fee(gross_price, protocol_fee_bps)?;
    gross_price.checked_add(protocol_fee)
}

/// Overflow-safe fee split with the same rounding behavior as [`compute_fee_split`].
pub fn checked_compute_fee_split(
    total: i128,
    creator_bps: u32,
    protocol_bps: u32,
) -> Option<(i128, i128)> {
    if total <= 0 {
        return Some((0, 0));
    }
    let protocol_amount = apply_percentage_fee(total, protocol_bps)?;
    let creator_amount = if creator_bps.checked_add(protocol_bps) == Some(BPS_MAX) {
        total.checked_sub(protocol_amount)?
    } else {
        apply_percentage_fee(total, creator_bps)?
    };
    Some((creator_amount, protocol_amount))
}

/// Splits `total` into the amount retained and the amount assigned by `share_bps`.
pub fn checked_split_bps_amount(total: i128, share_bps: u32) -> Option<(i128, i128)> {
    if total <= 0 {
        return Some((0, 0));
    }
    let shared_amount = apply_percentage_fee(total, share_bps)?;
    let remainder = total.checked_sub(shared_amount)?;
    Some((remainder, shared_amount))
}

pub fn checked_mul_i128(left: i128, right: i128) -> Option<i128> {
    left.checked_mul(right)
}

pub fn checked_div_i128(dividend: i128, divisor: i128) -> Option<i128> {
    if divisor == 0 {
        return None;
    }
    dividend.checked_div(divisor)
}

pub fn checked_sub_i128(left: i128, right: i128) -> Option<i128> {
    left.checked_sub(right)
}

pub fn checked_add_i128(left: i128, right: i128) -> Option<i128> {
    left.checked_add(right)
}

pub fn checked_fee_sum(creator_fee: i128, protocol_fee: i128) -> Option<i128> {
    creator_fee.checked_add(protocol_fee)
}
