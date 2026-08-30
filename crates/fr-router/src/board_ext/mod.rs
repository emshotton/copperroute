//! The `fr-board` types the router extends: `RoutingBoard`'s autoroute-facing methods and the
//! `board/{actions,optimize}` shove algorithms the maze drives.
//!
//! Plan-2 ruling 4 deliberately left `RoutingBoard`'s five autoroute methods out of `fr-board` —
//! they need an `AutorouteEngine`, and `fr-board` cannot name one. Plan-6 ruling 3 puts them
//! here instead, as [`RoutingBoardExt`], an extension trait over `fr_board::Board`, **shared
//! with Plan 7** (which adds `optChangedArea`, the pull-tight entry points and the tighteners).
//! The same reasoning puts `board.optimize.TraceShover` and `board.actions.DrillItemMover` here:
//! both take a `RoutingBoard` and both are reached only from the router.
//!
//! # The check half and the shove half
//!
//! Tasks 9 and 10 needed the shove algorithms only to *ask* whether a shove would work, so the
//! `check` family landed first: [`TraceShover`]'s static `check` and instance `check`,
//! [`DrillItemMover`]'s `check` and `tryShoveViaPoints`, [`ForcedPadRouter::check_forced_pad`]
//! and [`ForcedViaInserter`]'s `checkLayer` / `check`. Those four form one mutual recursion —
//! `TraceShover::check` and `DrillItemMover::check` both reach
//! `ForcedPadRouter::check_forced_pad`, which reaches both of them back — so Task 9 landed three
//! of them with the fourth as an `unimplemented!()` arm and Task 10 closed the cycle.
//!
//! The halves that *perform* a shove arrived next, and which plan owns them is a **controller
//! ruling (AA)** rather than this module's choice. `ForcedViaInserter::insert`,
//! `ForcedPadRouter::forced_pad`, `TraceShover::insert` and `DrillItemMover::{insert, shove_vias}`
//! are one chain Plan 6 genuinely needs — Task 15 reaches it at
//! `FoundConnectionInserter.java:754` — so ruling AA moved all five out of Plan 7 into a new
//! **Task 10b** between Tasks 10 and 11, where they now live. `TraceShover.springOverObstacles`
//! is **not** in that chain (nothing in Plan 6 reaches it) and keeps its `// added in Plan 7:`
//! marker in `trace_shover.rs`'s roster.
//!
//! # The two `normalize` catches, and what a `StopCheck` trip does to them
//!
//! `ForcedPadRouter.forcedPad:446-450` and `TraceShover.insert:571-575` both wrap
//! `PolylineTrace.normalize` in a bare `catch (Exception e) { FRLogger.error(…) }`. Plan-6
//! ruling 7 asks for the *degraded value* Java produces, and here it is "the substitute trace
//! stays as it was inserted, and the loop carries on" — so `swallow_normalize_error` drops the
//! error, exactly as Java drops the exception.
//!
//! It makes **one** exception, and it is not a Java one: `BoardError::Stopped` is not a value
//! Java can produce (Java has no cancellation here at all), it is the port's own answer to
//! plan-6 ruling 6's `StopCheck`. Swallowing it would mean a caller who asked the router to stop
//! is told the shove succeeded, which is the opposite of what the check is for — so it
//! propagates. Every other `BoardError` is Java's exception and is dropped.
//!
//! # Where the `StopCheck` reaches, and where it does not
//!
//! Plan-6 ruling 6 fixes cancellation at six `isStopRequested` sites in the maze **and** gives
//! `Board::{insert_via, insert_escape_via, split_traces}` a `StopCheck`, closing plan-3 ruling F.
//! The chain here threads that check into every `fr-board` walk below it that Java cannot leave
//! on a ladder board (quirk #76): `Board::split_trace_checked` under `normalize` and under
//! `split_traces`, and `Board::connection_items_checked` under the tail cleanup both `forcedPad`
//! and `TraceShover::insert` run. It adds **no** decision of its own — no method here consults
//! the check directly, so no run stops at a point Java's control flow does not reach.

mod drill_item_mover;
mod forced_pad_router;
mod forced_via_inserter;
mod routing_board_ext;
pub mod tightener;
mod trace_shover;

pub use drill_item_mover::DrillItemMover;
pub use forced_pad_router::{CheckDrillResult, ForcedPadRouter};
pub use forced_via_inserter::ForcedViaInserter;
pub use routing_board_ext::RoutingBoardExt;
pub use tightener::{
    PolylineTraceExt, TraceTightener, TraceTightener45, TraceTightener90, TraceTightenerAnyAngle,
};
pub use trace_shover::{SpringOverOutcome, TraceShover};

/// The degraded value of the two `catch (Exception e) { FRLogger.error("Couldn't normalize
/// trace.", e); }` blocks in this module's chain — `ForcedPadRouter.forcedPad:446-450` and
/// `TraceShover.insert:571-575` — as the module doc's "the two `normalize` catches" section
/// explains: the error is dropped and the caller carries on with the trace as it was inserted.
///
/// [`BoardError::Stopped`] is the one error that propagates, because it is the port's
/// cancellation signal rather than one of Java's exceptions.
pub(crate) fn swallow_normalize_error(
    result: Result<bool, fr_board::BoardError>,
) -> Result<(), fr_board::BoardError> {
    match result {
        Err(fr_board::BoardError::Stopped) => Err(fr_board::BoardError::Stopped),
        Ok(_) | Err(_) => Ok(()),
    }
}

// The deferral rosters for the four classes live in `trace_shover.rs`, `drill_item_mover.rs`,
// `forced_pad_router.rs` and `forced_via_inserter.rs` rather than here: `scripts/audit-map/fr-router.map` maps each class to its own file, and
// `audit-port.sh` searches only the mapped path once a map is supplied.
