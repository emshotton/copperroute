//! The pull-tight family: `board/optimize/TraceTightener.java` and its three regime subclasses,
//! plus the two `PolylineTrace.pullTight` overloads that drive them.
//!
//! # Why this is Plan 6's and not Plan 7's
//!
//! Plan-6 ruling 2 put the whole of `board/optimize`'s mutating half in Plan 7. Task 15 then
//! found that Java's trace-insertion path pull-tightens **every** inserted polyline
//! unconditionally — `FoundConnectionInserter:185` passes `tidyWidth = Integer.MAX_VALUE`, so
//! `RoutingBoard.insertForcedTracePolyline:860`'s `tidyWidth > 0` holds; `optNetNoArr` is empty,
//! so `PolylineTrace.pullTight:821`'s filter never fires; and `NetClass.pullTight` defaults to
//! `true` — so plan-6 ruling 1's per-connection geometry parity is unreachable without it.
//! **Controller ruling AB** amends ruling 2: these five classes are Plan 6's (this task, 15a),
//! while `ViaOptimizer`, `optChangedArea`'s batch callers and `removeItemsAndPullTight` stay
//! Plan 7's.
//!
//! # Reference identity is load-bearing here
//!
//! Java's three `pullTight` overrides loop `while (newResult != prevResult)` — **object**
//! identity, not value equality — and `PolylineTrace.pullTight:837` tests `newLines != lines` the
//! same way to decide whether the trace changed at all. Every step in this module therefore
//! answers `Option<Polyline>`, where `None` means "Java returned the argument object": the
//! `Option`'s discriminant *is* Java's `!=`. A value comparison would differ, because several
//! steps rebuild a polyline that happens to be value-equal to their input.
//!
//! # `TraceShover.springOverObstacles` is not needed
//!
//! `TraceTightener.avoidAcidTraps` (`:517-542`) is the family's only caller of it, and its first
//! statement is `if (true) { return polyline; }` — the rest of the method is dead code (quirk
//! #182). Controller ruling AA's line, which keeps `springOverObstacles` in Plan 7, therefore
//! holds unchanged.
//!
//! # `PolylineTrace.change` -> `additionalUpdateAfterChange`
//!
//! `PolylineTrace.change` calls `board.additionalUpdateAfterChange(this)`
//! (PolylineTrace.java:944) before it touches the search tree, and that call needs an
//! `AutorouteEngine`. Task 15a's entry points took none, so [`PolylineTraceExt::pull_tight_with`]
//! did not make it. **Task 15b threads it**, as controller ruling AB requires:
//! [`PolylineTraceExt::pull_tight_with_engine`] takes `Option<&mut AutorouteEngine>` — Java's
//! nullable `RoutingBoard.autorouteEngine` field (`:70`), which `additionalUpdateAfterChange:100`
//! tests before doing anything — and `pull_tight_with` is the `None` wrapper Task 15a's callers
//! and tests keep using. The call changes the engine's room/drill database, never the board's
//! item list, so no probe row in `tests/data/p6t15a-tightener.txt` or
//! `tests/data/p6t15b-insert-forced.txt` can see it; it is threaded because
//! `insertForcedTracePolyline` runs inside a live `autorouteConnection`, not because a fixture
//! catches it.

// added in Plan 7: `TraceTightener.optChangedArea` (TraceTightener.java:121-169) — the batch
// entry point that walks `board.changedArea` layer by layer and calls `PolylineTrace.pullTight`,
// `smoothenEndCornersAtTrace` and `ViaOptimizer.optViaLocation` over every overlapping object.
// Controller ruling AB keeps it, `ViaOptimizer` and `removeItemsAndPullTight` in Plan 7; the four
// methods it drives all landed here in Task 15a.

mod base;
mod tightener_45;
mod tightener_90;
mod tightener_any_angle;

use std::collections::BTreeSet;

use fr_board::datastructures::StopCheck;
use fr_board::prelude::*;
use fr_geometry::{FloatPoint, IntDirection, IntOctagon, Line, Point, Polyline, Side, Signum};

use crate::autoroute::maze::engine::AutorouteEngine;
use crate::board_ext::RoutingBoardExt;

pub(crate) use base::TightenerBase;
pub use tightener_45::TraceTightener45;
pub use tightener_90::TraceTightener90;
pub use tightener_any_angle::TraceTightenerAnyAngle;

