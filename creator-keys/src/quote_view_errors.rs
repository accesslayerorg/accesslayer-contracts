//! Error identifier constants for quote-view (read-only quote) methods.
//!
//! These constants are intended for use in read-only quote paths to provide
//! stable, shared error identifiers for off-chain consumers and internal use.

pub const ERR_NOT_REGISTERED: &str = "not_registered";
pub const ERR_FEE_CONFIG_NOT_SET: &str = "fee_config_not_set";
pub const ERR_OVERFLOW: &str = "overflow";
// Add more as needed for quote-view error cases.
