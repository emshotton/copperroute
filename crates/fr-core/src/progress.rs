//! Spec §10's `ProgressSink`, made shareable across the one thread boundary the port has.
//!
//! Plan 7's [`fr_router::pipeline::ProgressSink`] is a `&mut dyn` trait threaded through the
//! pipeline by a single owner (controller ruling AK replaced `NamedAlgorithm`'s three listener
//! lists with it, `autoroute/pipeline/NamedAlgorithm.java:26-31`). That is the right shape for a
//! single-threaded router and the wrong one for the MCP, whose tool thread must write progress
//! notifications through a `Mutex`-guarded stdout writer owned by the main thread (plan ruling 3).
//!
//! [`SyncProgressSink`] is the bridge: it owns an `Arc<Mutex<dyn FnMut(&RoutingEvent) + Send>>`
//! and hands out the `&mut dyn ProgressSink` view the pipeline takes.
//!
//! # Ruling 11 still holds: no port decision reads it
//!
//! The sink is an observer and never an input. Java's headless path runs with all three listener
//! lists empty, so a board routed with a recording sink must be byte-for-byte the board routed
//! with [`fr_router::pipeline::NoopProgressSink`]. Plan 7 pinned that for the router
//! (`crates/fr-router/tests/stop_and_progress.rs`); `crates/fr-core/tests/pipeline.rs` re-runs it
//! through [`crate::RoutingPipeline::run`], i.e. against the **whole** pipeline including the DRC
//! and unrouted-report passes this crate adds.

// ── `core/ProgressThrottler` — the audit rows for this crate ────────────────────────────────────
//
// Ported by **Plan 7 Task 4** as `fr_router::pipeline::ProgressThrottler`
// (`crates/fr-router/src/pipeline/stop.rs`), where its own doc records why the plan's "not ported"
// call was wrong: `shouldUpdate()` has five live call sites inside Plan 7's own classes. This
// crate re-exports it (plan-8 ruling 1), and `scripts/audit-map/fr-core.map` points the class at
// this file because `core/` is audited against `crates/fr-core/src`.
//
// renamed: ProgressThrottler.shouldUpdate -> `fr_router::pipeline::ProgressThrottler::should_update` (`ProgressThrottler.java:15-26`).
// renamed: ProgressThrottler.reset -> `fr_router::pipeline::ProgressThrottler::reset` (`:29-31`).

use std::sync::{Arc, Mutex};

use fr_router::pipeline::{ProgressSink, RoutingEvent};

/// A [`ProgressSink`] whose writes go through a `Mutex`, so it can be cloned into a routing
/// thread while another thread owns the writer.
///
/// Cloning shares the closure — two clones of one sink are one sink.
#[derive(Clone)]
pub struct SyncProgressSink {
    /// `None` is [`SyncProgressSink::noop`], which allocates nothing and locks nothing: the
    /// distinction matters because the CLI's every run uses it and a `Mutex` round trip per
    /// [`RoutingEvent`] on a board with thousands of them is pure cost.
    #[allow(clippy::type_complexity)]
    handler: Option<Arc<Mutex<dyn FnMut(&RoutingEvent) + Send>>>,
}

impl std::fmt::Debug for SyncProgressSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncProgressSink")
            .field(
                "handler",
                &if self.handler.is_some() {
                    "Some(<closure>)"
                } else {
                    "None"
                },
            )
            .finish()
    }
}

impl Default for SyncProgressSink {
    fn default() -> Self {
        SyncProgressSink::noop()
    }
}

impl SyncProgressSink {
    /// Java with every listener list empty, which is the headless CLI's state — and the
    /// [`fr_router::pipeline::NoopProgressSink`] every parity driver passes.
    pub fn noop() -> SyncProgressSink {
        SyncProgressSink { handler: None }
    }

    /// A sink that calls `f` for every event. `f` is `Send` and lives behind a `Mutex`, so the
    /// sink may be cloned into a routing thread.
    pub fn new(f: impl FnMut(&RoutingEvent) + Send + 'static) -> SyncProgressSink {
        SyncProgressSink {
            handler: Some(Arc::new(Mutex::new(f))),
        }
    }

    /// Whether this sink drops everything — the cheap path, and what a caller checks before
    /// building an event it would only throw away.
    pub fn is_noop(&self) -> bool {
        self.handler.is_none()
    }

    /// The `&mut dyn ProgressSink` view [`fr_router::pipeline::run_pipeline`] takes.
    ///
    /// Borrowed rather than owned so the caller keeps the sink across a run and can clone it into
    /// a second thread; the returned value carries the borrow.
    pub fn as_pipeline_sink(&self) -> SyncProgressSinkView<'_> {
        SyncProgressSinkView { sink: self }
    }

    /// Deliver one event, from any thread.
    ///
    /// A **poisoned** mutex — a previous handler panicked — is recovered with `into_inner`
    /// rather than propagated: the sink is an observer (ruling 11), so a panicking progress
    /// handler must not be able to abort a routing run that has already produced correct board
    /// bytes. Plan ruling 4's `catch_unwind` boundary is the MCP tool call, not this.
    pub fn emit(&self, event: &RoutingEvent) {
        let Some(handler) = self.handler.as_ref() else {
            return;
        };
        let mut guard = handler
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (guard)(event);
    }
}

/// The `&mut dyn ProgressSink` view of a [`SyncProgressSink`], from
/// [`SyncProgressSink::as_pipeline_sink`].
#[derive(Debug)]
pub struct SyncProgressSinkView<'a> {
    sink: &'a SyncProgressSink,
}

impl ProgressSink for SyncProgressSinkView<'_> {
    fn on_event(&mut self, event: &RoutingEvent) {
        self.sink.emit(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_router::pipeline::{NamedAlgorithmType, TaskState};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn an_event() -> RoutingEvent {
        RoutingEvent::TaskStateChanged {
            algorithm: NamedAlgorithmType::Router,
            state: TaskState::Started,
        }
    }

    #[test]
    fn a_noop_sink_drops_everything_without_locking() {
        let sink = SyncProgressSink::noop();
        assert!(sink.is_noop());
        let mut view = sink.as_pipeline_sink();
        view.on_event(&an_event());
    }

    #[test]
    fn clones_share_one_closure() {
        let seen = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&seen);
        let sink = SyncProgressSink::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let clone = sink.clone();
        sink.as_pipeline_sink().on_event(&an_event());
        clone.as_pipeline_sink().on_event(&an_event());
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }
}
