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
//! # This module is the check-only half
//!
//! Plan 6 needs the shove algorithms only to *ask* whether a shove would work; Plan 7 owns the
//! halves that perform it. So [`TraceShover`] carries the static `check` and the instance
//! `check`, [`DrillItemMover`] carries `check` and `tryShoveViaPoints`, [`ForcedPadRouter`]
//! carries `checkForcedPad` and [`ForcedViaInserter`] carries `checkLayer` and `check` — the
//! mutating `TraceShover.insert` / `springOverObstacles`, `DrillItemMover.insert` / `shoveVias`,
//! `ForcedPadRouter.forcedPad` and `ForcedViaInserter.insert` are `// added in Plan 7:` markers
//! in the four per-class files. `trace_shover_check_does_not_mutate_the_board` and
//! `check_forced_pad_does_not_mutate_the_board` pin the property that makes the split safe.
//!
//! The four form one mutual recursion — `TraceShover::check` and `DrillItemMover::check` both
//! reach `ForcedPadRouter::check_forced_pad`, which reaches both of them back — so Task 9 landed
//! three of them with the fourth as an `unimplemented!()` arm and Task 10 closed the cycle.

mod drill_item_mover;
mod forced_pad_router;
mod forced_via_inserter;
mod routing_board_ext;
mod trace_shover;

pub use drill_item_mover::DrillItemMover;
pub use forced_pad_router::{CheckDrillResult, ForcedPadRouter};
pub use forced_via_inserter::ForcedViaInserter;
pub use routing_board_ext::RoutingBoardExt;
pub use trace_shover::{SpringOverOutcome, TraceShover};

// The deferral rosters for the four classes live in `trace_shover.rs`, `drill_item_mover.rs`,
// `forced_pad_router.rs` and `forced_via_inserter.rs` rather than here: `scripts/audit-map/fr-router.map` maps each class to its own file, and
// `audit-port.sh` searches only the mapped path once a map is supplied.
