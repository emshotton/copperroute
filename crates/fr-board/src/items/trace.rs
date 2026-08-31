//! The trace item: Java's abstract `Trace` (`board/model/items/Trace.java`) and its only
//! concrete subclass `PolylineTrace` (`board/trace/PolylineTrace.java`), together with the
//! geometry helper `board/trace/PolylineTraceGeometry.java`.
//!
//! # What is here and what is not
//!
//! Java splits the trace across four files:
//!
//! * `Trace` — the two fields (`layer`, `halfWidth`), the `Item` overrides that need only them,
//!   and eleven abstract or board-dependent methods.
//! * `PolylineTrace` — the `lines` field plus every concrete body.
//! * `PolylineTraceGeometry` — a stateless bag of eleven one-liners that `PolylineTrace`
//!   delegates its pure geometry to. It has **no public members at all** (the class and all its
//!   methods are package-private), so it is inlined into [`PolylineTrace`]'s bodies here rather
//!   than reproduced as a second type; each body names the `PolylineTraceGeometry` line it came
//!   from.
//! * `PolylineTraceSearchTreeAdapter` — also package-private and also stateless; every one of
//!   its six methods is a forward to `board.searchTreeManager`, so it belongs to the board.
//!
//! # The board back-pointer
//!
//! Only two bodies in this file need it, and both take Task 6's [`ItemCtx`]:
//! `changePlacementSide` reads `board.getLayerCount()` (PolylineTrace.java:164-166), and the
//! constructor clamps the layer against it (Trace.java:45-47) — there through an explicit
//! `layer_count` argument, because the constructor runs before any item exists.
//!
//! # No memo fields
//!
//! Unlike [`crate::items::drill`] and [`crate::items::area`], `PolylineTrace` has **no**
//! `transient precalculated*` field: `lines` is its only state, and every derived value
//! (corners, length, bounding box, offset shapes) is recomputed per call, exactly as Java does.
//! The only cache a trace has is the `Item`-level search-tree shape cache in [`ItemHeader`].
//!
//! # What the rest of the trace is waiting for
//!
//! Every `Trace`/`PolylineTrace` member below the geometry needs either the board's search tree
//! or its item list. Each marker names the Java method, its lines, and the method that will
//! replace it.

