//! Cross-contract authorization-tree tests (issue #1318).
//!
//! Every test here pins the *exact* authorization tree a user signs for a flow
//! that crosses a contract boundary, using strict `mock_auths` (not
//! `mock_all_auths`) so any extra `require_auth` that is not in the tree makes
//! the call fail, and asserting `env.auths()` equals the documented tree so an
//! unexpected invocation or argument also fails the test.
//!
//! Flow mapping (see `docs/auth-trees.md`):
//!
//! - buy / sell: `buy_key`, `sell_key` and `swap` (which also calls the
//!   circuit breaker through `authorize_as_current_contract`).
//! - escrow: `KeySwapContract` P2P escrow and the creator performance bond.
//! - key transfer callbacks: `transfer_and_call` into a `KeyReceiver`.
//! - lending: the repository has no lending flow; nothing to test.

mod contract_test_env;

use circuit_breaker::{CircuitBreakerContract, CircuitBreakerContractClient, Thresholds};
use common::KEY_RECEIVED_ACK;
use contract_test_env::{register_test_creator, set_pricing_and_fees};
use creator_keys::{
    p2p_swap_dividend_stream_oracle::{KeySwapContract, KeySwapContractClient},
    CreatorKeysContract, CreatorKeysContractClient, SwapDirection,
};
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    Address, Bytes, Env, IntoVal, Symbol, Val, Vec,
};

// ── helpers ─────────────────────────────────────────────────────────────────

fn invocation(
    env: &Env,
    contract: &Address,
    fn_name: &str,
    args: Vec<Val>,
    sub_invocations: std::vec::Vec<AuthorizedInvocation>,
) -> AuthorizedInvocation {
    AuthorizedInvocation {
        function: AuthorizedFunction::Contract((contract.clone(), Symbol::new(env, fn_name), args)),
        sub_invocations,
    }
}

/// Fails if any address authorized anything beyond `expected`, including
/// unexpected sub-invocations or different arguments.
fn assert_exact_auths(env: &Env, expected: std::vec::Vec<(Address, AuthorizedInvocation)>) {
    assert_eq!(env.auths(), expected);
}

fn new_sac(env: &Env) -> Address {
    env.register_stellar_asset_contract_v2(Address::generate(env))
        .address()
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    StellarAssetClient::new(env, token).mint(to, &amount);
}

fn balance(env: &Env, token: &Address, owner: &Address) -> i128 {
    TokenClient::new(env, token).balance(owner)
}

struct Setup {
    env: Env,
    client: CreatorKeysContractClient<'static>,
    keys: Address,
    admin: Address,
    creator: Address,
}

/// Flat price curve so the bonding-curve circuit breaker never trips.
fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();
    let keys = env.register(CreatorKeysContract, ());
    let client: CreatorKeysContractClient<'static> = CreatorKeysContractClient::new(&env, &keys);
    let admin = set_pricing_and_fees(&env, &client, 100, 9_000, 1_000);
    let creator = register_test_creator(&env, &client, "authtree");
    Setup {
        env,
        client,
        keys,
        admin,
        creator,
    }
}

// ── buy / sell ──────────────────────────────────────────────────────────────

#[test]
fn buy_key_tree_is_single_root_with_no_sub_invocations() {
    let s = setup();
    let env = &s.env;
    let buyer = Address::generate(env);
    let args: Vec<Val> = (s.creator.clone(), buyer.clone(), 1_000i128, None::<i128>).into_val(env);

    env.mock_auths(&[MockAuth {
        address: &buyer,
        invoke: &MockAuthInvoke {
            contract: &s.keys,
            fn_name: "buy_key",
            args: args.clone(),
            sub_invokes: &[],
        },
    }]);
    assert_eq!(s.client.buy_key(&s.creator, &buyer, &1_000, &None), 1);

    assert_exact_auths(
        env,
        std::vec![(
            buyer.clone(),
            invocation(env, &s.keys, "buy_key", args, std::vec![])
        )],
    );
}

#[test]
fn buy_key_rejects_signature_for_different_arguments() {
    let s = setup();
    let env = &s.env;
    let buyer = Address::generate(env);
    // The buyer signed for a payment of 1_000; a relayer submitting 5_000
    // must not be able to reuse that signature.
    env.mock_auths(&[MockAuth {
        address: &buyer,
        invoke: &MockAuthInvoke {
            contract: &s.keys,
            fn_name: "buy_key",
            args: (s.creator.clone(), buyer.clone(), 1_000i128, None::<i128>).into_val(env),
            sub_invokes: &[],
        },
    }]);
    assert!(s
        .client
        .try_buy_key(&s.creator, &buyer, &5_000, &None)
        .is_err());
}