/// Port of `abstract class TraceTightener` (TraceTightener.java:31) together with its three
/// concrete subclasses: Java's dynamic dispatch is this enum's `match`.
///
/// Build one with [`TraceTightener::get_instance`], the port of the static factory at `:87-114`.
/// The state all three share lives in `TightenerBase`; the variants add none of their own,
/// exactly as Java's subclasses declare no fields.
///
/// The lifetime is plan-6 ruling 6's: Java's `Stoppable stoppableThread` field is a borrowed
/// [`StopCheck`] here.
pub enum TraceTightener<'a> {
    /// `TraceTightener90` — `AngleRestriction.NINETY_DEGREE` (`getInstance:98-101`).
    Ninety(TraceTightener90<'a>),
    /// `TraceTightener45` — `AngleRestriction.FORTYFIVE_DEGREE` (`:102-105`).
    FortyFive(TraceTightener45<'a>),
    /// `TraceTightenerAnyAngle` — every other restriction (`:106-110`).
    AnyAngle(TraceTightenerAnyAngle<'a>),
}

impl<'a> TraceTightener<'a> {
    /// Port of `TraceTightener.getInstance(RoutingBoard, int[], IntOctagon, int, Stoppable, int,
    /// Point, int)` (TraceTightener.java:87-114): "returns a new instance of TraceTightener. If
    /// onlyNetNo > 0, only traces with net number notNo are optimized. If stoppableThread !=
    /// null, the algorithm can be requested to be stopped. If timeLimit > 0; the algorithm will
    /// be stopped after timeLimit Milliseconds."
    ///
    /// The regime comes from `board.rules.getTraceAngleRestriction()` (`:97`) — the board's
    /// rules, **not** the search tree's own angle class.
    #[allow(clippy::too_many_arguments)]
    pub fn get_instance(
        board: &mut Board,
        only_net_no_arr: Vec<i32>,
        clip_shape: Option<IntOctagon>,
        min_translate_dist: i32,
        stoppable_thread: Option<StopCheck<'a>>,
        time_limit: i32,
        keep_point: Option<Point>,
        keep_point_layer: i32,
    ) -> TraceTightener<'a> {
        // :97.
        let angle_restriction = board.rules.trace_angle_restriction;
        let base = TightenerBase::new(
            only_net_no_arr,
            stoppable_thread,
            time_limit,
            keep_point,
            keep_point_layer,
        );
        // :98-110.
        let mut result = match angle_restriction {
            AngleRestriction::NinetyDegree => TraceTightener::Ninety(TraceTightener90::new(base)),
            AngleRestriction::FortyFiveDegree => {
                TraceTightener::FortyFive(TraceTightener45::new(base))
            }
            AngleRestriction::None => TraceTightener::AnyAngle(TraceTightenerAnyAngle::new(base)),
        };
        // :111-112.
        result.base_mut().current_clip_shape = clip_shape;
        result.base_mut().min_translate_dist = min_translate_dist.max(100);
        // :113.
        result
    }

    /// The shared state (Java's `super` fields).
    pub(crate) fn base(&self) -> &TightenerBase<'a> {
        match self {
            TraceTightener::Ninety(t) => &t.base,
            TraceTightener::FortyFive(t) => &t.base,
            TraceTightener::AnyAngle(t) => &t.base,
        }
    }

    /// The shared state, mutably.
    pub(crate) fn base_mut(&mut self) -> &mut TightenerBase<'a> {
        match self {
            TraceTightener::Ninety(t) => &mut t.base,
            TraceTightener::FortyFive(t) => &mut t.base,
            TraceTightener::AnyAngle(t) => &mut t.base,
        }
    }

    /// `TraceTightener.onlyNetNoArr` (TraceTightener.java:40), the one public field of the class.
    pub fn only_net_no_arr(&self) -> &[i32] {
        &self.base().only_net_no_arr
    }

    /// `TraceTightener.minTranslateDist` after `getInstance:112`'s `Math.max(.., 100)` clamp.
    pub fn min_translate_dist(&self) -> i32 {
        self.base().min_translate_dist
    }

    /// Port of the abstract `pullTight(Polyline)` (TraceTightener.java:192) — the regime
    /// dispatch.
    ///
    /// See `pull_tight_opt` for the version that preserves Java's reference identity.
    pub fn pull_tight(&mut self, board: &mut Board, polyline: &Polyline) -> Polyline {
        match self.pull_tight_opt(board, polyline) {
            Some(tightened) => tightened,
            None => polyline.clone(),
        }
    }

    /// [`Self::pull_tight`], answering `None` where Java hands the argument object back —
    /// which is what `PolylineTrace.pullTight:837`'s `newLines != lines` reads.
    pub(crate) fn pull_tight_opt(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        match self {
            TraceTightener::Ninety(t) => t.pull_tight(board, polyline),
            TraceTightener::FortyFive(t) => t.pull_tight(board, polyline),
            TraceTightener::AnyAngle(t) => t.pull_tight(board, polyline),
        }
    }

    /// Port of the six-argument `pullTight(Polyline, int, int, int[], int, Set<Pin>)`
    /// (TraceTightener.java:175-190): "function for optimizing a single trace polygon.
    /// `contactPins` are the pins at the end corners of polyline. Other pins are regarded as
    /// obstacles, even if they are of the own net."
    ///
    /// `:184-185` adds the **default** tree's clearance compensation to the half width, which is
    /// what makes `currentHalfWidth` a compensated width for every `checkTraceShape` below.
    // renamed: the six-argument `pullTight` overload -> `pull_tight_polyline` (Rust has no
    // overloading); the one-argument `pullTight(Polyline)` keeps the name.
    #[allow(clippy::too_many_arguments)]
    pub fn pull_tight_polyline(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
        layer: usize,
        half_width: i32,
        net_numbers: &[i32],
        clearance_class_index: usize,
        contact_pins: Option<BTreeSet<ItemId>>,
    ) -> Polyline {
        match self.pull_tight_polyline_opt(
            board,
            polyline,
            layer,
            half_width,
            net_numbers,
            clearance_class_index,
            contact_pins,
        ) {
            Some(tightened) => tightened,
            None => polyline.clone(),
        }
    }

    /// [`Self::pull_tight_polyline`], preserving Java's reference identity.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn pull_tight_polyline_opt(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
        layer: usize,
        half_width: i32,
        net_numbers: &[i32],
        clearance_class_index: usize,
        contact_pins: Option<BTreeSet<ItemId>>,
    ) -> Option<Polyline> {
        // :182-188. This is one of the **two** writers of `currentLayer`, `currentHalfWidth`,
        // `currentNetNumbers` and `currentClearanceClassIndex`; the other is
        // `TraceTightener.smoothenEndCornersAtTrace` (:406-409). Until one of them has run, Java's
        // `currentNetNumbers` is `null` and either `smoothen*CornerAtTrace` override throws inside
        // `BasicBoard.checkTraceShape:1017` — see that pair's doc comment.
        let compensation = board.trees.get_default_tree().clearance_compensation_value(
            clearance_class_index,
            layer,
            &board.rules,
        );
        let base = self.base_mut();
        base.current_layer = layer;
        base.current_half_width = half_width + compensation;
        base.current_net_numbers = net_numbers.to_vec();
        base.current_clearance_class_index = clearance_class_index;
        base.contact_pins = contact_pins;
        // :189.
        self.pull_tight_opt(board, polyline)
    }

    /// Port of `repositionLines(Polyline)` — the package-private `TraceTightener.java:215-230`
    /// and the `TraceTightenerAnyAngle.java:250-274` override, dispatched.
    ///
    /// `None` is Java's "returns the argument object".
    pub fn reposition_lines(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        match self {
            TraceTightener::Ninety(t) => t.base.reposition_lines(board, polyline),
            TraceTightener::FortyFive(t) => t.base.reposition_lines(board, polyline),
            TraceTightener::AnyAngle(t) => t.reposition_lines(board, polyline),
        }
    }

    /// Port of `repositionLine(Line[], int)` — the protected `TraceTightener.java:235-331` and
    /// the `TraceTightenerAnyAngle.java:497-657` override, dispatched. The two count their
    /// index argument from different ends; see the any-angle override's doc comment.
    pub fn reposition_line(
        &mut self,
        board: &mut Board,
        lines: &[Line],
        no: usize,
    ) -> Option<Line> {
        match self {
            TraceTightener::Ninety(t) => t.base.reposition_line(board, lines, no),
            TraceTightener::FortyFive(t) => t.base.reposition_line(board, lines, no),
            TraceTightener::AnyAngle(t) => t.reposition_line(board, lines, no),
        }
    }

    /// Port of the package-private `skipSegmentsOfLength0(Polyline)`
    /// (TraceTightener.java:338-399), which no subclass overrides.
    pub fn skip_segments_of_length_0(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        self.base_mut().skip_segments_of_length_0(board, polyline)
    }

    /// Port of `splitTracesAtKeepPoint()` (TraceTightener.java:476-491).
    pub fn split_traces_at_keep_point(&mut self, board: &mut Board) -> Result<bool, BoardError> {
        self.base_mut().split_traces_at_keep_point(board)
    }

    /// Port of the abstract `smoothenStartCornerAtTrace(PolylineTrace)`
    /// (TraceTightener.java:544) — the regime dispatch.
    ///
    /// # The instance must be primed first
    ///
    /// Both overrides read `currentLayer`, `currentHalfWidth`, `currentNetNumbers` and
    /// `currentClearanceClassIndex`, which **only** `smoothenEndCornersAtTrace:406-409` and the
    /// six-argument `pullTight:182-188` ever write. Called on a fresh instance, Java
    /// dereferences the `null` `currentNetNumbers` inside `BasicBoard.checkTraceShape:1017` and
    /// throws; this port's field is an empty `Vec`, so it would silently check against no nets
    /// at all. Reach these two through
    /// [`smoothen_end_corners_at_trace`](Self::smoothen_end_corners_at_trace), or prime the
    /// instance with [`pull_tight_polyline`](Self::pull_tight_polyline) first — which is what
    /// `P6T15aProbe.smoothen` and `crates/fr-router/tests/tightener.rs` do.
    pub fn smoothen_start_corner_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        match self {
            TraceTightener::Ninety(t) => t.smoothen_start_corner_at_trace(board, trace),
            TraceTightener::FortyFive(t) => t.smoothen_start_corner_at_trace(board, trace),
            TraceTightener::AnyAngle(t) => t.smoothen_start_corner_at_trace(board, trace),
        }
    }

    /// Port of the abstract `smoothenEndCornerAtTrace(PolylineTrace)`
    /// (TraceTightener.java:546) — the regime dispatch. Primed exactly as
    /// [`smoothen_start_corner_at_trace`](Self::smoothen_start_corner_at_trace) documents.
    pub fn smoothen_end_corner_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        match self {
            TraceTightener::Ninety(t) => t.smoothen_end_corner_at_trace(board, trace),
            TraceTightener::FortyFive(t) => t.smoothen_end_corner_at_trace(board, trace),
            TraceTightener::AnyAngle(t) => t.smoothen_end_corner_at_trace(board, trace),
        }
    }

    /// Port of `smoothenEndCornersAtTrace(PolylineTrace)` (TraceTightener.java:402-411):
    /// "smoothens acute angles with contact traces. Returns true, if something was changed."
    pub fn smoothen_end_corners_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Result<bool, BoardError> {
        // :403-405.
        let Some(Item::Trace(polyline_trace)) = board.items.get(&trace) else {
            return Ok(false);
        };
        if !self.base().only_net_no_arr.is_empty()
            && !polyline_trace.hdr.nets_equal(&self.base().only_net_no_arr)
        {
            return Ok(false);
        }
        // :406-409 — the second of the two writers of the `current*` fields the
        // `smoothen*CornerAtTrace` overrides read; see `TraceTightener.smoothenStartCornerAtTrace`'s
        // doc comment for what a call on an unprimed instance does in Java.
        let layer = polyline_trace.get_layer();
        let half_width = polyline_trace.get_half_width();
        let net_numbers = polyline_trace.hdr.net_nos.clone();
        let clearance_class_index = polyline_trace.hdr.clearance_class();
        let base = self.base_mut();
        base.current_layer = layer;
        base.current_half_width = half_width;
        base.current_net_numbers = net_numbers;
        base.current_clearance_class_index = clearance_class_index;
        // :410.
        self.smoothen_end_corners_at_trace_1(board, trace)
    }

    /// Port of the private `smoothenEndCornersAtTrace1(PolylineTrace)`
    /// (TraceTightener.java:414-470).
    ///
    /// Java's `board.removeItem(currentTrace)` appears **twice** — once before the insert
    /// (`:433`) and once after it (`:445`) — and the second call is a no-op on an item already
    /// off the board. Reproduced rather than folded away.
    ///
    /// The early `return true` at `:462` leaves `this.contactPins` at the `null` `:422` set,
    /// never restoring `savedContactPins`; that is Java's control flow and this port keeps it.
    // not ported: the `FRLogger.error` of the `normalizeTraces` catch (:453-459) — ruling 7's
    // degraded value is "the failure is swallowed and the loop carries on", which is what
    // dropping the `Err` does.
    fn smoothen_end_corners_at_trace_1(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Result<bool, BoardError> {
        // :416-418.
        let Some(item) = board.items.get(&trace) else {
            return Ok(false);
        };
        if item.is_shove_fixed(&board.rules) {
            return Ok(false);
        }
        // :419-422: allow the trace to slide to the end point of a contact trace, even if the
        // contact trace ends at a pin.
        let saved_contact_pins = self.base_mut().contact_pins.take();
        let mut result = false;
        let mut connection_to_trace_improved = true;
        let mut current_trace = trace;
        // :426.
        while connection_to_trace_improved {
            connection_to_trace_improved = false;
            // :428.
            let Some(adjusted_polyline) =
                self.smoothen_end_corners_at_trace_2(board, current_trace)
            else {
                continue;
            };
            // :430-441.
            let Some(Item::Trace(t)) = board.items.get(&current_trace) else {
                continue;
            };
            let trace_layer = t.get_layer();
            let current_cl_class = t.hdr.clearance_class();
            let current_fixed_state = t.hdr.get_fixed_state();
            let net_numbers = t.hdr.net_nos.clone();
            let current_half_width = self.base().current_half_width;
            board.remove_item(current_trace);
            let adj_ins_trace = board.insert_trace_without_cleaning(
                adjusted_polyline.clone(),
                trace_layer,
                current_half_width,
                net_numbers,
                current_cl_class,
                current_fixed_state,
            );
            // :442-465.
            if let Some(adj_ins_trace) = adj_ins_trace {
                result = true;
                connection_to_trace_improved = true;
                // :445 — a second removal of an item that is already off the board.
                board.remove_item(current_trace);
                current_trace = adj_ins_trace;
                let net_numbers = match board.items.get(&current_trace) {
                    Some(item) => item.net_nos().to_vec(),
                    None => Vec::new(),
                };
                for current_net_number in net_numbers {
                    let first_corner = adjusted_polyline
                        .first_corner()
                        .expect("an inserted trace has a first corner");
                    let last_corner = adjusted_polyline
                        .last_corner()
                        .expect("an inserted trace has a last corner");
                    let stop = self.base().stop_check();
                    board.split_traces_checked(
                        &first_corner,
                        trace_layer,
                        current_net_number,
                        stop,
                    )?;
                    board.split_traces_checked(
                        &last_corner,
                        trace_layer,
                        current_net_number,
                        stop,
                    )?;
                    // :451-459: `catch (Exception e)` — the failure is logged and dropped.
                    if let Err(fr_board::BoardError::Stopped) =
                        board.normalize_traces_checked(current_net_number, stop)
                    {
                        return Err(fr_board::BoardError::Stopped);
                    }
                    // :461-463.
                    if self.base_mut().split_traces_at_keep_point(board)? {
                        return Ok(true);
                    }
                }
            }
        }
        // :468-469.
        self.base_mut().contact_pins = saved_contact_pins;
        Ok(result)
    }

    /// Port of the private `smoothenEndCornersAtTrace2(PolylineTrace)`
    /// (TraceTightener.java:494-514): "smoothens acute angles with contact traces. Returns null,
    /// if something was changed" — the javadoc says the opposite of what the code does.
    fn smoothen_end_corners_at_trace_2(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        // :495-497.
        let on_the_board = board
            .items
            .get(&trace)
            .is_some_and(|item| item.is_on_the_board());
        if !on_the_board {
            return None;
        }
        // :498-508.
        let mut result = self.smoothen_start_corner_at_trace(board, trace);
        let layer = self.base().current_layer;
        match &result {
            None => {
                result = self.smoothen_end_corner_at_trace(board, trace);
                if let Some(end_result) = &result
                    && board.changed_area.is_some()
                {
                    let corner = end_result
                        .corner_approx(end_result.corner_count() - 1)
                        .expect("cornerCount - 1 is a corner index");
                    board.join_changed_area(&corner, layer);
                }
            }
            Some(start_result) => {
                if board.changed_area.is_some() {
                    let corner = start_result
                        .corner_approx(0)
                        .expect("a polyline has a first corner");
                    board.join_changed_area(&corner, layer);
                }
            }
        }
        // :509-512.
        let result = result?;
        self.base_mut().contact_pins = Some(board.touching_pins_at_end_corners(trace));
        Some(
            self.base_mut()
                .skip_segments_of_length_0(board, &result)
                .unwrap_or(result),
        )
    }
}