// renamed: `Trace.combine` (Trace.java:463) and `PolylineTrace.combine`
// (PolylineTrace.java:174-192), with the two private halves `combineAtStart`
// (PolylineTrace.java:201-332) and `combineAtEnd` (:341-456) -> `Board::combine_trace`,
// `Board::combine_trace_at_start`, `Board::combine_trace_at_end` (`board/trace_normalize.rs`).
// renamed: `PolylineTrace.normalize` (PolylineTrace.java:801-803) and the whole of
// `board/trace/PolylineTraceNormalization.java` -> `Board::normalize_trace` (recursion capped at
// `MAX_NORMALIZATION_DEPTH = 16`).
// renamed: `Trace.split(IntOctagon)` (Trace.java:471) and `PolylineTrace.split`
// (PolylineTrace.java:464-691) -> `Board::split_trace`. Only its `clipShape` filter
// (PolylineTrace.java:475-479) is pure — [`PolylineTrace::clip_intersects_segment`]; every
// intersection candidate comes from `defaultTree.overlappingTreeEntries`, and each split ends in
// `board.removeItem` + `board.insertTraceWithoutCleaning`.
// renamed: `Trace.split(Point)` (Trace.java:477) and `PolylineTrace.split(Point)`
// (PolylineTrace.java:698-712), plus the private `PolylineTrace.split(int, Line)` (:719-760) and
// `splitInsideDrillPadProhibited` (:768-792) -> `Board::split_trace_at_point`,
// `Board::split_trace_at_line` and `Board::split_inside_drill_pad_prohibited`. The geometry cores
// are [`PolylineTrace::split_polyline_at_point`] and
// [`PolylineTrace::split_polyline_at_line`].
// renamed: `PolylineTrace.change` (PolylineTrace.java:936-1005) -> `Board::change_trace`:
// the "reuse the search-tree entries" diff plus `normalize`.
// renamed: `Trace.getCompensatedHalfWidth` (Trace.java:86-89) ->
// `ShapeSearchTree::compensated_half_width`; it is `halfWidth +
// searchTree.clearanceCompensationValue(clearanceClassIndex(), layer)`.
// renamed: the protected `PolylineTrace.calculateTreeShapes`
// (PolylineTrace.java:132-135) and `PolylineTraceSearchTreeAdapter.calculateTreeShapes` (:18-20)
// -> `ShapeSearchTree::calculate_tree_shapes`. Note that Java's body
// (ShapeSearchTree.java:992-1004) does **not** call `Polyline.offsetShapes`: it loops
// `tileShapeCount()` times over the *virtual* `ShapeSearchTree.offsetShape(polyline, width, i)`
// (:1079-1081 = `polyline.offsetShape(width, i)`), which `ShapeSearchTree90Degree` overrides to
// `polyline.offsetBox(width, i)` (ShapeSearchTree90Degree.java:486-490) — a 90-degree tree keeps
// traces in `IntBox`es. `ShapeSearchTree45Degree` has no override. So
// [`PolylineTrace::offset_shapes`] is the *base-class* answer only; Task 10 must dispatch per
// tree angle, not reuse it unconditionally.
// renamed: `PolylineTraceSearchTreeAdapter.hasDefaultEntries` (:22-26) and `replaceGeometry`
// (:34-40) -> `Board::trace_has_default_entries` / `Board::replace_trace_geometry`, and its
// `mergeEntriesInFront` (:42-50), `mergeEntriesAtEnd` (:52-60) and `changeEntries` (:62-66) ->
// `Board::merge_trace_entries_in_front`, `Board::merge_trace_entries_at_end` and
// `Board::change_trace_entries`, which forward to `SearchTreeManager`; all five read the trace's
// board.
// renamed: `Trace.getStartContacts` (Trace.java:108-110), `Trace.getEndContacts` (:116-118) and
// `Trace.getNormalContacts(Point, boolean)` (:173-203) -> `Board::trace_start_contacts`,
// `Board::trace_end_contacts` and `Board::trace_normal_contacts_at`; all three go through
// `board.overlappingObjects`.
// renamed: `Trace.isCycle` (Trace.java:272-330) -> `Board::is_trace_cycle`; it walks
// `Item.isCycleRecu` over the start contacts. Its one non-GUI caller is
// `BasicBoard.removeIfCycle` (BasicBoard.java:1339).
// renamed: `Trace.touchingPinsAtEndCorners` (Trace.java:390-410) ->
// `Board::touching_pins_at_end_corners`; it calls `board.overlappingItemsWithClearance` on the
// enlarged surrounding octagon of each end corner (`BasicBoard.java:1066` is the non-router
// caller).
//
// The six markers below deliberately keep each Java name on the *same physical line* as its
// `renamed:` prefix: `scripts/audit-port.sh`'s marker check is a per-line
// `grep -E "renamed:.*\bName\b"` (and the same shape for `not ported:` and `added in
// Task|Plan N:`), so a name that wraps onto a continuation line does not count as covered.
// Plan 7 Task 5 landed all six and re-pointed them from deferral markers to `renamed:` ones.
//
// renamed: `Trace.checkConnectionToPin` (Trace.java:376) — abstract; the concrete body is `PolylineTrace`'s, and both are `fr_router::board_ext::PolylineTraceExt::check_connection_to_pin` (plan-rulings.md #4): its only callers are `PolylineTrace.correctConnectionToPin` and `PolylineTrace.pullTight:841-861`, which take a `TraceTightener`. Landed in Plan 7 Task 5.
// renamed: `PolylineTrace.checkConnectionToPin` (PolylineTrace.java:1013-1076) -> `fr_router::board_ext::PolylineTraceExt::check_connection_to_pin`; it needs the start/end contacts, `board.rules.getPinEdgeToTurnDist()` and `board.clearanceValue`, and it lives beside the pair it gates.
// renamed: `PolylineTrace.correctConnectionToPin` (PolylineTrace.java:1082-1245) -> `fr_router::board_ext::PolylineTraceExt::correct_connection_to_pin` — the acid-trap correction; it needs `board.checkPolylineTrace`, `board.insertTrace` and an `AutorouteEngine` for the `PolylineTrace.change` at `:1237`, which is `fr-router`'s type.
// renamed: `PolylineTrace.swapConnectionToPin` (PolylineTrace.java:1252-1313) -> `fr_router::board_ext::PolylineTraceExt::swap_connection_to_pin` — it needs the start contacts, `Pin.calcNearestExitRestrictionDirection` and an `AutorouteEngine` for the `combine()` at `:1313`.
// renamed: `Trace.pullTight` (Trace.java:483) and `PolylineTrace.pullTight` (both overloads, PolylineTrace.java:809-863 and :869-890) -> `fr_router::board_ext::PolylineTraceExt::{pull_tight_with, pull_tight}` (plan-rulings.md #4): both take a `TraceTightener`, which is `fr-router`'s type. **Controller ruling AB** moved that class family out of Plan 7 into Plan 6 Task 15a, because `RoutingBoard.insertForcedTracePolyline:861` pull-tightens every inserted polyline unconditionally and plan-6 ruling 1's geometry parity is unreachable without it.
// renamed: `PolylineTrace.smoothenEndCornersFork` (PolylineTrace.java:893-915) -> `fr_router::board_ext::TraceTightener::smoothen_end_corners_at_trace` (landed Plan 6 Task 15a), which is the whole of its body past the `TraceTightener.getInstance` at `:904-913`. It has **no caller anywhere in the Java tree** (`grep -rn smoothenEndCornersFork src` finds only the declaration), so the wrapper itself buys nothing; `optChangedArea` — the closest live analogue, ported in Plan 7 Task 5 — builds one tightener for the whole sweep rather than one per trace. Recorded as `renamed:` rather than `not ported:` because the body *is* ported, one factory call away.
// not ported: `PolylineTrace.write(ObjectOutputStream)` (PolylineTrace.java:926-934) — Java
// serialization, which `global-constraints.md` excludes.

