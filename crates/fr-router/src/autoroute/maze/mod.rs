//! The `app.freerouting.autoroute.maze` package: the maze search itself, its queue, its cost
//! model and the `AutorouteEngine` that owns the room database.
//!
//! Task 2 landed [`search_element`] — `MazeSearchElement` is the per-door-section scratch that
//! every `ExpandableObject` carries, so the expansion doors could not be built without it. Task 6
//! adds [`engine`], the room lifecycle half of [`AutorouteEngine`]: it owns the
//! `ExpansionRoomStore`, the compensated tree handle and the net number, and it is what every
//! later task in `maze/` is written against. Task 7 filled in its three drill hooks — the
//! `DrillPageArray` the constructor builds, `invalidateDrillPages` and `resetAllDoors`' last
//! line — plus the `drill_page_drills` borrow bridge. Task 8 adds the four leaf types the search
//! is written against: [`control`] ([`AutorouteControl`], the settings block every method reads),
//! [`destination_distance`] ([`DestinationDistance`], the admissible cost bound),
//! [`list_element`] ([`MazeListElement`], the queue's element and its non-total comparator) and
//! [`queue`] ([`MazeQueue`], the guarded `TreeSet` `MazeSearchEngine` installs). The search
//! Task 11 adds [`search`], the search's frame: the [`MazeSearchEngine`] struct, `getInstance`,
//! `init`, the pop loop and the four helpers those need. The cost model and the room-door
//! expansion (Task 12, `expand.rs`), the drill and ripup expanders (Task 13) and
//! `autorouteConnection` (Task 16) are still to come; the roster in
//! `scripts/audit-map/fr-router.map` records where each lands.

pub mod control;
pub mod destination_distance;
pub mod engine;
pub mod list_element;
pub mod queue;
pub mod search;
pub mod search_element;

pub use control::{AutorouteControl, ViaMask};
pub use destination_distance::DestinationDistance;
pub use engine::AutorouteEngine;
pub use list_element::MazeListElement;
pub use queue::MazeQueue;
pub use search::{ALREADY_RIPPED_COSTS, MazeResult, MazeSearchEngine, ShoveResult};
pub use search_element::{MazeAdjustment, MazeSearchElement};

/// `AutorouteEngine.TRACE_WIDTH_TOLERANCE` (`autoroute/maze/AutorouteEngine.java:41`):
/// `public static final int TRACE_WIDTH_TOLERANCE = 2`.
///
/// A bare literal, not engine state, and it is added to the caller's offset by
/// `ExpansionDoor.getSectionSegments` (`ExpansionDoor.java:106`) — which is why it has to exist
/// before the engine does. It lives on the package module rather than in `engine.rs` so that
/// Task 2 could port `getSectionSegments`.
///
/// obligation: `AutorouteEngine.TRACE_WIDTH_TOLERANCE` must have exactly one definition —
/// **discharged in Task 6**. The letter of Task 2's wording was "Task 6's `AutorouteEngine` must
/// `pub use` this constant"; a re-export would have no reader, because `engine.rs` never needs the
/// number (its only consumer is `ExpansionDoor::get_section_segments`, which imports it from
/// here). The intent — one definition — is met: `grep -rn "TRACE_WIDTH_TOLERANCE" crates/` finds
/// this line and `door.rs`'s `use`, and nothing in `engine.rs`.
pub const TRACE_WIDTH_TOLERANCE: i32 = 2;
