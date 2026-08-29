//! The `app.freerouting.autoroute.drill` package: the layer-change objects of the maze search
//! and the tiled index that finds them.
//!
//! # What a drill is
//!
//! An [`ExpansionDrill`] is a place where the maze search may change layer — a via that does not
//! exist yet. `DrillPage.getDrills` (DrillPage.java:63-131) manufactures them by taking the
//! page's rectangle, **cutting every obstacle out of it** and splitting the remainder into convex
//! pieces: each piece is somewhere a via would fit, and its centre of gravity is where the drill
//! is placed. The drill then binds one expansion room per layer
//! ([`ExpansionDrill::calculate_expansion_rooms`], ExpansionDrill.java:55-92), creating rooms
//! where none exists, and is discarded if any layer has no single room.
//!
//! [`DrillPageArray`] is the index: the board's bounding box tiled into pages of at most
//! `max(5 * defaultViaDiameter, 10000)` (AutorouteEngine.java:89-90), so the search can ask for
//! the drills near a room instead of the drills on the board.
//!
//! # Memoisation, and the two hazards it carries
//!
//! A page keeps its drill list until something invalidates it, and the guard is
//! `this.drills == null || autorouteEngine.getNetNumber() != this.netNumber` (`:64`). Two
//! consequences the port reproduces rather than fixes:
//!
//! 1. **Recomputing mutates the page's `getId()`.** `:65` writes `this.netNumber` and `:190-193`
//!    hashes it, so a page already stored in the maze's `TreeSet<MazeListElement>` (plan-6 ruling
//!    4, hazard B) starts sorting by a key that no longer matches where it sits. See
//!    [`DrillPage::get_drills`].
//! 2. **A cancelled split leaves the page memoised as empty.** `:66` installs the fresh list
//!    *before* the work, and the work can throw — see `docs/java-quirks.md` #168.
//!
//! # The port's shape
//!
//! Java's `DrillPage` owns its `ExpansionDrill`s and `AutorouteEngine` owns the
//! `DrillPageArray`; the port keeps the array on the engine but puts the drills in
//! [`ExpansionRoomStore::drills`](crate::autoroute::expansion::ExpansionRoomStore::drills),
//! because a drill is an `ExpandableObject` and the maze search stores one as a bare
//! [`DrillId`](crate::arena::DrillId) in a `MazeSearchElement`. A page therefore holds
//! `Vec<DrillId>`, exactly as it holds `Collection<ExpansionDrill>` in Java.
//!
//! `DrillPage.getDrills(AutorouteEngine, boolean)` takes the engine that owns the page's array,
//! which no `&mut` can express; [`AutorouteEngine::drill_page_drills`] is the borrow bridge, and
//! it is documented there.
//!
//! Because the arena has no collector, a page **frees its own drill ids** when it is invalidated
//! or recomputed — the two places Java drops the list and lets the collector take it
//! ([`DrillPage::invalidate`], `DrillPage.java:171`, and `:66`'s replacement). That method
//! carries the reachability argument for why no live holder is left dangling; the store's `clear`
//! deliberately does **not** free them, because `AutorouteEngine.clear` does not touch
//! `drillPageArray` either.
//!
//! [`AutorouteEngine::drill_page_drills`]:
//!     crate::autoroute::maze::AutorouteEngine::drill_page_drills

pub mod expansion_drill;
pub mod page;
pub mod page_array;

pub use expansion_drill::ExpansionDrill;
pub use page::{CutoutEntry, DrillPage};
pub use page_array::DrillPageArray;