#[test]
fn sell_key_tree_is_single_root_with_no_sub_invocations() {
    let s = setup();
    let env = &s.env;
    let seller = Address::generate(env);
    s.client.buy_key(&s.creator, &seller, &1_000, &None);
    env.ledger_advance();

    let args: Vec<Val> = (s.creator.clone(), seller.clone(), None::<i128>).into_val(env);
    env.mock_auths(&[MockAuth {
        address: &seller,
        invoke: &MockAuthInvoke {
            contract: &s.keys,
            fn_name: "sell_key",
            args: args.clone(),
            sub_invokes: &[],
        },
    }]);
    s.client.sell_key(&s.creator, &seller, &None);

    assert_exact_auths(
        env,
        std::vec![(
            seller.clone(),
            invocation(env, &s.keys, "sell_key", args, std::vec![])
        )],
    );
}

/// Advance one ledger so the flash-loan guard allows a sell after a buy.
trait LedgerAdvance {
    fn ledger_advance(&self);
}

impl LedgerAdvance for Env {
    fn ledger_advance(&self) {
        use soroban_sdk::testutils::Ledger;
        let mut info = self.ledger().get();
        info.sequence_number += 1;
        info.timestamp += 5;
        self.ledger().set(info);
    }
}

fn install_real_breaker(s: &Setup) -> Address {
    let breaker_id = s.env.register(CircuitBreakerContract, ());
    CircuitBreakerContractClient::new(&s.env, &breaker_id).initialize(
        &Address::generate(&s.env),
        &s.keys,
        &Thresholds {
            max_hourly_volume: i128::MAX,
            max_single_swap_impact_bps: 10_000,
            max_swaps_per_block: 100,
        },
    );
    s.client
        .set_circuit_breaker(&s.admin, &Some(breaker_id.clone()));
    breaker_id
}

fn mock_swap(s: &Setup, trader: &Address, direction: SwapDirection) -> Vec<Val> {
    let args: Vec<Val> =
        (s.creator.clone(), trader.clone(), 1u32, direction, 0u32).into_val(&s.env);
    s.env.mock_auths(&[MockAuth {
        address: trader,
        invoke: &MockAuthInvoke {
            contract: &s.keys,
            fn_name: "swap",
            args: args.clone(),
            sub_invokes: &[],
        },
    }]);
    args
}

#[test]
fn swap_tree_excludes_breaker_self_authorization() {
    let s = setup();
    let env = &s.env;
    let breaker = install_real_breaker(&s);
    let trader = Address::generate(env);

    let args = mock_swap(&s, &trader, SwapDirection::Buy);
    assert!(
        s.client
            .swap(&s.creator, &trader, &1, &SwapDirection::Buy, &0)
            > 0
    );
    // The breaker's `record_swap(caller = creator-keys, ...)` is authorized by
    // creator-keys itself via `authorize_as_current_contract`; the trader's
    // tree must not grow a sub-invocation for it. The real breaker requires
    // that authorization (`caller.require_auth()`), so the call above also
    // proves the self-authorization entry matches its arguments exactly.
    assert_exact_auths(
        env,
        std::vec![(
            trader.clone(),
            invocation(env, &s.keys, "swap", args, std::vec![])
        )],
    );
    assert!(!CircuitBreakerContractClient::new(env, &breaker).is_suspended(&s.creator));
}

#[contracttype]
enum EvilKey {
    Mode,
    Token,
    Attacker,
    Victim,
}

#[derive(Clone)]
#[contracttype]
enum BreakerMode {
    /// Spend creator-keys' self-authorization on a nested token transfer.
    DrainCaller,
    /// Ask for the trader's signature, which only covers `swap`.
    StealTrader,
}

/// Malicious circuit breaker installed by a compromised admin key.
#[contract]
struct MaliciousBreaker;

#[contractimpl]
impl MaliciousBreaker {
    pub fn configure_breaker(
        env: Env,
        mode: BreakerMode,
        token: Address,
        attacker: Address,
        victim: Address,
    ) {
        let st = env.storage().instance();
        st.set(&EvilKey::Mode, &mode);
        st.set(&EvilKey::Token, &token);
        st.set(&EvilKey::Attacker, &attacker);
        st.set(&EvilKey::Victim, &victim);
    }

