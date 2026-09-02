//! Spec §10's `CancelToken` and the deadline pair Java's monitor thread stands for.
//!
//! # Controller ruling AP, and why the token carries **two** atomics
//!
//! Ruling AP chose option (a): an external `Arc<AtomicBool>` that the routing thread copies into
//! [`RouterStop`] at the poll sites. Plan ruling 2 then corrected the *shape*: `RouterStop` is a
//! **three**-state machine (`core/StopRequestState.java`), and quirk #200 rides on the
//! difference — `--max-items` reaches `requestStop()` (`ALL`), which through
//! `RoutingPipeline.java:117` silently disables the optimizer, while `--max-passes` reaches
//! `requestStopAutoRouter()` (`AUTO_ROUTER_ONLY`), which does not. A single `AtomicBool` would
//! collapse the two and change behaviour without changing a test. So [`CancelToken`] carries
//! `cancel_all` **and** `cancel_auto_router`, and [`CancelToken::apply_to`] maps them onto
//! `RouterStop::{request_stop, request_stop_auto_router}` in that order.
//!
//! # Scan ruling R3: this task builds the token and its contract; the poll seam is a later task
//!
//! The plan draft assumed six `poll_deadline` call sites the adapter could join at. The committed
//! tree has **one** (`crates/fr-router/src/pipeline/batch_loop.rs:303`), `RouterStop` is
//! [`std::cell::Cell`]-based (`!Sync`) with private fields, and
//! [`fr_router::pipeline::run_pipeline`] takes `&RouterStop` and offers no closure hook. Ruling
//! AP's own escape — *"if a poll site turns out to be missed, add a poll, never a lock"* —
//! therefore applies, and scan ruling R3 makes that addition an **additive-and-wrapped,
//! driver-pinned** `fr-router` change.
//!
//! **Controller ruling BB assigns that `fr-router` change to Task 11**, the seam's first consumer
//! (Task 12 consumes it too, and each has a cancellation test that cannot pass without it). Task 0
//! builds the token, its three-state mapping and the two entry points the seam calls:
//!
//! * [`CancelToken::apply_to`] — copy the token's current state into an existing `RouterStop`.
//!   This is the one line an added poll site executes.
//! * [`CancelToken::as_router_stop`] — mint a `RouterStop` already carrying the token's state and
//!   its deadline, which is what a routing thread holds for a whole run.
//!
//! **Task 11 landed it.** `fr_router::pipeline::RouterStop::{with_cancel_poll, poll_cancel}` is the
//! additive half; [`CancelToken::as_router_stop`] installs `move |stop| token.apply_to(stop)` on
//! the stop it mints, and **four** sites run it — the three pass loop heads Task 11 landed
//! (`AutorouteBatchLoop::run`, `AutorouteBatchLoop.java:250-253`; `BatchFanout::fanout_board`,
//! `BatchFanout.java:111-116`; `BatchOptimizer::run_batch_loop`, `BatchOptimizer.java:172-176`)
//! plus the per-**item** loop of `AutoroutePassRunner::run_single_thread`
//! (`AutoroutePassRunner.java:202-205`), which **Plan 8 Task 12** added under controller ruling AI.
//!
//! Task 11 recorded the residual latency as one *pass*. Task 12 measured what a pass costs — 135
//! seconds for one auto-routing pass of `fixtures/Issue508-DAC2020_bm01.dsn` in a release build —
//! and took ruling AI's sanctioned fourth site, after which the same cancellation is observed in
//! about 0.03 s. The discharge note on [`CancelToken::apply_to`] carries the rest.
//!
//! # Why an uncancelled token is a no-op, and why that matters
//!
//! Every existing parity driver builds `RouterStop::new()` — state `None`, deadline `None`.
//! [`CancelToken::default`]'s `as_router_stop` is byte-identical to that: no flag set, no
//! deadline, and a closure that writes nothing when it runs. That is what kept `batch_parity`,
//! `p6t1` and `sweep-p7t9.sh` unchanged when Task 11 added the seam, and [`crate::cancel`]'s tests
//! assert it directly.

