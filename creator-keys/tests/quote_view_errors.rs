//! Tests asserting stable discriminant values for quote view error constants.

use creator_keys::ContractError;

#[test]
fn test_not_registered_discriminant() {
    assert_eq!(ContractError::NotRegistered as u32, 2);
}

#[test]
fn test_overflow_discriminant() {
    assert_eq!(ContractError::Overflow as u32, 3);
}

#[test]
fn test_key_price_not_set_discriminant() {
    assert_eq!(ContractError::KeyPriceNotSet as u32, 5);
}

#[test]
fn test_fee_config_not_set_discriminant() {
    assert_eq!(ContractError::FeeConfigNotSet as u32, 7);
}

#[test]
fn test_insufficient_balance_discriminant() {
    assert_eq!(ContractError::InsufficientBalance as u32, 9);
}
