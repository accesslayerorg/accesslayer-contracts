//! Tests for issue #1035 (key-pair liquidity depth oracle for batch trade
//! pre-validation): `get_max_buy` / `get_max_sell` return the maximum trade
//! amount executable within a price-impact tolerance plus the expected
//! post-trade price, without mutating curve state.

use crate::{
    ContractError, CreatorKeysContract, CreatorKeysContractClient, CurvePreset,
    LiquidityDepthQuote, RegisterCreatorParams,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

const BASE_PRICE: i128 = 1_000;
const SLOPE: i128 = 10;

struct Setup {
    env: Env,
    client: CreatorKeysContractClient<'static>,
    admin: Address,
    creator: Address,
}

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_treasury_address(&admin, &treasury);
    client.set_key_price(&admin, &BASE_PRICE);
    // A zero-sum fee config is rejected, so use the smallest legal split; at
    // 1 bp the fee floors to zero stroops on these price magnitudes and does
    // not affect the depth (curve-only) math under test.
    client.set_fee_config(&admin, &1u32, &0u32);
    client.set_protocol_fee_recipient(&admin, &treasury);
    client.set_curve_slope(&admin, &SLOPE);

    let creator = Address::generate(&env);
    register(&env, &client, &creator, "depth01", None, None);

    Setup {
        env,
        client,
        admin,
        creator,
    }
}

fn register(
    env: &Env,
    client: &CreatorKeysContractClient,
    creator: &Address,
    handle: &str,
    preset: Option<CurvePreset>,
    max_supply: Option<u32>,
) {
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &max_supply,
        &None,
        &preset,
        &None,
        &None,
    );
}

fn buy_keys(s: &Setup, creator: &Address, buyer: &Address, quantity: u32) {
    for _ in 0..quantity {
        s.client.buy_key(creator, buyer, &100_000i128, &None);
    }
}

/// Independent recomputation of the impact the oracle must respect:
/// `(post - current).abs() * 10_000 / current`, floored.
fn impact_bps(current: i128, post: i128) -> i128 {
    ((post - current).abs() * 10_000) / current
}

fn assert_depth_within_tolerance(
    s: &Setup,
    creator: &Address,
    depth: &LiquidityDepthQuote,
    supply: u32,
    tolerance_bps: u32,
    is_buy: bool,
) {
    let post_supply = if is_buy {
        supply + depth.max_amount
    } else {
        supply - depth.max_amount
    };
    assert_eq!(
        depth.expected_price,
        s.client.get_price_at_supply(creator, &(post_supply as u64)),
        "expected_price must equal the curve price after full execution"
    );
    assert!(
        impact_bps(depth.current_price, depth.expected_price) <= tolerance_bps as i128,
        "full execution must stay within the impact tolerance"
    );
    // Maximality: one more key must exceed the tolerance. For sells that extra
    // key only exists while the curve would not be walked past zero supply.
    let one_more_supply = if is_buy {
        Some(supply + depth.max_amount + 1)
    } else {
        supply
            .checked_sub(depth.max_amount)
            .and_then(|remaining| remaining.checked_sub(1))
    };
    if depth.max_amount > 0 {
        if let Some(one_more_supply) = one_more_supply {
            let one_more_price = s
                .client
                .get_price_at_supply(creator, &(one_more_supply as u64));
            assert!(
                impact_bps(depth.current_price, one_more_price) > tolerance_bps as i128,
                "returned amount must be maximal for the tolerance"
            );
        }
    }
}

// ─── get_max_buy ──────────────────────────────────────────────────────────

#[test]
fn get_max_buy_returns_amount_for_linear_curve_within_tolerance() {
    let s = setup();
    // P(s) = 1000 + 10s; from supply 0, impact(n) = 100n bps.
    // 1 % tolerance: n = 1 (impact 100 bps); n = 2 would be 200 bps.
    let depth = s.client.get_max_buy(&s.creator, &100u32);
    assert_eq!(depth.current_price, 1_000);
    assert_eq!(depth.max_amount, 1);
    assert_eq!(depth.expected_price, 1_010);
    assert_depth_within_tolerance(&s, &s.creator, &depth, 0, 100, true);
}

#[test]
fn get_max_buy_scales_with_tolerance() {
    let s = setup();
    // From supply 0: impact(n) = 100n bps, so 5 % permits exactly 5 keys.
    let depth = s.client.get_max_buy(&s.creator, &500u32);
    assert_eq!(depth.max_amount, 5);
    assert_eq!(depth.expected_price, 1_050);
    assert_depth_within_tolerance(&s, &s.creator, &depth, 0, 500, true);

    // 2.5 % permits 2 keys (200 bps), not 3 (300 bps).
    let depth = s.client.get_max_buy(&s.creator, &250u32);
    assert_eq!(depth.max_amount, 2);
    assert_eq!(depth.expected_price, 1_020);
    assert_depth_within_tolerance(&s, &s.creator, &depth, 0, 250, true);
}

