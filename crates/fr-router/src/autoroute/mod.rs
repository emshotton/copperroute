//! The `app.freerouting.autoroute` package: the maze/expansion autorouter.
//!
//! Task 1 landed the two leaf classes of the package root — [`attempt`] (the per-connection
//! outcome, which is plan-6 ruling 1(a)'s primary parity signal) and [`item_info`] (the per-item
//! scratch). Task 2 adds [`expansion`] (the rooms and doors the maze search walks) and the
//! `MazeSearchElement` half of [`maze`]. Task 3 adds [`tree_ext`], the extension trait that
//! carries `ShapeSearchTree.completeShape` and `divideLargeRoom` — the two methods `fr-board`
//! could not hold, because both take and return an expansion room. Tasks 4 and 5 add the three
//! `Sorted*RoomNeighbours` sorters, and Task 6 adds [`maze::engine`] — the `AutorouteEngine`
//! that owns every room and door of one routing run. Task 7 adds [`drill`], the layer-change
//! half: the page grid, the per-page drill memo and the expansion drills themselves. Task 8 adds
//! the four leaf types the search is written against, and Task 11 the search's own frame
//! ([`maze::search`]) — construction, `init` and the pop loop. Task 12 adds the room-door expansion
//! and the cost model, and Task 13 the drill/layer expanders, the ripup cost model and the first
//! member of [`path`] ([`Connection`], the memoised run of routable items the ripup price divides
//! by). The rest of `path/` arrives in Tasks 15-16; the roster at the foot of `lib.rs` and
//! `scripts/audit-map/fr-router.map` name each one and its task.

pub mod attempt;
pub mod drill;
pub mod expansion;
pub mod item_info;
pub mod maze;
pub mod path;
pub mod tree_ext;

pub use attempt::{AutorouteAttemptResult, AutorouteAttemptState};
pub use drill::{DrillPage, DrillPageArray, ExpansionDrill};
pub use expansion::{
    CompleteFreeSpaceExpansionRoom, ExpandableRef, ExpansionDoor, ExpansionRoomStore,
    FreeSpaceExpansionRoom, IncompleteFreeSpaceExpansionRoom, ObstacleExpansionRoom, RoomRef,
    TargetItemExpansionDoor,
};
pub use maze::{
    AutorouteControl, AutorouteEngine, DestinationDistance, MazeAdjustment, MazeExpansionEngine,
    MazeListElement, MazeQueue, MazeResult, MazeRipupResolver, MazeSearchElement, MazeSearchEngine,
    ShoveResult, ViaMask,
};
pub use path::Connection;
pub use tree_ext::AutorouteSearchTreeExt;