use fr_geometry::{
    FloatPoint, IntBox, IntOctagon, IntPoint, Line, LineSegment, Point, Polyline, PolylineError,
    TileShape, Vector,
};

use crate::ids::{ItemId, TreeId};
use crate::items::header::ItemHeader;
use crate::items::{Connectable, ItemCtx, copied_header};

/// Port of `PolylineTrace` (`board/trace/PolylineTrace.java`), the only concrete `Trace`.
///
/// Java's abstract `Trace` (`board/model/items/Trace.java`) has no separate representation here:
/// its fields (`layer`, `halfWidth`) and its `Item` overrides belong to this struct, and
/// `instanceof Trace` is [`crate::items::Item::is_trace`].
#[derive(Debug, Clone, PartialEq)]
pub struct PolylineTrace {
    /// The `Item` base-class state (Item.java:41-67).
    pub hdr: ItemHeader,
    /// Java `private Polyline lines` (PolylineTrace.java:42).
    lines: Polyline,
    /// Java `private int layer` (Trace.java:31) — not final: `setLayer` rewrites it.
    layer: usize,
    /// Java `private final int halfWidth` (Trace.java:30).
    half_width: i32,
}

impl PolylineTrace {
    /// Port of `PolylineTrace(Polyline, int, int, int[], int, int, int, FixedState, BasicBoard)`
    /// (PolylineTrace.java:45-60) together with the `Trace` constructor it chains to
    /// (Trace.java:33-49). The `Item` half is [`ItemHeader::new`].
    ///
    /// `layer_count` is Java's `board.getLayerCount()`: `Some(n)` for the `board != null` branch
    /// (Trace.java:45-47), `None` for a trace built without a board — which every Java caller
    /// outside the tests does have, but `PolylineTraceSplitTest` and this crate's tests do not.
    /// Java's `Math.max(layer, 0)` is free for a `usize`.
    ///
    /// Java's "polyline.lines.length >= 3 expected" warning (PolylineTrace.java:56-58) is
    /// dropped rather than turned into a `debug_assert!`: it is not an invariant guard. Java
    /// warns and then stores the short polyline, and the rest of the class is written to
    /// survive it — `PolylineTraceGeometry.tileShapeCount` floors the count at 0
    /// (:44) and `Polyline.corner` answers `null` (Polyline.java:296-299). Asserting would make
    /// a debug build panic on a path Java completes.
    ///
    /// # Panics
    /// If `layer_count` is `Some(0)`. Java computes `Math.min(layer, -1)` there and stores a
    /// negative layer, which every later `layerStructure.layers[layer]` throws on; the port's
    /// layers are `usize`, so the break moves to the constructor.
    // totalized: a zero layer count panics here instead of storing Java's negative layer.
    // not ported: the "polyline.lines.length >= 3 expected" FRLogger.warn
    // (PolylineTrace.java:57).
    pub fn new(
        hdr: ItemHeader,
        lines: Polyline,
        layer: usize,
        half_width: i32,
        layer_count: Option<usize>,
    ) -> PolylineTrace {
        let layer = match layer_count {
            // Trace.java:45-47.
            Some(count) => layer.min(count.checked_sub(1).expect(
                "Trace(...): board.getLayerCount() is 0, so Java stores a negative layer \
                 (Trace.java:46)",
            )),
            None => layer,
        };
        PolylineTrace {
            hdr,
            lines,
            layer,
            half_width,
        }
    }

