//! The three-state stop flag, ruling AI's deadline, the budget knob and the two progress gates
//! — Plan 7 Task 4.
//!
//! # The three sites the stop state is read from, and why they disagree
//!
//! Java's routing pipeline shares **one** `StoppableThread` (`core/StoppableThread.java`) between
//! the fanout, the pass loop, the pass runner and the optimizer, and that thread carries a
//! *three*-state flag rather than a boolean:
//!
//! | Java call | effect (`StoppableThread.java`) |
//! |---|---|
//! | `requestStop()` | `stopRequestState = ALL` (`:23-25`) |
//! | `requestStopAutoRouter()` | `NONE -> AUTO_ROUTER_ONLY` **only** (`:33-37`) |
//! | `isStopRequested()` | `== ALL` (`:28-30`) |
//! | `isStopAutoRouterRequested()` | `!= NONE` (`:40-42`) |
//!
//! The two queries are adjacent, they differ by one comparison, and the difference is load
//! bearing: `AutoroutePassRunner.java:219` requests `ALL` when `--max-items` is reached while
//! `AutorouteBatchLoop.java:271` requests `AUTO_ROUTER_ONLY` when `--max-passes` is, and
//! `RoutingPipeline.java:117` gates the optimizer stage on `isStopRequested()`. So the two limits
//! are not symmetric — **quirk #202**.
//!
//! # Ruling AI: the deadline is a poll, not a thread
//!
//! Java gives a job a wall-clock timeout through a **monitor thread**
//! (`management/jobs/RoutingJobSchedulerActionThread.java:55-90`), which sleeps a second at a
//! time and, on expiry, calls `job.thread.requestStop()` (`:75`) and only *then*, after a 30 s
//! grace period, writes `job.state = RoutingJobState.TIMED_OUT` (`:84`). Controller ruling AI:
//! the port has **no watchdog thread**. [`RouterStop::poll_deadline`] reproduces the monitor's
//! observable effect and is called at exactly the points Java reads `job.state` or would have
//! seen the flag flipped, and nowhere else.
//!
//! ## Ruling AI's six sites are **two** Java mechanisms, not one
//!
//! This is the distinction Tasks 11-14 must not flatten. Ruling AI names six read sites; only the
//! first is the job-level flag:
//!
//! | site | Java clock | Java action |
//! |---|---|---|
//! | `AutorouteBatchLoop:251` | `job.state == TIMED_OUT`, written by the monitor thread | `requestStopAutoRouter()` — dead code, quirk #203; the flag is already `ALL` |
//! | `BatchFanout:111`, `:396` | `BatchFanout.deadlineMs`, from `settings.fanout.timeoutString` (`BatchFanout.java:94-99`) | `this.isTimedOut = true;` + `break` — **the stop flag is never written** |
//! | `BatchOptimizer:172`, `:308` | `BatchOptimizer.deadlineMs`, from `settings.optimizer.timeoutString` (`BatchOptimizer.java:153-159`) | `this.isTimedOut = true;` + `break`/`return` — **the stop flag is never written** |
//! | `AutoroutePassRunner:203` | the flag itself (`isStopAutoRouterRequested()`) | `break` |
//!
//! A tree-wide grep confirms it: `requestStop` appears in neither `BatchFanout.java` nor
//! `BatchOptimizer.java`, and the only four `isTimedOut = true` writes are the four rows above.
//! So the four **per-stage** sites end their own stage and leave every other stage running —
//! a fanout timeout does not stop the router, and a router timeout does not stop the optimizer.
//! [`RouterStop::poll_deadline`] requests `ALL`, which through `RoutingPipeline.java:117` *would*
//! suppress the optimizer stage. Calling it at a per-stage site would therefore diverge.
//!
//! obligation: Task 11/12 (`BatchFanout.fanoutBoard`, `BatchFanout.fanoutPass`) and Task 13/14
//! (`BatchOptimizer.runBatchLoop`, `BatchOptimizer.optRoutePass`) must give their stage its own
//! per-stage deadline — a plain `fr_board::TimeLimit` built from the stage's `timeoutString`,
//! setting a stage-local `is_timed_out` — and must **not** call
//! [`RouterStop::poll_deadline`] there. Only Task 10 (`AutorouteBatchLoop.run`) and Task 9
//! (`AutoroutePassRunner.runPass`) read the job-level flag through this type.
//!
//! Unreachable today: no `poll_deadline` call site exists yet, and every parity run uses
//! [`RouterStop::new`] (deadline `None`), so nothing can observe the difference until Tasks 11-14
//! land.
//!
//! # pub seam: Plan 8's `CancelToken`
//!
//! Controller ruling AP puts Plan 8's `CancelToken` (spec §10) at these same poll sites as an
//! `Arc<AtomicBool>` copied in. This module deliberately introduces **no atomics**: the state is
//! a [`Cell`] because the pipeline is single-threaded (plan-7 §Tech Stack, "no threads"), and the
//! token joins it at the call site rather than replacing it.

