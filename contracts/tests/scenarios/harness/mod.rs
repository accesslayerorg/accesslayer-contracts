//! Shared harness for the cross-contract lifecycle scenarios.
//!
//! Every scenario builds the same deployment: creator-keys, token/oracle mocks,
//! keeper and escrow mocks, multisig-admin, a protocol admin, a vault and four
//! trade actors. The harness records fee liabilities, escrow obligations,
//! resource samples and a deterministic RNG seed so each step can be replayed.

#![allow(dead_code)]

mod mocks;

use self::mocks::{
    MockEscrowContract, MockEscrowContractClient, MockKeeperContract, MockKeeperContractClient,
};
use creator_keys::{
    fee, ContractError, CreatorKeysContract, CreatorKeysContractClient, CurvePreset, QuoteResponse,
    RegisterCreatorParams,
};
use multisig_admin::{
    ActionKind, ContractCallAction, MultisigAdminContract, MultisigAdminContractClient,
    ProposalAction,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, IntoVal, String, Symbol, TryFromVal, Val, Vec as SorobanVec,
};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    string::String as StdString,
    vec::Vec as StdVec,
};
use test_utils::{MockOracle, MockOracleContractClient, MockToken, MockTokenContractClient};

pub const BASE_SEED: u64 = 0x1320_2026_1127;
pub const DEFAULT_TIMESTAMP: u64 = 1_700_000_000;
pub const KEY_PRICE: i128 = 10_000;
pub const CREATOR_BPS: u32 = 9_000;
pub const PROTOCOL_BPS: u32 = 1_000;
pub const STARTING_BALANCE: i128 = 2_000_000;