    /// Port of `PolylineTrace.copy` (PolylineTrace.java:62-79).
    ///
    /// Java hands the constructor `board`, so [`PolylineTrace::new`]'s layer clamp runs a second
    /// time. It is a no-op for every trace the constructor itself built — but **not** for one
    /// whose layer was pushed out of range afterwards by `Trace.setLayer` (Trace.java:71-73),
    /// which does not clamp. `Item::copy` carries no [`ItemCtx`], so the copy keeps the layer as
    /// it stands; the only Java caller that can create such a trace is
    /// `PolylineTrace.changePlacementSide` on a board with fewer layers than the trace's index,
    /// which panics here first (see [`PolylineTrace::change_placement_side`]).
    pub fn copy(&self, new_id: ItemId) -> PolylineTrace {
        PolylineTrace {
            hdr: copied_header(&self.hdr, new_id),
            lines: self.lines.clone(),
            layer: self.layer,
            half_width: self.half_width,
        }
    }

    // -- primary data ---------------------------------------------------------------------------

    /// Port of `PolylineTrace.polyline` (PolylineTrace.java:122-125).
    pub fn polyline(&self) -> &Polyline {
        &self.lines
    }

    /// Port of the package-private `PolylineTrace.setPolyline` (PolylineTrace.java:127-130).
    ///
    /// `pub` rather than `pub(crate)` because its only Java callers are
    /// `PolylineTraceSearchTreeAdapter.replaceGeometry` (:37) and the two `combine` bodies,
    /// which are Task 9/11 — a `pub(crate)` method with no in-crate caller is dead code under
    /// `-D warnings`. Same call as `BoardOutline::get_keepout_lines` (Task 7).
    pub fn set_polyline(&mut self, new_polyline: Polyline) {
        self.lines = new_polyline;
    }

    /// Port of `Trace.getLayer` (Trace.java:67-69).
    pub fn get_layer(&self) -> usize {
        self.layer
    }

    /// Port of `Trace.setLayer` (Trace.java:71-73). Note it does **not** clamp, unlike the
    /// constructor.
    pub fn set_layer(&mut self, layer: usize) {
        self.layer = layer;
    }

    /// Port of `Trace.firstLayer` (Trace.java:57-60).
    pub fn first_layer(&self) -> usize {
        self.get_layer()
    }

    /// Port of `Trace.lastLayer` (Trace.java:62-65).
    pub fn last_layer(&self) -> usize {
        self.get_layer()
    }

    /// Port of `Trace.getHalfWidth` (Trace.java:75-77).
    pub fn get_half_width(&self) -> i32 {
        self.half_width
    }

    // -- pure geometry (PolylineTraceGeometry.java) ---------------------------------------------

    /// Port of `PolylineTrace.firstCorner` (PolylineTrace.java:90-93) ->
    /// `PolylineTraceGeometry.firstCorner` (:23-25) = `lines.corner(0)`.
    ///
    /// `None` is Java's `null`, which `Polyline.corner` answers for a polyline of under two
    /// lines (Polyline.java:296-299).
    pub fn first_corner(&self) -> Option<Point> {
        self.lines.corner(0)
    }

    /// Port of `PolylineTrace.lastCorner` (PolylineTrace.java:99-102) ->
    /// `PolylineTraceGeometry.lastCorner` (:27-29) = `lines.corner(lines.lines.length - 2)`,
    /// which is [`Polyline::last_corner`].
    pub fn last_corner(&self) -> Option<Point> {
        self.lines.last_corner()
    }

