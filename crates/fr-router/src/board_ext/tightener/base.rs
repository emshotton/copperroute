//! [`TightenerBase`]: the state and the shared algorithms of `board/optimize/TraceTightener.java`
//! — Java's `abstract class TraceTightener`, minus the three abstract methods and minus the two
//! entry points that dispatch to them, which live on [`TraceTightener`](super::TraceTightener).

use std::collections::BTreeSet;

use fr_board::datastructures::StopCheck;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId, TimeLimit};
use fr_geometry::{FloatPoint, IntOctagon, IntPoint, Line, Point, Polyline, Side, Signum};

/// `TraceTightener.c_max_cos_angle` (TraceTightener.java:33): "with angles too close to 180
/// degree the algorithm becomes numerically unstable".
pub(crate) const C_MAX_COS_ANGLE: f64 = 0.999;

/// `TraceTightener.c_min_corner_dist_square` (TraceTightener.java:36).
pub(crate) const C_MIN_CORNER_DIST_SQUARE: f64 = 0.9;

/// Java's `new Polyline(Line[])`, the normalising constructor every tightening step funnels
/// through (Polyline.java:73-102).
///
/// [`Polyline::from_lines`] answers `Err` on the one input class where Java throws — the
/// `tmpArr[-1]` read of `Polyline.removeOverlaps` (Polyline.java:148, quirk #22). No `catch`
/// stands between any tightener and `AutorouteConnectionRouter.route:155-158`, so the port
/// panics there and the caller's `catch_unwind` boundary turns it into that method's bare
/// `FAILED`, exactly as Task 14 ruled for `FoundConnectionLocator`'s latent NPEs.
// totalized: Java's ArrayIndexOutOfBoundsException out of `new Polyline(Line[])` becomes a panic.
pub(crate) fn new_polyline(lines: Vec<Line>) -> Polyline {
    Polyline::from_lines(lines).unwrap_or_else(|e| {
        panic!("new Polyline(Line[]) threw (Polyline.java:148, quirk #22): {e}")
    })
}

/// The never-tripping stand-in for a `null` `Stoppable`, matching the `&|| false` wrappers
/// `fr-board` uses for the same purpose.
static NEVER_STOP: fn() -> bool = || false;

/// The state Java keeps on `abstract class TraceTightener` (TraceTightener.java:31-61) plus the
/// algorithms that are neither abstract nor dispatch to something abstract.
///
/// # What is not a field here
///
/// Java's `protected final RoutingBoard board` (`:37`) is a **parameter** of every method below.
/// `fr-router` cannot hold a `&mut fr_board::Board` across the tightener's lifetime — the maze
/// hands the same board to `Board::check_trace_shape` on the next line — and plan-6 ruling 3
/// already passes the board into every `board_ext` algorithm for the same reason.
///
/// Java's `private final Stoppable stoppableThread` (`:43`) is plan-6 ruling 6's
/// [`StopCheck`] instead, which is why this type carries a lifetime.
pub(crate) struct TightenerBase<'a> {
    /// `TraceTightener.onlyNetNoArr` (`:40`): "if only_net_no > 0, only nets with this net
    /// numbers are optimized".
    pub(crate) only_net_no_arr: Vec<i32>,
    /// `TraceTightener.stoppableThread` (`:43`), as plan-6 ruling 6's borrowed predicate.
    stoppable_thread: Option<StopCheck<'a>>,
    /// `TraceTightener.timeLimit` (`:45`) — `null` unless the constructor was given a positive
    /// millisecond budget (`:73-77`).
    time_limit: Option<TimeLimit>,
    /// `TraceTightener.keepPoint` (`:51`): "traces containing the keepPoint must also contain
    /// the keepPoint after optimizing".
    keep_point: Option<Point>,
    /// `TraceTightener.keepPointLayer` (`:53`). Java's `int`, `-1` when there is no keep point.
    keep_point_layer: i32,
    /// `TraceTightener.currentLayer` (`:54`).
    pub(crate) current_layer: usize,
    /// `TraceTightener.currentHalfWidth` (`:55`) — already clearance-compensated by `:184-185`.
    pub(crate) current_half_width: i32,
    /// `TraceTightener.currentNetNumbers` (`:56`).
    pub(crate) current_net_numbers: Vec<i32>,
    /// `TraceTightener.currentClearanceClassIndex` (`:57`).
    pub(crate) current_clearance_class_index: usize,
    /// `TraceTightener.currentClipShape` (`:58`), written by `getInstance:111`.
    pub(crate) current_clip_shape: Option<IntOctagon>,
    /// `TraceTightener.contactPins` (`:59`): "the pins at the end corners of polyline. Other pins
    /// are regarded as obstacles, even if they are of the own net" (`:172-173`). `null` in Java
    /// means "no pin is exempt", which [`Board::check_trace_shape`] spells `None`.
    pub(crate) contact_pins: Option<BTreeSet<ItemId>>,
    /// `TraceTightener.minTranslateDist` (`:60`), written by `getInstance:112` as
    /// `Math.max(minTranslateDist, 100)`.
    pub(crate) min_translate_dist: i32,
}