// ── `core/StoppableThread` — the audit rows for this crate ──────────────────────────────────────
//
// The class itself is **fr-router's** port: `fr_router::pipeline::RouterStop`
// (`crates/fr-router/src/pipeline/stop.rs`, plan 7 Task 4), which this crate re-exports
// (plan-8 ruling 1). `scripts/audit-map/fr-core.map` points `core/StoppableThread.java` and
// `core/StopRequestState.java` at this file, because this is where the mapping onto them lives,
// so the rows are here:
//
// renamed: StoppableThread.requestStop -> `fr_router::pipeline::RouterStop::request_stop` (`StoppableThread.java:23-25`).
// renamed: StoppableThread.requestStopAutoRouter -> `RouterStop::request_stop_auto_router` (`:33-37`).
// renamed: StoppableThread.isStopRequested -> `RouterStop::is_stop_requested` (`:28-30`).
// renamed: StoppableThread.isStopAutoRouterRequested -> `RouterStop::is_stop_auto_router_requested` (`:40-42`).
// not ported: StoppableThread.run — `java.lang.Thread`'s entry point (`:16-19`), whose whole body
// is `threadAction()`. The port has no threads below `crates/freerouting/src/mcp` (plan ruling 3)
// and no watchdog thread (controller ruling AI); `crates/fr-router/src/pipeline/stop.rs` carries
// the same marker at the site that would have needed it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fr_router::pipeline::RouterStop;

use crate::timespan::GRACE_PERIOD_SECONDS;

/// The observable effect of `RoutingJobSchedulerActionThread`'s monitor thread
/// (`management/jobs/RoutingJobSchedulerActionThread.java:55-90`), which the port does **not**
/// run (quirk label Y, quirk row #237 — the thread never exits, because its loop condition is
/// `while ((job != null) && (job.thread != null))` at `:58` and `job.thread` is never nulled).
///
/// Two instants, because the thread writes two things at two times:
///
/// | Java | when | this field |
/// |---|---|---|
/// | `job.thread.requestStop()` (`:75`) | `!Instant.now().isBefore(job.timeoutAt)` | [`Deadline::stop_at`] |
/// | `job.state = RoutingJobState.TIMED_OUT` (`:84`) | after spinning while `now < timeoutAt.plusSeconds(GRACE_PERIOD)` (`:77`) | [`Deadline::timed_out_at`] |
///
/// The ordering is what makes `AutorouteBatchLoop.java:251-253`'s `requestStopAutoRouter()` dead
/// code (quirk #203): by the time anything can read `TIMED_OUT` the flag is already `ALL`, and
/// `requestStopAutoRouter` only upgrades from `NONE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    /// `job.timeoutAt` — `job.startedAt.plusSeconds(timeout)`
    /// (`RoutingJobSchedulerActionThread.java:51`), the instant the monitor requests `ALL`.
    pub stop_at: Instant,
    /// `stop_at` + `GRACE_PERIOD` (`:25`, `:77`) — the instant `job.state = TIMED_OUT` is
    /// written, which `AutorouteBatchLoop.java:251-253` reads and `:578-584` reports as a
    /// [`fr_router::pipeline::TaskState::TimedOut`].
    pub timed_out_at: Instant,
}

impl Deadline {
    /// `job.startedAt.plusSeconds(timeout)` (`:51`) plus the monitor's grace (`:25`, `:77`),
    /// measured from `base` — Java's `job.startedAt`.
    ///
    /// `seconds` is **signed**, because `TextManager.parseTimespanString` answers a signed
    /// `Long`: a negative timeout produces a `stop_at` already in the past, which the monitor
    /// trips on its first tick. [`Instant`] arithmetic can fail near the platform's monotonic
    /// origin (a process younger than `seconds`), in which case both arms saturate at `base`;
    /// that is a **totalisation** of an operation Java performs on a wall clock with no such
    /// floor, and it is unobservable because both `base` and the saturated value are already
    /// `<= now`.
    pub fn from_base(base: Instant, seconds: i64) -> Deadline {
        let stop_at = offset(base, seconds);
        Deadline {
            stop_at,
            // `:77` — `job.timeoutAt.plusSeconds(GRACE_PERIOD)`.
            timed_out_at: offset(stop_at, GRACE_PERIOD_SECONDS),
        }
    }