    pub fn is_suspended(_env: Env, _key_id: Address) -> bool {
        false
    }

    pub fn record_swap(
        env: Env,
        caller: Address,
        _key_id: Address,
        amount: i128,
        _price_impact_bps: u32,
    ) -> Result<bool, circuit_breaker::CircuitBreakerError> {
        caller.require_auth();
        let st = env.storage().instance();
        let mode: BreakerMode = st.get(&EvilKey::Mode).unwrap();
        let token: Address = st.get(&EvilKey::Token).unwrap();
        let attacker: Address = st.get(&EvilKey::Attacker).unwrap();
        let victim: Address = st.get(&EvilKey::Victim).unwrap();
        match mode {
            BreakerMode::DrainCaller => {
                TokenClient::new(&env, &token).transfer(&caller, &attacker, &amount);
            }
            BreakerMode::StealTrader => {
                victim.require_auth();
                TokenClient::new(&env, &token).transfer(&victim, &attacker, &amount);
            }
        }
        Ok(true)
    }
}

fn install_malicious_breaker(s: &Setup, mode: BreakerMode, victim: &Address) -> (Address, Address) {
    let env = &s.env;
    let token = new_sac(env);
    let attacker = Address::generate(env);
    mint(env, &token, &s.keys, 1_000_000);
    mint(env, &token, victim, 1_000_000);
    let breaker = env.register(MaliciousBreaker, ());
    MaliciousBreakerClient::new(env, &breaker).configure_breaker(&mode, &token, &attacker, victim);
    s.client.set_circuit_breaker(&s.admin, &Some(breaker));
    (token, attacker)
}

#[test]
fn malicious_breaker_cannot_extend_contract_self_authorization() {
    let s = setup();
    let env = &s.env;
    let trader = Address::generate(env);
    let (token, attacker) = install_malicious_breaker(&s, BreakerMode::DrainCaller, &trader);

    mock_swap(&s, &trader, SwapDirection::Buy);
    assert!(s
        .client
        .try_swap(&s.creator, &trader, &1, &SwapDirection::Buy, &0)
        .is_err());
    assert_eq!(balance(env, &token, &attacker), 0);
    assert_eq!(balance(env, &token, &s.keys), 1_000_000);
}

#[test]
fn malicious_breaker_cannot_reuse_trader_signature() {
    let s = setup();
    let env = &s.env;
    let trader = Address::generate(env);
    let (token, attacker) = install_malicious_breaker(&s, BreakerMode::StealTrader, &trader);

    mock_swap(&s, &trader, SwapDirection::Buy);
    assert!(s
        .client
        .try_swap(&s.creator, &trader, &1, &SwapDirection::Buy, &0)
        .is_err());
    assert_eq!(balance(env, &token, &attacker), 0);
    assert_eq!(balance(env, &token, &trader), 1_000_000);
}

// ── escrow: P2P key swap ────────────────────────────────────────────────────

/// Token that, on `transfer`, also tries to move a second token out of the
/// sender's wallet using the same signature.
#[contract]
struct MaliciousToken;

#[contractimpl]
impl MaliciousToken {
    pub fn configure_token(env: Env, token: Address, attacker: Address) {
        env.storage().instance().set(&EvilKey::Token, &token);
        env.storage().instance().set(&EvilKey::Attacker, &attacker);
    }

    pub fn balance(_env: Env, _id: Address) -> i128 {
        i128::MAX
    }

    pub fn transfer(env: Env, from: Address, _to: Address, amount: i128) {
        from.require_auth();
        let victim_token: Address = env.storage().instance().get(&EvilKey::Token).unwrap();
        let attacker: Address = env.storage().instance().get(&EvilKey::Attacker).unwrap();
        TokenClient::new(&env, &victim_token).transfer(&from, &attacker, &amount);
    }
}

fn propose_args(
    env: &Env,
    proposer: &Address,
    offered: &Address,
    requested: &Address,
    counterparty: &Address,
) -> Vec<Val> {
    (
        proposer.clone(),
        offered.clone(),
        50i128,
        requested.clone(),
        30i128,
        counterparty.clone(),
    )
        .into_val(env)
}

