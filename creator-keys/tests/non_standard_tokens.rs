//! Issue #1327: Non-standard token compatibility tests for buy, sell, escrow,
//! claim, and distribution paths.

use creator_keys::p2p_swap_dividend_stream_oracle::{
    KeyDividendContract, KeyDividendContractClient, KeySwapContract, KeySwapContractClient,
    PayoutStreamContract, PayoutStreamContractClient,
};
use creator_keys::{BondError, ContractError, CreatorKeysContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::TokenClient,
    vec, Address, Env,
};
use test_utils::{
    FeeOnTransferToken, NoOpTransferToken, PanickingToken, RebasingToken, UnusualDecimalToken,
};

mod contract_test_env;
use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};

const KEY_PRICE: i128 = 1_000_000;

fn setup_creator_keys(
    env: &Env,
) -> (
    CreatorKeysContractClient<'_>,
    Address,
    Address,
    Address,
    Address,
) {
    let (client, contract_id) = register_creator_keys(env);
    let admin = set_pricing_and_fees(env, &client, KEY_PRICE, 500, 500);
    let treasury = Address::generate(env);
    client.set_treasury_address(&admin, &treasury);
    let creator = register_test_creator(env, &client, "compatcreator");
    let buyer = Address::generate(env);
    (client, contract_id, admin, creator, buyer)
}

fn configure_payment_asset(
    env: &Env,
    client: &CreatorKeysContractClient,
    admin: &Address,
    token: &Address,
    decimals: u32,
) {
    let oracle = Address::generate(env);
    client.allowlist_token(admin, token);
    client.add_payment_asset(admin, token, &decimals, &oracle);
    client.set_payment_asset_rate(&oracle, token, &10_000_000i128);
}

// ---------------------------------------------------------------------------
// 1. Admin Allowlist Checks
// ---------------------------------------------------------------------------

#[test]
fn only_allowlisted_tokens_can_be_configured() {
    let env = test_env_with_auths();
    let (client, _contract, admin, _creator, _buyer) = setup_creator_keys(&env);
    let unvetted = Address::generate(&env);
    let oracle = Address::generate(&env);

    // Non-allowlisted token cannot be added as a payment asset
    assert_eq!(
        client.try_add_payment_asset(&admin, &unvetted, &7, &oracle),
        Err(Ok(ContractError::UnsupportedToken))
    );

    // Non-allowlisted token cannot be configured as performance bond token
    assert_eq!(
        client.try_set_xlm_token_address(&admin, &unvetted),
        Err(Ok(BondError::UnsupportedToken))
    );

    // Allowlisting enables configuration, and revoking disables new configuration
    client.allowlist_token(&admin, &unvetted);
    assert!(client.is_token_allowlisted(&unvetted));
    client.add_payment_asset(&admin, &unvetted, &7, &oracle);

    let unvetted2 = Address::generate(&env);
    client.allowlist_token(&admin, &unvetted2);
    client.remove_allowlisted_token(&admin, &unvetted2);
    assert!(!client.is_token_allowlisted(&unvetted2));
    assert_eq!(
        client.try_set_xlm_token_address(&admin, &unvetted2),
        Err(Ok(BondError::UnsupportedToken))
    );
}

// ---------------------------------------------------------------------------
// 2. Buy Path (buy_with) across all 5 mock behaviors
// ---------------------------------------------------------------------------

#[test]
fn buy_with_fee_on_transfer_reverts_before_accounting() {
    let env = test_env_with_auths();
    let (client, contract_id, admin, creator, buyer) = setup_creator_keys(&env);

    let (token_client, token_addr) = FeeOnTransferToken::register(&env);
    token_client.set_fee_bps(&500); // 5% fee
    token_client.mint(&buyer, &100_000_000);
    configure_payment_asset(&env, &client, &admin, &token_addr, 7);

    let res = client.try_buy_with(&buyer, &creator, &1, &token_addr, &10_000_000);
    assert_eq!(res, Err(Ok(ContractError::UnsupportedToken)));

    // Accounting and balances remain untouched
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_payment_asset_reserve(&token_addr), 0);
    assert_eq!(client.get_payment_base_reserve(), 0);
    assert_eq!(token_client.balance(&contract_id), 0);
}

