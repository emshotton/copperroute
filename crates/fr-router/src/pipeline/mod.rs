//! `fr_router::pipeline` — the batch pipeline above the maze search: the fanout pre-pass, the
//! pass loop, the pass runner, the batch autorouter and the optimizer (Plan 7).
//!
//! Plan 6 stopped at `AutorouteConnectionRouter.route`'s step 5, i.e. at one connection
//! ([`crate::route_connection`]). Everything between that and a `.ses` file lives here.
//!
//! # State
//!
//! **Task 2 of 18.** The module holds [`BoardHistory`] — `autoroute/BoardHistory.java`, the pass
//! loop's best-board memory (controller ruling AF). Task 4 adds `RouterStop`, `RouterCounters`,
//! `TaskState`, `ProgressSink` and `RoutingEvent` beside it; Tasks 8-15 add the pipeline classes
//! proper. `crates/fr-router/src/lib.rs`'s roster names every class still deferred and the task
//! that owns it.

pub mod board_history;

pub use board_history::{BoardHistory, BoardHistoryEntry, java_float_compare};
