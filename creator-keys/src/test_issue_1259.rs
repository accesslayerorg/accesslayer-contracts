//! Unit tests for Issue #1259:
//! Add multi-asset payment support with allowlisted tokens and decimal normalization.

use crate::multi_asset_payment::{self, PaymentAssetInfo};
use crate::{ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String,
};

const KEY_PRICE: i128 = 1_000_000; // 0.1 XLM in 7 decimals
const SLOPE: i128 = 100_000;
const CREATOR_BPS: u32 = 500;
const PROTOCOL_BPS: u32 = 500;

struct TestContext {
    env: Env,
    client: CreatorKeysContractClient<'static>,
    admin: Address,
    creator: Address,
    buyer: Address,
}

fn setup_test() -> TestContext {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.set_protocol_admin(&admin, &admin);
    client.set_treasury_address(&admin, &treasury);
    client.set_key_price(&admin, &KEY_PRICE);
    client.set_curve_slope(&admin, &SLOPE);
    client.set_fee_config(&admin, &CREATOR_BPS, &PROTOCOL_BPS);
    client.set_protocol_fee_recipient(&admin, &treasury);

    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "creator_alice"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    TestContext {
        env,
        client,
        admin,
        creator,
        buyer,
    }
}

#[test]
fn test_decimal_normalization_conversions() {
    // 6 decimals (e.g., USDC)
    let amount_6 = 1_000_000i128; // 1 USDC
    let internal_6 = multi_asset_payment::to_internal(amount_6, 6).unwrap();
    assert_eq!(internal_6, 10_000_000i128); // 1.0 in 7 decimals
    let back_6 = multi_asset_payment::from_internal(internal_6, 6).unwrap();
    assert_eq!(back_6, amount_6);

    // 7 decimals (native XLM stroops)
    let amount_7 = 50_000_000i128; // 5 XLM
    let internal_7 = multi_asset_payment::to_internal(amount_7, 7).unwrap();
    assert_eq!(internal_7, amount_7);
    let back_7 = multi_asset_payment::from_internal(internal_7, 7).unwrap();
    assert_eq!(back_7, amount_7);

    // 18 decimals (e.g., DAI / ETH)
    let amount_18 = 2_500_000_000_000_000_000i128; // 2.5 tokens
    let internal_18 = multi_asset_payment::to_internal(amount_18, 18).unwrap();
    assert_eq!(internal_18, 25_000_000i128); // 2.5 in 7 decimals
    let back_18 = multi_asset_payment::from_internal(internal_18, 18).unwrap();
    assert_eq!(back_18, amount_18);

    // Rounding preserves value within one unit
    let exact_18 = 1_234_567_890_123_456_789i128;
    let internal_round = multi_asset_payment::to_internal(exact_18, 18).unwrap();
    let back_round = multi_asset_payment::from_internal(internal_round, 18).unwrap();
    let diff = (exact_18 - back_round).abs();
    assert!(diff < 100_000_000_000i128); // Within 1 unit of 7-decimal scale (10^11)
}

#[test]
fn test_allowlist_management_and_views() {
    let ctx = setup_test();
    let token_usdc = Address::generate(&ctx.env);
    let oracle = Address::generate(&ctx.env);

    // Initially allowlist is empty
    assert_eq!(ctx.client.payment_assets().len(), 0);

    // Non-admin cannot add asset
    let fake_admin = Address::generate(&ctx.env);
    let res = ctx
        .client
        .try_add_payment_asset(&fake_admin, &token_usdc, &6, &oracle);
    assert_eq!(res, Err(Ok(ContractError::Unauthorized)));

    // Admin adds payment asset
    ctx.client
        .add_payment_asset(&ctx.admin, &token_usdc, &6, &oracle);

    let assets = ctx.client.payment_assets();
    assert_eq!(assets.len(), 1);
    assert_eq!(
        assets.get(0).unwrap(),
        PaymentAssetInfo {
            token: token_usdc.clone(),
            decimals: 6,
        }
    );

    // Remove payment asset
    ctx.client.remove_payment_asset(&ctx.admin, &token_usdc);

    // After removal, active allowlist is empty
    assert_eq!(ctx.client.payment_assets().len(), 0);
}