#[test]
fn buy_with_no_op_token_reverts_before_accounting() {
    let env = test_env_with_auths();
    let (client, _contract_id, admin, creator, buyer) = setup_creator_keys(&env);

    let (token_client, token_addr) = NoOpTransferToken::register(&env);
    token_client.mint(&buyer, &100_000_000);
    configure_payment_asset(&env, &client, &admin, &token_addr, 7);

    let res = client.try_buy_with(&buyer, &creator, &1, &token_addr, &10_000_000);
    assert_eq!(res, Err(Ok(ContractError::UnsupportedToken)));
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_payment_asset_reserve(&token_addr), 0);
    assert_eq!(client.get_payment_base_reserve(), 0);
}

#[test]
fn buy_with_rebasing_on_transfer_reverts_before_accounting() {
    let env = test_env_with_auths();
    let (client, _contract_id, admin, creator, buyer) = setup_creator_keys(&env);

    let (token_client, token_addr) = RebasingToken::register(&env);
    token_client.mint(&buyer, &100_000_000);
    token_client.set_rebase_on_transfer_bps(&9_000); // -10% rebase during transfer
    configure_payment_asset(&env, &client, &admin, &token_addr, 7);

    let res = client.try_buy_with(&buyer, &creator, &1, &token_addr, &10_000_000);
    assert_eq!(res, Err(Ok(ContractError::UnsupportedToken)));
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_payment_asset_reserve(&token_addr), 0);
}

#[test]
fn buy_with_panicking_token_leaves_state_unchanged() {
    let env = test_env_with_auths();
    let (client, _contract_id, admin, creator, buyer) = setup_creator_keys(&env);

    let (token_client, token_addr) = PanickingToken::register(&env);
    token_client.mint(&buyer, &100_000_000);
    configure_payment_asset(&env, &client, &admin, &token_addr, 7);

    let res = client.try_buy_with(&buyer, &creator, &1, &token_addr, &10_000_000);
    assert!(res.is_err());
    assert_eq!(client.get_total_key_supply(&creator), 0);
    assert_eq!(client.get_payment_asset_reserve(&token_addr), 0);
    assert_eq!(client.get_payment_base_reserve(), 0);
}

#[test]
fn buy_with_unusual_decimals_validates_bounds_and_matches_balance() {
    let env = test_env_with_auths();
    let (client, contract_id, admin, creator, buyer) = setup_creator_keys(&env);
    let oracle = Address::generate(&env);

    // 0 decimals and 25 decimals are rejected at configuration time
    let (_t0, addr0) = UnusualDecimalToken::register(&env);
    client.allowlist_token(&admin, &addr0);
    assert_eq!(
        client.try_add_payment_asset(&admin, &addr0, &0, &oracle),
        Err(Ok(ContractError::UnsupportedToken))
    );

    let (_t25, addr25) = UnusualDecimalToken::register(&env);
    client.allowlist_token(&admin, &addr25);
    assert_eq!(
        client.try_add_payment_asset(&admin, &addr25, &25, &oracle),
        Err(Ok(ContractError::UnsupportedToken))
    );

    // 2 decimals and 18 decimals are supported and reserve matches actual balance
    let (t2, addr2) = UnusualDecimalToken::register(&env);
    t2.set_decimals(&2);
    t2.mint(&buyer, &1_000_000);
    configure_payment_asset(&env, &client, &admin, &addr2, 2);

    let (t18, addr18) = UnusualDecimalToken::register(&env);
    t18.set_decimals(&18);
    t18.mint(&buyer, &100_000_000_000_000_000_000i128);
    configure_payment_asset(&env, &client, &admin, &addr18, 18);

    client.buy_with(&buyer, &creator, &1, &addr2, &1_000_000);
    assert_eq!(
        client.get_payment_asset_reserve(&addr2),
        t2.balance(&contract_id)
    );

    client.buy_with(
        &buyer,
        &creator,
        &1,
        &addr18,
        &100_000_000_000_000_000_000i128,
    );
    assert_eq!(
        client.get_payment_asset_reserve(&addr18),
        t18.balance(&contract_id)
    );
}

// ---------------------------------------------------------------------------
// 3. Sell Path (sell_with) across all 5 mock behaviors
// ---------------------------------------------------------------------------

#[test]
fn sell_with_fee_on_transfer_reverts_with_unsupported_token() {
    let env = test_env_with_auths();
    let (client, _contract_id, admin, creator, buyer) = setup_creator_keys(&env);

    let (token_client, token_addr) = FeeOnTransferToken::register(&env);
    token_client.set_fee_bps(&0); // 0% during buy so we can fund & hold a key
    token_client.mint(&buyer, &100_000_000);
    configure_payment_asset(&env, &client, &admin, &token_addr, 7);

    client.buy_with(&buyer, &creator, &1, &token_addr, &10_000_000);
    env.ledger().set_sequence_number(10);
    let reserve_before = client.get_payment_asset_reserve(&token_addr);

    // Enable fee-on-transfer before sell
    token_client.set_fee_bps(&500);
    let res = client.try_sell_with(&buyer, &creator, &1, &token_addr, &0);
    assert_eq!(res, Err(Ok(ContractError::UnsupportedToken)));
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(
        client.get_payment_asset_reserve(&token_addr),
        reserve_before
    );
}

