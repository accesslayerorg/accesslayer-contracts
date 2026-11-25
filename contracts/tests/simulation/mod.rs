//! Economic attack simulation harness.
//!
//! Scripted adversaries (sandwich, wash trading, reward farming, governance
//! buy-and-vote, drain-the-reserve) run randomized multi-agent sequences against
//! the protocol. After every sequence we check reserve solvency and that no
//! adversary profits beyond the documented tolerance without taking risk.

pub mod agents;
pub mod harness;
pub mod report;

pub use agents::{Agent, AgentKind, Strategy};
pub use harness::{Harness, HarnessConfig, SequenceOutcome};
pub use report::{Report, SimulationReport};

/// Documented tolerance: an adversary may net at most this many basis points of
/// its starting capital, and only if it accepted risk (a non-zero exposure).
pub const PROFIT_TOLERANCE_BPS: i128 = 10;

/// Default seed used when none is supplied via `SIM_SEED`.
pub const DEFAULT_SEED: u64 = 0xC0FF_EE00_D15E_A5E5;
