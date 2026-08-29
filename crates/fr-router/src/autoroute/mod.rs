//! The `app.freerouting.autoroute` package: the maze/expansion autorouter.
//!
//! Task 1 landed the two leaf classes of the package root — [`attempt`] (the per-connection
//! outcome, which is plan-6 ruling 1(a)'s primary parity signal) and [`item_info`] (the per-item
//! scratch). Task 2 adds [`expansion`] (the rooms and doors the maze search walks) and the
//! `MazeSearchElement` half of [`maze`]. The `drill/` and `path/` submodules and the rest of
//! `maze/` arrive in Tasks 3-17; the roster at the foot of `lib.rs` and
//! `scripts/audit-map/fr-router.map` name each one and its task.

pub mod attempt;
pub mod expansion;
pub mod item_info;
pub mod maze;

pub use attempt::{AutorouteAttemptResult, AutorouteAttemptState};
pub use expansion::{
    CompleteFreeSpaceExpansionRoom, ExpandableRef, ExpansionDoor, ExpansionRoomStore,
    FreeSpaceExpansionRoom, IncompleteFreeSpaceExpansionRoom, ObstacleExpansionRoom, RoomRef,
    TargetItemExpansionDoor,
};
pub use maze::{MazeAdjustment, MazeSearchElement};