    /// [`Deadline::from_base`] with `base` read from the clock — `job.startedAt = Instant.now()`
    /// (`:38`).
    pub fn in_seconds(seconds: i64) -> Deadline {
        Deadline::from_base(Instant::now(), seconds)
    }

    /// Whether the monitor would have called `requestStop()` by `now` (`:71`, `:75`).
    pub fn is_stop_due_at(&self, now: Instant) -> bool {
        // Java's guard is `!Instant.now().isBefore(job.timeoutAt)`, i.e. `now >= timeoutAt`.
        now >= self.stop_at
    }

    /// Whether the monitor would have written `TIMED_OUT` by `now` (`:77`, `:84`).
    pub fn is_timed_out_at(&self, now: Instant) -> bool {
        now >= self.timed_out_at
    }
}

/// Adds a signed second count to an instant, saturating at `base` rather than panicking. See
/// [`Deadline::from_base`] for why saturation is the right totalisation here.
fn offset(base: Instant, seconds: i64) -> Instant {
    if seconds >= 0 {
        base.checked_add(Duration::from_secs(seconds as u64))
            .unwrap_or(base)
    } else {
        base.checked_sub(Duration::from_secs(seconds.unsigned_abs()))
            .unwrap_or(base)
    }
}

/// Spec §10's `CancelToken`, shaped so it can cross a thread boundary to the MCP reader while
/// preserving Java's three-state stop (plan ruling 2).
///
/// [`RouterStop`] is `Cell`-based and therefore `!Sync`; **this** is the type that is shared, and
/// [`CancelToken::as_router_stop`] is what the routing thread holds. Cloning shares the flags —
/// two clones of one token are one token, which is the point.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    /// `StoppableThread.requestStop()` -> `ALL` (`core/StoppableThread.java:23-25`). Set by the
    /// MCP's `notifications/cancelled` and by a deadline expiry
    /// (`RoutingJobSchedulerActionThread.java:75`).
    cancel_all: Arc<AtomicBool>,
    /// `StoppableThread.requestStopAutoRouter()` -> `NONE -> AUTO_ROUTER_ONLY` **only**
    /// (`:33-37`). Quirk #200 rides on the difference; see the module doc.
    cancel_auto_router: Arc<AtomicBool>,
    /// Ruling AI's job deadline, or `None` for "no job timeout" — the CLI's default and what
    /// every parity run uses.
    deadline: Option<Deadline>,
}

impl CancelToken {
    /// An uncancelled token with no deadline. Its [`CancelToken::as_router_stop`] is
    /// indistinguishable from `RouterStop::new()`, which is what keeps every existing parity
    /// driver byte-unchanged.
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    /// A token carrying `job.timeoutAt` (`RoutingJobSchedulerActionThread.java:44-52`), measured
    /// from now.
    ///
    /// `total` is a [`Duration`] and therefore non-negative; the ladder that can produce a
    /// negative one is [`crate::timespan::job_timeout_deadline`], and
    /// [`CancelToken::with_deadline`] is how its answer is attached.
    pub fn with_timeout(total: Duration) -> CancelToken {
        CancelToken::with_deadline(Deadline::in_seconds(
            total.as_secs().min(i64::MAX as u64) as i64
        ))
    }

    /// A token carrying an already-computed [`Deadline`] — what
    /// [`crate::timespan::job_timeout_deadline`] answers.
    pub fn with_deadline(deadline: Deadline) -> CancelToken {
        CancelToken {
            deadline: Some(deadline),
            ..CancelToken::default()
        }
    }

    /// **This** token with a job deadline attached: the same two flags — so a cancellation that
    /// has already been requested, or arrives later, still reaches the run — plus ruling AI's
    /// `job.timeoutAt` (`RoutingJobSchedulerActionThread.java:44-52`).
    ///
    /// [`CancelToken::with_deadline`] mints a **fresh** token, which is right for the CLI (it
    /// owns the only token there is) and wrong for a caller that was *handed* one: the MCP's
    /// `route_board` receives the transport's token, whose `cancel_all` an inbound
    /// `notifications/cancelled` sets, and must not swap it for a token nothing can reach.
    #[must_use]
    pub fn with_deadline_from(&self, deadline: Deadline) -> CancelToken {
        CancelToken {
            deadline: Some(deadline),
            ..self.clone()
        }
    }

