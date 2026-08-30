//! Port of `app.freerouting.autoroute.path` — the connection the router found, and the pieces
//! that locate and insert it.
//!
//! Task 13 lands the first of them, [`Connection`]: the memoised "run of routable items between
//! two forks or terminals" that `MazeRipupResolver`'s cost model divides by. The rest of the
//! package (`FoundConnectionLocator`, its two angle-restricted subclasses and
//! `FoundConnectionInserter`) is Tasks 15-16's; `scripts/audit-map/fr-router.map` already points
//! each of them at a file here.

mod connection;

pub use connection::Connection;
