//! Tests for Issue #1172:
//! Implement slippage protection parameter on buy and sell with revert on breach.

use crate::events::{SlippageExceeded, SlippageExceededEvent, SLIPPAGE_EXCEEDED_EVENT_NAME};
use crate::{ContractError, CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, IntoVal, String, Symbol,
};

const KEY_PRICE: i128 = 1_000;
const SLOPE: i128 = 100;
// Set 5% creator fee + 5% protocol fee (10% total fee)
// so that net proceeds on 1000 key price are 900 (100 fee = 1000 bps slippage).
const CREATOR_BPS: u32 = 500;
const PROTOCOL_BPS: u32 = 500;

struct TestContext {
    env: Env,
    client: CreatorKeysContractClient<'static>,
    contract_id: Address,
    creator: Address,
}

fn setup_test() -> TestContext {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
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
            handle: String::from_str(&env, "alice"),
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
        contract_id,
        creator,
    }
}

fn advance_ledger(env: &Env) {
    let mut l = env.ledger().get();
    l.sequence_number += 1;
    env.ledger().set(l);
}

// -----------------------------------------------------------------------------
// Acceptance Criterion 1 & 2: Revert on breach + SlippageExceeded event payload
// -----------------------------------------------------------------------------

#[test]
fn test_buy_slippage_breach_reverts_and_emits_event_payload() {
    let ctx = setup_test();
    let buyer1 = Address::generate(&ctx.env);

    // Initial buy to move supply to 1 (spot price moves from 1000 to 1100)
    let quote = ctx.client.get_buy_quote(&ctx.creator);
    ctx.client
        .buy_key(&ctx.creator, &buyer1, &quote.total_amount, &None);

    let buyer2 = Address::generate(&ctx.env);
    let next_quote = ctx.client.get_buy_quote(&ctx.creator);

    // Tolerance of 100 bps is breached because actual slippage is 1000 bps
    let result = ctx.client.try_buy_key_with_slippage(
        &ctx.creator,
        &buyer2,
        &next_quote.total_amount,
        &None,
        &Some(100),
    );
    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));

    // Verify SlippageExceeded event is published with accurate fields
    let mut found_event = false;
    for (contract, topics, data) in ctx.env.events().all().iter() {
        if contract != ctx.contract_id {
            continue;
        }
        let name: Symbol = topics.get(0).unwrap().into_val(&ctx.env);
        if name == SLIPPAGE_EXCEEDED_EVENT_NAME {
            let payload: SlippageExceeded = data.clone().into_val(&ctx.env);
            assert_eq!(payload.quoted_price, 1_000);
            assert_eq!(payload.executed_price, 1_100);
            assert_eq!(payload.slippage_bps, 1_000);
            found_event = true;
        }
    }
    assert!(found_event, "expected SlippageExceeded event on breach");
}

#[test]
fn test_sell_slippage_breach_reverts_and_emits_event_payload() {
    let ctx = setup_test();
    let holder = Address::generate(&ctx.env);

    // Buy 1 key first
    let quote = ctx.client.get_buy_quote(&ctx.creator);
    ctx.client
        .buy_key(&ctx.creator, &holder, &quote.total_amount, &None);

    advance_ledger(&ctx.env);

    // Sell with strict slippage (100 bps tolerance, but fee impact is 1000 bps)
    let result = ctx
        .client
        .try_sell_key_with_slippage(&ctx.creator, &holder, &None, &Some(100));
    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));

    let mut found_event = false;
    for (contract, topics, data) in ctx.env.events().all().iter() {
        if contract != ctx.contract_id {
            continue;
        }
        let name: Symbol = topics.get(0).unwrap().into_val(&ctx.env);
        if name == SLIPPAGE_EXCEEDED_EVENT_NAME {
            let payload: SlippageExceededEvent = data.clone().into_val(&ctx.env);
            assert_eq!(payload.quoted_price, 1_000); // curve_price at supply 0
            assert_eq!(payload.executed_price, 900); // proceeds after fees (1000 - 100)
            assert_eq!(payload.slippage_bps, 1_000); // |900 - 1000| / 1000 * 10000 = 1000 bps
            found_event = true;
        }
    }
    assert!(
        found_event,
        "expected SlippageExceeded event on sell breach"
    );
}

// -----------------------------------------------------------------------------
// Acceptance Criterion 3: max_slippage_bps = 0 allows trade regardless
// -----------------------------------------------------------------------------

#[test]
fn test_buy_max_slippage_bps_zero_disables_check() {
    let ctx = setup_test();
    let buyer1 = Address::generate(&ctx.env);

    let quote1 = ctx.client.get_buy_quote(&ctx.creator);
    let supply1 = ctx.client.buy_key_with_slippage(
        &ctx.creator,
        &buyer1,
        &quote1.total_amount,
        &None,
        &Some(0),
    );
    assert_eq!(supply1, 1);

    // Even when price moves, max_slippage_bps = 0 allows trade
    let buyer2 = Address::generate(&ctx.env);
    let quote2 = ctx.client.get_buy_quote(&ctx.creator);
    let supply2 = ctx.client.buy_key_with_slippage(
        &ctx.creator,
        &buyer2,
        &quote2.total_amount,
        &None,
        &Some(0),
    );
    assert_eq!(supply2, 2);
}