#[test]
fn p2p_swap_escrow_trees_are_minimal() {
    let env = Env::default();
    env.mock_all_auths();
    let swap_id = env.register(KeySwapContract, ());
    let swap = KeySwapContractClient::new(&env, &swap_id);
    let offered = new_sac(&env);
    let requested = new_sac(&env);
    let proposer = Address::generate(&env);
    let counterparty = Address::generate(&env);
    mint(&env, &offered, &proposer, 100);
    mint(&env, &requested, &counterparty, 100);

    // propose_swap: proposer -> propose_swap -> offered.transfer(proposer, escrow, 50)
    let args = propose_args(&env, &proposer, &offered, &requested, &counterparty);
    let transfer_args: Vec<Val> = (proposer.clone(), swap_id.clone(), 50i128).into_val(&env);
    env.mock_auths(&[MockAuth {
        address: &proposer,
        invoke: &MockAuthInvoke {
            contract: &swap_id,
            fn_name: "propose_swap",
            args: args.clone(),
            sub_invokes: &[MockAuthInvoke {
                contract: &offered,
                fn_name: "transfer",
                args: transfer_args.clone(),
                sub_invokes: &[],
            }],
        },
    }]);
    let id = swap.propose_swap(&proposer, &offered, &50, &requested, &30, &counterparty);
    assert_exact_auths(
        &env,
        std::vec![(
            proposer.clone(),
            invocation(
                &env,
                &swap_id,
                "propose_swap",
                args,
                std::vec![invocation(
                    &env,
                    &offered,
                    "transfer",
                    transfer_args,
                    std::vec![]
                )],
            )
        )],
    );

    // accept_swap: counterparty -> accept_swap -> requested.transfer(counterparty, proposer, 30).
    // The escrow release (offered.transfer(escrow, counterparty, 50)) is
    // authorized by the escrow contract as direct invoker, not by any user.
    let accept_args: Vec<Val> = (counterparty.clone(), id).into_val(&env);
    let pay_args: Vec<Val> = (counterparty.clone(), proposer.clone(), 30i128).into_val(&env);
    env.mock_auths(&[MockAuth {
        address: &counterparty,
        invoke: &MockAuthInvoke {
            contract: &swap_id,
            fn_name: "accept_swap",
            args: accept_args.clone(),
            sub_invokes: &[MockAuthInvoke {
                contract: &requested,
                fn_name: "transfer",
                args: pay_args.clone(),
                sub_invokes: &[],
            }],
        },
    }]);
    swap.accept_swap(&counterparty, &id);
    assert_exact_auths(
        &env,
        std::vec![(
            counterparty.clone(),
            invocation(
                &env,
                &swap_id,
                "accept_swap",
                accept_args,
                std::vec![invocation(
                    &env,
                    &requested,
                    "transfer",
                    pay_args,
                    std::vec![]
                )],
            )
        )],
    );
    assert_eq!(balance(&env, &offered, &counterparty), 50);
    assert_eq!(balance(&env, &requested, &proposer), 30);
}

#[test]
fn p2p_swap_cancel_tree_has_no_sub_invocations() {
    let env = Env::default();
    env.mock_all_auths();
    let swap_id = env.register(KeySwapContract, ());
    let swap = KeySwapContractClient::new(&env, &swap_id);
    let offered = new_sac(&env);
    let requested = new_sac(&env);
    let proposer = Address::generate(&env);
    let counterparty = Address::generate(&env);
    mint(&env, &offered, &proposer, 100);
    let id = swap.propose_swap(&proposer, &offered, &50, &requested, &30, &counterparty);

    let cancel_args: Vec<Val> = (proposer.clone(), id).into_val(&env);
    env.mock_auths(&[MockAuth {
        address: &proposer,
        invoke: &MockAuthInvoke {
            contract: &swap_id,
            fn_name: "cancel_swap",
            args: cancel_args.clone(),
            sub_invokes: &[],
        },
    }]);
    swap.cancel_swap(&proposer, &id);
    assert_exact_auths(
        &env,
        std::vec![(
            proposer.clone(),
            invocation(&env, &swap_id, "cancel_swap", cancel_args, std::vec![])
        )],
    );
    assert_eq!(balance(&env, &offered, &proposer), 100);
}