use std::cell::Cell;
use std::time::Instant;

use fr_board::TimeLimit;

// =================================================================================================
// `StopRequestState` — core/StopRequestState.java
// =================================================================================================

/// Port of `core/StopRequestState.java` (11 lines) — the stop state of a `StoppableThread`.
///
/// The enum is **its own Java file**; `StoppableThread.java:8` is the field that holds it. The
/// variant order is Java's declaration order, and therefore its `ordinal()` order, which is what
/// [`PartialOrd`]/[`Ord`] follow here: `None < AutoRouterOnly < All` is the same lattice
/// `requestStopAutoRouter`'s one-way upgrade walks.
///
/// Pinned by `crates/fr-router/tests/data/p7t4-stop-and-counters.txt`'s `[enums]` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum StopRequestState {
    /// No stop is requested (`StopRequestState.java:6`).
    #[default]
    None,
    /// Only the auto-router is requested to stop (`:8`).
    AutoRouterOnly,
    /// The entire thread is requested to stop (`:10`).
    All,
}

impl StopRequestState {
    /// Java's `Enum.ordinal()` — the declaration index, which every `switch` table and every Gson
    /// round trip keys on.
    pub fn ordinal(self) -> i32 {
        match self {
            StopRequestState::None => 0,
            StopRequestState::AutoRouterOnly => 1,
            StopRequestState::All => 2,
        }
    }
}

// =================================================================================================
// `PassRecord` — ruling 1(a)'s per-pass diagnostic tuple
// =================================================================================================

/// Ruling 1(a)'s per-pass tuple: what the acceptance ladder compares pass by pass when a whole
/// board's SES bytes differ.
///
/// renamed: this is **not** a Java type. Java scatters the same six numbers across `FRLogger`
/// lines that the port drops (plan-7 §Rulings, "no `FRLogger`"), so the record is the port's own
/// shape and each field names the Java site it is read from:
///
/// | field | Java source |
/// |---|---|
/// | `pass` | `AutorouteBatchLoop.java:275-277` (`job.setCurrentPass`) |
/// | `score` | `BoardStatistics.getNormalizedScore` via `AutorouteBatchLoop.java:135-138` |
/// | `incomplete_count` | `BoardStatistics.connections.incompleteCount` (`BoardStatistics.java:271`) |
/// | `clearance_violations` | `BoardStatistics.clearanceViolations.totalCount` (`:277`) |
/// | `via_count` | `BoardStatistics.items.viaCount` |
/// | `trace_count` | `BoardStatistics.items.traceCount` |
///
/// Produced in Task 4 rather than in Task 15 (scan ruling 6): Task 10's batch-loop result is
/// where ruling 1(a)'s ladder first bites, five tasks before `run_pipeline` exists.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassRecord {
    /// The 1-based pass number.
    pub pass: i32,
    /// `BoardStatistics`' normalized score after the pass.
    pub score: f32,
    /// Connections still in the ratsnest after the pass.
    pub incomplete_count: usize,
    /// Clearance violations on the board after the pass.
    pub clearance_violations: usize,
    /// Vias on the board after the pass.
    pub via_count: usize,
    /// Traces on the board after the pass.
    pub trace_count: usize,
}

// =================================================================================================
// `RouterStop` — the pipeline's single `Stoppable`
// =================================================================================================

