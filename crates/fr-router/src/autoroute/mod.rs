//! The `app.freerouting.autoroute` package: the maze/expansion autorouter.
//!
//! Task 1 lands the two leaf classes of the package root — [`attempt`] (the per-connection
//! outcome, which is plan-6 ruling 1(a)'s primary parity signal) and [`item_info`] (the per-item
//! scratch). The `maze/`, `expansion/`, `drill/` and `path/` submodules arrive in Tasks 2-17; the
//! roster at the foot of `lib.rs` names each one and its task.

pub mod attempt;
pub mod item_info;

pub use attempt::{AutorouteAttemptResult, AutorouteAttemptState};
