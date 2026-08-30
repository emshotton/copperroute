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
//! `check`, and [`DrillItemMover`] carries `check` and `tryShoveViaPoints` — the mutating
//! `insert` / `springOverObstacles` / `shoveVias` are `// added in Plan 7:` markers below.
//! `trace_shover_check_does_not_mutate_the_board` in `tests/board_ext.rs` pins the property that
//! makes the split safe.

mod drill_item_mover;
mod routing_board_ext;
mod trace_shover;

pub use drill_item_mover::DrillItemMover;
pub use routing_board_ext::RoutingBoardExt;
pub use trace_shover::{SpringOverOutcome, TraceShover};

// The deferral rosters for the two classes live in `trace_shover.rs` and `drill_item_mover.rs`
// rather than here: `scripts/audit-map/fr-router.map` maps each class to its own file, and
// `audit-port.sh` searches only the mapped path once a map is supplied.
