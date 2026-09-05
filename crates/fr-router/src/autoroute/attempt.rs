use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum AutorouteAttemptState {
    #[default]
    Unknown,
    Skipped,
    NoUnconnectedNets,
    ConnectedToPlane,
    AlreadyConnected,
    NoConnections,
    Routed,
    Failed,
    InsertError,
}

impl AutorouteAttemptState {
    pub const ALL: [AutorouteAttemptState; 9] = [
        AutorouteAttemptState::Unknown,
        AutorouteAttemptState::Skipped,
        AutorouteAttemptState::NoUnconnectedNets,
        AutorouteAttemptState::ConnectedToPlane,
        AutorouteAttemptState::AlreadyConnected,
        AutorouteAttemptState::NoConnections,
        AutorouteAttemptState::Routed,
        AutorouteAttemptState::Failed,
        AutorouteAttemptState::InsertError,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AutorouteAttemptState::Unknown => "UNKNOWN",
            AutorouteAttemptState::Skipped => "SKIPPED",
            AutorouteAttemptState::NoUnconnectedNets => "NO_UNCONNECTED_NETS",
            AutorouteAttemptState::ConnectedToPlane => "CONNECTED_TO_PLANE",
            AutorouteAttemptState::AlreadyConnected => "ALREADY_CONNECTED",
            AutorouteAttemptState::NoConnections => "NO_CONNECTIONS",
            AutorouteAttemptState::Routed => "ROUTED",
            AutorouteAttemptState::Failed => "FAILED",
            AutorouteAttemptState::InsertError => "INSERT_ERROR",
        }
    }
}

impl fmt::Display for AutorouteAttemptState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutorouteAttemptResult {
    pub state: AutorouteAttemptState,
    pub details: Option<String>,
}

impl AutorouteAttemptResult {
    pub fn new(state: AutorouteAttemptState) -> AutorouteAttemptResult {
        AutorouteAttemptResult {
            state,
            details: None,
        }
    }

    pub fn with_details(state: AutorouteAttemptState, details: String) -> AutorouteAttemptResult {
        AutorouteAttemptResult {
            state,
            details: Some(details),
        }
    }

    pub fn details(&self) -> &str {
        self.details.as_deref().unwrap_or("")
    }

    pub fn is_routed(&self) -> bool {
        self.state == AutorouteAttemptState::Routed
    }
}

impl fmt::Display for AutorouteAttemptResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.state, self.details())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_state_is_javas_unknown() {
        assert_eq!(
            AutorouteAttemptResult::default(),
            AutorouteAttemptResult::new(AutorouteAttemptState::Unknown)
        );
    }

    #[test]
    fn is_routed_is_the_state_test() {
        assert!(AutorouteAttemptResult::new(AutorouteAttemptState::Routed).is_routed());
        assert!(!AutorouteAttemptResult::new(AutorouteAttemptState::Failed).is_routed());
    }

    #[test]
    fn an_explicitly_empty_detail_string_prints_like_no_details() {
        let bare = AutorouteAttemptResult::new(AutorouteAttemptState::Failed);
        let empty = AutorouteAttemptResult::with_details(AutorouteAttemptState::Failed, "".into());
        assert_eq!(bare.to_string(), empty.to_string());
        assert_eq!(bare.details(), empty.details());
    }

    #[test]
    fn the_variant_order_is_javas_ordinal_order() {
        assert!(AutorouteAttemptState::Unknown < AutorouteAttemptState::Skipped);
        assert!(AutorouteAttemptState::Routed < AutorouteAttemptState::Failed);
        assert!(AutorouteAttemptState::Failed < AutorouteAttemptState::InsertError);
    }
}