impl<'a> TightenerBase<'a> {
    /// Port of the constructor `TraceTightener(RoutingBoard, int[], Stoppable, int, Point, int)`
    /// (TraceTightener.java:63-80).
    ///
    /// `currentClipShape` and `minTranslateDist` are **not** constructor arguments in Java
    /// either: `getInstance` writes them onto the finished object (`:111-112`), and
    /// [`TraceTightener::get_instance`](super::TraceTightener::get_instance) does the same.
    pub(crate) fn new(
        only_net_no_arr: Vec<i32>,
        stoppable_thread: Option<StopCheck<'a>>,
        time_limit: i32,
        keep_point: Option<Point>,
        keep_point_layer: i32,
    ) -> TightenerBase<'a> {
        TightenerBase {
            only_net_no_arr,
            stoppable_thread,
            // TraceTightener.java:73-77.
            time_limit: if time_limit > 0 {
                Some(TimeLimit::new(time_limit))
            } else {
                None
            },
            keep_point,
            keep_point_layer,
            // Java's fields default to 0 / null until `pullTight`'s six-argument entry writes
            // them (`:182-188`).
            current_layer: 0,
            current_half_width: 0,
            current_net_numbers: Vec::new(),
            current_clearance_class_index: 0,
            current_clip_shape: None,
            contact_pins: None,
            min_translate_dist: 0,
        }
    }

    /// Port of `isStopRequested()` (TraceTightener.java:195-212): "terminates the pull tight
    /// algorithm, if the user has made a stop request".
    // not ported: the two `FRLogger` calls of the time-limit branch (`:205-209`); the first of
    // them tests `this.board == null`, which cannot happen — `board` is a `final` field written
    // from a non-null constructor argument.
    pub(crate) fn is_stop_requested(&self) -> bool {
        // :196-198.
        if let Some(stop) = self.stoppable_thread
            && stop()
        {
            return true;
        }
        // :199-201.
        let Some(time_limit) = &self.time_limit else {
            return false;
        };
        // :202-211.
        time_limit.is_exceeded()
    }

    /// The tightener's own `Stoppable` as a [`StopCheck`], for the `fr-board` walks Java calls
    /// without one — `splitTraces` and `normalizeTraces` under
    /// `smoothenEndCornersAtTrace1:448-452`. `null` becomes a never-tripping check, so a
    /// tightener built without a `Stoppable` behaves exactly as Java's does.
    pub(crate) fn stop_check(&self) -> StopCheck<'a> {
        self.stoppable_thread.unwrap_or(&NEVER_STOP)
    }

    /// `board.checkTraceShape(shapeToCheck, currentLayer, currentNetNumbers,
    /// currentClearanceClassIndex, this.contactPins)` — the guard every tightening step in this
    /// file and in the three regimes runs before it accepts a candidate.
    pub(crate) fn check(&self, board: &mut Board, shape: &fr_geometry::TileShape) -> bool {
        board.check_trace_shape(
            shape,
            self.current_layer,
            &self.current_net_numbers,
            self.current_clearance_class_index,
            self.contact_pins.as_ref(),
        )
    }

    /// `currentClipShape.isOutside(point)` — Java calls `TileShape.isOutside` (TileShape.java:143)
    /// through the `IntOctagon` the clip shape always is.
    pub(crate) fn clip_is_outside(&self, point: &Point) -> bool {
        match &self.current_clip_shape {
            Some(clip) => fr_geometry::TileShape::Octagon(*clip).is_outside(point),
            None => false,
        }
    }

    /// `currentClipShape == null || currentClipShape.contains(floatPoint)` — the `FloatPoint`
    /// overload (`IntOctagon.contains(FloatPoint)`, quirk #17: inclusive on the border).
    pub(crate) fn clip_contains(&self, point: &FloatPoint) -> bool {
        match &self.current_clip_shape {
            Some(clip) => clip.contains_float(point),
            None => true,
        }
    }

    /// Port of `avoidAcidTraps(Polyline)` (TraceTightener.java:517-542): "wraps around pins of
    /// the own net to avoid acid traps".
    ///
    /// The method's first statement is `if (true) { return polyline; }` (`:518-520`), so the
    /// whole body below it — a `TraceShover.springOverObstacles` call and a
    /// `board.checkPolylineTrace` guard — is **unreachable**, and every one of the three
    /// `pullTight` overrides that opens with `avoidAcidTraps(polyline)` therefore receives its
    /// own argument back. Answering `None` ("the argument, unchanged") is that behaviour; see
    /// docs/java-quirks.md #182.
    ///
    /// This is also why the tightener family needs **none** of `TraceShover.springOverObstacles`,
    /// which controller ruling AA keeps in Plan 7.
    // Java bug: TraceTightener.avoidAcidTraps:518 is `if (true) return polyline;` — the rest of
    // the method is dead. See docs/java-quirks.md.
    pub(crate) fn avoid_acid_traps(&self, _polyline: &Polyline) -> Option<Polyline> {
        // TraceTightener.java:518-520.
        None
    }

    /// Port of `repositionLines(Polyline)` (TraceTightener.java:215-230): "tries to shorten
    /// polyline by relocating its lines".
    ///
    /// `None` is Java's "returns the argument object" — the identity the three `pullTight` loops
    /// compare with `!=`.
    ///
    /// `TraceTightenerAnyAngle` overrides this (`TraceTightenerAnyAngle.java:250-274`), so this
    /// body is the 90- and 45-degree regimes' only.
    pub(crate) fn reposition_lines(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        // :216-218.
        if polyline.lines().len() < 5 {
            return None;
        }
        // :219.
        for i in 2..polyline.lines().len() - 2 {
            let Some(new_line) = self.reposition_line(board, polyline.lines(), i) else {
                continue;
            };
            // :221-227.
            let mut lines = polyline.lines().to_vec();
            lines[i] = new_line;
            let result = new_polyline(lines);
            return Some(match self.skip_segments_of_length_0(board, &result) {
                Some(skipped) => skipped,
                None => result,
            });
        }
        // :229.
        None
    }

    /// The base `repositionLine(Line[], int)` (TraceTightener.java:235-331): "tries to reposition
    /// the line with index `no` to make the polyline consisting of `lines` shorter".
    ///
    /// `TraceTightenerAnyAngle` overrides it (`TraceTightenerAnyAngle.java:496-657`) with a
    /// method whose `startNo` counts from a different end, so the two are never interchangeable.
    ///
    /// The `checkLines[1].equals(translateLine)` guard at `:281` is **quirk #34's** site: Java's
    /// `Line.equals` is the *geometric* test, and `Line.translate` with `|dist| < 1` answers a
    /// line with different end points that denotes the same line of the plane. Comparing with
    /// the port's derived structural `==` would let the loop run on with a line that never
    /// moves, so this uses [`Line::equals_geometric`].
    // `maxTranslateDist` is written at `:292` and never read again — Java's own dead store; the
    // value the algorithm uses is `sign`, taken from it at `:269` before the loop.
    #[allow(unused_assignments)]
    pub(crate) fn reposition_line(
        &mut self,
        board: &mut Board,
        lines: &[Line],
        no: usize,
    ) -> Option<Line> {
        // :236-238.
        if (lines.len() as i64) - (no as i64) < 3 {
            return None;
        }
        // :239-247: the corners of the line to translate must be inside the clip shape.
        if self.current_clip_shape.is_some() {
            for i in [-1_i64, 0] {
                let index = (no as i64 + i) as usize;
                let current_corner = lines[index].intersection(&lines[index + 1]);
                if self.clip_is_outside(&current_corner) {
                    return None;
                }
            }
        }
        // :248-252.
        let translate_line = lines[no];
        let prev_corner = lines[no - 2].intersection(&lines[no - 1]);
        let next_corner = lines[no + 1].intersection(&lines[no + 2]);
        let prev_dist = translate_line.signed_distance(&prev_corner.to_float());
        let next_dist = translate_line.signed_distance(&next_corner.to_float());
        // :253-256.
        if Signum::of_f64(prev_dist) != Signum::of_f64(next_dist) {
            return None;
        }
        // :257-265.
        let (nearest_point, mut max_translate_dist) = if prev_dist.abs() < next_dist.abs() {
            (prev_corner, prev_dist)
        } else {
            (next_corner, next_dist)
        };
        // :266-274.
        let mut translate_dist = max_translate_dist;
        let mut delta_dist = max_translate_dist;
        let side_of_nearest_point = translate_line.side_of(&nearest_point);
        let sign = Signum::as_int_f64(max_translate_dist);
        let mut new_line: Option<Line> = None;
        let check_line_0 = lines[no - 1];
        let check_line_2 = lines[no + 1];
        let mut first_time = true;
        // :275.
        while first_time || delta_dist.abs() > f64::from(self.min_translate_dist) {
            // :276-280.
            let check_line_1 = match (first_time, &nearest_point) {
                (true, Point::Int(p)) => Line::from_direction(*p, &translate_line.direction()),
                _ => translate_line.translate(-translate_dist),
            };
            // :281-284 — quirk #34.
            if check_line_1.equals_geometric(&translate_line) {
                return None;
            }
            // :285-296.
            let new_line_side_of_nearest_point = check_line_1.side_of(&nearest_point);
            if new_line_side_of_nearest_point != side_of_nearest_point
                && new_line_side_of_nearest_point != Side::Collinear
            {
                let shorten_value = f64::from(sign) * 0.5;
                max_translate_dist -= shorten_value;
                translate_dist -= shorten_value;
                delta_dist -= shorten_value;
                continue;
            }
            // :297-309.
            let tmp = new_polyline(vec![check_line_0, check_line_1, check_line_2]);
            let mut check_ok = false;
            if tmp.lines().len() == 3 {
                let shape_to_check = tmp
                    .offset_shape(self.current_half_width, 0)
                    .expect("a three-line polyline has one offset shape");
                check_ok = self.check(board, &shape_to_check);
            }
            // :310-321.
            delta_dist /= 2.0;
            if check_ok {
                new_line = Some(check_line_1);
                if first_time {
                    // :313-316: biggest possible change.
                    break;
                }
                translate_dist += delta_dist;
            } else {
                translate_dist -= delta_dist;
            }
            first_time = false;
        }
        // :323-329.
        if let Some(new_line) = new_line
            && board.changed_area.is_some()
        {
            let layer = self.current_layer;
            board.join_changed_area(&check_line_0.intersection_approx(&new_line), layer);
            board.join_changed_area(&check_line_2.intersection_approx(&new_line), layer);
            board.join_changed_area(&lines[no - 1].intersection_approx(&lines[no]), layer);
            board.join_changed_area(&lines[no].intersection_approx(&lines[no + 1]), layer);
        }
        // :330.
        new_line
    }

    /// Port of `skipSegmentsOfLength0(Polyline)` (TraceTightener.java:338-399): "tries to skip
    /// line segments of length 0. A check is necessary before skipping because new dog ears may
    /// occur."
    ///
    /// `None` is Java's `return polyline` at `:396`, the identity the callers compare with `!=`.
    pub(crate) fn skip_segments_of_length_0(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        // :339-340.
        let mut polyline_changed = false;
        let mut current_polyline = polyline.clone();
        // :341 — `currentPolyline` is reassigned inside the loop, so the bound is re-read every
        // round and `--i` at `:391` re-examines the index the skip moved into.
        let mut i: usize = 1;
        while i + 1 < current_polyline.lines().len() {
            // :342-353.
            let try_skip = if i == 1 || i == current_polyline.lines().len() - 2 {
                // The position of the first and the last corner must be retained exactly.
                let prev_corner = current_polyline
                    .corner(i - 1)
                    .expect("i - 1 is below cornerCount");
                let current_corner = current_polyline.corner(i).expect("i is below cornerCount");
                current_corner == prev_corner
            } else {
                let prev_corner = current_polyline
                    .corner_approx(i - 1)
                    .expect("i - 1 is below cornerCount");
                let current_corner = current_polyline
                    .corner_approx(i)
                    .expect("i is below cornerCount");
                current_corner.distance_square(&prev_corner) < C_MIN_CORNER_DIST_SQUARE
            };

            if try_skip {
                // :357-361: drop line `i`.
                let mut current_lines: Vec<Line> =
                    Vec::with_capacity(current_polyline.lines().len() - 1);
                current_lines.extend_from_slice(&current_polyline.lines()[..i]);
                current_lines.extend_from_slice(&current_polyline.lines()[i + 1..]);
                let tmp = new_polyline(current_lines.clone());
                // :362.
                let mut check_ok = tmp.lines().len() == current_lines.len();
                // :363-387: no check is necessary for 45-degree lines.
                if check_ok && !current_polyline.lines()[i].is_multiple_of_45_degree() {
                    if i > 1 {
                        let shape_to_check = tmp
                            .offset_shape(self.current_half_width, i - 2)
                            .expect("i - 2 is below tileShapeCount");
                        check_ok = self.check(board, &shape_to_check);
                    }
                    if check_ok && i < current_polyline.lines().len() - 2 {
                        let shape_to_check = tmp
                            .offset_shape(self.current_half_width, i - 1)
                            .expect("i - 1 is below tileShapeCount");
                        check_ok = self.check(board, &shape_to_check);
                    }
                }
                // :388-392.
                if check_ok {
                    polyline_changed = true;
                    current_polyline = tmp;
                    i -= 1;
                }
            }
            i += 1;
        }
        // :395-398.
        if !polyline_changed {
            return None;
        }
        Some(current_polyline)
    }

    /// Port of `splitTracesAtKeepPoint()` (TraceTightener.java:476-491): "splits the traces
    /// containing this.keepPoint if this.keepPoint != null. Returns true, if something was
    /// split."
    ///
    /// Java's `ItemSelectionFilter(TRACES)` is the `Item::Trace` test below —
    /// [`Board::pick_items`] has no filter argument (see `fr-board`'s `not ported:` note on
    /// `ItemSelectionFilter`).
    ///
    /// `Trace.split(Point)` is [`Board::split_trace_at_point`], whose `Err` is
    /// [`BoardError::Stopped`] out of quirk #76's ladder walk; Java has no cancellation here at
    /// all, so the error propagates rather than being read as "nothing was split".
    pub(crate) fn split_traces_at_keep_point(
        &mut self,
        board: &mut Board,
    ) -> Result<bool, BoardError> {
        // :477-479.
        let Some(keep_point) = self.keep_point.clone() else {
            return Ok(false);
        };
        // :480-483.
        let layer = usize::try_from(self.keep_point_layer).ok();
        let picked_items = board.pick_items(&keep_point, layer);
        // :484-489.
        for current_item in picked_items {
            if !matches!(board.items.get(&current_item), Some(Item::Trace(_))) {
                continue;
            }
            let split_pieces = board.split_trace_at_point(current_item, &keep_point)?;
            if split_pieces.is_some() {
                return Ok(true);
            }
        }
        // :490.
        Ok(false)
    }

    /// `Line.getInstance(currentCorner.round(), dir)` — the `FloatPoint` rounding the three
    /// regimes do before they build a translate line.
    pub(crate) fn line_through(corner: &FloatPoint, dir: &fr_geometry::Direction) -> Line {
        let rounded: IntPoint = corner.round();
        Line::from_direction_any(rounded, dir)
            .expect("a Direction built from IntPoint differences is an IntDirection")
    }
}
