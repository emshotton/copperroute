//! Port of `app.freerouting.autoroute.path` — the connection the router found, and the pieces
//! that locate and insert it.
//!
//! Task 13 landed the first of them, [`Connection`]: the memoised "run of routable items between
//! two forks or terminals" that `MazeRipupResolver`'s cost model divides by. Task 14 adds
//! [`FoundConnectionLocator`] and its two angle-restricted overrides, which walk a found
//! `MazeSearchEngine::Result` backwards into the [`ResultItem`] list to insert.
//! `FoundConnectionInserter` is Task 15's; `scripts/audit-map/fr-router.map` points it at
//! `inserter.rs`.
//!
//! # `connectionItems` holds traces, not vias
//!
//! The brief models one entry as a `ConnectionItem::Trace | ::Via` enum. Java has no such thing:
//! `ResultItem` (FoundConnectionLocator.java:542-551) is a corner list plus a layer, and
//! `FoundConnectionInserter` (`:47-75`) derives every via from the **layer change between two
//! consecutive `ResultItem`s** — `insertVia(currentNewItem.corners[0], currentLayer,
//! currentNewItem.layer)` — plus one final via back to `startLayer` (`:74`). So a locator that
//! crosses a drill emits three traces on layers 0, 1, 0 and *no* via entry; the two vias appear
//! only when the inserter runs. `P6T14Probe`'s mode `ripup` is exactly that case.

mod connection;
pub mod locator;
// The two overrides export nothing public — `calculate_next_trace_corners` is `pub(crate)` and
// every helper is private — so they are not part of the crate's surface. They are separate
// modules because `scripts/audit-map/fr-router.map` points
// `FoundConnectionLocator45Degree`/`FoundConnectionLocatorAnyAngle` at these two files.
pub(crate) mod locator_45;
pub(crate) mod locator_any_angle;

pub use connection::Connection;
pub use locator::{
    BacktrackElement, FoundConnectionLocator, LocatorKind, ResultItem, calculate_additional_corner,
};