#[test]
fn test_tokens_not_on_allowlist_are_rejected() {
    let ctx = setup_test();
    let unapproved_token = Address::generate(&ctx.env);

    let res = ctx.client.try_buy_with(
        &ctx.buyer,
        &ctx.creator,
        &1u32,
        &unapproved_token,
        &10_000_000i128,
    );
    assert_eq!(res, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_buy_with_6_7_and_18_decimal_tokens() {
    let ctx = setup_test();
    let oracle = Address::generate(&ctx.env);

    let token_6 = Address::generate(&ctx.env); // 6 decimals (USDC)
    let token_7 = Address::generate(&ctx.env); // 7 decimals (EURC)
    let token_18 = Address::generate(&ctx.env); // 18 decimals (DAI)

    ctx.client
        .add_payment_asset(&ctx.admin, &token_6, &6, &oracle);
    ctx.client
        .add_payment_asset(&ctx.admin, &token_7, &7, &oracle);
    ctx.client
        .add_payment_asset(&ctx.admin, &token_18, &18, &oracle);

    // Set 1:1 rate (10^7) for each token
    ctx.client
        .set_payment_asset_rate(&oracle, &token_6, &10_000_000i128);
    ctx.client
        .set_payment_asset_rate(&oracle, &token_7, &10_000_000i128);
    ctx.client
        .set_payment_asset_rate(&oracle, &token_18, &10_000_000i128);

    // 1. Buy 1 key with 6-decimal token
    let supply_1 = ctx
        .client
        .buy_with(&ctx.buyer, &ctx.creator, &1u32, &token_6, &50_000_000i128);
    assert_eq!(supply_1, 1);
    assert!(ctx.client.get_payment_asset_reserve(&token_6) > 0);

    // 2. Buy 1 key with 7-decimal token
    let supply_2 = ctx
        .client
        .buy_with(&ctx.buyer, &ctx.creator, &1u32, &token_7, &50_000_000i128);
    assert_eq!(supply_2, 2);
    assert!(ctx.client.get_payment_asset_reserve(&token_7) > 0);

    // 3. Buy 1 key with 18-decimal token
    let supply_3 = ctx.client.buy_with(
        &ctx.buyer,
        &ctx.creator,
        &1u32,
        &token_18,
        &50_000_000_000_000_000_000i128,
    );
    assert_eq!(supply_3, 3);
    assert!(ctx.client.get_payment_asset_reserve(&token_18) > 0);

    // Reserve accounting across assets
    let base_res = ctx.client.get_payment_base_reserve();
    assert!(base_res > 0);
}

#[test]
fn test_max_pay_protection_reverts_on_rate_movement() {
    let ctx = setup_test();
    let oracle = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    ctx.client
        .add_payment_asset(&ctx.admin, &token, &6, &oracle);

    // Simulate key cost: ~100_000 in 6 decimals at 1:1 rate
    ctx.client
        .set_payment_asset_rate(&oracle, &token, &10_000_000i128);

    // Setting max_pay too low (e.g. 1 unit) must revert with SlippageExceeded
    let res = ctx
        .client
        .try_buy_with(&ctx.buyer, &ctx.creator, &1u32, &token, &1i128);
    assert_eq!(res, Err(Ok(ContractError::SlippageExceeded)));
}

#[test]
fn test_oracle_staleness_guard() {
    let ctx = setup_test();
    let oracle = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    ctx.client
        .add_payment_asset(&ctx.admin, &token, &6, &oracle);

    ctx.env.ledger().set_timestamp(1_000);
    ctx.client
        .set_payment_asset_rate(&oracle, &token, &10_000_000i128);

    // Advance time past default staleness threshold (3600 seconds)
    ctx.env.ledger().set_timestamp(1_000 + 3_601);

    let res = ctx
        .client
        .try_buy_with(&ctx.buyer, &ctx.creator, &1u32, &token, &10_000_000i128);
    assert_eq!(res, Err(Ok(ContractError::OraclePriceNotSet)));

    // Refresh rate -> succeeds
    ctx.client
        .set_payment_asset_rate(&oracle, &token, &10_000_000i128);
    let supply = ctx
        .client
        .buy_with(&ctx.buyer, &ctx.creator, &1u32, &token, &10_000_000i128);
    assert_eq!(supply, 1);
}

#[test]
fn test_removing_asset_blocks_new_buys_but_allows_reserve_redemption() {
    let ctx = setup_test();
    let oracle = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    ctx.client
        .add_payment_asset(&ctx.admin, &token, &6, &oracle);
    ctx.client
        .set_payment_asset_rate(&oracle, &token, &10_000_000i128);

    // Buy 1 key to generate reserve
    ctx.client
        .buy_with(&ctx.buyer, &ctx.creator, &1u32, &token, &10_000_000i128);
    let reserve = ctx.client.get_payment_asset_reserve(&token);
    assert!(reserve > 0);

    // Admin removes asset
    ctx.client.remove_payment_asset(&ctx.admin, &token);

    // New buys are blocked
    let res = ctx
        .client
        .try_buy_with(&ctx.buyer, &ctx.creator, &1u32, &token, &10_000_000i128);
    assert_eq!(res, Err(Ok(ContractError::Unauthorized)));

    // Existing reserve can still be redeemed
    let redeemed = ctx
        .client
        .redeem_payment_asset(&ctx.admin, &token, &reserve);
    assert_eq!(redeemed, reserve);
    assert_eq!(ctx.client.get_payment_asset_reserve(&token), 0);
}