#[test]
fn sell_with_rebasing_on_transfer_reverts_with_unsupported_token() {
    let env = test_env_with_auths();
    let (client, _contract_id, admin, creator, buyer) = setup_creator_keys(&env);

    let (token_client, token_addr) = RebasingToken::register(&env);
    token_client.mint(&buyer, &100_000_000);
    configure_payment_asset(&env, &client, &admin, &token_addr, 7);

    client.buy_with(&buyer, &creator, &1, &token_addr, &10_000_000);
    env.ledger().set_sequence_number(10);
    token_client.set_rebase_on_transfer_bps(&9_000);

    let res = client.try_sell_with(&buyer, &creator, &1, &token_addr, &0);
    assert_eq!(res, Err(Ok(ContractError::UnsupportedToken)));
    assert_eq!(client.get_total_key_supply(&creator), 1);
}

#[test]
fn sell_with_panicking_token_leaves_state_unchanged() {
    let env = test_env_with_auths();
    let (client, _contract_id, admin, creator, buyer) = setup_creator_keys(&env);

    let (token_client, token_addr) = PanickingToken::register(&env);
    token_client.set_panic_on_transfer(&false);
    token_client.mint(&buyer, &100_000_000);
    configure_payment_asset(&env, &client, &admin, &token_addr, 7);

    client.buy_with(&buyer, &creator, &1, &token_addr, &10_000_000);
    env.ledger().set_sequence_number(10);
    let reserve_before = client.get_payment_asset_reserve(&token_addr);

    token_client.set_panic_on_transfer(&true);
    let res = client.try_sell_with(&buyer, &creator, &1, &token_addr, &0);
    assert!(res.is_err());
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(
        client.get_payment_asset_reserve(&token_addr),
        reserve_before
    );
}

#[test]
fn sell_with_unusual_decimals_matches_actual_balance() {
    let env = test_env_with_auths();
    let (client, contract_id, admin, creator, buyer) = setup_creator_keys(&env);

    let (t18, addr18) = UnusualDecimalToken::register(&env);
    t18.set_decimals(&18);
    t18.mint(&buyer, &100_000_000_000_000_000_000i128);
    configure_payment_asset(&env, &client, &admin, &addr18, 18);

    client.buy_with(
        &buyer,
        &creator,
        &2,
        &addr18,
        &100_000_000_000_000_000_000i128,
    );
    env.ledger().set_sequence_number(10);
    client.sell_with(&buyer, &creator, &1, &addr18, &0);
    assert_eq!(client.get_total_key_supply(&creator), 1);
    assert_eq!(
        client.get_payment_asset_reserve(&addr18),
        t18.balance(&contract_id)
    );
}

// ---------------------------------------------------------------------------
// 4. Escrow Path (performance_bonds, KeySwapContract, PayoutStreamContract)
// ---------------------------------------------------------------------------

