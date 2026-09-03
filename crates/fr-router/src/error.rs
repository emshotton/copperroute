//! [`RouterError`], the crate's one error type.
//!
//! Java's autoroute package signals failure three ways: a returned `AutorouteAttemptResult` with
//! a `FAILED`/`INSERT_ERROR` state, a `null`, and a thrown exception caught at one of six
//! `catch (Exception)` sites (plan-6 ruling 7). The first two are *values*, not errors, and stay
//! values here — [`crate::AutorouteAttemptResult`] and `Option`. This type is only for the third:
//! a failure that has to travel out of a call, and which the six recovery boundaries turn back
//! into the specific degraded value Java produces. (Boundary #6,
//! `RoutingBoard.insertForcedTracePolyline:787-841`, is discharged through this very channel
//! rather than by a `catch_unwind` — see the crate README's table.)

use thiserror::Error;

/// A failure inside the router.
#[derive(Debug, Error)]
pub enum RouterError {
    /// A `fr-board` operation failed — inserting a trace or a via, normalising, splitting.
    #[error(transparent)]
    Board(#[from] fr_board::BoardError),

    /// Ported geometry panicked and a plan-6 ruling 7 boundary caught it with
    /// `std::panic::catch_unwind`. The payload is the panic message where one could be
    /// recovered, so that the degraded `AutorouteAttemptResult` can carry it.
    #[error("a ported geometry operation panicked: {0}")]
    Panicked(String),

    /// The same boundary, when the throw arrived **after** rooms had already been committed to
    /// the database — quirk #166, fixed at Plan 9 Task 8.
    ///
    /// `AutorouteEngine.completeExpansionRoom` declares its `result` inside the `try` (`:422`)
    /// and its `catch` answers `new ArrayList<>()` (`:520`), so a partially failed call is told
    /// "no rooms were completed" about rooms that exist, are in the search tree and have doors on
    /// them. `result` is hoisted out of the `try` in the port and comes back here, with the panic
    /// message beside it: the run really was degraded and a caller may want to say so, but the
    /// rooms are not lost. [`AutorouteEngine::complete_expansion_room_or_committed`] is how every
    /// caller reads it.
    ///
    /// [`AutorouteEngine::complete_expansion_room_or_committed`]:
    ///     crate::autoroute::maze::AutorouteEngine::complete_expansion_room_or_committed
    #[error("a ported geometry operation panicked after committing {n} room(s): {message}", n = rooms.len())]
    PanickedWithRooms {
        /// The panic message, as [`RouterError::Panicked`] carries it.
        message: String,
        /// The rooms `addCompleteRoom` had already appended to `completeExpansionRooms` and
        /// inserted into the autoroute search tree before the throw.
        rooms: Vec<fr_board::RoomId>,
    },

    /// `AutorouteBatchLoop.run`'s `anyRoutable` check (`AutorouteBatchLoop.java:44-56`) found no
    /// layer that is both active in the settings and a signal layer, and Java throws
    /// `IllegalArgumentException("Cannot start autorouter: all layers are disabled.")` at `:55`.
    ///
    /// Plan-7 ruling 7's **sole** new recovery boundary, and the only one that **propagates**:
    /// `RoutingPipeline.run` does not catch it, so it escapes to the job scheduler. The event at
    /// `:53-54` — a `TaskState.CANCELLED` — is fired before the throw and the port fires it too.
    #[error("cannot start autorouter: all layers are disabled")]
    NoRoutableLayer,

    /// The router was asked to stop: the `TimeLimit` expired or the caller's `StopCheck` fired
    /// (`AutorouteEngine.isStopRequested`, AutorouteEngine.java:294-304).
    #[error("the routing run was stopped")]
    Stopped,

    /// A `fanout.timeout` or `optimizer.timeout` the port cannot read (#224).
    ///
    // fixed: T1 (#224) — Java swallows the `DateTimeParseException` at
    // `util/TextManager.java:91-93`, answers `null`, and runs the stage with **no** timeout —
    // which is the opposite of what the operator asked for, and silent. A run that was given a
    // budget it cannot read stops here instead, with the string named.
    #[error(transparent)]
    Timespan(#[from] crate::pipeline::TimespanError),
}