/// The port's single `Stoppable`, shared by the whole pipeline exactly as Java shares one
/// `StoppableThread` (`core/StoppableThread.java`, and survey Appendix B's six
/// `AutorouteEngine.isStopRequested` sites).
///
/// It also carries ruling AI's deadline, which is the observable effect of Java's per-job monitor
/// thread (`management/jobs/RoutingJobSchedulerActionThread.java:55-90`) — see the module doc.
///
/// # Interior mutability, and why it is a `Cell`
///
/// Java's four methods are `synchronized` on a thread object every stage holds a reference to, so
/// they mutate through a shared borrow. The port's callers hold `&RouterStop` for the same
/// reason, and the crate is single-threaded (plan-7 §Tech Stack), so [`Cell`] is the whole of the
/// machinery. `pub seam:` Plan 8's `CancelToken` (`Arc<AtomicBool>`, controller ruling AP) is
/// copied in **at the poll sites**, not folded in here.
///
/// # Building the two `StopCheck`s
///
/// `fr_board::StopCheck<'a>` is `&'a dyn Fn() -> bool` and so cannot be returned from a method.
/// Callers build the one they need at the call site:
///
/// ```
/// use fr_board::StopCheck;
/// use fr_router::pipeline::RouterStop;
///
/// let stop = RouterStop::new();
/// let auto: StopCheck<'_> = &|| stop.is_stop_auto_router_requested();
/// let all: StopCheck<'_> = &|| stop.is_stop_requested();
/// assert!(!auto() && !all());
/// ```
#[derive(Debug)]
pub struct RouterStop {
    /// `StoppableThread.stopRequestState` (`StoppableThread.java:8`).
    state: Cell<StopRequestState>,
    /// Ruling AI's deadline. `None` is "no job timeout", which is the CLI's default and what
    /// every parity run uses.
    deadline: Option<TimeLimit>,
    /// Java's `job.state == RoutingJobState.TIMED_OUT`, which
    /// `AutorouteBatchLoop.java:251-253` reads and `:578-584` reports. Written by
    /// [`RouterStop::poll_deadline`] only.
    timed_out: Cell<bool>,
}

// not ported: `StoppableThread.run` — `java.lang.Thread`'s entry point (`StoppableThread.java:16-19`),
// which just calls the abstract `threadAction()`. The port has no threads (plan-7 §Tech Stack) and
// no watchdog thread (controller ruling AI): the stop flag is polled, and
// `management/jobs/RoutingJobSchedulerActionThread`'s monitor thread (`:55-90`) is reproduced by
// [`RouterStop::poll_deadline`]. `threadAction` is `protected` and so is not an audit surface.

impl Default for RouterStop {
    fn default() -> Self {
        Self::new()
    }
}

impl RouterStop {
    /// A stop with no deadline — `StoppableThread`'s own initial state
    /// (`StoppableThread.java:8`, `stopRequestState = NONE`).
    pub fn new() -> RouterStop {
        RouterStop {
            state: Cell::new(StopRequestState::None),
            deadline: None,
            timed_out: Cell::new(false),
        }
    }

    /// A stop carrying `job.timeoutAt` as a [`TimeLimit`], measured from now
    /// (`RoutingJobSchedulerActionThread.java:44-51` computes the instant; the port keeps the
    /// span, because [`TimeLimit`] does).
    ///
    /// `limit_ms` is a span in milliseconds. A run that never polls, or never reaches the limit,
    /// cannot tell this apart from [`RouterStop::new`] — which is ruling AI's determinism claim,
    /// pinned by `an_unexpired_deadline_is_invisible`.
    pub fn with_deadline(limit_ms: i32) -> RouterStop {
        RouterStop {
            state: Cell::new(StopRequestState::None),
            deadline: Some(TimeLimit::new(limit_ms)),
            timed_out: Cell::new(false),
        }
    }

    /// `StoppableThread.requestStop` (`:23-25`) — sets `ALL`, unconditionally, from any state.
    pub fn request_stop(&self) {
        self.state.set(StopRequestState::All);
    }

    /// `StoppableThread.requestStopAutoRouter` (`:33-37`) — upgrades `NONE -> AUTO_ROUTER_ONLY`
    /// and **only** that. From `AUTO_ROUTER_ONLY` and from `ALL` it is a no-op, which is why
    /// `AutorouteBatchLoop.java:251-253` can never fire in production (quirk #203).
    pub fn request_stop_auto_router(&self) {
        if self.state.get() == StopRequestState::None {
            self.state.set(StopRequestState::AutoRouterOnly);
        }
    }

