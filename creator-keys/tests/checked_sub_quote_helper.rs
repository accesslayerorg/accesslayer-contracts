//! Targeted unit-style tests for checked subtraction helper used in quote math.

use creator_keys::fee;

#[test]
fn checked_sub_quote_helper_nominal_inputs() {
    assert_eq!(fee::checked_sub_i128(100, 40), Some(60));
    assert_eq!(fee::checked_sub_i128(0, 0), Some(0));
}

#[test]
fn checked_sub_quote_helper_underflow_prone_inputs() {
    assert_eq!(fee::checked_sub_i128(i128::MIN, 1), None);
    assert_eq!(fee::checked_sub_i128(-5, 10), Some(-15));
}