#[test]
fn escrow_performance_bond_rejects_non_standard_tokens() {
    // Fee-on-transfer
    {
        let env = test_env_with_auths();
        let (client, contract_id, admin, creator, _) = setup_creator_keys(&env);
        let (t, addr) = FeeOnTransferToken::register(&env);
        t.set_fee_bps(&500);
        t.mint(&creator, &10_000);
        client.allowlist_token(&admin, &addr);
        client.set_xlm_token_address(&admin, &addr);
        client.set_min_performance_bond(&admin, &1_000);

        assert_eq!(
            client.try_deposit_performance_bond(&creator, &10_000),
            Err(Ok(BondError::UnsupportedToken))
        );
        assert_eq!(t.balance(&contract_id), 0);
        assert_eq!(client.get_bond_status(&creator).deposited, 0);
    }

    // No-op transfer
    {
        let env = test_env_with_auths();
        let (client, _, admin, creator, _) = setup_creator_keys(&env);
        let (t, addr) = NoOpTransferToken::register(&env);
        t.mint(&creator, &10_000);
        client.allowlist_token(&admin, &addr);
        client.set_xlm_token_address(&admin, &addr);
        client.set_min_performance_bond(&admin, &1_000);

        assert_eq!(
            client.try_deposit_performance_bond(&creator, &10_000),
            Err(Ok(BondError::UnsupportedToken))
        );
        assert_eq!(client.get_bond_status(&creator).deposited, 0);
    }

    // Rebasing on transfer
    {
        let env = test_env_with_auths();
        let (client, _, admin, creator, _) = setup_creator_keys(&env);
        let (t, addr) = RebasingToken::register(&env);
        t.mint(&creator, &10_000);
        t.set_rebase_on_transfer_bps(&8_000);
        client.allowlist_token(&admin, &addr);
        client.set_xlm_token_address(&admin, &addr);
        client.set_min_performance_bond(&admin, &1_000);

        assert_eq!(
            client.try_deposit_performance_bond(&creator, &10_000),
            Err(Ok(BondError::UnsupportedToken))
        );
        assert_eq!(client.get_bond_status(&creator).deposited, 0);
    }

    // Panicking
    {
        let env = test_env_with_auths();
        let (client, _, admin, creator, _) = setup_creator_keys(&env);
        let (t, addr) = PanickingToken::register(&env);
        t.mint(&creator, &10_000);
        client.allowlist_token(&admin, &addr);
        client.set_xlm_token_address(&admin, &addr);
        client.set_min_performance_bond(&admin, &1_000);

        assert!(client
            .try_deposit_performance_bond(&creator, &10_000)
            .is_err());
        assert_eq!(client.get_bond_status(&creator).deposited, 0);
    }

    // Unusual decimal (18 decimals, exact transfer) succeeds and matches balance
    {
        let env = test_env_with_auths();
        let (client, contract_id, admin, creator, _) = setup_creator_keys(&env);
        let (t, addr) = UnusualDecimalToken::register(&env);
        t.set_decimals(&18);
        t.mint(&creator, &10_000);
        client.allowlist_token(&admin, &addr);
        client.set_xlm_token_address(&admin, &addr);
        client.set_min_performance_bond(&admin, &1_000);

        client.deposit_performance_bond(&creator, &10_000);
        assert_eq!(client.get_bond_status(&creator).deposited, 10_000);
        assert_eq!(t.balance(&contract_id), 10_000);
    }
}