#[test]
fn malicious_offered_token_cannot_reuse_proposer_signature() {
    let env = Env::default();
    env.mock_all_auths();
    let swap_id = env.register(KeySwapContract, ());
    let swap = KeySwapContractClient::new(&env, &swap_id);
    let victim_token = new_sac(&env);
    let requested = new_sac(&env);
    let proposer = Address::generate(&env);
    let counterparty = Address::generate(&env);
    let attacker = Address::generate(&env);
    mint(&env, &victim_token, &proposer, 1_000);
    let evil = env.register(MaliciousToken, ());
    MaliciousTokenClient::new(&env, &evil).configure_token(&victim_token, &attacker);

    // The proposer signs exactly the documented tree for offering `evil`.
    let args = propose_args(&env, &proposer, &evil, &requested, &counterparty);
    env.mock_auths(&[MockAuth {
        address: &proposer,
        invoke: &MockAuthInvoke {
            contract: &swap_id,
            fn_name: "propose_swap",
            args,
            sub_invokes: &[MockAuthInvoke {
                contract: &evil,
                fn_name: "transfer",
                args: (proposer.clone(), swap_id.clone(), 50i128).into_val(&env),
                sub_invokes: &[],
            }],
        },
    }]);
    assert!(swap
        .try_propose_swap(&proposer, &evil, &50, &requested, &30, &counterparty)
        .is_err());
    assert_eq!(balance(&env, &victim_token, &attacker), 0);
    assert_eq!(balance(&env, &victim_token, &proposer), 1_000);
}

#[test]
fn exact_auth_assertion_detects_unexpected_nested_invocation() {
    // Under permissive mocking the malicious token's extra request succeeds,
    // and the recorded tree differs from the documented one. This is the
    // regression guard: an unexpected invocation makes `assert_exact_auths`
    // fail.
    let env = Env::default();
    env.mock_all_auths();
    let swap_id = env.register(KeySwapContract, ());
    let swap = KeySwapContractClient::new(&env, &swap_id);
    let victim_token = new_sac(&env);
    let requested = new_sac(&env);
    let proposer = Address::generate(&env);
    let counterparty = Address::generate(&env);
    let attacker = Address::generate(&env);
    mint(&env, &victim_token, &proposer, 1_000);
    let evil = env.register(MaliciousToken, ());
    MaliciousTokenClient::new(&env, &evil).configure_token(&victim_token, &attacker);

    swap.propose_swap(&proposer, &evil, &50, &requested, &30, &counterparty);
    let documented = std::vec![(
        proposer.clone(),
        invocation(
            &env,
            &swap_id,
            "propose_swap",
            propose_args(&env, &proposer, &evil, &requested, &counterparty),
            std::vec![invocation(
                &env,
                &evil,
                "transfer",
                (proposer.clone(), swap_id.clone(), 50i128).into_val(&env),
                std::vec![],
            )],
        )
    )];
    let recorded = env.auths();
    assert_ne!(recorded, documented);
    // The difference is exactly the smuggled nested transfer.
    assert_eq!(recorded.len(), 1);
    let evil_transfer = &recorded[0].1.sub_invocations[0];
    assert_eq!(
        evil_transfer.sub_invocations,
        std::vec![invocation(
            &env,
            &victim_token,
            "transfer",
            (proposer.clone(), attacker.clone(), 50i128).into_val(&env),
            std::vec![],
        )]
    );
}

// ── escrow: performance bond ────────────────────────────────────────────────

#[test]
fn performance_bond_deposit_tree_is_minimal() {
    let s = setup();
    let env = &s.env;
    let xlm = new_sac(env);
    s.client.set_xlm_token_address(&s.admin, &xlm);
    s.client.set_min_performance_bond(&s.admin, &1_000);
    mint(env, &xlm, &s.creator, 5_000);

    let args: Vec<Val> = (s.creator.clone(), 2_000i128).into_val(env);
    let transfer_args: Vec<Val> = (s.creator.clone(), s.keys.clone(), 2_000i128).into_val(env);
    env.mock_auths(&[MockAuth {
        address: &s.creator,
        invoke: &MockAuthInvoke {
            contract: &s.keys,
            fn_name: "deposit_performance_bond",
            args: args.clone(),
            sub_invokes: &[MockAuthInvoke {
                contract: &xlm,
                fn_name: "transfer",
                args: transfer_args.clone(),
                sub_invokes: &[],
            }],
        },
    }]);
    s.client.deposit_performance_bond(&s.creator, &2_000);

    assert_exact_auths(
        env,
        std::vec![(
            s.creator.clone(),
            invocation(
                env,
                &s.keys,
                "deposit_performance_bond",
                args,
                std::vec![invocation(
                    env,
                    &xlm,
                    "transfer",
                    transfer_args,
                    std::vec![]
                )],
            )
        )],
    );
    assert_eq!(balance(env, &xlm, &s.keys), 2_000);
}

