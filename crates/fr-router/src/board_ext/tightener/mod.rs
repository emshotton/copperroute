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

// `TraceTightener.optChangedArea` (TraceTightener.java:121-169) — the batch entry point that
// walks `board.changedArea` layer by layer — landed in **Plan 7 Task 5** as
// [`TraceTightener::opt_changed_area`], together with the two `PolylineTrace` `ConnectionToPin`
// methods `PolylineTrace.pullTight:841-861` drives. Controller ruling AB kept it, `ViaOptimizer`
// and `removeItemsAndPullTight` in Plan 7; the four methods it drives all landed here in Task 15a.

mod base;
mod tightener_45;
mod tightener_90;
mod tightener_any_angle;

use std::collections::BTreeSet;

use fr_board::datastructures::StopCheck;
use fr_board::prelude::*;
use fr_geometry::{
    Direction, FloatPoint, IntDirection, IntOctagon, Line, Point, Polyline, Shape, Side, Signum,
    TileShape, java_max,
};
use fr_settings::ExpansionCostFactor;

use crate::autoroute::maze::engine::AutorouteEngine;
use crate::board_ext::{RoutingBoardExt, ViaOptimizer};

// ---- Plan 7 Task 8b: the level-7 `optChangedArea` sweep ledger ---------------------------------
//
// Instrumentation, not behaviour: the function answers `false` unless `P7T8B_OCA` is set in the
// environment, it is read once into a `LazyLock`, and its only callers are the three `eprintln!`
// blocks in [`TraceTightener::opt_changed_area`]. The Java side of the pair is the `OCA` /
// `OCAPT` / `OCASM` / `OCAVIA` marker set `scripts/differential/java/p6t17b-bisect.patch` adds to
// `TraceTightener.optChangedArea`. Both write to **stderr**. Quirk #210.
fn p7t8b_oca_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T8B_OCA").is_some());
    *ON
}
// ---- end Plan 7 Task 8b ------------------------------------------------------------------------

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

    /// Port of `TraceTightener.optChangedArea(ExpansionCostFactor[])` (TraceTightener.java:121-169):
    /// "function for optimizing the route in an internal marked area. If clipShape != null, the
    /// optimizing area is restricted to clipShape. traceCosts is used for optimizing vias and may
    /// be null."
    ///
    /// The class's last unported method, and the batch entry point every routed connection, every
    /// tail removal and every fanout pin reaches through
    /// [`RoutingBoardExt::opt_changed_area`].
    ///
    /// # The two trace arms break on different conditions
    ///
    /// Both trace arms `break` out of the item loop — the `pullTight` arm only when
    /// `splitTracesAtKeepPoint()` answers `true` (`:153-155`), the `smoothenEndCornersAtTrace` arm
    /// unconditionally (`:156-159`, "because items may be removed"). So a *smoothened* trace ends
    /// that layer's item walk for this pass while a merely *tightened* one does not, unless a keep
    /// point split fired. That asymmetry is Java's and it is not obvious; the plan's prose folded
    /// the two into one unconditional `break`, and this port follows the source.
    ///
    /// # The stop check is the budget
    ///
    /// `:147-149` is the only cut in the sweep, and `isStopRequested` (`:195-212`) reads both the
    /// `Stoppable` and the `TimeLimit` built from `getInstance`'s `timeLimit` argument — which is
    /// controller ruling AI's knob, `RouterBudget::opt_changed_area_ms`, defaulting to Java's own
    /// literal `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000`. A trip returns with the rest of the
    /// board untightened and `board.changedArea` already emptied for every layer walked so far —
    /// the parity hazard `crates/fr-router/tests/opt_changed_area.rs` pins as a fact.
    // not ported: `board.joinGraphicsUpdateBox(changedRegion.boundingBox())` (`:137`) — the GUI
    // repaint box; `Board` has no `joinGraphicsUpdateBox` and nothing headless reads it.
    pub fn opt_changed_area(
        &mut self,
        board: &mut Board,
        mut engine: Option<&mut AutorouteEngine>,
        trace_costs: Option<&[ExpansionCostFactor]>,
    ) -> Result<(), BoardError> {
        // :122-124.
        if board.changed_area.is_none() {
            return Ok(());
        }
        // :126-128: "starting with curr_min_translate_dist big is a try to avoid fine
        // approximation at the beginning to avoid problems with dog ears" — Java's comment; the
        // code it describes is gone, only the flag remains.
        let mut something_changed = true;
        // :129.
        while something_changed {
            something_changed = false;
            // :131.
            for i in 0..board.get_layer_count() {
                // :132-135.
                let Some(changed_area) = &board.changed_area else {
                    // Unreachable in Java: `optChangedArea` never nulls the field, and its one
                    // caller nulls it only after this method returns
                    // (RoutingBoardOperations.java:78). Reached here only if a callee did.
                    return Ok(());
                };
                let changed_region = changed_area.get_area(i);
                if changed_region.is_empty() {
                    continue;
                }
                // :136 — emptied **before** the work, so a trace this sweep changes re-marks the
                // area it moved into and the outer `while` sees it on the next pass.
                if let Some(changed_area) = &mut board.changed_area {
                    changed_area.set_empty(i);
                }
                // :138-141. Java's `clearanceMatrix.maxValue(i) + 2 * rules.getMaxTraceHalfWidth()`
                // is `int` arithmetic before the promotion to `double`, and this reproduces it —
                // including the operand order, which decides the rounding. Java would *wrap* on
                // overflow where a debug build panics; unreachable at any realistic clearance and
                // half width (both are board-rule values in the thousands), and recorded here
                // because the port's convention is to say so at such sites rather than to widen
                // silently.
                let changed_area_offset = 1.5
                    * f64::from(
                        board.rules.clearance_matrix.max_value_on_layer(i)
                            + 2 * board.rules.get_max_trace_half_width(),
                    );
                // :142.
                let changed_region = changed_region.enlarge(changed_area_offset);
                // :145 — a **mixed** room/item set: `TreeObject`'s `Ord` is Java's
                // `Item.compareTo`/`CompleteFreeSpaceExpansionRoom.compareTo` pair, so the
                // forward walk of this `BTreeSet` is Java's `TreeSet` iteration order (rooms
                // first, then items, both by descending id).
                let items = board.overlapping_objects(&TileShape::Octagon(changed_region), Some(i));
                // Plan 7 Task 8b's level-7 ledger — `OCA`, the twin of the marker
                // `scripts/differential/java/p6t17b-bisect.patch` adds to
                // `TraceTightener.optChangedArea:145`. Off unless `P7T8B_OCA` is set; stderr only.
                if p7t8b_oca_ledger() {
                    let objs = items
                        .iter()
                        .map(|o| match o {
                            TreeObject::Item(id) => id.0.to_string(),
                            _ => "R".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    eprintln!(
                        "OCA layer={i} region=({},{},{},{},{},{},{},{}) n={} objs=[{objs}]",
                        changed_region.left_x,
                        changed_region.bottom_y,
                        changed_region.right_x,
                        changed_region.top_y,
                        changed_region.upper_left_diagonal_x,
                        changed_region.lower_right_diagonal_x,
                        changed_region.lower_left_diagonal_x,
                        changed_region.upper_right_diagonal_x,
                        items.len(),
                    );
                }
                // :146.
                for current_object in items {
                    // :147-149.
                    if self.base().is_stop_requested() {
                        return Ok(());
                    }
                    match current_object {
                        // :150-159.
                        TreeObject::Item(item_id)
                            if matches!(board.items.get(&item_id), Some(Item::Trace(_))) =>
                        {
                            // :151.
                            let pulled = <Board as PolylineTraceExt>::pull_tight_with_engine(
                                board,
                                item_id,
                                self,
                                engine.as_deref_mut(),
                            );
                            if p7t8b_oca_ledger() {
                                eprintln!("OCAPT id={} res={pulled}", item_id.0);
                            }
                            if pulled {
                                // :152-155.
                                something_changed = true;
                                if self.split_traces_at_keep_point(board)? {
                                    break;
                                }
                            } else {
                                let smoothed =
                                    self.smoothen_end_corners_at_trace(board, item_id)?;
                                if p7t8b_oca_ledger() {
                                    eprintln!("OCASM id={} res={smoothed}", item_id.0);
                                }
                                if smoothed {
                                    // :156-158 — "because items may be removed".
                                    something_changed = true;
                                    break;
                                }
                            }
                        }
                        // :160-165.
                        TreeObject::Item(via_id)
                            if trace_costs.is_some()
                                && matches!(board.items.get(&via_id), Some(Item::Via(_))) =>
                        {
                            // `ViaOptimizer.optViaLocation(this.board, via, traceCosts,
                            // this.minTranslateDist, 10)` (:161-164) — Task 6 landed the entry
                            // point and Task 7 the three `repositionVia` overloads, so the arm is
                            // complete. Note which of Java's two `int` parameters gets
                            // `minTranslateDist`: `tracePullTightAccuracy`, **not** the recursion
                            // depth, which is the literal `10`.
                            //
                            // No `engine` is threaded: `opt_via_location` takes none, because none
                            // of the three things it calls accepts one — see that method's
                            // "No `AutorouteEngine` parameter" section.
                            //
                            // Plan 7 Task 6's `obligation: ViaOptimizer.repositionVia — Task 7` is
                            // **discharged**: `p7t3` mode 4 (vias offered to the optimiser) is 0
                            // diffs against the HEAD jar on all three boards, and
                            // `tests/opt_changed_area.rs`'s
                            // `the_whole_sweep_matches_the_jvm_on_a_real_board` replays it.
                            let unchanged_trace_costs = trace_costs.expect("just matched");
                            let moved = ViaOptimizer::opt_via_location(
                                board,
                                via_id,
                                Some(unchanged_trace_costs),
                                self.base().min_translate_dist,
                                10,
                            )?;
                            if p7t8b_oca_ledger() {
                                eprintln!("OCAVIA id={} res={moved}", via_id.0);
                            }
                            if moved {
                                something_changed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Port of the abstract `pullTight(Polyline)` (TraceTightener.java:192) — the regime
    /// dispatch — answering `None` where Java hands the **argument object** back, which is what
    /// `PolylineTrace.pullTight:837`'s `newLines != lines` reads.
    ///
    /// Java's `:192` has exactly one caller, `TraceTightener.java:189` inside the six-argument
    /// overload; here that is [`Self::pull_tight_polyline`], and it needs the `None`. A second,
    /// `Polyline`-returning wrapper existed here until the Plan 6 final review (finding S7): it
    /// collapsed `None` to `polyline.clone()`, had no caller in the workspace, and would have
    /// silently discarded exactly the reference identity quirk #74 depends on.
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
        let mut p7t8b_iter = 0;
        while connection_to_trace_improved {
            connection_to_trace_improved = false;
            // :428.
            let adjusted = self.smoothen_end_corners_at_trace_2(board, current_trace);
            // Plan 7 Task 8b's level-7 ledger — `SM1`. Off unless `P7T8B_OCA` is set.
            if p7t8b_oca_ledger() {
                p7t8b_iter += 1;
                eprintln!(
                    "SM1 iter={p7t8b_iter} trace={} adj={}",
                    current_trace.0,
                    adjusted
                        .as_ref()
                        .map_or_else(|| "none".to_string(), |p| p.corner_count().to_string())
                );
            }
            let Some(adjusted_polyline) = adjusted else {
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
                    if p7t8b_oca_ledger() {
                        eprintln!(
                            "SMSPL which=first pt={first_corner:?} layer={trace_layer} \
                             net={current_net_number}"
                        );
                    }
                    board.split_traces_checked(
                        &first_corner,
                        trace_layer,
                        current_net_number,
                        stop,
                    )?;
                    if p7t8b_oca_ledger() {
                        eprintln!(
                            "SMSPL which=last pt={last_corner:?} layer={trace_layer} \
                             net={current_net_number}"
                        );
                    }
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
///
/// # The walk is `.rev()`ed, and that is load-bearing
///
/// `trace.getStartContacts()` / `getEndContacts()` answer `Trace.getNormalContacts`'s
/// `new TreeSet<>()` (Trace.java:179) under `Item.compareTo == item.id - id`
/// (Item.java:95-102), so Java walks a contact set in **descending** item id — quirk #44's
/// ordering, the same one `Board::change_trace`'s callers and `Item.isCycle`'s roots need. The
/// port's [`BTreeSet<ItemId>`](std::collections::BTreeSet) is ascending, so this walk is
/// `.rev()`ed like every other `TreeSet<Item>` walk in the workspace.
///
/// It is not cosmetic here: `TraceTightener45.java:511-515` **overwrites**
/// `otherTraceCornerApprox`, `otherTraceLine`, `prevCornerSide` and `otherPrevTraceLine` on
/// every contact that matches, so the *last* match wins, and with two or more matching contacts
/// the direction of the walk picks a different one. That choice sets `newLineDir` (`:523-527`),
/// the `translateLine` built from it (`:528`) and the `addLine` the smoothed polyline starts
/// with (`:545`) — a different corner, a different `splitTraces` point, a different number of
/// smoothing iterations. Quirk **#210** records the measurement: without the `.rev()`,
/// `router-dac2020-bm01` at `ripupPassNo = 1` diverges from the jar at connection 175 of 294
/// (Plan 7 Task 8b bisected it there); with it, all 294 MATCH at both passes.
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
    // Plan 7 Task 8b's level-7 ledger — `SSC`. Off unless `P7T8B_OCA` is set; stderr only.
    if p7t8b_oca_ledger() {
        eprintln!(
            "SSC c0={current_end_corner:?} c1={current_prev_end_corner:?} \
             ldir={line_direction:?} pldir={prev_line_direction:?} contacts=[{}]",
            contacts
                .iter()
                .map(|c| c.0.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    // :481 — **descending**, see this function's doc comment and quirk #210.
    for current_contact in contacts.iter().rev() {
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
        if p7t8b_oca_ledger() {
            eprintln!(
                "SSCC id={} otl={} optl={} approx={:?} side={:?} proj={:?} orth={} \
                 acute={} bend={} found={other_trace_found}",
                current_contact.0,
                p7t8b_line(&current_other_trace_line),
                p7t8b_line(&current_other_prev_trace_line),
                current_other_trace_corner_approx,
                current_prev_corner_side,
                current_projection,
                current_other_trace_line.direction().is_orthogonal(),
                scan.acute_angle,
                scan.bend,
            );
        }
        // :511-516.
        if other_trace_found {
            scan.other_trace_corner_approx = Some(current_other_trace_corner_approx);
            scan.other_trace_line = Some(current_other_trace_line);
            scan.prev_corner_side = Some(current_prev_corner_side);
            scan.other_prev_trace_line = Some(current_other_prev_trace_line);
        }
    }
    if p7t8b_oca_ledger() {
        eprintln!(
            "SSCF acute={} bend={} otl={} side={:?} approx={:?}",
            scan.acute_angle,
            scan.bend,
            scan.other_trace_line
                .map_or_else(|| "null".to_string(), |l| p7t8b_line(&l)),
            scan.prev_corner_side,
            scan.other_trace_corner_approx,
        );
    }
    Some(scan)
}

/// `Line.a + "-" + Line.b` — the shape `TraceTightener45`'s Java `p7pts` helper prints.
pub(crate) fn p7t8b_line(l: &Line) -> String {
    format!("({},{})-({},{})", l.a.x, l.a.y, l.b.x, l.b.y)
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

    /// Port of `PolylineTrace.checkConnectionToPin(boolean)` (PolylineTrace.java:1013-1076) and
    /// the abstract `Trace.checkConnectionToPin` (`board/model/items/Trace.java:376`) it
    /// overrides: "checks that the connection restrictions to the contact pins are satisfied. If
    /// atStart, the start of this trace is checked, else the end. Returns false if a pin is at
    /// that end where the connection is checked and the connection is not ok."
    ///
    /// # Not already ported
    ///
    /// The plan's scan ruling 5 records this method as landed in Plan 6 and instructs Task 5 to
    /// reuse it. It had not: a workspace-wide search for `checkConnectionToPin`, for
    /// `TraceExitRestriction` in `fr-router` and for any `preserveLength`-shaped body found only
    /// the two deferral markers in `crates/fr-board/src/items/trace.rs`. Java wins over
    /// the plan text, so the 64 lines are transcribed here with the pair that needs them —
    /// `correctConnectionToPin`'s first statement is a call to this method (`:1083`) and it is
    /// unimplementable without it.
    fn check_connection_to_pin(board: &Board, trace: ItemId, at_start: bool) -> bool;

    /// Port of `PolylineTrace.correctConnectionToPin(boolean, AngleRestriction)`
    /// (PolylineTrace.java:1082-1245): "tries to correct a connection restriction of this trace.
    /// If atStart, the start of the trace polygon is corrected, else the end. Returns true, if
    /// this trace was changed."
    ///
    /// The acid-trap correction: it walks the polygon around the border of the offset pin shape
    /// from the trace's latest entrance point to the nearest legal pin exit ray, replaces the
    /// trace's head with that polygon and inserts a `SHOVE_FIXED` exit stub.
    ///
    /// The second argument is Java's `AngleRestriction`, **not** an accuracy (the plan's first
    /// draft typed it `int accuracy`); `pullTight:853`/`:857` pass the board's own
    /// `angleRestriction` local, which is what selects the bounding box / bounding octagon
    /// rounding of `:1201-1205`.
    fn correct_connection_to_pin(
        board: &mut Board,
        engine: Option<&mut AutorouteEngine>,
        trace: ItemId,
        at_start: bool,
        angle_restriction: AngleRestriction,
    ) -> Result<bool, BoardError>;

    /// Port of `PolylineTrace.swapConnectionToPin(boolean)` (PolylineTrace.java:1252-1313):
    /// "looks, if another pin connection restriction fits better than the current connection
    /// restriction and changes this trace in this case. If atStart, the start of the trace polygon
    /// is changed, else the end. Returns true, if this trace was changed."
    ///
    /// It never edits a polyline itself: the whole effect is `contactTrace.setFixedState(...)`
    /// followed by `this.combine()`, i.e. the `SHOVE_FIXED` exit stub `correctConnectionToPin`
    /// left behind is released and swallowed into this trace, so the next `pullTight` may route
    /// the pin exit a different way.
    ///
    /// `combine()`'s loop is transcribed in the body rather than delegated to
    /// [`Board::combine_trace`], because `:188`'s `board.additionalUpdateAfterChange(this)` runs
    /// **inside** it — once per merge, after the merge, never when nothing merges. See the
    /// comment at that site. `engine` is `None` on every path that has no live
    /// `autorouteConnection`, which is what both differential drivers pass.
    fn swap_connection_to_pin(
        board: &mut Board,
        engine: Option<&mut AutorouteEngine>,
        trace: ItemId,
        at_start: bool,
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
        mut engine: Option<&mut AutorouteEngine>,
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
            // update. `fr-board` cannot make that call — it needs an `AutorouteEngine` — so
            // `Board::change_trace` carries an `obligation:` marker
            // (`crates/fr-board/src/board/trace_normalize.rs:994-1001`) recording that the call is
            // made **at the call site**; this is that call site, and the order is Java's.
            if let Some(engine) = engine.as_deref_mut()
                && board.items.get(&trace).is_some_and(Item::is_on_the_board)
            {
                board.additional_update_after_change(engine, trace);
            }
            board.change_trace(trace, new_lines);
            return true;
        }
        // :841-861. Plan 6 could not reach this branch and left it a bare `return false`:
        // `FoundConnectionInserter.insertTrace:140-141` sets `pinEdgeToTurnDist` to `-1` for the
        // whole insert and restores it at `:447`. `optChangedArea` calls `pullTight` **outside**
        // that window, so Plan 7 Task 5 wires the four calls up.
        let angle_restriction = board.rules.trace_angle_restriction;
        if angle_restriction != AngleRestriction::NinetyDegree
            && board.rules.get_pin_edge_to_turn_dist() > 0.0
        {
            // Ruling 7's degraded value for the `Err` arm: the port's `BoardError` is its own
            // normalisation channel — Java has none on this path and cannot fail — and both
            // methods only reach their fallible statement **after** the board mutation that makes
            // Java answer `true`, so an `Err` continues exactly where Java continues.
            //
            // :844-846.
            if <Board as PolylineTraceExt>::swap_connection_to_pin(
                board,
                engine.as_deref_mut(),
                trace,
                true,
            )
            .unwrap_or(true)
            {
                // The recursion's own answer is discarded, as Java discards it.
                <Board as PolylineTraceExt>::pull_tight_with_engine(
                    board,
                    trace,
                    algo,
                    engine.as_deref_mut(),
                );
                return true;
            }
            // :848-850.
            if <Board as PolylineTraceExt>::swap_connection_to_pin(
                board,
                engine.as_deref_mut(),
                trace,
                false,
            )
            .unwrap_or(true)
            {
                <Board as PolylineTraceExt>::pull_tight_with_engine(
                    board,
                    trace,
                    algo,
                    engine.as_deref_mut(),
                );
                return true;
            }
            // :852-856: "optimize algorithm could not improve the trace, try to remove acid
            // traps".
            if <Board as PolylineTraceExt>::correct_connection_to_pin(
                board,
                engine.as_deref_mut(),
                trace,
                true,
                angle_restriction,
            )
            .unwrap_or(true)
            {
                <Board as PolylineTraceExt>::pull_tight_with_engine(
                    board,
                    trace,
                    algo,
                    engine.as_deref_mut(),
                );
                return true;
            }
            // :857-860.
            if <Board as PolylineTraceExt>::correct_connection_to_pin(
                board,
                engine.as_deref_mut(),
                trace,
                false,
                angle_restriction,
            )
            .unwrap_or(true)
            {
                <Board as PolylineTraceExt>::pull_tight_with_engine(board, trace, algo, engine);
                return true;
            }
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

    fn check_connection_to_pin(board: &Board, trace: ItemId, at_start: bool) -> bool {
        // :1014-1016. `this.board == null` cannot happen in the port — an `ItemId` is only
        // meaningful against the board it was drawn from — and a trace that is no longer in the
        // item map is the closest thing to it, so it answers `true` the same way.
        let Some(Item::Trace(polyline_trace)) = board.items.get(&trace) else {
            return true;
        };
        // :1017-1019.
        if polyline_trace.corner_count() < 2 {
            return true;
        }
        let layer = polyline_trace.get_layer();
        let half_width = polyline_trace.get_half_width();
        let clearance_class_index = polyline_trace.hdr.clearance_class();
        let trace_polyline = polyline_trace.polyline().clone();
        // :1020-1025.
        let contact_list = if at_start {
            board.trace_start_contacts(trace)
        } else {
            board.trace_end_contacts(trace)
        };
        // :1026-1032. Java's `TreeSet<Item>` is **descending** id (`Item.compareTo`,
        // Item.java:95-101), so the walk is `.rev()` and the pin picked at a corner touching two
        // of them is the higher-numbered one.
        let Some(contact_pin_id) = contact_list
            .into_iter()
            .rev()
            .find(|id| matches!(board.items.get(id), Some(Item::Pin(_))))
        else {
            // :1033-1035.
            return true;
        };
        let Some(Item::Pin(contact_pin)) = board.items.get(&contact_pin_id) else {
            unreachable!("just matched")
        };
        let pin_clearance_class_index = contact_pin.hdr.clearance_class();
        // :1036-1041.
        let ctx = board.ctx();
        let trace_exit_restrictions = contact_pin.get_trace_exit_restrictions(layer, &ctx);
        if trace_exit_restrictions.is_empty() {
            return true;
        }
        // :1042-1051.
        let (end_corner, prev_end_corner) = if at_start {
            (trace_polyline.first_corner(), trace_polyline.corner(1))
        } else {
            (
                trace_polyline.last_corner(),
                trace_polyline.corner(trace_polyline.corner_count() - 2),
            )
        };
        let (Some(end_corner), Some(prev_end_corner)) = (end_corner, prev_end_corner) else {
            // Java would throw here rather than answer; the corner count check of `:1017` has
            // already ruled out the only polyline that could reach it.
            return true;
        };
        // :1052-1055.
        let Some(trace_end_direction) = Direction::between(&end_corner, &prev_end_corner) else {
            return true;
        };
        // :1056-1062. `Direction.equals` is `compareTo(other) == 0`, i.e. the geometric
        // comparison, which is what `PartialEq` on this port's `Direction` is.
        let Some(matching_exit_restriction) = trace_exit_restrictions
            .iter()
            .find(|restriction| restriction.direction == trace_end_direction)
        else {
            // :1063-1065.
            return false;
        };
        // :1066-1069. Quirk #205: `< 0`, not `<= 0`, while the only caller of this method's own
        // only caller demands `> 0` (`pullTight:842`) — so the `edgeToTurnDist == 0` band is dead
        // acceptance. Reproduced as written.
        let edge_to_turn_dist = board.rules.get_pin_edge_to_turn_dist();
        if edge_to_turn_dist < 0.0 {
            return false;
        }
        // :1070.
        let end_line_length = end_corner.to_float().distance(&prev_end_corner.to_float());
        // :1071-1073.
        let current_clearance = f64::from(board.clearance_value(
            clearance_class_index,
            pin_clearance_class_index,
            layer,
        ));
        // :1074-1076.
        let add_width = java_max(edge_to_turn_dist, current_clearance + 1.0);
        let preserve_length =
            matching_exit_restriction.min_length + f64::from(half_width) + add_width;
        preserve_length <= end_line_length
    }

    fn correct_connection_to_pin(
        board: &mut Board,
        engine: Option<&mut AutorouteEngine>,
        trace: ItemId,
        at_start: bool,
        angle_restriction: AngleRestriction,
    ) -> Result<bool, BoardError> {
        // :1083-1085.
        if <Board as PolylineTraceExt>::check_connection_to_pin(board, trace, at_start) {
            return Ok(false);
        }
        let Some(Item::Trace(polyline_trace)) = board.items.get(&trace) else {
            return Ok(false);
        };
        // Java reads these five off `this` throughout, and `this` survives every mutation below
        // — including the `change` at `:1237` that may normalize the trace off the board. Read
        // once, up front, so the port answers from the same values Java's fields hold.
        let layer = polyline_trace.get_layer();
        let half_width = polyline_trace.get_half_width();
        let clearance_class_index = polyline_trace.hdr.clearance_class();
        let net_numbers = polyline_trace.hdr.net_nos.clone();
        // :1087-1096.
        let (trace_polyline, contact_list) = if at_start {
            (
                polyline_trace.polyline().clone(),
                board.trace_start_contacts(trace),
            )
        } else {
            // ruling AE: `Polyline.reverse` rebuilds every line, so the identity tokens of the
            // reversed polyline are new — which is what Java's `new Polyline(Line[])` does too.
            let reversed = match polyline_trace.polyline().reverse() {
                Ok(reversed) => reversed,
                // Java's `reverse()` cannot fail; a `PolylineError` here would be a port-side
                // degeneracy, and refusing the correction is the degraded value ruling 7 asks for.
                Err(_) => return Ok(false),
            };
            (reversed, board.trace_end_contacts(trace))
        };
        // :1097-1103 — `.rev()` for Java's descending `TreeSet<Item>`, as in `check`.
        let Some(contact_pin_id) = contact_list
            .into_iter()
            .rev()
            .find(|id| matches!(board.items.get(id), Some(Item::Pin(_))))
        else {
            // :1104-1106.
            return Ok(false);
        };
        let Some(Item::Pin(contact_pin)) = board.items.get(&contact_pin_id) else {
            unreachable!("just matched")
        };
        let pin_clearance_class_index = contact_pin.hdr.clearance_class();
        let pin_center = contact_pin.get_center(&board.ctx());
        // :1107-1112.
        let ctx = board.ctx();
        let trace_exit_restrictions = contact_pin.get_trace_exit_restrictions(layer, &ctx);
        if trace_exit_restrictions.is_empty() {
            return Ok(false);
        }
        // :1113-1117. The `checked_sub` is Java's negative index: `Trace.getNormalContacts:184`
        // already filtered the contact list by `sharesLayer`, so `firstLayer() <= getLayer()` and
        // the difference cannot go negative — but Java would answer `null` there and refuse
        // (`DrillItem.getShape` range-checks), and a `usize` underflow would panic instead.
        let pin_first_layer = contact_pin.first_layer(&ctx);
        let Some(pad_index) = layer.checked_sub(pin_first_layer) else {
            return Ok(false);
        };
        let Some(Shape::Tile(pin_shape)) = contact_pin.get_shape(pad_index, &ctx) else {
            // Java's `!(pinShape instanceof TileShape)` — a `PolygonShape` pad, or a `null` from
            // an out-of-range layer index.
            return Ok(false);
        };
        // :1118-1121 — the same `< 0` as `checkConnectionToPin:1067`, quirk #205.
        let edge_to_turn_dist = board.rules.get_pin_edge_to_turn_dist();
        if edge_to_turn_dist < 0.0 {
            return Ok(false);
        }
        // :1122-1125.
        let current_clearance = f64::from(board.clearance_value(
            clearance_class_index,
            pin_clearance_class_index,
            layer,
        ));
        // :1126-1128.
        let add_width = java_max(edge_to_turn_dist, current_clearance + 1.0);
        let mut offset_pin_shape = pin_shape.offset(f64::from(half_width) + add_width);
        // :1129-1134.
        if angle_restriction == AngleRestriction::NinetyDegree || offset_pin_shape.is_int_box() {
            offset_pin_shape = TileShape::Box(offset_pin_shape.bounding_box());
        } else if angle_restriction == AngleRestriction::FortyFiveDegree {
            match offset_pin_shape.bounding_octagon() {
                Some(octagon) => offset_pin_shape = TileShape::Octagon(octagon),
                // totalized: `boundingOctagon()` answers `null` for an empty shape and Java then
                // throws at `:1136`'s `entrancePoints`. Nothing to correct against, so refuse.
                None => return Ok(false),
            }
        }
        // :1135-1139.
        let entries = offset_pin_shape.entrance_points(&trace_polyline);
        let Some(latest_entry_tuple) = entries.last().copied() else {
            return Ok(false);
        };
        // :1140-1143.
        let Some(entry_border_line) = offset_pin_shape.border_line(latest_entry_tuple[1]) else {
            return Ok(false);
        };
        let trace_entry_location_approx =
            trace_polyline.lines()[latest_entry_tuple[0]].intersection_approx(&entry_border_line);
        // :1144-1150: calculate the nearest legal pin exit point to traceEntryLocationApprox.
        let mut min_exit_corner_distance = f64::MAX;
        let mut nearest_pin_exit_ray: Option<Line> = None;
        let mut nearest_border_line_no: usize = 0;
        let mut pin_exit_direction: Option<Direction> = None;
        let mut nearest_exit_corner: Option<FloatPoint> = None;
        // :1151.
        let tolerance = 1.0_f64;
        // :1153-1191.
        for current_exit_restriction in &trace_exit_restrictions {
            // :1154-1155.
            let Some(current_intersecting_border_line_no) = offset_pin_shape
                .intersecting_border_line_no(&pin_center, &current_exit_restriction.direction)
            else {
                // Java's `intersectingBorderLineNo` answers `-1` when the centre is outside the
                // shape, and `borderLine(-1)` then throws. Unreachable for a pin's own offset
                // shape; skipping the restriction is the degraded value.
                continue;
            };
            // :1156.
            let Point::Int(pin_center_int) = pin_center else {
                // Java's `new Line(Point, Direction)` only supports an `IntPoint` centre.
                return Ok(false);
            };
            let Some(current_pin_exit_ray) =
                Line::from_direction_any(pin_center_int, &current_exit_restriction.direction)
            else {
                continue;
            };
            // :1157-1160.
            let Some(current_border_line) =
                offset_pin_shape.border_line(current_intersecting_border_line_no)
            else {
                continue;
            };
            let current_exit_corner =
                current_pin_exit_ray.intersection_approx(&current_border_line);
            // :1161.
            let current_exit_corner_distance =
                current_exit_corner.distance_square(&trace_entry_location_approx);
            // :1162-1163.
            let mut new_nearest_corner_found = false;
            if current_exit_corner_distance + tolerance < min_exit_corner_distance {
                // :1164-1166.
                new_nearest_corner_found = true;
            } else if current_exit_corner_distance < min_exit_corner_distance + tolerance {
                // :1167-1182: the distances are near equal, compare to the previous corners of
                // tracePolyline. `nearestExitCorner` is `null` on the first iteration, but this
                // branch cannot be reached there: `minExitCornerDistance` is `Double.MAX_VALUE`,
                // so the `:1164` test above always wins.
                let Some(old_exit_corner) = nearest_exit_corner else {
                    unreachable!(
                        "PolylineTrace.correctConnectionToPin:1174 dereferences a null \
                         nearestExitCorner — unreachable while minExitCornerDistance is \
                         Double.MAX_VALUE"
                    )
                };
                for i in 1..trace_polyline.corner_count() {
                    let Some(current_trace_corner) = trace_polyline.corner_approx(i) else {
                        break;
                    };
                    let current_trace_corner_distance =
                        current_trace_corner.distance_square(&current_exit_corner);
                    let old_trace_corner_distance =
                        current_trace_corner.distance_square(&old_exit_corner);
                    if current_trace_corner_distance + tolerance < old_trace_corner_distance {
                        new_nearest_corner_found = true;
                        break;
                    } else if current_trace_corner_distance > old_trace_corner_distance + tolerance
                    {
                        break;
                    }
                }
            }
            // :1184-1190.
            if new_nearest_corner_found {
                min_exit_corner_distance = current_exit_corner_distance;
                nearest_pin_exit_ray = Some(current_pin_exit_ray);
                nearest_border_line_no = current_intersecting_border_line_no;
                pin_exit_direction = Some(current_exit_restriction.direction.clone());
                nearest_exit_corner = Some(current_exit_corner);
            }
        }
        // Java leaves `nearestPinExitRay` null when no restriction produced a corner and throws
        // at `:1207`. The `trace_exit_restrictions.is_empty()` guard of `:1109` makes that
        // unreachable in Java; the `continue`s above are the port's own, so answer `false`.
        let (Some(nearest_pin_exit_ray), Some(pin_exit_direction)) =
            (nearest_pin_exit_ray, pin_exit_direction)
        else {
            return Ok(false);
        };
        // :1193-1213: append the polygon piece around the border of the pin shape.
        let corner_count = offset_pin_shape.border_line_count();
        // Java's `%` on an `int` sum that cannot go negative here — both differences are taken
        // modulo `cornerCount` after adding it.
        let clock_wise_side_diff =
            (nearest_border_line_no + corner_count - latest_entry_tuple[1]) % corner_count;
        let counter_clock_wise_side_diff =
            (latest_entry_tuple[1] + corner_count - nearest_border_line_no) % corner_count;
        let mut current_border_line_no = nearest_border_line_no;
        let mut current_lines: Vec<Option<Line>>;
        if counter_clock_wise_side_diff <= clock_wise_side_diff {
            current_lines = vec![None; counter_clock_wise_side_diff + 3];
            for i in 0..=counter_clock_wise_side_diff {
                current_lines[i + 1] = offset_pin_shape.border_line(current_border_line_no);
                current_border_line_no = (current_border_line_no + 1) % corner_count;
            }
        } else {
            current_lines = vec![None; clock_wise_side_diff + 3];
            for i in 0..=clock_wise_side_diff {
                current_lines[i + 1] = offset_pin_shape.border_line(current_border_line_no);
                current_border_line_no = (current_border_line_no + corner_count - 1) % corner_count;
            }
        }
        // :1211-1213.
        let current_lines_len = current_lines.len();
        current_lines[0] = Some(nearest_pin_exit_ray);
        current_lines[current_lines_len - 1] = Some(trace_polyline.lines()[latest_entry_tuple[0]]);
        let Some(border_lines) = current_lines.into_iter().collect::<Option<Vec<Line>>>() else {
            return Ok(false);
        };
        // :1215. Ruling AE: `new Polyline(Line[])` normalises the caller's array **in place**,
        // and `:1226` reads `currentLines[currentLines.length - 2]` back out of it afterwards —
        // one of the sites [`Polyline::from_lines_in_place`] exists for. A `from_lines` here would
        // cut the trace at the pre-normalisation line.
        let mut border_lines = border_lines;
        let Ok(border_polyline) = Polyline::from_lines_in_place(&mut border_lines) else {
            // Java's constructor cannot fail; a `PolylineError` is the port's own degeneracy and
            // refusing the correction is ruling 7's degraded value.
            return Ok(false);
        };
        // :1216-1222.
        if !board.check_polyline_trace(
            &border_polyline,
            layer,
            half_width,
            &net_numbers,
            clearance_class_index,
        ) {
            return Ok(false);
        }
        // :1224-1227.
        let trace_lines = trace_polyline.lines();
        let mut cut_lines = Vec::with_capacity(trace_lines.len() - latest_entry_tuple[0] + 1);
        cut_lines.push(border_lines[border_lines.len() - 2]);
        cut_lines.extend_from_slice(&trace_lines[latest_entry_tuple[0]..]);
        let Ok(cut_polyline) = Polyline::from_lines(cut_lines) else {
            return Ok(false);
        };
        // :1228-1234.
        let mut changed_polyline = if cut_polyline.first_corner() == cut_polyline.last_corner() {
            border_polyline.clone()
        } else {
            match border_polyline.combine(&cut_polyline) {
                Ok(combined) => combined,
                Err(_) => return Ok(false),
            }
        };
        // :1235-1236.
        if !at_start {
            changed_polyline = match changed_polyline.reverse() {
                Ok(reversed) => reversed,
                Err(_) => return Ok(false),
            };
        }
        // :1237 — `PolylineTrace.change(Polyline)` (`:937`), whose first act on a trace that is on
        // the board is `board.additionalUpdateAfterChange(this)` (`:944`). `fr-board` cannot make
        // that call, so it is made here, in the order Java makes it — the same contract
        // `pull_tight_with_engine` follows.
        if let Some(engine) = engine
            && board.items.get(&trace).is_some_and(Item::is_on_the_board)
        {
            board.additional_update_after_change(engine, trace);
        }
        board.change_trace(trace, changed_polyline);
        // :1239-1244: create a shoveFixed exit line.
        let Some(nearest_border_line) = offset_pin_shape.border_line(nearest_border_line_no) else {
            return Ok(false);
        };
        let Point::Int(pin_center_int) = pin_center else {
            return Ok(false);
        };
        let Some(exit_stub_line) =
            Line::from_direction_any(pin_center_int, &pin_exit_direction.turn_45_degree(2))
        else {
            return Ok(false);
        };
        let Ok(exit_line_segment) = Polyline::from_lines(vec![
            exit_stub_line,
            nearest_pin_exit_ray,
            nearest_border_line,
        ]) else {
            return Ok(false);
        };
        board.insert_trace(
            exit_line_segment,
            layer,
            half_width,
            net_numbers,
            clearance_class_index,
            FixedState::ShoveFixed,
        );
        // not reachable: `BasicBoard.insertTrace` runs `normalizeTrace`, which reaches
        // `PolylineTrace.change` and therefore `additionalUpdateAfterChange`, from inside
        // `fr-board`. Controller ruling AJ rosters that interior site rather than threading an
        // engine through `fr-board`'s signature.
        // :1245.
        Ok(true)
    }

    fn swap_connection_to_pin(
        board: &mut Board,
        engine: Option<&mut AutorouteEngine>,
        trace: ItemId,
        at_start: bool,
    ) -> Result<bool, BoardError> {
        let Some(Item::Trace(polyline_trace)) = board.items.get(&trace) else {
            return Ok(false);
        };
        let layer = polyline_trace.get_layer();
        let half_width = polyline_trace.get_half_width();
        let fixed_state = polyline_trace.hdr.get_fixed_state();
        // :1253-1262.
        let (trace_polyline, contact_list) = if at_start {
            (
                polyline_trace.polyline().clone(),
                board.trace_start_contacts(trace),
            )
        } else {
            let reversed = match polyline_trace.polyline().reverse() {
                Ok(reversed) => reversed,
                Err(_) => return Ok(false),
            };
            (reversed, board.trace_end_contacts(trace))
        };
        // :1263-1265.
        if contact_list.len() != 1 {
            return Ok(false);
        }
        // :1266-1271.
        let current_contact = *contact_list
            .iter()
            .next()
            .expect("the size test above found exactly one");
        let Some(contact_item) = board.items.get(&current_contact) else {
            return Ok(false);
        };
        if contact_item.get_fixed_state() != FixedState::ShoveFixed {
            return Ok(false);
        }
        let Item::Trace(contact_trace) = contact_item else {
            return Ok(false);
        };
        // :1272-1274.
        let contact_polyline = contact_trace.polyline().clone();
        let contact_lines = contact_polyline.lines();
        if contact_lines.len() < 2 || trace_polyline.lines().len() < 2 {
            // Java indexes `[length - 2]` and `[1]` unguarded; a two-line polyline cannot be a
            // trace on the board, so this only closes the port's own indexing.
            return Ok(false);
        }
        let contact_last_line = contact_lines[contact_lines.len() - 2];
        // :1275-1277: look, if this trace has a sharp angle with the contact trace.
        let first_line = trace_polyline.lines()[1];
        let mut check_swap = contact_last_line
            .direction()
            .projection(&first_line.direction())
            == Signum::Negative;
        // :1278-1289.
        if !check_swap {
            let half_width_f = f64::from(half_width);
            let near_start = match (
                trace_polyline.corner_approx(0),
                trace_polyline.corner_approx(1),
            ) {
                (Some(first), Some(second)) => {
                    first.distance_square(&second) <= half_width_f * half_width_f
                }
                _ => false,
            };
            if trace_polyline.lines().len() > 3 && near_start {
                // check also for sharp angle with the second line
                check_swap = contact_last_line
                    .direction()
                    .projection(&trace_polyline.lines()[2].direction())
                    == Signum::Negative;
            }
        }
        // :1290-1292.
        if !check_swap {
            return Ok(false);
        }
        // :1293-1301 — `.rev()` for Java's descending `TreeSet<Item>`.
        let contact_trace_start_contacts = board.trace_start_contacts(current_contact);
        let Some(contact_pin_id) = contact_trace_start_contacts
            .into_iter()
            .rev()
            .find(|id| matches!(board.items.get(id), Some(Item::Pin(_))))
        else {
            // :1302-1304.
            return Ok(false);
        };
        // :1305-1311.
        let combined_polyline = match contact_polyline.combine(&trace_polyline) {
            Ok(combined) => combined,
            Err(_) => return Ok(false),
        };
        let Some(Item::Pin(contact_pin)) = board.items.get(&contact_pin_id) else {
            unreachable!("just matched")
        };
        let ctx = board.ctx();
        let nearest_pin_exit_direction = contact_pin.calc_nearest_exit_restriction_direction(
            &combined_polyline,
            half_width,
            layer,
            &ctx,
        );
        // :1308-1311: `null`, or the direction the contact trace already leaves the pin in —
        // "direction would not be changed".
        let Some(nearest_pin_exit_direction) = nearest_pin_exit_direction else {
            return Ok(false);
        };
        if nearest_pin_exit_direction == Direction::Int(contact_lines[1].direction()) {
            return Ok(false);
        }
        // :1312 — release the `SHOVE_FIXED` exit stub so `combine` may swallow it.
        if let Some(Item::Trace(contact_trace)) = board.items.get_mut(&current_contact) {
            contact_trace.hdr.set_fixed_state(fixed_state);
        }
        // :1313 — `PolylineTrace.combine()` (`:174-192`), transcribed **here** rather than
        // delegated to [`Board::combine_trace`], because Java's loop body carries a call
        // `fr-board` cannot make.
        //
        // The loop is `while (isOnTheBoard() && (combineAtStart(true) || combineAtEnd(true))) {
        // …; board.additionalUpdateAfterChange(this); }` (`:183-190`), so `:188` runs **once per
        // successful merge**, **after** that merge, and **not at all** when neither end can grow.
        // `combineAtStart` (`:201-332`) and `combineAtEnd` (`:341-456`) never call `change()`
        // themselves — they `removeItem` the absorbed trace and rebuild this one — so `:188` is
        // `combine`'s only route to `additionalUpdateAfterChange`, and its payload is the shape
        // the trace has *after* growing. Invalidating the **pre**-merge shape once, which an
        // earlier draft of this method did, is a different set of expansion rooms
        // (`RoutingBoard.additionalUpdateAfterChange:103-104` removes the complete free-space
        // rooms touching a shape of the item).
        //
        // [`Board::combine_trace`] is this same loop with an empty body, and its `:188` marker
        // is now a `// not reachable:`: **Task 8** settled `fr-board`'s other `combine` callers
        // under controller ruling AJ, which found `retainAutorouteDatabase` to be a Java
        // benchmark-only system property, so `additionalUpdateAfterChange` is dead on every live
        // path in both languages. This is the one caller Task 5 owns, so it drives the two halves
        // itself instead of widening `fr-board`'s signature — additive, and correct either way.
        //
        // Java's observer notification at `:184-187` is dropped for the reason
        // `Board::combine_trace` already records: `global-constraints.md` forbids board observers.
        let mut engine = engine;
        while board.items.get(&trace).is_some_and(Item::is_on_the_board)
            && (board.combine_trace_at_start(trace, true)?
                || board.combine_trace_at_end(trace, true)?)
        {
            // :188. Java has **no** `isOnTheBoard()` guard here — the loop condition tested it
            // *before* the merge — and `additionalUpdateAfterChange:97-99` answers an item the
            // board does not know with an early return, which this port's
            // `RoutingBoardExt::additional_update_after_change` reproduces. So the call is
            // unconditional, as Java's is.
            if let Some(engine) = engine.as_deref_mut() {
                board.additional_update_after_change(engine, trace);
            }
        }
        // Java's `combine()` answers `somethingChanged`; `:1313` discards it, and so does this.
        // :1314.
        Ok(true)
    }
}