    /// The token's deadline, if any.
    pub fn deadline(&self) -> Option<Deadline> {
        self.deadline
    }

    /// `StoppableThread.requestStop()` -> `ALL`.
    ///
    /// [`Ordering::SeqCst`] on both the store and the load: the flag is the whole of the
    /// synchronisation between the MCP's reader thread and the routing thread, there is exactly
    /// one of them per run, and a weaker ordering would buy nothing measurable while making the
    /// one concurrency in the port harder to reason about.
    pub fn cancel(&self) {
        self.cancel_all.store(true, Ordering::SeqCst);
    }

    /// `StoppableThread.requestStopAutoRouter()` -> `AUTO_ROUTER_ONLY`.
    ///
    /// Java's own method is a one-way upgrade from `NONE` and a no-op from `ALL` (`:33-37`); the
    /// token records the *request* and [`CancelToken::apply_to`] performs the upgrade through
    /// `RouterStop::request_stop_auto_router`, which is where Java's guard lives. Setting this
    /// after [`CancelToken::cancel`] therefore leaves the stop at `ALL`, exactly as in Java.
    pub fn cancel_auto_router(&self) {
        self.cancel_auto_router.store(true, Ordering::SeqCst);
    }

    /// Whether `requestStop()` has been asked for — Java's `stopRequestState == ALL` **request**,
    /// not the deadline.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_all.load(Ordering::SeqCst)
    }

    /// Whether `requestStopAutoRouter()` has been asked for.
    pub fn is_auto_router_cancelled(&self) -> bool {
        self.cancel_auto_router.load(Ordering::SeqCst)
    }

    /// True once `deadline.timed_out_at` has passed — Java's `job.state == TIMED_OUT`, which
    /// `AutorouteBatchLoop.java:251-253` reads.
    pub fn is_timed_out(&self) -> bool {
        self.deadline
            .is_some_and(|d| d.is_timed_out_at(Instant::now()))
    }

    /// Ruling AP's adapter, applied to an existing [`RouterStop`]: copy the token's flags in, in
    /// Java's own order.
    ///
    /// `ALL` is written first and `AUTO_ROUTER_ONLY` second precisely because
    /// `request_stop_auto_router` is a one-way upgrade from `None`
    /// (`StoppableThread.java:33-37`): applying them the other way round would still land on
    /// `ALL`, but only by accident of `request_stop` being unconditional. This order is the one
    /// Java's two call sites can produce.
    ///
    /// **A token with neither flag set touches nothing** — no state write, no observable
    /// difference from not calling this at all. That is the property every added poll site
    /// depends on, and `an_uncancelled_token_is_a_no_op` pins it.
    ///
    // **DISCHARGED in Plan 8 Task 11** (controller ruling BB — the seam is owned by its first
    // consumer; Task 12 consumes it too). What landed, exactly as the obligation specified it:
    // `fr_router::pipeline::RouterStop::{with_cancel_poll, poll_cancel}` — an additive,
    // constructor-defaulted `Option<Box<dyn Fn(&RouterStop)>>` — called from three loop heads,
    // `pipeline/batch_loop.rs`'s job-level pass loop
    // (`AutorouteBatchLoop.java:250-253`) and the two per-stage sites plan-7 ruling AI enumerates,
    // `pipeline/fanout.rs` (`BatchFanout.java:111-116`) and `pipeline/optimizer.rs`
    // (`BatchOptimizer.java:172-176`) — and, since **Task 12**, a **fourth**: see below. Every
    // existing signature is unchanged and the poll is
    // wrapped: both `RouterStop` constructors leave the closure `None`, so the added line is a
    // `None` test on every run that does not install one. [`CancelToken::as_router_stop`] is the
    // only installer in the tree, and what it installs is this method. The gate was re-run and is
    // in the Task 11 report: `cargo test -p fr-router --test batch_parity`, `run.sh p6t1` (all six
    // rows) and `run.sh p8t1` all byte-unchanged.
    //
    // ~~The residual latency is one *pass*, not one *run*… Task 12 may add a fourth site if a
    // whole pass turns out to be too coarse; ruling AI's own list allows `AutoroutePassRunner:203`
    // as a job-level one.~~ **Task 12 measured it and did.** One auto-routing pass of
    // `fixtures/Issue508-DAC2020_bm01.dsn` is **135 seconds** in a release build (`--max-passes 1`,
    // fanout and optimizer off), so a whole pass is far too coarse: an operator's
    // `notifications/cancelled` would have taken over two minutes. `pipeline/pass_runner.rs`'s
    // per-item loop (`AutoroutePassRunner.java:202-205`) now carries the fourth `poll_cancel`, and
    // the same cancellation is observed in about **0.03 s** —
    // `crates/freerouting/tests/mcp_stdio.rs::cancelling_route_board_mid_run_…` bounds it at 60 s,
    // so the site cannot be removed without that test taking minutes. `batch_parity` and `p8t1`
    // were re-run byte-unchanged.
    pub fn apply_to(&self, stop: &RouterStop) {
        if self.is_cancelled() {
            // StoppableThread.requestStop (:23-25).
            stop.request_stop();
        }
        if self.is_auto_router_cancelled() {
            // StoppableThread.requestStopAutoRouter (:33-37) — a no-op from ALL.
            stop.request_stop_auto_router();
        }
    }

    /// Ruling AP's adapter as a **constructor** (scan ruling R3): the [`RouterStop`] a routing
    /// thread holds for a whole run, already carrying this token's flags and its deadline.
    ///
    /// The deadline crosses as a **span in milliseconds**, because that is what
    /// `RouterStop::with_deadline` takes — `fr_board::TimeLimit` keeps a span, not an instant
    /// (`RoutingJobSchedulerActionThread.java:44-51` computes the instant; plan-7 kept the span).
    /// A `stop_at` already in the past becomes a negative span, which `TimeLimit::is_exceeded`
    /// reports as exceeded immediately — Java's answer for a negative or elapsed timeout.
    /// A span longer than [`i32::MAX`] milliseconds (~24.8 days) saturates, which the 24 h
    /// `MAX_TIMEOUT` cap (`:24`) puts out of reach on the ported path.
    pub fn as_router_stop(&self) -> RouterStop {
        let stop = match self.deadline {
            None => RouterStop::new(),
            Some(deadline) => {
                let now = Instant::now();
                let limit_ms = if deadline.stop_at >= now {
                    i64::try_from(deadline.stop_at.duration_since(now).as_millis())
                        .unwrap_or(i64::MAX)
                } else {
                    // Already expired: any negative span is exceeded on the first poll
                    // (`TimeLimit.java:18-21` compares `elapsed > limit`).
                    -1
                };
                RouterStop::with_deadline(limit_ms.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
            }
        };
        // Controller ruling BB's seam, installed: the routing thread now re-reads this token at
        // every loop head rather than only here. `self.clone()` shares the two `Arc<AtomicBool>`s
        // — two clones of one token are one token — so the closure sees a `cancel()` that arrives
        // after this line, which is the whole of what the seam buys.
        let token = self.clone();
        let stop = stop.with_cancel_poll(Box::new(move |stop| token.apply_to(stop)));
        // The state as of *now*, so a token already cancelled before the run starts is observed
        // without waiting for a loop head — the pre-seam behaviour, kept.
        self.apply_to(&stop);
        stop
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_router::pipeline::StopRequestState;

    #[test]
    fn a_fresh_token_is_a_fresh_router_stop() {
        let token = CancelToken::new();
        let stop = token.as_router_stop();
        assert_eq!(stop.state(), StopRequestState::None);
        assert!(!stop.is_timed_out());
        assert!(!stop.poll_deadline());
    }

    #[test]
    fn the_grace_period_lands_on_timed_out_at_and_never_on_stop_at() {
        let base = Instant::now();
        let deadline = Deadline::from_base(base, 60);
        assert_eq!(deadline.stop_at, base + Duration::from_secs(60));
        assert_eq!(deadline.timed_out_at, base + Duration::from_secs(90));
    }
}
