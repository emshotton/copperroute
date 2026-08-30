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
//! Tasks 9 and 10 needed the shove algorithms only to *ask* whether a shove would work. So
//! [`TraceShover`] carries the static `check` and the instance `check`, [`DrillItemMover`]
//! carries `check` and `tryShoveViaPoints`, [`ForcedPadRouter`] carries `checkForcedPad` and
//! [`ForcedViaInserter`] carries `checkLayer` and `check`.
//!
//! The halves that *perform* a shove are split between two later owners, and the split is a
//! **controller ruling (AA)**, not this module's choice. `ForcedViaInserter.insert`,
//! `ForcedPadRouter.forcedPad`, `TraceShover.insert` and `DrillItemMover.{insert, shoveVias}`
//! are one chain that Plan 6 genuinely needs — Task 15 reaches it at
//! `FoundConnectionInserter.java:754` — so ruling AA puts all five in a new **Task 10b** between
//! Tasks 10 and 11, and their markers read `// added in Task 10b:`. `TraceShover.springOverObstacles`
//! is **not** in that chain (nothing in Plan 6 reaches it) and keeps its `// added in Plan 7:`
//! marker. `trace_shover_check_does_not_mutate_the_board` and
//! `check_forced_pad_does_not_mutate_the_board` pin the property that makes the split safe until
//! Task 10b lands.
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