// =================================================================================================
// The contact loop the two 45-degree and the two any-angle smoothen methods share
// =================================================================================================

/// What `smoothenStartCornerAtTrace`'s / `smoothenEndCornerAtTrace`'s contact loop leaves behind
/// (TraceTightener45.java:463-467 and `:481-520`, and the same four locals in the three sibling
/// methods).
///
/// Java writes these as method locals that the loop overwrites on every *matching* contact, so
/// the **last** match wins; `acuteAngle` and `bend` are sticky and are never reset.
pub(crate) struct ContactScan {
    pub(crate) acute_angle: bool,
    pub(crate) bend: bool,
    pub(crate) other_trace_corner_approx: Option<FloatPoint>,
    pub(crate) other_trace_line: Option<Line>,
    pub(crate) other_prev_trace_line: Option<Line>,
    pub(crate) prev_corner_side: Option<Side>,
}

/// The contact loop of `TraceTightener45.smoothenStartCornerAtTrace:481-520`, which
/// `smoothenEndCornerAtTrace:589-628` and both `TraceTightenerAnyAngle` overrides repeat
/// verbatim apart from two guards:
///
/// * `require_orthogonal` — the 45-degree regime additionally demands
///   `currentOtherTraceLine.direction().isOrthogonal()` before it calls the angle acute
///   (TraceTightener45.java:501-504); the any-angle regime does not
///   (TraceTightenerAnyAngle.java:808-811).
/// * `require_contact_corner_count_gt_2` — `TraceTightenerAnyAngle.smoothenEndCornerAtTrace:914`
///   wraps its whole body in `if (contactTracePolyline.cornerCount() > 2)`, so a two-corner
///   contact trace is skipped there rather than returning `null`. Its three siblings have no
///   such test.
///
/// `None` is Java's `return null` from the enclosing method, which the `else` arm at `:517-519`
/// takes for any contact that is not a non-shove-fixed `PolylineTrace`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scan_contacts(
    board: &Board,
    contacts: &BTreeSet<ItemId>,
    trace_polyline: &Polyline,
    current_end_corner: &Point,
    current_prev_end_corner: &Point,
    line_direction: &IntDirection,
    prev_line_direction: &IntDirection,
    require_orthogonal: bool,
    require_contact_corner_count_gt_2: bool,
) -> Option<ContactScan> {
    let mut scan = ContactScan {
        acute_angle: false,
        bend: false,
        other_trace_corner_approx: None,
        other_trace_line: None,
        other_prev_trace_line: None,
        prev_corner_side: None,
    };
    for current_contact in contacts {
        // :482 and :517-519.
        let item = board.items.get(current_contact)?;
        let Item::Trace(contact_trace) = item else {
            return None;
        };
        if item.is_shove_fixed(&board.rules) {
            return None;
        }
        let contact_trace_polyline = contact_trace.polyline().clone();
        // TraceTightenerAnyAngle.java:914.
        if require_contact_corner_count_gt_2 && contact_trace_polyline.corner_count() <= 2 {
            continue;
        }
        // :487-496.
        let (
            current_other_trace_corner_approx,
            current_other_trace_line,
            current_other_prev_trace_line,
        ) = if contact_trace_polyline.first_corner().as_ref() == Some(current_end_corner) {
            (
                contact_trace_polyline
                    .corner_approx(1)
                    .expect("a polyline has a second corner"),
                contact_trace_polyline.lines()[1],
                contact_trace_polyline.lines()[2],
            )
        } else {
            let current_corner_no = contact_trace_polyline.corner_count() - 2;
            (
                contact_trace_polyline
                    .corner_approx(current_corner_no)
                    .expect("cornerCount - 2 is a corner index"),
                contact_trace_polyline.lines()[current_corner_no + 1].opposite(),
                contact_trace_polyline.lines()[current_corner_no],
            )
        };
        // :497-498.
        let current_prev_corner_side =
            current_prev_end_corner.side_of_line(&current_other_trace_line);
        let current_projection = line_direction.projection(&current_other_trace_line.direction());
        let mut other_trace_found = false;
        // :500-510.
        if current_projection == Signum::Positive && current_prev_corner_side != Side::Collinear {
            if !require_orthogonal || current_other_trace_line.direction().is_orthogonal() {
                scan.acute_angle = true;
                other_trace_found = true;
            }
        } else if current_projection == Signum::Zero
            && trace_polyline.corner_count() > 2
            && prev_line_direction.projection(&current_other_trace_line.direction())
                == Signum::Positive
        {
            scan.bend = true;
            other_trace_found = true;
        }
        // :511-516.
        if other_trace_found {
            scan.other_trace_corner_approx = Some(current_other_trace_corner_approx);
            scan.other_trace_line = Some(current_other_trace_line);
            scan.prev_corner_side = Some(current_prev_corner_side);
            scan.other_prev_trace_line = Some(current_other_prev_trace_line);
        }
    }
    Some(scan)
}