#[test]
fn test_sell_max_slippage_bps_zero_disables_check() {
    let ctx = setup_test();
    let holder = Address::generate(&ctx.env);

    let quote = ctx.client.get_buy_quote(&ctx.creator);
    ctx.client
        .buy_key(&ctx.creator, &holder, &quote.total_amount, &None);

    advance_ledger(&ctx.env);

    let supply = ctx
        .client
        .sell_key_with_slippage(&ctx.creator, &holder, &None, &Some(0));
    assert_eq!(supply, 0);
}

// -----------------------------------------------------------------------------
// Acceptance Criterion 4 & 5: Trades within tolerance succeed + correct formula
// -----------------------------------------------------------------------------

#[test]
fn test_buy_within_tolerance_succeeds() {
    let ctx = setup_test();
    let buyer1 = Address::generate(&ctx.env);

    let quote1 = ctx.client.get_buy_quote(&ctx.creator);
    ctx.client
        .buy_key(&ctx.creator, &buyer1, &quote1.total_amount, &None);

    let buyer2 = Address::generate(&ctx.env);
    let quote2 = ctx.client.get_buy_quote(&ctx.creator);

    // Actual slippage is 1000 bps. Tolerance of 1000 bps (equality) must pass.
    let supply = ctx.client.buy_key_with_slippage(
        &ctx.creator,
        &buyer2,
        &quote2.total_amount,
        &None,
        &Some(1_000),
    );
    assert_eq!(supply, 2);
    assert_eq!(ctx.client.get_key_balance(&ctx.creator, &buyer2), 1);
}

#[test]
fn test_sell_within_tolerance_succeeds() {
    let ctx = setup_test();
    let holder = Address::generate(&ctx.env);

    let quote = ctx.client.get_buy_quote(&ctx.creator);
    ctx.client
        .buy_key(&ctx.creator, &holder, &quote.total_amount, &None);

    advance_ledger(&ctx.env);

    // Actual sell slippage from fees is 1000 bps. Tolerance of 1000 bps must pass.
    let supply = ctx
        .client
        .sell_key_with_slippage(&ctx.creator, &holder, &None, &Some(1_000));
    assert_eq!(supply, 0);
}

// -----------------------------------------------------------------------------
// Aliases: buy_with_slippage, sell, sell_with_slippage, buy_keys_with_slippage
// -----------------------------------------------------------------------------

#[test]
fn test_buy_and_sell_aliases_with_slippage() {
    let ctx = setup_test();
    let trader = Address::generate(&ctx.env);

    let quote = ctx.client.get_buy_quote(&ctx.creator);

    // buy alias
    let supply_buy = ctx
        .client
        .buy(&ctx.creator, &trader, &quote.total_amount, &None);
    assert_eq!(supply_buy, 1);

    advance_ledger(&ctx.env);

    // sell alias
    let supply_sell = ctx.client.sell(&ctx.creator, &trader, &None);
    assert_eq!(supply_sell, 0);

    // buy_with_slippage alias
    let quote2 = ctx.client.get_buy_quote(&ctx.creator);
    let supply_buy_slip = ctx.client.buy_with_slippage(
        &ctx.creator,
        &trader,
        &quote2.total_amount,
        &None,
        &Some(10_000),
    );
    assert_eq!(supply_buy_slip, 1);

    advance_ledger(&ctx.env);

    // sell_with_slippage alias
    let supply_sell_slip =
        ctx.client
            .sell_with_slippage(&ctx.creator, &trader, &None, &Some(10_000));
    assert_eq!(supply_sell_slip, 0);
}

#[test]
fn test_multi_key_buy_with_slippage() {
    let ctx = setup_test();
    let trader = Address::generate(&ctx.env);

    // Multi-key buy: 3 keys with generous tolerance
    let total_payment = 100_000i128;
    let supply_after_buys = ctx.client.buy_keys_with_slippage(
        &ctx.creator,
        &trader,
        &3u32,
        &total_payment,
        &None,
        &Some(10_000),
    );
    assert_eq!(supply_after_buys, 3);
    assert_eq!(ctx.client.get_key_balance(&ctx.creator, &trader), 3);

    // Next multi-key buy with strict tolerance reverts
    let trader2 = Address::generate(&ctx.env);
    let result = ctx.client.try_buy_keys_with_slippage(
        &ctx.creator,
        &trader2,
        &2u32,
        &total_payment,
        &None,
        &Some(1),
    );
    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));
}