#[test]
fn get_max_buy_zero_tolerance_on_rising_curve_returns_zero() {
    let s = setup();
    let depth = s.client.get_max_buy(&s.creator, &0u32);
    assert_eq!(depth.max_amount, 0);
    assert_eq!(depth.current_price, 1_000);
    assert_eq!(depth.expected_price, 1_000);
}

#[test]
fn get_max_buy_includes_exact_boundary_impact() {
    let s = setup();
    buy_keys(&s, &s.creator, &Address::generate(&s.env), 10);
    // From supply 10, P0 = 1100; impact(n) = floor(10n * 10000 / 1100).
    // n = 11 -> floor(1_100_000/1100) = 1000 bps exactly, still within 1000.
    let depth = s.client.get_max_buy(&s.creator, &1000u32);
    assert_eq!(depth.current_price, 1_100);
    assert_eq!(depth.max_amount, 11);
    assert_eq!(depth.expected_price, 1_210);
    assert_depth_within_tolerance(&s, &s.creator, &depth, 10, 1000, true);
}

#[test]
fn get_max_buy_respects_supply_cap() {
    let s = setup();
    let capped = Address::generate(&s.env);
    register(&s.env, &s.client, &capped, "capped", None, Some(3));
    // Wide tolerance, but only 3 keys can ever be minted.
    let depth = s.client.get_max_buy(&capped, &10_000u32);
    assert_eq!(depth.max_amount, 3);
    assert_eq!(depth.expected_price, 1_030);
}

#[test]
fn get_max_buy_flat_curve_returns_full_scan_depth() {
    let s = setup();
    let flat = Address::generate(&s.env);
    register(
        &s.env,
        &s.client,
        &flat,
        "flatky",
        Some(CurvePreset::Flat),
        None,
    );
    // Price never moves on a flat curve, so any tolerance admits the entire
    // (uncapped) scan depth.
    let depth = s.client.get_max_buy(&flat, &0u32);
    assert_eq!(depth.max_amount, crate::MAX_DEPTH_SCAN_LIMIT);
    assert_eq!(depth.current_price, BASE_PRICE);
    assert_eq!(depth.expected_price, BASE_PRICE);

    // With a cap the depth is limited to what can still be minted.
    let flat_capped = Address::generate(&s.env);
    register(
        &s.env,
        &s.client,
        &flat_capped,
        "flatcp",
        Some(CurvePreset::Flat),
        Some(20),
    );
    let depth = s.client.get_max_buy(&flat_capped, &0u32);
    assert_eq!(depth.max_amount, 20);
    assert_eq!(depth.expected_price, BASE_PRICE);
}

#[test]
fn get_max_buy_quadratic_curve_math() {
    let s = setup();
    let quad = Address::generate(&s.env);
    register(
        &s.env,
        &s.client,
        &quad,
        "quadk",
        Some(CurvePreset::Quadratic),
        None,
    );
    // Quadratic: P(s) = 1000 + s^2 (slope 1). From supply 0,
    // impact(n) = floor(n^2 * 10000 / 1000) = 10n^2 bps.
    // 10 % tolerance: n = 10 -> impact 1000 bps exactly (within);
    // n = 11 -> 1210 bps (exceeds).
    s.client.set_curve_slope(&s.admin, &1i128);
    let depth = s.client.get_max_buy(&quad, &1000u32);
    assert_eq!(depth.current_price, 1_000);
    assert_eq!(depth.max_amount, 10);
    assert_eq!(depth.expected_price, 1_100);
    assert_depth_within_tolerance(&s, &quad, &depth, 0, 1000, true);
}

#[test]
fn get_max_buy_matches_subsequent_execution_price() {
    let s = setup();
    let depth = s.client.get_max_buy(&s.creator, &500u32);
    assert_eq!(depth.max_amount, 5);

    let buyer = Address::generate(&s.env);
    buy_keys(&s, &s.creator, &buyer, depth.max_amount);
    assert_eq!(s.client.get_creator_supply(&s.creator), 5u32);
    // The curve price the oracle predicted is the price now sitting at the
    // post-trade supply.
    assert_eq!(
        s.client.get_price_at_supply(&s.creator, &5u64),
        depth.expected_price
    );
}

// ─── get_max_sell ─────────────────────────────────────────────────────────

#[test]
fn get_max_sell_returns_amount_within_tolerance() {
    let s = setup();
    let holder = Address::generate(&s.env);
    buy_keys(&s, &s.creator, &holder, 5);
    // From supply 5, P0 = 1050; impact(n) = floor(10n * 10000 / 1050).
    // n = 1 -> 95 bps (within 100); n = 2 -> 190 bps.
    let depth = s.client.get_max_sell(&s.creator, &100u32);
    assert_eq!(depth.current_price, 1_050);
    assert_eq!(depth.max_amount, 1);
    assert_eq!(depth.expected_price, 1_040);
    assert_depth_within_tolerance(&s, &s.creator, &depth, 5, 100, false);
}

