//! Findings summary and reproduction artifacts.

use std::fmt::Write as _;

use super::agents::AgentKind;
use super::harness::SequenceOutcome;

#[derive(Debug, Clone)]
pub struct SequenceRecord {
    pub index: u32,
    pub seed: u64,
    pub max_profit: i128,
    pub max_profit_kind: AgentKind,
    pub steps: Vec<String>,
    pub violations: Vec<String>,
    pub solvent: bool,
}

impl From<&SequenceOutcome> for SequenceRecord {
    fn from(o: &SequenceOutcome) -> Self {
        let (max_profit, max_profit_kind) = o
            .agent_pnl
            .iter()
            .filter(|(k, _, _)| *k != AgentKind::Honest)
            .map(|(k, p, _)| (*p, *k))
            .max_by_key(|(p, _)| *p)
            .unwrap_or((0, AgentKind::Honest));
        let steps = o
            .actions
            .iter()
            .map(|(agent, action)| format!("agent {agent}: {action:?}"))
            .collect();
        SequenceRecord {
            index: o.index,
            seed: o.seed,
            max_profit,
            max_profit_kind,
            steps,
            violations: o.violations.clone(),
            solvent: o.solvent,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimulationReport {
    pub seed: u64,
    pub sequences: u32,
    pub violations_total: usize,
    pub insolvent_sequences: u32,
    pub most_profitable: Option<SequenceRecord>,
    pub per_strategy_best: Vec<(AgentKind, i128)>,
}

pub type Report = SimulationReport;

impl SimulationReport {
    pub fn build(seed: u64, outcomes: &[SequenceOutcome]) -> Self {
        let records: Vec<SequenceRecord> = outcomes.iter().map(SequenceRecord::from).collect();
        let violations_total = records.iter().map(|r| r.violations.len()).sum();
        let insolvent_sequences = records.iter().filter(|r| !r.solvent).count() as u32;
        let most_profitable = records.iter().max_by_key(|r| r.max_profit).cloned();

        let mut per_strategy_best: Vec<(AgentKind, i128)> = AgentKind::ADVERSARIAL
            .iter()
            .map(|k| {
                let best = outcomes
                    .iter()
                    .flat_map(|o| o.agent_pnl.iter())
                    .filter(|(kind, _, _)| kind == k)
                    .map(|(_, p, _)| *p)
                    .max()
                    .unwrap_or(0);
                (*k, best)
            })
            .collect();
        per_strategy_best.sort_by_key(|(k, _)| k.name());

        SimulationReport {
            seed,
            sequences: outcomes.len() as u32,
            violations_total,
            insolvent_sequences,
            most_profitable,
            per_strategy_best,
        }
    }

    pub fn passed(&self) -> bool {
        self.violations_total == 0 && self.insolvent_sequences == 0
    }

    /// Render a human-readable markdown summary.
    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# Economic Attack Simulation Report\n");
        let _ = writeln!(s, "- Seed: `{}`", self.seed);
        let _ = writeln!(s, "- Sequences: {}", self.sequences);
        let _ = writeln!(s, "- Violations: {}", self.violations_total);
        let _ = writeln!(s, "- Insolvent sequences: {}", self.insolvent_sequences);
        let _ = writeln!(s, "- Result: {}\n", if self.passed() { "PASS" } else { "FAIL" });

        let _ = writeln!(s, "## Best profit per strategy\n");
        for (kind, best) in &self.per_strategy_best {
            let _ = writeln!(s, "- {}: {}", kind.name(), best);
        }

        if let Some(m) = &self.most_profitable {
            let _ = writeln!(s, "\n## Most profitable sequence\n");
            let _ = writeln!(s, "- Sequence: {} (seed `{}`)", m.index, m.seed);
            let _ = writeln!(s, "- Profit: {} by `{}`", m.max_profit, m.max_profit_kind.name());
            let _ = writeln!(s, "- Reproduce: `SIM_SEED={} SIM_SEQUENCE={}`\n", self.seed, m.index);
            for step in &m.steps {
                let _ = writeln!(s, "  1. {step}");
            }
        }

        if !self.passed() {
            let _ = writeln!(s, "\n## Violations\n");
            let _ = writeln!(s, "Violations detected; see per-sequence logs.");
        }
        s
    }

    pub fn write_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.render())
    }
}
