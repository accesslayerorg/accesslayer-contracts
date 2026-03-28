//! Behavior-neutral tests for shared creator read method names.

use creator_keys::constants::creator_reads;

#[test]
fn test_creator_read_method_names_are_stable() {
    assert_eq!(creator_reads::PROFILE, "get_creator");
    assert_eq!(creator_reads::DETAILS, "get_creator_details");
    assert_eq!(creator_reads::SUPPLY, "get_creator_supply");
    assert_eq!(creator_reads::FEE_RECIPIENT, "get_creator_fee_recipient");
    assert_eq!(creator_reads::FEE_CONFIG, "get_creator_fee_config");
    assert_eq!(creator_reads::FEE_BPS, "get_creator_fee_bps");
    assert_eq!(creator_reads::TREASURY_SHARE, "get_creator_treasury_share");
    assert_eq!(creator_reads::HOLDER_KEY_COUNT, "get_holder_key_count");
}