    /// `StoppableThread.isStopRequested` (`:28-30`) — `stopRequestState == ALL`.
    ///
    /// **Not** interchangeable with [`RouterStop::is_stop_auto_router_requested`]: this one is
    /// what `RoutingPipeline.java:117` gates the optimizer stage on, so an `AUTO_ROUTER_ONLY`
    /// stop leaves the optimizer running and an `ALL` stop does not — **quirk #202**.
    ///
    // obligation: Task 9 (`AutoroutePassRunner`) must carry the `// Java bug:` marker for quirk
    // #202 at its `maxItems` site, which is the *cause*; this method is only the reader. The
    // marker's Java method name must sit on its own line (Plan 2 ruling 13).
    pub fn is_stop_requested(&self) -> bool {
        self.state.get() == StopRequestState::All
    }

    /// `StoppableThread.isStopAutoRouterRequested` (`:40-42`) — `stopRequestState != NONE`.
    ///
    /// This is the one the pass loop and the item loop read
    /// (`AutorouteBatchLoop.java:250`, `AutoroutePassRunner.java:203`, `:208`), so both stop
    /// states end the routing stage.
    pub fn is_stop_auto_router_requested(&self) -> bool {
        self.state.get() != StopRequestState::None
    }

    /// The raw flag, for the callers that report it and for the tests that pin the transition
    /// table. Java has no accessor; the field is private (`StoppableThread.java:8`) and
    /// `P7T4Probe` reaches it by reflection.
    pub fn state(&self) -> StopRequestState {
        self.state.get()
    }

    /// Ruling AI's deadline poll. Returns whether the deadline has expired, and on the first
    /// expiry performs Java's *observable* action.
    ///
    /// Java's monitor thread calls `requestStop()` — `ALL` — at
    /// `RoutingJobSchedulerActionThread.java:75`, and writes `job.state = TIMED_OUT` at `:84`,
    /// **30 seconds later** (`GRACE_PERIOD`). By the time anything can read `TIMED_OUT` the flag
    /// is therefore already `ALL`, which is what makes `AutorouteBatchLoop.java:251-253`'s
    /// `requestStopAutoRouter()` dead code (quirk #203). The port collapses the two writes into
    /// this one call: it requests `ALL` and it raises [`RouterStop::is_timed_out`].
    ///
    /// # Which of ruling AI's six sites may call this
    ///
    /// **Two only** — `AutorouteBatchLoop:251` (Task 10) and the top of `AutoroutePassRunner`'s
    /// item loop at `:203` (Task 9). Those are the job-level flag's readers.
    ///
    /// The other four — `BatchFanout:111` and `:396`, `BatchOptimizer:172` and `:308` — read a
    /// **per-stage** `deadlineMs` (`BatchFanout.java:94-99` from `settings.fanout.timeoutString`,
    /// `BatchOptimizer.java:153-159` from `settings.optimizer.timeoutString`) and their action is
    /// `this.isTimedOut = true;` + `break`, which **never touches the stop flag**. Requesting
    /// `ALL` there would suppress the optimizer stage through `RoutingPipeline.java:117`, where
    /// Java runs it. See the module doc's table and its `obligation:` line.
    ///
    /// With no deadline this is a constant `false` and touches nothing.
    pub fn poll_deadline(&self) -> bool {
        let Some(deadline) = self.deadline.as_ref() else {
            return false;
        };
        if !deadline.is_exceeded() {
            return false;
        }
        // RoutingJobSchedulerActionThread.java:75 — `job.thread.requestStop()`.
        self.request_stop();
        // :84 — `job.state = RoutingJobState.TIMED_OUT`, after the grace period the port has no
        // thread to wait out.
        self.timed_out.set(true);
        true
    }

    /// Java's `job.state == RoutingJobState.TIMED_OUT`, which `AutorouteBatchLoop.java:251-253`
    /// reads and `:578-584` turns into a `TaskState::TIMED_OUT`.
    ///
    /// It exists only for that report: nothing branches on it that does not also branch on
    /// [`RouterStop::is_stop_requested`], because [`RouterStop::poll_deadline`] sets both.
    pub fn is_timed_out(&self) -> bool {
        self.timed_out.get()
    }
}

// =================================================================================================
// `RouterBudget` — ruling AI's knob (amendment ruling 10: four fields)
// =================================================================================================