    /// Port of `PolylineTrace.cornerCount` (PolylineTrace.java:108-110) ->
    /// `PolylineTraceGeometry.cornerCount` (:31-33) = `lines.lines.length - 1`, which is
    /// [`Polyline::corner_count`].
    pub fn corner_count(&self) -> usize {
        self.lines.corner_count()
    }

    /// Port of `PolylineTrace.getLength` (PolylineTrace.java:112-115) ->
    /// `PolylineTraceGeometry.length` (:35-37) = `lines.lengthApprox()`.
    pub fn get_length(&self) -> f64 {
        self.lines.length_approx()
    }

    /// Port of `PolylineTrace.boundingBox` (PolylineTrace.java:117-120) ->
    /// `PolylineTraceGeometry.boundingBox` (:39-41) = `lines.boundingBox().offset(halfWidth)`.
    ///
    /// Note this is *not* what Task 5's stub guessed (`lines.boundingBox(0, lineCount - 1)`) —
    /// the box is the whole polyline's, enlarged by the half width.
    pub fn bounding_box(&self) -> IntBox {
        self.lines.bounding_box().offset(f64::from(self.half_width))
    }

    /// Port of `PolylineTrace.tileShapeCount` (PolylineTrace.java:138-141) ->
    /// `PolylineTraceGeometry.tileShapeCount` (:43-45) = `Math.max(lines.lines.length - 2, 0)`.
    ///
    /// The `max` matters: Java's `int` subtraction would be negative for an empty polyline, and
    /// Task 5's stub dropped it.
    pub fn tile_shape_count(&self) -> usize {
        self.lines.lines().len().saturating_sub(2)
    }

    /// The trace's uncompensated tree shapes: `polyline.offsetShapes(halfWidth)`, which is what
    /// `ShapeSearchTree.calculateTreeShapes(PolylineTrace)` computes once it has added the
    /// clearance compensation to the half width.
    ///
    /// Not a `PolylineTrace` method in Java — the search tree calls `offsetShapes` on the
    /// polyline directly. It is exposed here so Task 10 has one place to add the compensation
    /// to, and so the geometry is testable without a tree.
    // The compensated caller is `ShapeSearchTree::calculate_tree_shapes`.
    pub fn offset_shapes(&self, half_width: i32) -> Vec<TileShape> {
        self.lines.offset_shapes(half_width)
    }

    /// Port of `Trace.nearestEndPoint` (Trace.java:255-269): whichever end corner is nearer to
    /// `from_point`.
    ///
    /// Java compares with a strict `<`, so a point equidistant from both ends answers the
    /// **last** corner. `None` stands for Java's `NullPointerException` on a polyline with no
    /// corners — `firstCorner()` would be `null` and `p1.toFloat()` would throw.
    // totalized: a corner-less polyline answers None instead of a NullPointerException; no Java
    // caller can reach it (the only one is the GUI's `RouteState.java:91`, which picks a trace
    // off the board, and `BasicBoard.insertTrace` rejects a polyline with under 3 lines).
    pub fn nearest_end_point(&self, from_point: &Point) -> Option<Point> {
        let p1 = self.first_corner()?;
        let p2 = self.last_corner()?;
        let from_point_float = from_point.to_float();
        let d1 = from_point_float.distance(&p1.to_float());
        let d2 = from_point_float.distance(&p2.to_float());
        Some(if d1 < d2 { p1 } else { p2 })
    }

    // -- transforms (PolylineTrace.java:143-168) -------------------------------------------------

    /// Port of `PolylineTrace.translateBy` (PolylineTrace.java:143-147) ->
    /// `PolylineTraceGeometry.translate` (:47-49).
    ///
    /// `Polyline.translateBy` re-runs the normalising `Polyline(Line[])` constructor, so it
    /// carries [`PolylineError`] (Plan 1 ruling 12: thread it, never swallow it).
    pub fn translate_by(&mut self, vector: &Vector) -> Result<(), PolylineError> {
        self.lines = self.lines.translate_by(vector)?;
        self.hdr.clear_derived_data();
        Ok(())
    }

