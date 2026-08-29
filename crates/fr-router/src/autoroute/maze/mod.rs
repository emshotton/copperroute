//! The `app.freerouting.autoroute.maze` package: the maze search itself, its queue, its cost
//! model and the `AutorouteEngine` that owns the room database.
//!
//! Task 2 landed [`search_element`] — `MazeSearchElement` is the per-door-section scratch that
//! every `ExpandableObject` carries, so the expansion doors could not be built without it. Task 6
//! adds [`engine`], the room lifecycle half of [`AutorouteEngine`]: it owns the
//! `ExpansionRoomStore`, the compensated tree handle and the net number, and it is what every
//! later task in `maze/` is written against. The control block, the queue and the search itself
//! arrive in Tasks 8-13 and 16; the roster in `scripts/audit-map/fr-router.map` records where
//! each lands.

pub mod engine;
pub mod search_element;

pub use engine::AutorouteEngine;
pub use search_element::{MazeAdjustment, MazeSearchElement};

/// `AutorouteEngine.TRACE_WIDTH_TOLERANCE` (`autoroute/maze/AutorouteEngine.java:41`):
/// `public static final int TRACE_WIDTH_TOLERANCE = 2`.
///
/// A bare literal, not engine state, and it is added to the caller's offset by
/// `ExpansionDoor.getSectionSegments` (`ExpansionDoor.java:106`) — which is why it has to exist
/// before the engine does. It lives on the package module rather than in `engine.rs` so that
/// Task 2 could port `getSectionSegments`.
///
/// obligation: Task 6's `AutorouteEngine` must `pub use` this constant rather than redeclare it,
/// so there is one definition of the number.
pub const TRACE_WIDTH_TOLERANCE: i32 = 2;