#[test]
fn get_max_sell_large_tolerance_returns_entire_supply() {
    let s = setup();
    let holder = Address::generate(&s.env);
    buy_keys(&s, &s.creator, &holder, 5);
    // A 100 % tolerance admits selling everything down to zero supply.
    let depth = s.client.get_max_sell(&s.creator, &10_000u32);
    assert_eq!(depth.max_amount, 5);
    assert_eq!(depth.expected_price, BASE_PRICE);
    assert_depth_within_tolerance(&s, &s.creator, &depth, 5, 10_000, false);
}

#[test]
fn get_max_sell_zero_supply_returns_empty_depth_gracefully() {
    let s = setup();
    // Registered but no keys traded: zero sellable liquidity.
    let depth = s.client.get_max_sell(&s.creator, &10_000u32);
    assert_eq!(depth.max_amount, 0);
    assert_eq!(depth.current_price, BASE_PRICE);
    assert_eq!(depth.expected_price, BASE_PRICE);
}

#[test]
fn get_max_sell_zero_tolerance_on_rising_curve_returns_zero() {
    let s = setup();
    let holder = Address::generate(&s.env);
    buy_keys(&s, &s.creator, &holder, 3);
    let depth = s.client.get_max_sell(&s.creator, &0u32);
    assert_eq!(depth.max_amount, 0);
    assert_eq!(depth.current_price, 1_030);
    assert_eq!(depth.expected_price, 1_030);
}

#[test]
fn get_max_sell_flat_curve_returns_entire_supply() {
    let s = setup();
    let flat = Address::generate(&s.env);
    register(
        &s.env,
        &s.client,
        &flat,
        "flatsl",
        Some(CurvePreset::Flat),
        None,
    );
    buy_keys(&s, &flat, &Address::generate(&s.env), 7);
    // Price never moves, so everything is sellable within any tolerance.
    let depth = s.client.get_max_sell(&flat, &0u32);
    assert_eq!(depth.max_amount, 7);
    assert_eq!(depth.current_price, BASE_PRICE);
    assert_eq!(depth.expected_price, BASE_PRICE);
}

// ─── read-only behaviour ──────────────────────────────────────────────────

#[test]
fn depth_oracles_do_not_mutate_curve_state() {
    let s = setup();
    let holder = Address::generate(&s.env);
    buy_keys(&s, &s.creator, &holder, 4);

    let supply_before = s.client.get_creator_supply(&s.creator);
    let balance_before = s.client.get_key_balance(&s.creator, &holder);
    let slope_before = s.client.get_curve_slope();
    let bid_ask_before = s.client.get_bid_ask_price(&s.creator);
    let buy_quote_before = s.client.get_buy_quote(&s.creator);

    let buy_depth = s.client.get_max_buy(&s.creator, &300u32);
    let sell_depth = s.client.get_max_sell(&s.creator, &300u32);
    // Repeated calls are stable.
    let buy_depth_again = s.client.get_max_buy(&s.creator, &300u32);
    let sell_depth_again = s.client.get_max_sell(&s.creator, &300u32);

    assert_eq!(buy_depth, buy_depth_again);
    assert_eq!(sell_depth, sell_depth_again);

    assert_eq!(s.client.get_creator_supply(&s.creator), supply_before);
    assert_eq!(
        s.client.get_key_balance(&s.creator, &holder),
        balance_before
    );
    assert_eq!(s.client.get_curve_slope(), slope_before);
    assert_eq!(s.client.get_bid_ask_price(&s.creator), bid_ask_before);
    assert_eq!(s.client.get_buy_quote(&s.creator), buy_quote_before);
}

// ─── error handling ───────────────────────────────────────────────────────

#[test]
fn depth_oracles_reject_unregistered_key() {
    let s = setup();
    let unknown = Address::generate(&s.env);
    assert_eq!(
        s.client.try_get_max_buy(&unknown, &100u32),
        Err(Ok(ContractError::NotRegistered))
    );
    assert_eq!(
        s.client.try_get_max_sell(&unknown, &100u32),
        Err(Ok(ContractError::NotRegistered))
    );
}

#[test]
fn depth_oracles_reject_impact_above_100_percent() {
    let s = setup();
    assert_eq!(
        s.client.try_get_max_buy(&s.creator, &10_001u32),
        Err(Ok(ContractError::InvalidImpactTolerance))
    );
    assert_eq!(
        s.client.try_get_max_sell(&s.creator, &10_001u32),
        Err(Ok(ContractError::InvalidImpactTolerance))
    );
    // Exactly 100 % is accepted: impact(n) = 100n bps, so n = 100 fits.
    assert_eq!(
        s.client.try_get_max_buy(&s.creator, &10_000u32),
        Ok(Ok(LiquidityDepthQuote {
            max_amount: 100,
            current_price: 1_000,
            expected_price: 2_000,
        }))
    );
}

#[test]
fn depth_oracles_report_missing_key_price() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    let creator = Address::generate(&env);
    register(&env, &client, &creator, "noprc", None, None);

    assert_eq!(
        client.try_get_max_buy(&creator, &100u32),
        Err(Ok(ContractError::KeyPriceNotSet))
    );
    assert_eq!(
        client.try_get_max_sell(&creator, &100u32),
        Err(Ok(ContractError::KeyPriceNotSet))
    );
}