#[test]
fn performance_bond_deposit_rejects_unbound_token_signature() {
    // A signature for a bare `xlm.transfer(creator, keys, amount)` (not
    // rooted at `deposit_performance_bond`) must not be enough, because the
    // contract requires the creator's authorization for the deposit itself.
    let s = setup();
    let env = &s.env;
    let xlm = new_sac(env);
    s.client.set_xlm_token_address(&s.admin, &xlm);
    s.client.set_min_performance_bond(&s.admin, &1_000);
    mint(env, &xlm, &s.creator, 5_000);

    env.mock_auths(&[MockAuth {
        address: &s.creator,
        invoke: &MockAuthInvoke {
            contract: &xlm,
            fn_name: "transfer",
            args: (s.creator.clone(), s.keys.clone(), 2_000i128).into_val(env),
            sub_invokes: &[],
        },
    }]);
    assert!(s
        .client
        .try_deposit_performance_bond(&s.creator, &2_000)
        .is_err());
    assert_eq!(balance(env, &xlm, &s.keys), 0);
}

// ── key transfer callbacks ──────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
enum ReceiverMode {
    Honest,
    StealSender,
}

#[contract]
struct AuthTreeReceiver;

#[contractimpl]
impl AuthTreeReceiver {
    pub fn configure_receiver(env: Env, mode: ReceiverMode, token: Address, attacker: Address) {
        let st = env.storage().instance();
        st.set(&EvilKey::Mode, &mode);
        st.set(&EvilKey::Token, &token);
        st.set(&EvilKey::Attacker, &attacker);
    }

    pub fn on_key_received(
        env: Env,
        from: Address,
        _key_id: Address,
        _amount: u32,
        _data: Bytes,
    ) -> Symbol {
        let st = env.storage().instance();
        let mode: ReceiverMode = st.get(&EvilKey::Mode).unwrap();
        if let ReceiverMode::StealSender = mode {
            let token: Address = st.get(&EvilKey::Token).unwrap();
            let attacker: Address = st.get(&EvilKey::Attacker).unwrap();
            TokenClient::new(&env, &token).transfer(&from, &attacker, &100);
        }
        KEY_RECEIVED_ACK
    }
}

fn transfer_and_call_case(mode: ReceiverMode) {
    let s = setup();
    let env = &s.env;
    let sender = Address::generate(env);
    s.client.buy_key(&s.creator, &sender, &1_000, &None);
    let token = new_sac(env);
    let attacker = Address::generate(env);
    mint(env, &token, &sender, 1_000);
    let receiver = env.register(AuthTreeReceiver, ());
    AuthTreeReceiverClient::new(env, &receiver).configure_receiver(&mode, &token, &attacker);

    let data = Bytes::from_slice(env, b"hi");
    let args: Vec<Val> = (
        s.creator.clone(),
        sender.clone(),
        receiver.clone(),
        1u32,
        data.clone(),
    )
        .into_val(env);
    env.mock_auths(&[MockAuth {
        address: &sender,
        invoke: &MockAuthInvoke {
            contract: &s.keys,
            fn_name: "transfer_and_call",
            args: args.clone(),
            sub_invokes: &[],
        },
    }]);
    let result = s
        .client
        .try_transfer_and_call(&s.creator, &sender, &receiver, &1, &data);

    match mode {
        ReceiverMode::Honest => {
            assert!(result.is_ok());
            assert_exact_auths(
                env,
                std::vec![(
                    sender.clone(),
                    invocation(env, &s.keys, "transfer_and_call", args, std::vec![])
                )],
            );
        }
        ReceiverMode::StealSender => {
            assert!(result.is_err());
            assert_eq!(balance(env, &token, &attacker), 0);
            assert_eq!(balance(env, &token, &sender), 1_000);
        }
    }
}

#[test]
fn transfer_and_call_tree_excludes_receiver_callback() {
    transfer_and_call_case(ReceiverMode::Honest);
}

#[test]
fn malicious_receiver_cannot_reuse_sender_signature() {
    transfer_and_call_case(ReceiverMode::StealSender);
}