#[test]
fn escrow_p2p_swap_and_stream_reject_non_standard_tokens() {
    let env = test_env_with_auths();
    let swap_id = env.register(KeySwapContract, ());
    let swap = KeySwapContractClient::new(&env, &swap_id);
    let proposer = Address::generate(&env);
    let counterparty = Address::generate(&env);
    let req_token = Address::generate(&env);

    // Fee-on-transfer
    let (fee_t, fee_addr) = FeeOnTransferToken::register(&env);
    fee_t.mint(&proposer, &1_000);
    assert_eq!(
        swap.try_propose_swap(
            &proposer,
            &fee_addr,
            &1_000,
            &req_token,
            &500,
            &counterparty
        ),
        Err(Ok(ContractError::UnsupportedToken))
    );

    // No-op
    let (noop_t, noop_addr) = NoOpTransferToken::register(&env);
    noop_t.mint(&proposer, &1_000);
    assert_eq!(
        swap.try_propose_swap(
            &proposer,
            &noop_addr,
            &1_000,
            &req_token,
            &500,
            &counterparty
        ),
        Err(Ok(ContractError::UnsupportedToken))
    );

    // Rebasing on transfer
    let (reb_t, reb_addr) = RebasingToken::register(&env);
    reb_t.mint(&proposer, &1_000);
    reb_t.set_rebase_on_transfer_bps(&9_000);
    assert_eq!(
        swap.try_propose_swap(
            &proposer,
            &reb_addr,
            &1_000,
            &req_token,
            &500,
            &counterparty
        ),
        Err(Ok(ContractError::UnsupportedToken))
    );

    // Panicking
    let (pan_t, pan_addr) = PanickingToken::register(&env);
    pan_t.mint(&proposer, &1_000);
    assert!(swap
        .try_propose_swap(
            &proposer,
            &pan_addr,
            &1_000,
            &req_token,
            &500,
            &counterparty
        )
        .is_err());

    // Stream escrow
    let stream_id = env.register(PayoutStreamContract, ());
    let stream = PayoutStreamContractClient::new(&env, &stream_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let key_id = Address::generate(&env);
    stream.init_stream(&admin, &fee_addr);
    let sid = stream.create_stream(&creator, &key_id, &10, &1_000);
    assert_eq!(
        stream.try_fund_stream(&proposer, &sid, &1_000),
        Err(Ok(ContractError::UnsupportedToken))
    );
    assert_eq!(stream.stream_info(&sid).funded, 0);
}

// ---------------------------------------------------------------------------
// 5. Distribution & Claim Paths (KeyDividendContract, PayoutStreamContract)
// ---------------------------------------------------------------------------

#[test]
fn distribution_and_claim_paths_reject_non_standard_tokens_and_preserve_accounting() {
    let env = test_env_with_auths();
    env.ledger().set_sequence_number(10);

    let div_id = env.register(KeyDividendContract, ());
    let div = KeyDividendContractClient::new(&env, &div_id);
    let creator = Address::generate(&env);
    let holder = Address::generate(&env);

    // Key token (unusual 2-decimal token works as key token)
    let (key_t, key_addr) = UnusualDecimalToken::register(&env);
    key_t.set_decimals(&2);
    key_t.mint(&holder, &100);
    let holders = vec![&env, holder.clone()];

    // Fee-on-transfer distribution rejected
    let (fee_t, fee_addr) = FeeOnTransferToken::register(&env);
    fee_t.mint(&creator, &1_000);
    assert_eq!(
        div.try_declare_dividend(&creator, &key_addr, &fee_addr, &1_000, &holders, &100),
        Err(Ok(ContractError::UnsupportedToken))
    );

    // No-op distribution rejected
    let (noop_t, noop_addr) = NoOpTransferToken::register(&env);
    noop_t.mint(&creator, &1_000);
    assert_eq!(
        div.try_declare_dividend(&creator, &key_addr, &noop_addr, &1_000, &holders, &100),
        Err(Ok(ContractError::UnsupportedToken))
    );

    // Rebasing distribution rejected
    let (reb_t, reb_addr) = RebasingToken::register(&env);
    reb_t.mint(&creator, &1_000);
    reb_t.set_rebase_on_transfer_bps(&9_000);
    assert_eq!(
        div.try_declare_dividend(&creator, &key_addr, &reb_addr, &1_000, &holders, &100),
        Err(Ok(ContractError::UnsupportedToken))
    );

    // Panicking distribution leaves state unchanged; panicking claim leaves state unchanged
    let (pan_t, pan_addr) = PanickingToken::register(&env);
    pan_t.mint(&creator, &1_000);
    assert!(div
        .try_declare_dividend(&creator, &key_addr, &pan_addr, &1_000, &holders, &100)
        .is_err());

    // Fund with panic disabled, then enable panic for claim
    pan_t.set_panic_on_transfer(&false);
    let did = div.declare_dividend(&creator, &key_addr, &pan_addr, &1_000, &holders, &100);
    assert_eq!(pan_t.balance(&div_id), 1_000);

    pan_t.set_panic_on_transfer(&true);
    assert!(div.try_claim_dividend(&holder, &key_addr, &did).is_err());
    assert_eq!(div.get_dividend(&key_addr, &did).claimed, 0);
    assert_eq!(pan_t.balance(&div_id), 1_000);

    // Supported unusual-decimal distribution & claim match balances
    let (u_t, u_addr) = UnusualDecimalToken::register(&env);
    u_t.set_decimals(&18);
    u_t.mint(&creator, &1_000);
    let did2 = div.declare_dividend(&creator, &key_addr, &u_addr, &1_000, &holders, &100);
    assert_eq!(u_t.balance(&div_id), 1_000);
    let claimed = div.claim_dividend(&holder, &key_addr, &did2);
    assert_eq!(claimed, 1_000);
    assert_eq!(u_t.balance(&holder), 1_000);
    assert_eq!(u_t.balance(&div_id), 0);

    // Stream claim path with panicking token leaves withdrawn == 0
    let stream_id = env.register(PayoutStreamContract, ());
    let stream = PayoutStreamContractClient::new(&env, &stream_id);
    let admin = Address::generate(&env);
    pan_t.set_panic_on_transfer(&false);
    pan_t.mint(&creator, &1_000);
    stream.init_stream(&admin, &pan_addr);
    let sid = stream.create_stream(&creator, &key_addr, &100, &1_000);
    stream.fund_stream(&creator, &sid, &1_000);
    env.ledger().set_sequence_number(20);
    pan_t.set_panic_on_transfer(&true);
    assert!(stream.try_withdraw(&creator, &sid, &500).is_err());
    assert_eq!(stream.stream_info(&sid).withdrawn, 0);
    assert_eq!(TokenClient::new(&env, &pan_addr).balance(&stream_id), 1_000);
}
