//! The `app.freerouting.autoroute.maze` package: the maze search itself, its queue, its cost
//! model and the `AutorouteEngine` that owns the room database.
//!
//! Task 2 lands only [`search_element`] — `MazeSearchElement` is the per-door-section scratch
//! that every `ExpandableObject` carries, so the expansion doors cannot be built without it.
//! The engine, the control block, the queue and the search arrive in Tasks 6 and 8-13; the
//! roster in `scripts/audit-map/fr-router.map` records where each lands.

pub mod search_element;

pub use search_element::{MazeAdjustment, MazeSearchElement};