    /// Port of `PolylineTrace.turn90Degree` (PolylineTrace.java:149-153) ->
    /// `PolylineTraceGeometry.turn90Degree` (:51-53).
    pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) -> Result<(), PolylineError> {
        self.lines = self.lines.turn_90_degree(factor, pole)?;
        self.hdr.clear_derived_data();
        Ok(())
    }

    /// Port of `PolylineTrace.rotateApprox` (PolylineTrace.java:155-159) ->
    /// `PolylineTraceGeometry.rotateApprox` (:55-57), which converts the angle to radians.
    //
    // Java bug: alone among the four transforms, this one does **not** call
    // `clearDerivedData()` — `translateBy` (:146), `turn90Degree` (:152) and
    // `changePlacementSide` (:167) all do, as do every sibling item's `rotateApprox`
    // (`DrillItem` :84, `Pin` :376, `ObstacleArea` :243, `ComponentOutline` :174). A rotated
    // trace therefore keeps the search-tree shapes and the autoroute scratch of its old
    // position. Reproduced; see docs/java-quirks.md #60.
    pub fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint) {
        self.lines = self.lines.rotate_approx(angle_in_degree.to_radians(), pole);
    }

    /// Port of `PolylineTrace.changePlacementSide` (PolylineTrace.java:160-168) ->
    /// `PolylineTraceGeometry.mirrorVertical` (:59-61), plus the layer flip.
    ///
    /// `ctx` replaces Java's `this.board`; Java guards the flip with `board != null`
    /// (PolylineTrace.java:164) and an [`ItemCtx`] is always present, so the guard is always
    /// taken — the same call [`crate::items::ObstacleArea::change_placement_side`] makes.
    ///
    /// # Panics
    /// If the trace's layer is at or past the end of the board stack; Java stores a negative
    /// layer there (see [`PolylineTrace::new`]).
    pub fn change_placement_side(
        &mut self,
        pole: &IntPoint,
        ctx: &ItemCtx<'_>,
    ) -> Result<(), PolylineError> {
        self.lines = self.lines.mirror_vertical(pole)?;
        let layer_count = ctx.rules.layer_structure().count();
        self.set_layer(layer_count.checked_sub(self.get_layer() + 1).expect(
            "PolylineTrace.changePlacementSide: layer >= board.getLayerCount() — Java stores a \
             negative layer here (PolylineTrace.java:165)",
        ));
        self.hdr.clear_derived_data();
        Ok(())
    }

    // -- the pure half of split (PolylineTrace.java:699-737) -------------------------------------

    /// The geometry core of the private `PolylineTrace.split(int lineIndex, Line newEndLine)`
    /// (PolylineTrace.java:719-760): `lines.split(lineIndex, newEndLine)` plus Java's
    /// "array of length 2 expected" guard (:734-737).
    ///
    /// Everything else in that method is board work — the `isOnTheBoard` and
    /// `isDeletionForbidden` guards (:720-729), `splitInsideDrillPadProhibited` (:738-740) and
    /// the `removeItem`/`insertTraceWithoutCleaning` pair (:741-758).
    // The board wrapper is `Board::split_trace_at_line`, which adds the two guards,
    // `splitInsideDrillPadProhibited` (PolylineTrace.java:768-792) and the remove/insert pair.
    pub fn split_polyline_at_line(
        &self,
        line_index: usize,
        new_end_line: &Line,
    ) -> Result<Option<[Polyline; 2]>, PolylineError> {
        // `Polyline::split` already answers `None` for Java's out-of-range warning and for the
        // "touches only at an end point" case, and it can only ever produce two pieces, so
        // Java's `splitPolylines.length != 2` guard is structurally satisfied by the `[_; 2]`.
        self.lines.split(line_index, new_end_line)
    }

    /// The line Java splits at when it wants to cut this trace at `point`: the perpendicular
    /// through `point` of segment `segment_index` — `segment.getLine().direction()
    /// .turn45Degree(2)`, then `new Line(point, thatDirection)`.
    ///
    /// `None` when `segment_index` is out of range, when the segment does not contain `point`,
    /// or when `point` is not an [`IntPoint`] — Java's `LineSegment.contains` already answers
    /// `false` for the last (LineSegment.java:155-171), so it never builds a line there.
    ///
    /// Java writes this three-line step twice, inline: once in `split(Point)`
    /// (PolylineTrace.java:702-704) and once in the `DrillItem` branch of `split(IntOctagon)`
    /// (PolylineTrace.java:655-660), which cuts the trace at a drill centre it passes through.
    /// It is a named function here so Task 9's board-side `split` can reuse it for that branch
    /// rather than re-deriving it.
    pub fn perpendicular_split_line(&self, segment_index: usize, point: &Point) -> Option<Line> {
        let segment = LineSegment::from_polyline(&self.lines, segment_index + 1)?;
        if !segment.contains(point) {
            return None;
        }
        let Point::Int(int_point) = point else {
            return None;
        };
        let split_line_direction = segment.get_line().direction().turn_45_degree(2);
        Some(Line::from_direction(*int_point, &split_line_direction))
    }

    /// The geometry core of `PolylineTrace.split(Point)` (PolylineTrace.java:698-712): scan the
    /// line segments for the first that contains `point`, build
    /// [`PolylineTrace::perpendicular_split_line`] through it, and split there.
    ///
    /// `None` is Java's `null` — either no segment contains the point, or every candidate split
    /// was refused by `Polyline.split`.
    ///
    /// **Task 9 must not wrap this method as-is.** Java's loop calls the *private*
    /// `split(int, Line)` (PolylineTrace.java:719-760) per candidate, and that method can refuse
    /// for four board reasons — the trace is off the board (:720-722), it is deletion-forbidden
    /// (:727-729), the polyline split failed (:731-733), or `splitInsideDrillPadProhibited`
    /// rejected the intersection (:738-740) — after which the `for` **continues to the next
    /// segment** (:706-709). So the board wrapper has to run the same per-segment loop with its
    /// own guards inside, using [`PolylineTrace::perpendicular_split_line`] and
    /// [`PolylineTrace::split_polyline_at_line`] as its two pure steps. This method is the
    /// board-free shape of that loop, and is what the geometry tests pin.
    // The board wrapper is `Board::split_trace_at_point` — the per-candidate loop described
    // above, which replaces the two returned polylines with two inserted traces.
    pub fn split_polyline_at_point(
        &self,
        point: &Point,
    ) -> Result<Option<[Polyline; 2]>, PolylineError> {
        for i in 0..self.tile_shape_count() {
            let Some(split_line) = self.perpendicular_split_line(i, point) else {
                continue;
            };
            if let Some(pieces) = self.split_polyline_at_line(i + 1, &split_line)? {
                return Ok(Some(pieces));
            }
        }
        Ok(None)
    }

    /// The `clipShape` filter of `PolylineTrace.split(IntOctagon)` (PolylineTrace.java:475-479):
    /// whether segment `index` of this trace is worth examining at all.
    ///
    /// Extracted because it is the only part of that method that needs no search tree; the loop
    /// it guards is Task 9's.
    // The board half of `PolylineTrace.split(IntOctagon)` (PolylineTrace.java:464-691) is
    // `Board::split_trace` — every intersection candidate comes from
    // `defaultTree.overlappingTreeEntries`, so nothing else of it is pure.
    pub fn clip_intersects_segment(&self, index: usize, clip: &IntOctagon) -> bool {
        match LineSegment::from_polyline(&self.lines, index + 1) {
            Some(segment) => clip.intersects_box(&segment.bounding_box()),
            // Java would build a `LineSegment` with null lines and throw in `boundingBox()`.
            None => false,
        }
    }
}

impl Connectable for PolylineTrace {
    fn header(&self) -> &ItemHeader {
        &self.hdr
    }

    /// Port of `PolylineTrace.getTraceConnectionShape` (PolylineTrace.java:917-924) ->
    /// `PolylineTraceGeometry.connectionShape` (:63-66): the one-dimensional simplex of line
    /// segment `index + 1`, simplified.
    ///
    /// Both the `ShapeSearchTree` and the [`ItemCtx`] are unused — Java's parameter is ignored
    /// by the body, and the trait's other implementors need the context.
    fn get_trace_connection_shape(
        &self,
        _tree: TreeId,
        index: usize,
        _ctx: &ItemCtx<'_>,
    ) -> Option<TileShape> {
        if index >= self.tile_shape_count() {
            // PolylineTrace.java:919-922 warns and returns null.
            return None;
        }
        let segment = LineSegment::from_polyline(&self.lines, index + 1)?;
        Some(segment.to_simplex().simplify())
    }
}