/// Ruling AI's budget knob: Java's four wall-clock literals become explicit parameters, so a
/// parity run can disable them on **both** sides and neither side's answer depends on how fast
/// the machine is. The defaults are Java's literals.
///
/// Amendment ruling 10 split this into **four** fields, not three: `250` and `1000` are two
/// different Java throttles, and conflating them would have turned a 250 ms gate into a 1000 ms
/// one.
///
/// Pinned by `the_budget_defaults_are_javas_literals`, which carries every Java line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouterBudget {
    /// `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` — the `optChangedArea` pull-tight budget. Declared
    /// four times over (`autoroute/pipeline/AutorouteConnectionRouter.java:22`,
    /// `BatchAutorouter.java:43`, `BatchAutorouterThread.java:38`,
    /// `AutoroutePassRunner.java:32`) and twice more as a local
    /// (`board/facade/RoutingBoard.java:961` and `:1100`, both feeding `optChangedArea`),
    /// always **1000**.
    ///
    /// `0` disables the limit exactly as Java does: `TraceTightener`'s constructor only builds a
    /// `TimeLimit` when `timeLimit > 0` (`board/optimize/TraceTightener.java:73-77`), so the
    /// "off" value needs no port-only branch. See [`RouterBudget::opt_changed_area_limit`].
    pub opt_changed_area_ms: i32,
    /// `settings.fanout.maxMillisecondsPerPin`'s fallback, `10000L`
    /// (`autoroute/pipeline/BatchFanout.java:175-178`). Multiplied by the pass number at
    /// `:231-232`, where the `TimeLimit` is built **unconditionally** — there is no `> 0` guard
    /// here, so `0` would be a *tight* limit rather than no limit, and
    /// [`RouterBudget::disabled`] uses [`i32::MAX`] for this field.
    pub fanout_ms_per_pin: i32,
    /// `BatchAutorouter.shouldFireBoardUpdate`'s gate, **250 ms**
    /// (`autoroute/pipeline/BatchAutorouter.java:335-343`, `currentTime - lastBoardUpdateTimestamp
    /// > 250`). Progress only (ruling 11), but a knob so a driver can pin it.
    pub board_update_throttle_ms: i32,
    /// `core.ProgressThrottler`'s interval. **Java has no constant here** — the interval is a
    /// constructor argument, and all three construction sites pass `1000`
    /// (`autoroute/pipeline/BatchOptimizer.java:29`, `BatchAutorouterThread.java:43`,
    /// `BatchFanout.java:28`). Progress only.
    pub progress_throttle_ms: i32,
}

impl Default for RouterBudget {
    /// Java's four literals.
    fn default() -> Self {
        RouterBudget {
            opt_changed_area_ms: 1000,
            fanout_ms_per_pin: 10000,
            board_update_throttle_ms: 250,
            progress_throttle_ms: 1000,
        }
    }
}

impl RouterBudget {
    /// Every wall clock off — what every `p7t*` parity driver sets, on both sides.
    ///
    /// The "off" values are **not** all zero, and each is the value Java's own code makes inert:
    /// `0` where Java guards on `> 0` (`TraceTightener.java:73-77` for `opt_changed_area_ms`,
    /// and the two progress gates, which the port documents as "`<= 0` never suppresses"), and
    /// [`i32::MAX`] for `fanout_ms_per_pin`, whose `TimeLimit` is built unconditionally
    /// (`BatchFanout.java:231-232`).
    pub fn disabled() -> RouterBudget {
        RouterBudget {
            opt_changed_area_ms: 0,
            fanout_ms_per_pin: i32::MAX,
            board_update_throttle_ms: 0,
            progress_throttle_ms: 0,
        }
    }

    /// `TraceTightener`'s constructor decision, verbatim
    /// (`board/optimize/TraceTightener.java:73-77`):
    ///
    /// ```java
    /// if (timeLimit > 0) { this.timeLimit = new TimeLimit(timeLimit); } else { this.timeLimit = null; }
    /// ```
    ///
    /// `None` is Java's `null`, i.e. "no limit". Note the `> 0`, not `!= 0`: a negative budget
    /// also means no limit.
    pub fn opt_changed_area_limit(&self) -> Option<TimeLimit> {
        if self.opt_changed_area_ms > 0 {
            Some(TimeLimit::new(self.opt_changed_area_ms))
        } else {
            None
        }
    }

