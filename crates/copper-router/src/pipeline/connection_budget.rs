use copper_settings::RouterSettings;

use crate::pipeline::stop::DeterministicWorkBudget;

/// Maze steps one connection's search may spend unless the settings say otherwise; `0` in the
/// settings lifts the cap.
pub const DEFAULT_CONNECTION_SEARCH_STEPS: u64 = 250_000;

/// What one connection's search may spend on top of the engine's own time limit.
#[derive(Debug)]
pub struct ConnectionBudget {
    steps: Option<DeterministicWorkBudget>,
}

impl ConnectionBudget {
    #[must_use]
    pub fn start(settings: &RouterSettings) -> Self {
        let steps = settings
            .connection_search_steps
            .map_or(Some(DEFAULT_CONNECTION_SEARCH_STEPS), |steps| {
                u64::try_from(steps).ok()
            })
            .filter(|steps| *steps > 0)
            .map(DeterministicWorkBudget::new);
        ConnectionBudget { steps }
    }

    /// Search steps polled so far.
    #[must_use]
    pub fn spent(&self) -> u64 {
        self.steps
            .as_ref()
            .map_or(0, DeterministicWorkBudget::spent)
    }

    #[must_use]
    pub fn step_cap(&self) -> Option<u64> {
        self.steps.as_ref().map(DeterministicWorkBudget::limit)
    }

    /// Polled once per search step; `true` once the cap is spent.
    pub fn exceeded(&self) -> bool {
        self.steps
            .as_ref()
            .is_some_and(DeterministicWorkBudget::poll)
    }
}
