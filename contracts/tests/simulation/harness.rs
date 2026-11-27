//! Multi-agent sequence runner and invariant checks.

use super::agents::{Agent, AgentKind, Rng, PRICE_SCALE};
use super::report::SequenceRecord;
use super::PROFIT_TOLERANCE_BPS;

/// Protocol-level parameters under test. `fee_bps` is the swap fee; a
/// deliberately weakened configuration sets it to zero.
#[derive(Debug, Clone)]
pub struct HarnessConfig {
    pub seed: u64,
    pub sequences: u32,
    pub steps_per_sequence: u32,
    pub agents_per_kind: usize,
    pub initial_reserve: i128,
    pub fee_bps: i128,
    pub initial_price: i128,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        HarnessConfig {
            seed: super::DEFAULT_SEED,
            sequences: 64,
            steps_per_sequence: 40,
            agents_per_kind: 2,
            initial_reserve: 1_000_000,
            fee_bps: 30,
            initial_price: PRICE_SCALE,
        }
    }
}

/// Discrete actions an agent can take against the protocol.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Buy { agent: usize, amount: i128 },
    Sell { agent: usize, amount: i128 },
    Stake { agent: usize, amount: i128 },
    ClaimRewards { agent: usize },
    Vote { agent: usize, power: i128 },
    Redeem { agent: usize, amount: i128 },
}

/// Simplified AMM-style protocol state the agents interact with.
#[derive(Debug, Clone)]
pub struct ProtocolState {
    pub reserve: i128,
    pub supply: i128,
    pub price: i128,
    pub fee_bps: i128,
    pub staked: i128,
    pub reward_pool: i128,
    pub fees_collected: i128,
}

impl ProtocolState {
    pub fn new(cfg: &HarnessConfig) -> Self {
        ProtocolState {
            reserve: cfg.initial_reserve,
            supply: cfg.initial_reserve,
            price: cfg.initial_price,
            fee_bps: cfg.fee_bps,
            staked: 0,
            reward_pool: cfg.initial_reserve / 100,
            fees_collected: 0,
        }
    }

    /// Apply an action to protocol and agent state. Returns whether it executed.
    pub fn apply(&mut self, agents: &mut [Agent], action: &Action) -> bool {
        match *action {
            Action::Buy { agent, amount } => {
                let a = &mut agents[agent];
                let cost = amount.min(a.balance);
                if cost <= 0 {
                    return false;
                }
                let fee = cost * self.fee_bps / 10_000;
                let net = cost - fee;
                let tokens = net * PRICE_SCALE / self.price.max(1);
                a.balance -= cost;
                a.holdings += tokens;
                a.risk_taken += cost;
                self.reserve += net;
                self.fees_collected += fee;
                // Price impact: buying pushes the price up proportionally.
                self.price += net * PRICE_SCALE / (self.reserve.max(1) * 10);
                true
            }
            Action::Sell { agent, amount } => {
                let a = &mut agents[agent];
                let sell = amount.min(a.holdings);
                if sell <= 0 {
                    return false;
                }
                let gross = sell * self.price / PRICE_SCALE;
                let fee = gross * self.fee_bps / 10_000;
                let pay = (gross - fee).min(self.reserve.max(0));
                a.holdings -= sell;
                a.balance += pay;
                self.reserve -= pay;
                self.fees_collected += fee;
                self.price -= pay * PRICE_SCALE / (self.reserve.max(1) * 10);
                self.price = self.price.max(1);
                true
            }
            Action::Stake { agent, amount } => {
                let a = &mut agents[agent];
                let s = amount.min(a.holdings);
                if s <= 0 {
                    return false;
                }
                a.holdings -= s;
                a.voting_power += 0;
                self.staked += s;
                true
            }
            Action::ClaimRewards { agent } => {
                let a = &mut agents[agent];
                if self.staked <= 0 {
                    return false;
                }
                let share = self.reward_pool * a.holdings.max(0) / self.staked.max(1);
                let share = share.min(self.reward_pool);
                self.reward_pool -= share;
                a.balance += share;
                true
            }
            Action::Vote { agent, power } => {
                let a = &mut agents[agent];
                a.voting_power = power.max(0);
                true
            }
            Action::Redeem { agent, amount } => {
                let a = &mut agents[agent];
                let r = amount.min(a.holdings).min(self.reserve.max(0));
                if r <= 0 {
                    return false;
                }
                a.holdings -= r;
                let pay = r * self.price / PRICE_SCALE;
                let pay = pay.min(self.reserve);
                a.balance += pay;
                self.reserve -= pay;
                true
            }
        }
    }
}