pub fn scenario_seed(label: &str) -> u64 {
    let mut hash = BASE_SEED ^ 0xcbf2_9ce4_8422_2325;
    for byte in label.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[derive(Clone, Copy)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn gen_range(&mut self, min: u64, max_exclusive: u64) -> u64 {
        if max_exclusive <= min {
            return min;
        }
        min + (self.next_u64() % (max_exclusive - min))
    }

    pub fn gen_quantity(&mut self, min: u32, max_inclusive: u32) -> u32 {
        self.gen_range(u64::from(min), u64::from(max_inclusive) + 1) as u32
    }
}

#[derive(Clone, Debug)]
pub struct ResourceSample {
    pub step: u32,
    pub label: StdString,
    pub cpu: u64,
    pub memory: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Liabilities {
    pub protocol_fee: i128,
    pub creator_fee: i128,
    pub royalty: i128,
}

pub struct Harness {
    pub env: &'static Env,
    pub keys_id: Address,
    pub keys: CreatorKeysContractClient<'static>,
    pub token_id: Address,
    pub token: MockTokenContractClient<'static>,
    pub oracle_id: Address,
    pub oracle: MockOracleContractClient<'static>,
    pub keeper_id: Address,
    pub keeper: MockKeeperContractClient<'static>,
    pub escrow_id: Address,
    pub escrow: MockEscrowContractClient<'static>,
    pub multisig_id: Address,
    pub multisig: MultisigAdminContractClient<'static>,
    pub admin: Address,
    pub vault: Address,
    pub arbitrator: Address,
    pub protocol_recipient: Address,
    pub governors: StdVec<Address>,
    pub actors: StdVec<Address>,
    pub creators: StdVec<Address>,
    pub seed: u64,
    pub liabilities: Liabilities,
    pub escrow_creator: Option<Address>,
    pub buyback_liability: i128,
    pub dispute_liability: i128,
    pub enforce_protocol_invariants: bool,
    rng: Rng,
    step: u32,
    resources: StdVec<ResourceSample>,
}

impl Harness {
    pub fn new(seed: u64) -> Self {
        let env: &'static Env = Box::leak(Box::new(Env::default()));
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();
        let mut ledger = env.ledger().get();
        ledger.timestamp = DEFAULT_TIMESTAMP;
        env.ledger().set(ledger);

        let admin = Address::generate(env);
        let vault = Address::generate(env);
        let arbitrator = Address::generate(env);
        let protocol_recipient = Address::generate(env);
        let governors: StdVec<Address> = (0..3).map(|_| Address::generate(env)).collect();
        let governor_vec = Self::to_soroban_vec(env, &governors);

        let keys_id = env.register(CreatorKeysContract, ());
        let keys = CreatorKeysContractClient::new(env, &keys_id);
        keys.set_protocol_admin(&admin, &admin);
        keys.set_protocol_fee_recipient(&admin, &protocol_recipient);
        keys.set_treasury_address(&admin, &protocol_recipient);
        keys.set_key_price(&admin, &KEY_PRICE);
        keys.set_curve_slope(&admin, &0);
        keys.set_fee_config(&admin, &CREATOR_BPS, &PROTOCOL_BPS);

        let (token, token_id) = MockToken::register(env);
        let (oracle, oracle_id) = MockOracle::register(env);
        oracle.set_price(&Self::asset_symbol(env), &KEY_PRICE);

        let keeper_id = env.register(MockKeeperContract, ());
        let keeper = MockKeeperContractClient::new(env, &keeper_id);
        keeper.initialize(&admin, &oracle_id);

        let escrow_id = env.register(MockEscrowContract, ());
        let escrow = MockEscrowContractClient::new(env, &escrow_id);
        escrow.setup(&arbitrator, &token_id);

        let multisig_id = env.register(MultisigAdminContract, ());
        let multisig = MultisigAdminContractClient::new(env, &multisig_id);
        multisig.initialize(&governors[0], &governor_vec, &2);

        let mut ratifiers = governors.clone();
        ratifiers.push(multisig_id.clone());
        keys.set_global_pause_admins(&admin, &Self::to_soroban_vec(env, &ratifiers));

        let actors: StdVec<Address> = (0..4).map(|_| Address::generate(env)).collect();
        for actor in &actors {
            token.mint(actor, &STARTING_BALANCE);
        }

        Self {
            env,
            keys_id,
            keys,
            token_id,
            token,
            oracle_id,
            oracle,
            keeper_id,
            keeper,
            escrow_id,
            escrow,
            multisig_id,
            multisig,
            admin,
            vault,
            arbitrator,
            protocol_recipient,
            governors,
            actors,
            creators: StdVec::new(),
            seed,
            liabilities: Liabilities::default(),
            escrow_creator: None,
            buyback_liability: 0,
            dispute_liability: 0,
            enforce_protocol_invariants: true,
            rng: Rng::new(seed),
            step: 0,
            resources: StdVec::new(),
        }
    }

    fn asset_symbol(env: &Env) -> Symbol {
        Symbol::new(env, "KEY")
    }

    fn to_soroban_vec<T>(env: &Env, values: &[T]) -> SorobanVec<T>
    where
        T: Clone + IntoVal<Env, Val> + TryFromVal<Env, Val>,
    {
        let mut out = SorobanVec::new(env);
        for value in values {
            out.push_back(value.clone());
        }
        out
    }

    pub fn register_creator(&mut self, handle: &str) -> Address {
        let creator = Address::generate(self.env);
        self.keys.register_creator(
            &RegisterCreatorParams {
                creator: creator.clone(),
                handle: String::from_str(self.env, handle),
            },
            &None,
            &None,
            &None,
            &Some(CurvePreset::Linear),
            &None,
            &None,
        );
        self.token.mint(&creator, &STARTING_BALANCE);
        self.creators.push(creator.clone());
        creator
    }

    pub fn configure_graduated_curve(&self, creator: &Address, milestones: &[(u32, u32)]) {
        let mut curve = SorobanVec::new(self.env);
        for (threshold, exponent) in milestones {
            curve.push_back((*threshold, *exponent));
        }
        self.keys.set_graduated_curve(creator, &curve);
    }

    pub fn set_royalty(&self, creator: &Address, buy_bps: u32, sell_bps: u32) {
        self.keys.set_royalty(creator, &buy_bps, &sell_bps);
    }

    pub fn prefund_royalty(&self, creator: &Address, amount: i128) {
        self.token.transfer(creator, &self.vault, &amount);
    }

    pub fn buy_one(&mut self, creator: &Address, buyer: &Address) -> i128 {
        let quote = self.keys.get_buy_quote(creator);
        self.keys
            .buy_key(creator, buyer, &quote.total_amount, &None);
        self.token.transfer(buyer, &self.vault, &quote.total_amount);
        self.note_buy(creator, &quote);
        quote.total_amount
    }

    pub fn sell_one(&mut self, creator: &Address, seller: &Address) -> i128 {
        let quote = self.keys.get_sell_quote(creator, seller);
        self.keys
            .sell_key(creator, seller, &Some(quote.total_amount));
        self.token
            .transfer(&self.vault, seller, &quote.total_amount);
        self.note_sell(creator, &quote);
        quote.total_amount
    }

    pub fn shuffled_actors(&mut self) -> StdVec<Address> {
        let mut actors = self.actors.clone();
        if actors.len() > 1 {
            for index in (1..actors.len()).rev() {
                let swap_with = self.rng.gen_range(0, index as u64 + 1) as usize;
                actors.swap(index, swap_with);
            }
        }
        actors
    }

    pub fn random_quantity(&mut self, min: u32, max_inclusive: u32) -> u32 {
        self.rng.gen_quantity(min, max_inclusive)
    }

    pub fn advance_time(&mut self, seconds: u64) {
        let mut ledger = self.env.ledger().get();
        ledger.timestamp += seconds;
        ledger.sequence_number += 1;
        self.env.ledger().set(ledger);
    }

    pub fn sync_keeper(&mut self, _creator: &Address, price: i128) {
        let asset = Self::asset_symbol(self.env);
        self.keeper.record_price(&self.admin, &asset, &price);
        assert_eq!(self.oracle.get_price(&asset), price);
    }

    pub fn note_buy(&mut self, creator: &Address, quote: &QuoteResponse) {
        let royalty_bps = self
            .keys
            .get_royalty_config(creator)
            .map(|config| config.buy_fee_bps)
            .unwrap_or(0);
        let royalty = fee::apply_percentage_fee(quote.price, royalty_bps).unwrap_or(0);
        self.liabilities.protocol_fee += quote.protocol_fee;
        self.liabilities.creator_fee += quote.creator_fee;
        self.liabilities.royalty += royalty;
    }

    pub fn note_sell(&mut self, creator: &Address, quote: &QuoteResponse) {
        let royalty_bps = self
            .keys
            .get_royalty_config(creator)
            .map(|config| config.sell_fee_bps)
            .unwrap_or(0);
        let royalty = fee::apply_percentage_fee(quote.price, royalty_bps).unwrap_or(0);
        self.liabilities.protocol_fee += quote.protocol_fee;
        self.liabilities.creator_fee += quote.creator_fee;
        self.liabilities.royalty += royalty;
    }

    pub fn settle_fees(&mut self, creator: &Address) {
        let protocol_fee = self.liabilities.protocol_fee;
        let creator_fee = self.liabilities.creator_fee + self.liabilities.royalty;
        let creator_recipient = self.keys.get_creator_fee_recipient(creator);

        assert_eq!(self.keys.get_treasury_balance(), protocol_fee);
        assert_eq!(self.keys.get_creator_fee_balance(creator), creator_fee);

        let protocol_before = self.token.balance(&self.protocol_recipient);
        let creator_before = self.token.balance(&creator_recipient);
        self.token
            .transfer(&self.vault, &self.protocol_recipient, &protocol_fee);
        self.token
            .transfer(&self.vault, &creator_recipient, &creator_fee);
        assert_eq!(
            self.token.balance(&self.protocol_recipient) - protocol_before,
            protocol_fee
        );
        assert_eq!(
            self.token.balance(&creator_recipient) - creator_before,
            creator_fee
        );
        self.liabilities = Liabilities::default();
    }

    pub fn deposit_escrow(&mut self, creator: &Address, amount: i128) {
        self.escrow.deposit(creator, &amount);
        self.escrow_creator = Some(creator.clone());
    }

    pub fn note_buyback_liability(&mut self, amount: i128) {
        self.buyback_liability += amount;
    }

    pub fn note_dispute_liability(&mut self, amount: i128) {
        self.dispute_liability += amount;
    }

    pub fn release_buyback(&mut self, creator: &Address, holder: &Address, payout: i128) {
        self.escrow
            .release(&self.arbitrator, creator, holder, &payout);
        self.buyback_liability -= payout;
    }

    pub fn resolve_dispute(&mut self, creator: &Address, claimant: &Address, amount: i128) {
        self.escrow
            .resolve(&self.arbitrator, creator, claimant, &amount);
        self.dispute_liability -= amount;
    }

    pub fn ratify_upgrade(&mut self, action_id: u32, approver: &Address) {
        let mut args: SorobanVec<Val> = SorobanVec::new(self.env);
        args.push_back(approver.clone().into_val(self.env));
        args.push_back(action_id.into_val(self.env));
        let action = ProposalAction::ContractCall(ContractCallAction {
            kind: ActionKind::AclChange,
            target: self.keys_id.clone(),
            function: Symbol::new(self.env, "approve_upgrade"),
            args,
        });
        let proposal_id = self.multisig.propose_action(&self.governors[0], &action);
        self.multisig.sign_action(&self.governors[0], &proposal_id);
        self.multisig.sign_action(&self.governors[1], &proposal_id);
        self.multisig
            .execute_action(&self.governors[0], &proposal_id);
    }

    pub fn step<F, R>(&mut self, label: &str, action: F) -> R
    where
        F: FnOnce(&mut Harness) -> R,
    {
        self.step += 1;
        let step = self.step;
        let result = catch_unwind(AssertUnwindSafe(|| action(self)));
        let budget = self.env.cost_estimate().budget();
        self.resources.push(ResourceSample {
            step,
            label: label.to_string(),
            cpu: budget.cpu_instruction_cost(),
            memory: budget.memory_bytes_cost(),
        });
        match result {
            Ok(value) => {
                self.assert_invariants(label);
                value
            }
            Err(panic) => {
                eprintln!(
                    "scenario failure: seed={:#018x} step={} label={}",
                    self.seed, step, label
                );
                self.print_resources();
                std::panic::resume_unwind(panic);
            }
        }
    }

    pub fn assert_invariants(&self, label: &str) {
        if self.enforce_protocol_invariants {
            self.assert_protocol_invariants(label);
        }
        self.assert_reserve_invariants(label);
    }

    fn assert_protocol_invariants(&self, label: &str) {
        for creator in &self.creators {
            let supply = self.keys.get_total_key_supply(creator);
            let mut summed = 0u32;
            let mut holders = 0u32;
            for actor in &self.actors {
                let balance = self.keys.get_key_balance(creator, actor);
                summed += balance;
                if balance > 0 {
                    holders += 1;
                }
            }
            assert_eq!(
                summed, supply,
                "{label}: supply must equal the sum of tracked balances"
            );
            assert_eq!(
                holders,
                self.keys.get_creator_holder_count(creator),
                "{label}: holder count must match tracked nonzero balances"
            );
            assert!(
                self.keys.get_creator_fee_balance(creator) >= 0,
                "{label}: creator fee balance cannot be negative"
            );
        }

        assert!(
            self.keys.get_treasury_balance() >= 0,
            "{label}: treasury cannot be negative"
        );
    }

    fn assert_reserve_invariants(&self, label: &str) {
        let required_vault =
            self.liabilities.protocol_fee + self.liabilities.creator_fee + self.liabilities.royalty;
        let vault_balance = self.token.balance(&self.vault);
        assert!(
            vault_balance >= required_vault,
            "{label}: vault reserve {vault_balance} below fee obligations {required_vault}"
        );

        if let Some(creator) = &self.escrow_creator {
            let escrow_balance = self.escrow.balance(creator);
            let required_escrow = self.buyback_liability + self.dispute_liability;
            assert!(
                escrow_balance >= required_escrow,
                "{label}: escrow reserve {escrow_balance} below obligations {required_escrow}"
            );
        }
    }

    pub fn assert_contract_error<T, E>(
        &self,
        result: Result<T, Result<ContractError, E>>,
        expected: ContractError,
    ) where
        E: std::fmt::Debug,
    {
        match result {
            Err(Ok(actual)) => assert_eq!(actual, expected),
            Err(Err(error)) => panic!("unexpected contract error: {error:?}"),
            Ok(_) => panic!("expected contract error: {expected:?}"),
        }
    }

    pub fn print_resources(&self) {
        let mut samples = self.resources.clone();
        samples.sort_by(|a, b| b.cpu.cmp(&a.cpu));
        for sample in samples.iter().take(10) {
            eprintln!(
                "  step {:>2} {:38} cpu={:>10} memory={:>8}",
                sample.step, sample.label, sample.cpu, sample.memory
            );
        }
    }
}