    /// The per-pin fanout budget for pass `pass_no`, `baseMillisPerPin * (passNo + 1)`
    /// (`autoroute/pipeline/BatchFanout.java:231-232`).
    ///
    /// Java computes the product as a `double` and narrows it with a `(int)` cast, which
    /// saturates at the `i32` bounds — so an [`i32::MAX`] budget stays [`i32::MAX`] rather than
    /// wrapping, and the limit is never exceeded.
    pub fn fanout_limit_for_pass(&self, pass_no: i32) -> TimeLimit {
        let max_milliseconds = f64::from(self.fanout_ms_per_pin) * f64::from(pass_no + 1);
        TimeLimit::new(max_milliseconds as i32)
    }

    /// The gate `BatchAutorouter.shouldFireBoardUpdate` is (`:335-343`) — **strict** `>`.
    pub fn board_update_throttler(&self) -> ProgressThrottler {
        ProgressThrottler::board_update_gate(self.board_update_throttle_ms)
    }

    /// The gate `core.ProgressThrottler` is (`ProgressThrottler.java:15-26`) — non-strict `>=`.
    pub fn progress_throttler(&self) -> ProgressThrottler {
        ProgressThrottler::new(self.progress_throttle_ms)
    }
}

// =================================================================================================
// `ProgressThrottler` — the two progress gates, with a clock seam
// =================================================================================================

/// Port of `core/ProgressThrottler.java` (32 lines) — the wall clock that decides *whether* a
/// progress event fires.
///
/// # Why this is ported when the plan first said it would not be
///
/// The plan's Task 4 text rostered `ProgressThrottler` `// not ported:` on the ground that
/// [`crate::pipeline::ProgressSink`] "replaces the whole mechanism". `ProgressSink` replaces the
/// *listener lists* (`NamedAlgorithm.java:26-31`, ruling AK) — the delivery half. It does not
/// replace the gate: `shouldUpdate()` has **five** live call sites inside Plan 7's own classes
/// (`BatchAutorouterThread.java:404`, `BatchOptimizer.java:335, 407, 480`,
/// `BatchFanout.java:520` — a tree-wide grep answers six lines, of which
/// `core/ProgressThrottler.java:15` is the declaration itself) and `reset()` has **three**
/// (`BatchAutorouterThread.java:309`,
/// `BatchOptimizer.java:284`, `BatchFanout.java:203`), and amendment ruling 10 keeps a
/// `progress_throttle_ms` knob on [`RouterBudget`] precisely so a driver can pin it. Java wins:
/// the class is ported, and the task report records the deviation.
///
/// # The two gates are one millisecond apart
///
/// `ProgressThrottler.shouldUpdate` is `now - lastUpdateMs >= intervalMs` (`:21`) while
/// `BatchAutorouter.shouldFireBoardUpdate` is `currentTime - lastBoardUpdateTimestamp > 250`
/// (`BatchAutorouter.java:337-338`). One type, two constructors: [`ProgressThrottler::new`] is
/// Java's class, [`ProgressThrottler::board_update_gate`] is `BatchAutorouter`'s inline gate.
/// Both fire on their first call — Java's sentinel is `lastUpdateMs == 0` (`:17-20`) and
/// `BatchAutorouter`'s `long` field simply starts at `0`, which any real clock is more than 250
/// past.
///
/// # The clock seam
///
/// [`ProgressThrottler::should_update`] reads [`Instant::now`], the way Java reads
/// `System.currentTimeMillis()`. [`ProgressThrottler::should_update_at`] takes the instant
/// instead, so the gate's behaviour is unit-testable without a single sleep. Java has no such
/// method; it is `// added in Plan 7:`-shaped rather than a port, and nothing but tests and
/// [`ProgressThrottler::should_update`] calls it.
///
/// Progress only (ruling 11): no port decision reads this.
#[derive(Debug)]
pub struct ProgressThrottler {
    /// `ProgressThrottler.intervalMs` (`:5`). `<= 0` disables the gate — every call fires. That
    /// value is unreachable in Java (its three construction sites pass `1000` and
    /// `shouldFireBoardUpdate`'s literal is `250`); it exists so [`RouterBudget::disabled`] can
    /// take the wall clock out of a parity run.
    interval_ms: i32,
    /// `true` for `BatchAutorouter.shouldFireBoardUpdate`'s strict `>` (`:337-338`), `false` for
    /// `ProgressThrottler.shouldUpdate`'s `>=` (`:21`).
    strict: bool,
    /// `ProgressThrottler.lastUpdateMs` (`:6`). `None` is Java's `0` sentinel, which
    /// [`ProgressThrottler::reset`] restores (`:29-31`).
    last_update: Cell<Option<Instant>>,
}