/// Result of a single randomized multi-agent sequence.
#[derive(Debug, Clone)]
pub struct SequenceOutcome {
    pub index: u32,
    pub seed: u64,
    pub actions: Vec<(usize, Action)>,
    pub agent_pnl: Vec<(AgentKind, i128, i128)>, // kind, pnl, risk_taken
    pub reserve_before: i128,
    pub reserve_after: i128,
    pub solvent: bool,
    pub violations: Vec<String>,
}

pub struct Harness {
    pub cfg: HarnessConfig,
}

impl Harness {
    pub fn new(cfg: HarnessConfig) -> Self {
        Harness { cfg }
    }

    fn build_agents(&self) -> Vec<Agent> {
        let mut agents = Vec::new();
        let mut id = 0usize;
        for kind in AgentKind::ADVERSARIAL.iter().chain(std::iter::once(&AgentKind::Honest)) {
            for _ in 0..self.cfg.agents_per_kind {
                agents.push(Agent::new(id, *kind, 10_000));
                id += 1;
            }
        }
        agents
    }

    /// Run one randomized sequence. The RNG is derived from the run seed and
    /// the sequence index so each sequence is independently reproducible.
    pub fn run_sequence(&self, index: u32) -> SequenceOutcome {
        let seed = self.cfg.seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut rng = Rng::new(seed);
        let mut agents = self.build_agents();
        let mut state = ProtocolState::new(&self.cfg);
        let reserve_before = state.reserve;
        let mut actions = Vec::new();

        for _ in 0..self.cfg.steps_per_sequence {
            let idx = rng.below(agents.len() as u64) as usize;
            let strategy_action = {
                let agent = &agents[idx];
                let mut strat = agent.strategy();
                strat.next_action(&state, agent, &mut rng)
            };
            if let Some(action) = strategy_action {
                if state.apply(&mut agents, &action) {
                    agents[idx].steps_taken += 1;
                    actions.push((idx, action));
                }
            }
        }

        let price = state.price;
        let agent_pnl = agents
            .iter()
            .map(|a| (a.kind, a.pnl(price), a.risk_taken))
            .collect::<Vec<_>>();

        let solvent = state.reserve >= 0 && state.reserve <= reserve_before + state.fees_collected + state.reserve_growth_bound();
        let mut violations = Vec::new();
        if !solvent {
            violations.push(format!("reserve insolvent: {}", state.reserve));
        }
        for a in &agents {
            if a.kind == AgentKind::Honest {
                continue;
            }
            let pnl = a.pnl(price);
            let tol = a.start_balance * PROFIT_TOLERANCE_BPS / 10_000;
            // Profit beyond tolerance is only a violation when it was obtained
            // without taking risk (risk_taken == 0 means pure extraction).
            if pnl > tol && a.risk_taken == 0 {
                violations.push(format!("{} agent {} extracted {} riskless", a.kind.name(), a.id, pnl));
            }
            if pnl > tol && a.risk_taken > 0 && pnl > self.cfg.initial_reserve / 10 {
                violations.push(format!("{} agent {} excessive profit {}", a.kind.name(), a.id, pnl));
            }
        }

        SequenceOutcome {
            index,
            seed,
            actions,
            agent_pnl,
            reserve_before,
            reserve_after: state.reserve,
            solvent,
            violations,
        }
    }

    pub fn run_all(&self) -> Vec<SequenceOutcome> {
        (0..self.cfg.sequences).map(|i| self.run_sequence(i)).collect()
    }

    pub fn record(outcome: &SequenceOutcome) -> SequenceRecord {
        SequenceRecord::from(outcome)
    }
}

impl ProtocolState {
    /// Upper bound on legitimate reserve growth from reward pool accrual.
    fn reserve_growth_bound(&self) -> i128 {
        0
    }
}
