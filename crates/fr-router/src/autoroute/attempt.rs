//! Port of `autoroute.AutorouteAttemptState` (AutorouteAttemptState.java:1-14) and
//! `autoroute.AutorouteAttemptResult` (AutorouteAttemptResult.java:1-25) — the outcome of
//! routing one connection, which is also the primary parity signal (plan-6 ruling 1(a)).

use std::fmt;

/// Port of `autoroute.AutorouteAttemptState` (AutorouteAttemptState.java:4-13): "the possible
/// results of auto-routing a connection".
///
/// The variants are Java's constants, verbatim and **in declaration order** — the order is part
/// of the port, because Java's `enum` ordinal is observable through `values()`/`ordinal()` and
/// because the per-connection reference dumps compare names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum AutorouteAttemptState {
    /// `UNKNOWN` — unknown result (AutorouteAttemptState.java:5).
    ///
    /// `Default` maps here because it is the constant Java's callers use for "no result yet".
    /// It is *not* a claim about Java's field defaults: an unassigned `AutorouteAttemptState`
    /// field is `null` there, and this port has no null — every construction site assigns.
    #[default]
    Unknown,
    /// `SKIPPED` — item was skipped (:6).
    Skipped,
    /// `NO_UNCONNECTED_NETS` — item has no unconnected nets (:7).
    NoUnconnectedNets,
    /// `CONNECTED_TO_PLANE` — item is connected to a conduction plane (:8).
    ConnectedToPlane,
    /// `ALREADY_CONNECTED` — item is already connected (:9).
    AlreadyConnected,
    /// `NO_CONNECTIONS` — the item has no connections to nets (:10).
    NoConnections,
    /// `ROUTED` — item was successfully routed (:11).
    Routed,
    /// `FAILED` — routing failed (:12).
    Failed,
    /// `INSERT_ERROR` — error inserting item (:13).
    ///
    /// **No producer exists anywhere at HEAD.** `grep -rn INSERT_ERROR` over the Java tree gives
    /// this declaration, two *consumers* — `AutorouteConnectionRouter.java:124` (the necked-retry
    /// guard, Plan 7's) and `BatchFanout.java:341` — and one **stale javadoc**,
    /// `AutorouteEngine.java:125-128`, which claims `autorouteConnection` returns
    /// "ALREADY_CONNECTED, ROUTED, NOT_ROUTED, or INSERT_ERROR"; the method can return none of
    /// those three (and `NOT_ROUTED` is not even a constant of this enum). The arm plan-6's
    /// Task 15 note pointed at, `RoutingBoard.java:918-999`, does not produce it either.
    /// So the port constructs it nowhere, and Task 17's corpus cannot reach it.
    // not ported: `AutorouteAttemptState.INSERT_ERROR`'s producer — there is none at HEAD; the
    // constant itself is ported because `values()`/`ordinal()` are observable (see `ALL`).
    InsertError,
}

impl AutorouteAttemptState {
    /// Every variant in Java's declaration order, i.e. `AutorouteAttemptState.values()`.
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

    /// The Java constant's name, i.e. `Enum.toString()`.
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

/// Port of `autoroute.AutorouteAttemptResult` (AutorouteAttemptResult.java:4-24): "the outcome of
/// an autoroute attempt, including its state and detail message".
///
/// Java's fields are both public and mutable; so are these.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutorouteAttemptResult {
    /// Java `public AutorouteAttemptState state` (AutorouteAttemptResult.java:6).
    pub state: AutorouteAttemptState,
    /// Java `public String details` (:7).
    ///
    /// `None` is Java's empty message, which is what the one-argument constructor stores
    /// (`this.details = ""`, :12) — the two forms are indistinguishable through
    /// [`details`](AutorouteAttemptResult::details) and through `toString`. Java's *other*
    /// falsy value, a literal `null`, is unreachable: both constructors assign, and no code
    /// writes the field afterwards.
    pub details: Option<String>,
}

impl AutorouteAttemptResult {
    /// Port of `AutorouteAttemptResult(AutorouteAttemptState)` (AutorouteAttemptResult.java:
    /// 10-13): the state with empty details.
    pub fn new(state: AutorouteAttemptState) -> AutorouteAttemptResult {
        AutorouteAttemptResult {
            state,
            details: None,
        }
    }

    /// Port of `AutorouteAttemptResult(AutorouteAttemptState, String)`
    /// (AutorouteAttemptResult.java:16-19).
    ///
    /// The messages themselves are built by `AutorouteEngine.describeConnection`
    /// (AutorouteEngine.java:282-287); Task 16 owns them.
    pub fn with_details(state: AutorouteAttemptState, details: String) -> AutorouteAttemptResult {
        AutorouteAttemptResult {
            state,
            details: Some(details),
        }
    }

    /// The detail message, with Java's empty string for "no details".
    pub fn details(&self) -> &str {
        self.details.as_deref().unwrap_or("")
    }

    /// Whether the attempt routed the connection.
    // pub seam: none in Java — `AutorouteAttemptResult` has no `isRouted()`; its `state` is a
    // public field every Java caller compares directly. Plan 6 predicted Plan 7's pass loop would
    // become the caller, and **Plan 7 Task 17 measured that it did not**: `AutoroutePassRunner`,
    // `AutorouteBatchLoop`, `BatchFanout` and `RoutingBoardExt::fanout` all `match` on
    // `AutorouteAttemptState` directly (13 sites, `grep -rn "AutorouteAttemptState::Routed"
    // crates/fr-router/src`), because that is what Java's `result.state == ROUTED` chains are and
    // a helper would be the port inventing a shape Java does not have. The seam therefore
    // **stays open on purpose**, with its own-file unit test as its only caller; Plan 8's manifest
    // layer is free to use it. Plan 7 scan ruling 4's "closed by Tasks 9/10" is corrected here.
    pub fn is_routed(&self) -> bool {
        self.state == AutorouteAttemptState::Routed
    }
}

// renamed: `AutorouteAttemptResult.toString` (AutorouteAttemptResult.java:21-24) is this
// `Display` impl — `state.toString().toUpperCase() + ": " + details`, where the `toUpperCase` is
// a no-op because Java enum constant names are already upper case.
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
        // Java cannot tell `new AutorouteAttemptResult(s)` from `new AutorouteAttemptResult(s,
        // "")`; neither can `to_string`, even though the two are `!=` here.
        let bare = AutorouteAttemptResult::new(AutorouteAttemptState::Failed);
        let empty = AutorouteAttemptResult::with_details(AutorouteAttemptState::Failed, "".into());
        assert_eq!(bare.to_string(), empty.to_string());
        assert_eq!(bare.details(), empty.details());
    }

    #[test]
    fn the_variant_order_is_javas_ordinal_order() {
        // `Ord` is derived, so it follows declaration order — i.e. Java's `ordinal()`.
        assert!(AutorouteAttemptState::Unknown < AutorouteAttemptState::Skipped);
        assert!(AutorouteAttemptState::Routed < AutorouteAttemptState::Failed);
        assert!(AutorouteAttemptState::Failed < AutorouteAttemptState::InsertError);
    }
}