// =================================================================================================
// `PolylineTrace.pullTight`
// =================================================================================================

/// The two `PolylineTrace.pullTight` overloads (`board/trace/PolylineTrace.java:809-863` and
/// `:869-890`) and `Trace.pullTight`'s abstract declaration (`Trace.java:483`).
///
/// They live in `fr-router` rather than in `fr-board` because their single argument is a
/// [`TraceTightener`], which is this crate's type (plan-6 ruling 3 / plan-rulings #4); `fr-board`
/// keeps a `renamed:` marker pointing here.
pub trait PolylineTraceExt {
    /// Port of `PolylineTrace.pullTight(TraceTightener)` (PolylineTrace.java:809-863): "tries to
    /// shorten this trace without creating clearance violations. Returns true, if the trace was
    /// changed."
    ///
    /// The `None`-engine wrapper over [`Self::pull_tight_with_engine`]: equivalent to Java on a
    /// board whose `autorouteEngine` field is `null`, which is every board outside a live
    /// `autorouteConnection`.
    fn pull_tight_with(board: &mut Board, trace: ItemId, algo: &mut TraceTightener<'_>) -> bool;

    /// [`Self::pull_tight_with`] with Java's `RoutingBoard.autorouteEngine` (RoutingBoard.java:70)
    /// passed in, so the `PolylineTrace.change` this method performs can run
    /// `board.additionalUpdateAfterChange(this)` (PolylineTrace.java:944) — see the module docs.
    fn pull_tight_with_engine(
        board: &mut Board,
        trace: ItemId,
        algo: &mut TraceTightener<'_>,
        engine: Option<&mut AutorouteEngine>,
    ) -> bool;

    /// Port of `PolylineTrace.pullTight(boolean, int, Stoppable)` (PolylineTrace.java:869-890):
    /// "tries to pull this trace tight without creating clearance violations. Returns true, if
    /// the trace was changed."
    fn pull_tight(
        board: &mut Board,
        trace: ItemId,
        own_net_only: bool,
        pull_tight_accuracy: i32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError>;
}

impl PolylineTraceExt for Board {
    fn pull_tight_with(board: &mut Board, trace: ItemId, algo: &mut TraceTightener<'_>) -> bool {
        <Board as PolylineTraceExt>::pull_tight_with_engine(board, trace, algo, None)
    }

    fn pull_tight_with_engine(
        board: &mut Board,
        trace: ItemId,
        algo: &mut TraceTightener<'_>,
        engine: Option<&mut AutorouteEngine>,
    ) -> bool {
        // :811-820.
        let Some(item) = board.items.get(&trace) else {
            return false;
        };
        if !item.is_on_the_board() {
            // This trace may have been deleted in a trace split, for example.
            return false;
        }
        if item.is_shove_fixed(&board.rules) {
            return false;
        }
        if !item.nets_normal() {
            return false;
        }
        // :821-823.
        if !algo.base().only_net_no_arr.is_empty()
            && !item.nets_equal_to(&algo.base().only_net_no_arr)
        {
            return false;
        }
        // :824-828.
        let net_numbers = item.net_nos().to_vec();
        if let Some(first_net) = net_numbers.first() {
            // Java's `nets.get(netNumbers[0])` answers `null` for a number past the end of the
            // net list and then throws at `.getNetClass()`. Unreachable from here: `is_shove_fixed`
            // three lines up already walked the same list and `expect`ed on it (Trace.java:244-250).
            let net = board.rules.nets.get(*first_net).expect(
                "PolylineTrace.pullTight:825: nets.get(netNumbers[0]) is null — Java throws a \
                 NullPointerException at .getNetClass()",
            );
            let net_class = net.get_net_class();
            if !board.rules.net_classes.get(net_class).get_pull_tight() {
                return false;
            }
        }
        // :829-836.
        let Some(Item::Trace(polyline_trace)) = board.items.get(&trace) else {
            return false;
        };
        let lines = polyline_trace.polyline().clone();
        let layer = polyline_trace.get_layer();
        let half_width = polyline_trace.get_half_width();
        let clearance_class_index = polyline_trace.hdr.clearance_class();
        let contact_pins = board.touching_pins_at_end_corners(trace);
        let new_lines = algo.pull_tight_polyline_opt(
            board,
            &lines,
            layer,
            half_width,
            &net_numbers,
            clearance_class_index,
            Some(contact_pins),
        );
        // :837-840 — a **reference** comparison in Java.
        if let Some(new_lines) = new_lines {
            // `PolylineTrace.change:938-944`: a trace that is not on the board just has its
            // polyline replaced (`fr-board`'s `change_trace` reproduces that first test); every
            // other one runs `board.additionalUpdateAfterChange(this)` **before** the search-tree
            // update. `fr-board` cannot make that call — it needs an `AutorouteEngine` — so it
            // carries a `// added in Plan 6:` marker and the call is made here, in Java's order.
            if let Some(engine) = engine
                && board.items.get(&trace).is_some_and(Item::is_on_the_board)
            {
                board.additional_update_after_change(engine, trace);
            }
            board.change_trace(trace, new_lines);
            return true;
        }
        // :841-861.
        let angle_restriction = board.rules.trace_angle_restriction;
        if angle_restriction != AngleRestriction::NinetyDegree
            && board.rules.get_pin_edge_to_turn_dist() > 0.0
        {
            // added in Plan 7: `PolylineTrace.swapConnectionToPin` (PolylineTrace.java:1252-1313) and `PolylineTrace.correctConnectionToPin` (:1082-1245), the four calls of :844-860 — both carry their own `// added in Plan 7:` marker in `fr-board`'s `items/trace.rs`, and neither is reachable from Plan 6's insertion path: `FoundConnectionInserter.insertTrace:140-141` sets `pinEdgeToTurnDist` to `-1` for the whole insert and restores it at `:447`, so this whole branch is skipped there.
            return false;
        }
        // :862.
        false
    }

    fn pull_tight(
        board: &mut Board,
        trace: ItemId,
        own_net_only: bool,
        pull_tight_accuracy: i32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        // :870-872: `this.board instanceof RoutingBoard` — the port has one `Board` type, so the
        // test is always true. Recorded rather than dropped.
        // :873-878.
        let opt_net_no_arr = if own_net_only {
            match board.items.get(&trace) {
                Some(item) => item.net_nos().to_vec(),
                None => return Ok(false),
            }
        } else {
            Vec::new()
        };
        // :879-888.
        let mut pull_tight_algo = TraceTightener::get_instance(
            board,
            opt_net_no_arr,
            None,
            pull_tight_accuracy,
            Some(stop),
            -1,
            None,
            -1,
        );
        // :889. Java's `this.board` here is a `RoutingBoard` whose `autorouteEngine` this entry
        // point cannot name; its callers are Plan 7's batch optimizer, which runs outside
        // `autorouteConnection`.
        Ok(<Board as PolylineTraceExt>::pull_tight_with(
            board,
            trace,
            &mut pull_tight_algo,
        ))
    }
}
