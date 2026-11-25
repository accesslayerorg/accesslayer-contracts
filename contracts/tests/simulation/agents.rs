//! Adversarial strategies that drive the protocol.

use super::harness::{Action, ProtocolState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    Sandwich,
    WashTrade,
    RewardFarm,
    GovernanceBuyAndVote,
    DrainReserve,
    Honest,
}

impl AgentKind {
    pub const ADVERSARIAL: [AgentKind; 5] = [
        AgentKind::Sandwich,
        AgentKind::WashTrade,
        AgentKind::RewardFarm,
        AgentKind::GovernanceBuyAndVote,
        AgentKind::DrainReserve,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            AgentKind::Sandwich => "sandwich",
            AgentKind::WashTrade => "wash_trade",
            AgentKind::RewardFarm => "reward_farm",
            AgentKind::GovernanceBuyAndVote => "governance_buy_and_vote",
            AgentKind::DrainReserve => "drain_reserve",
            AgentKind::Honest => "honest",
        }
    }
}

/// A strategy decides the next action for an agent given the protocol state
/// and its own portfolio.
pub trait Strategy {
    fn next_action(&mut self, state: &ProtocolState, agent: &Agent, rng: &mut Rng) -> Option<Action>;
}

/// Lightweight deterministic PRNG (xorshift64*) so seeded runs are reproducible
/// without pulling in extra dependencies.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform value in `0..n` (n > 0).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }
}

#[derive(Debug, Clone)]
pub struct Agent {
    pub id: usize,
    pub kind: AgentKind,
    /// Starting capital in base units (what profit is measured against).
    pub start_balance: i128,
    pub balance: i128,
    /// Tokens/shares currently held in the protocol.
    pub holdings: i128,
    /// Governance voting power accumulated.
    pub voting_power: i128,
    /// Exposure: capital actually put at risk (non-zero means risk was taken).
    pub risk_taken: i128,
    pub steps_taken: u32,
}

impl Agent {
    pub fn new(id: usize, kind: AgentKind, start_balance: i128) -> Self {
        Agent {
            id,
            kind,
            start_balance,
            balance: start_balance,
            holdings: 0,
            voting_power: 0,
            risk_taken: 0,
            steps_taken: 0,
        }
    }

    /// Net profit (positive) or loss (negative) against starting capital,
    /// marking holdings at the current protocol price.
    pub fn pnl(&self, price: i128) -> i128 {
        self.balance + self.holdings * price / PRICE_SCALE - self.start_balance
    }

    pub fn strategy(&self) -> Box<dyn Strategy> {
        match self.kind {
            AgentKind::Sandwich => Box::new(SandwichStrategy),
            AgentKind::WashTrade => Box::new(WashTradeStrategy),
            AgentKind::RewardFarm => Box::new(RewardFarmStrategy),
            AgentKind::GovernanceBuyAndVote => Box::new(GovernanceStrategy),
            AgentKind::DrainReserve => Box::new(DrainReserveStrategy),
            AgentKind::Honest => Box::new(HonestStrategy),
        }
    }
}

/// Fixed-point scale for prices (1e6 = 1.0).
pub const PRICE_SCALE: i128 = 1_000_000;

/// Front-run a pending buy, then back-run it; relies on price impact.
pub struct SandwichStrategy;
impl Strategy for SandwichStrategy {
    fn next_action(&mut self, state: &ProtocolState, agent: &Agent, rng: &mut Rng) -> Option<Action> {
        if agent.holdings == 0 {
            let size = (agent.balance / 4).max(1) as u64;
            Some(Action::Buy { agent: agent.id, amount: (rng.below(size) + 1) as i128 })
        } else {
            let _ = state;
            Some(Action::Sell { agent: agent.id, amount: agent.holdings })
        }
    }
}

/// Trade against oneself to farm volume-based rewards.
pub struct WashTradeStrategy;
impl Strategy for WashTradeStrategy {
    fn next_action(&mut self, _state: &ProtocolState, agent: &Agent, rng: &mut Rng) -> Option<Action> {
        let amt = (rng.below(1_000) + 1) as i128;
        if agent.steps_taken % 2 == 0 {
            Some(Action::Buy { agent: agent.id, amount: amt })
        } else {
            Some(Action::Sell { agent: agent.id, amount: agent.holdings.min(amt) })
        }
    }
}

/// Repeatedly stake and claim rewards to farm emissions.
pub struct RewardFarmStrategy;
impl Strategy for RewardFarmStrategy {
    fn next_action(&mut self, _state: &ProtocolState, agent: &Agent, rng: &mut Rng) -> Option<Action> {
        if agent.steps_taken % 3 == 0 {
            Some(Action::Stake { agent: agent.id, amount: (rng.below(500) + 1) as i128 })
        } else {
            Some(Action::ClaimRewards { agent: agent.id })
        }
    }
}

/// Buy governance power then vote on a parameter change that benefits holder.
pub struct GovernanceStrategy;
impl Strategy for GovernanceStrategy {
    fn next_action(&mut self, _state: &ProtocolState, agent: &Agent, rng: &mut Rng) -> Option<Action> {
        if agent.steps_taken == 0 {
            Some(Action::Buy { agent: agent.id, amount: (rng.below(2_000) + 1) as i128 })
        } else if agent.voting_power == 0 {
            Some(Action::Vote { agent: agent.id, power: agent.holdings })
        } else {
            Some(Action::Sell { agent: agent.id, amount: agent.holdings })
        }
    }
}

/// Attempt to withdraw as much reserve as possible.
pub struct DrainReserveStrategy;
impl Strategy for DrainReserveStrategy {
    fn next_action(&mut self, state: &ProtocolState, agent: &Agent, rng: &mut Rng) -> Option<Action> {
        let want = (state.reserve / 3).max(1);
        let amt = (rng.below(want as u64) + 1) as i128;
        Some(Action::Redeem { agent: agent.id, amount: amt.min(agent.holdings.max(1)) })
    }
}

/// Benign participant providing honest liquidity.
pub struct HonestStrategy;
impl Strategy for HonestStrategy {
    fn next_action(&mut self, _state: &ProtocolState, agent: &Agent, rng: &mut Rng) -> Option<Action> {
        if rng.below(2) == 0 {
            Some(Action::Buy { agent: agent.id, amount: (rng.below(300) + 1) as i128 })
        } else {
            Some(Action::Sell { agent: agent.id, amount: agent.holdings.min(100) })
        }
    }
}
