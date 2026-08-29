//! The `app.freerouting.autoroute` package: the maze/expansion autorouter.
//!
//! Task 1 landed the two leaf classes of the package root — [`attempt`] (the per-connection
//! outcome, which is plan-6 ruling 1(a)'s primary parity signal) and [`item_info`] (the per-item
//! scratch). Task 2 adds [`expansion`] (the rooms and doors the maze search walks) and the
//! `MazeSearchElement` half of [`maze`]. Task 3 adds [`tree_ext`], the extension trait that
//! carries `ShapeSearchTree.completeShape` and `divideLargeRoom` — the two methods `fr-board`
//! could not hold, because both take and return an expansion room. The `drill/` and `path/`
//! submodules and the rest of `maze/` arrive in Tasks 4-17; the roster at the foot of `lib.rs`
//! and `scripts/audit-map/fr-router.map` name each one and its task.

pub mod attempt;
pub mod expansion;
pub mod item_info;
pub mod maze;
pub mod tree_ext;

pub use attempt::{AutorouteAttemptResult, AutorouteAttemptState};
pub use expansion::{
    CompleteFreeSpaceExpansionRoom, ExpandableRef, ExpansionDoor, ExpansionRoomStore,
    FreeSpaceExpansionRoom, IncompleteFreeSpaceExpansionRoom, ObstacleExpansionRoom, RoomRef,
    TargetItemExpansionDoor,
};
pub use maze::{MazeAdjustment, MazeSearchElement};
pub use tree_ext::AutorouteSearchTreeExt;
