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
}