impl ProgressThrottler {
    /// `ProgressThrottler(long intervalMs)` (`ProgressThrottler.java:9-12`) — the non-strict
    /// gate, `now - last >= interval`.
    pub fn new(interval_ms: i32) -> ProgressThrottler {
        ProgressThrottler {
            interval_ms,
            strict: false,
            last_update: Cell::new(None),
        }
    }

    /// `BatchAutorouter.shouldFireBoardUpdate`'s gate (`autoroute/pipeline/BatchAutorouter.java:335-343`)
    /// — the strict one, `now - last > interval`. It is an inline literal and a bare `long`
    /// field in Java rather than a `ProgressThrottler`; the port gives it the same type so both
    /// gates are one knob away from each other.
    pub fn board_update_gate(interval_ms: i32) -> ProgressThrottler {
        ProgressThrottler {
            interval_ms,
            strict: true,
            last_update: Cell::new(None),
        }
    }

    /// The gate's interval in milliseconds (`ProgressThrottler.java:5` — a private field with no
    /// accessor; exposed so a driver can assert on it).
    pub fn interval_ms(&self) -> i32 {
        self.interval_ms
    }

    /// `ProgressThrottler.shouldUpdate` (`:15-26`), reading the clock the way Java does.
    pub fn should_update(&self) -> bool {
        self.should_update_at(Instant::now())
    }

    /// [`ProgressThrottler::should_update`] with the clock supplied — the seam that makes the
    /// gate testable without sleeping. No Java counterpart.
    ///
    /// `now` earlier than the recorded instant (which [`Instant`] makes impossible for a real
    /// monotonic clock, but a caller supplying its own can construct) is treated as zero elapsed,
    /// so the gate can never fire *backwards*. Java reads `System.currentTimeMillis()`, which an
    /// NTP step can move, and has no such protection — the same deviation
    /// `fr_board::TimeLimit`'s doc records.
    pub fn should_update_at(&self, now: Instant) -> bool {
        if self.interval_ms <= 0 {
            // Not reachable at Java's own values; see the field's doc.
            self.last_update.set(Some(now));
            return true;
        }
        let Some(last) = self.last_update.get() else {
            // ProgressThrottler.java:17-20 — the `lastUpdateMs == 0` sentinel arms and fires.
            self.last_update.set(Some(now));
            return true;
        };
        let elapsed_ms = now.saturating_duration_since(last).as_millis() as i128;
        let limit = i128::from(self.interval_ms);
        let fires = if self.strict {
            elapsed_ms > limit
        } else {
            elapsed_ms >= limit
        };
        if fires {
            self.last_update.set(Some(now));
        }
        fires
    }

    /// `ProgressThrottler.reset` (`:29-31`) — back to the `0` sentinel, so the next call fires.
    pub fn reset(&self) {
        self.last_update.set(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_stop_is_none() {
        let stop = RouterStop::default();
        assert_eq!(stop.state(), StopRequestState::None);
        assert!(!stop.is_stop_requested());
        assert!(!stop.is_stop_auto_router_requested());
        assert!(!stop.is_timed_out());
    }

    #[test]
    fn the_state_lattice_is_declaration_order() {
        // StopRequestState.java:6, 8, 10 — and `requestStopAutoRouter` only ever walks up it.
        assert!(StopRequestState::None < StopRequestState::AutoRouterOnly);
        assert!(StopRequestState::AutoRouterOnly < StopRequestState::All);
        assert_eq!(StopRequestState::None.ordinal(), 0);
        assert_eq!(StopRequestState::AutoRouterOnly.ordinal(), 1);
        assert_eq!(StopRequestState::All.ordinal(), 2);
    }

    #[test]
    fn the_fanout_limit_scales_with_the_pass_number() {
        // BatchFanout.java:231 — `baseMillisPerPin * (passNo + 1)`.
        let budget = RouterBudget::default();
        assert_eq!(budget.fanout_limit_for_pass(0).limit_ms(), 10_000);
        assert_eq!(budget.fanout_limit_for_pass(2).limit_ms(), 30_000);
        // i32::MAX saturates rather than wrapping, so a disabled budget stays disabled.
        assert_eq!(
            RouterBudget::disabled().fanout_limit_for_pass(9).limit_ms(),
            i32::MAX
        );
    }
}
