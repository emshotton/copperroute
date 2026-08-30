//! Port of `app.freerouting.geometry.planar.Polyline`.
//!
//! "A Polyline is a sequence of lines, where no 2 consecutive lines may be parallel. A Polyline of
//! n lines defines a Polygon of n-1 intersection points of consecutive lines. The lines of the
//! objects of class Polyline are normally defined by points with integer coordinates, whereas the
//! intersections of Lines can be represented in general only by infinite precision rational
//! points. We use polylines with integer coordinates instead of polygons with infinite precision
//! rational coordinates because of its better performance in geometric calculations."
//! (Polyline.java:9-16)
//!
//! **Caching.** Java memoises the corners, the float corners and the bounding box in three
//! transient fields (Polyline.java:24-26). This port drops all three, as
//! [`crate::line_segment::LineSegment`] and [`crate::simplex::Simplex`] already do: every corner
//! is a pure function of two consecutive lines. The one place where the memo could have been
//! observable is the `Polyline(Line[])` constructor, which fills
//! `precalculatedFloatCorners[i]` *before* possibly replacing `lines[i]` by its opposite
//! (Polyline.java:85-100) — but `Line::intersection_approx` negates both the numerator and the
//! denominator when a line is flipped, which is exact in IEEE 754, so the recomputed value is
//! bit-identical.
//!
//! **Equality.** Java does not override `equals`, so Java `Polyline`s compare by identity. This
//! port derives structural equality over the line list, matching [`crate::line::Line`].
//!
// not ported: the private `debugPoint(Point)` helper (Polyline.java:859-864) and the two
// `FRLogger.trace` calls in `split` (Polyline.java:767-799) — diagnostics only.

use crate::direction::Direction;
use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::int_vector::IntVector;
use crate::limits::{java_max, java_min};
use crate::line::Line;
use crate::line_segment::LineSegment;
use crate::point::Point;
use crate::polygon::Polygon;
use crate::side::Side;
use crate::tile_shape::TileShape;
use crate::vector::Vector;

/// Java `Polyline.USE_BOUNDING_OCTAGON_FOR_OFFSET_SHAPES` (Polyline.java:19).
const USE_BOUNDING_OCTAGON_FOR_OFFSET_SHAPES: bool = true;

/// The one failure the `Polyline(Line[])` normalisation can produce.
///
/// Java signals it by *crashing* — `removeOverlaps` reads `tmpArr[-1]` and throws
/// `ArrayIndexOutOfBoundsException` (Polyline.java:147-155). That crash is observable by a real
/// caller: `PolylineTrace.combine_at_end` (PolylineTrace.java:303-311) builds
/// `joinedPolyline = new Polyline(newLines)` and then compares `joinedPolyline.lines.length`
/// against `newLineCount`, so swallowing the crash into an *empty* polyline would silently
/// replace a trace's geometry with nothing, where Java aborts the whole autorouting pass through
/// its pass-level `catch (Exception)`. Surfacing it as an error keeps that choice with the
/// caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolylineError {
    /// `Polyline(Line[])`'s overlap removal consumed its whole output buffer and Java would read
    /// index -1 (Polyline.java:148).
    NormalizationIndexUnderflow,
}

impl std::fmt::Display for PolylineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolylineError::NormalizationIndexUnderflow => f.write_str(
                "Polyline normalisation: removeOverlaps ran out of lines \
                 (Java throws ArrayIndexOutOfBoundsException: Index -1, Polyline.java:148)",
            ),
        }
    }
}

impl std::error::Error for PolylineError {}

/// A sequence of lines, where no 2 consecutive lines may be parallel.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Polyline {
    lines: Vec<Line>,
}

// -------------------------------------------------------------------------------------------
// Constructors (Polyline.java:28-176)
// -------------------------------------------------------------------------------------------

impl Polyline {
    /// Creates a polyline of length `polygon.corner_count + 1` from `polygon`, so that the i-th
    /// corner of `polygon` is the intersection of the i-th and the i+1-th lines of the new
    /// polyline. `polygon` must have at least 2 corners (Polyline.java:28-52).
    ///
    /// # Panics
    /// If any corner is a [`Point::Rational`]. Java only logs "Line(a, b) only implemented for
    /// IntPoints till now" and builds a `Line` that every later operation rejects with a
    /// `ClassCastException`; this port's [`Line`] cannot hold rational end points at all, so the
    /// break is moved forward to the constructor — as in [`crate::int_box::IntBox::translate_by`].
    // totalized: rational polygon corners panic here instead of failing later in Java.
    pub fn from_polygon(polygon: &Polygon) -> Polyline {
        let points = polygon.corner_array();
        if points.len() < 2 {
            // Java: FRLogger.warn("Polyline: must contain at least 2 different points")
            return Polyline { lines: Vec::new() };
        }
        let pts: Vec<IntPoint> = points.iter().map(int_point_of).collect();
        let mut lines: Vec<Line> = Vec::with_capacity(pts.len() + 1);
        // Placeholder for lines[0], overwritten below; Java allocates the array with a null slot.
        lines.push(Line::new(pts[0], pts[0]));
        for i in 1..pts.len() {
            lines.push(Line::new(pts[i - 1], pts[i]));
        }
        // construct perpendicular lines at the start and at the end to represent
        // the first and the last point of points as intersection of lines.

        let dir = Direction::between(&points[0], &points[1])
            .expect("Polygon has no two equal consecutive corners");
        lines[0] = line_from_direction(pts[0], &dir.turn_45_degree(2));

        let last = pts.len() - 1;
        let dir = Direction::between(&points[last], &points[last - 1])
            .expect("Polygon has no two equal consecutive corners");
        lines.push(line_from_direction(pts[last], &dir.turn_45_degree(2)));
        Polyline { lines }
    }

    /// Creates a polyline from an array of points (Polyline.java:54-57).
    pub fn from_points(points: &[Point]) -> Polyline {
        Polyline::from_polygon(&Polygon::new(points.to_vec()))
    }

    /// Creates a polyline consisting of three lines (Polyline.java:59-71).
    ///
    /// Note that Java recomputes the closing direction as `from_corner -> to_corner` (line 69),
    /// not `to_corner -> from_corner` as [`Polyline::from_polygon`] does, so the two constructors
    /// produce opposite (but geometrically identical) closing lines for the same two points.
    // Java bug: Polyline.java:69 repeats line 66 verbatim; see docs/java-quirks.md.
    pub fn from_two_points(from_corner: &Point, to_corner: &Point) -> Polyline {
        if from_corner == to_corner {
            return Polyline { lines: Vec::new() };
        }
        let from = int_point_of(from_corner);
        let to = int_point_of(to_corner);
        let dir = Direction::between(from_corner, to_corner).expect("the corners differ");
        let l0 = line_from_direction(from, &dir.turn_45_degree(2));
        let l1 = Line::new(from, to);
        let dir = Direction::between(from_corner, to_corner).expect("the corners differ");
        let l2 = line_from_direction(to, &dir.turn_45_degree(2));
        Polyline {
            lines: vec![l0, l1, l2],
        }
    }

    /// Creates a polyline from an array of lines. Lines which are parallel to the previous line
    /// are skipped. The directed lines are normalized, so that they intersect the previous line
    /// before the next line (Polyline.java:73-102).
    ///
    /// This is the normalising constructor that every trace transformation funnels through.
    ///
    /// Returns [`PolylineError::NormalizationIndexUnderflow`] on the one input class where Java
    /// throws; every path Java completes normally — including its two "fewer than 3 lines"
    /// exits, which yield an empty polyline — is an `Ok`.
    pub fn from_lines(input_lines: Vec<Line>) -> Result<Polyline, PolylineError> {
        Ok(Polyline::build(input_lines)?.0)
    }

    /// [`Polyline::from_lines`] for the callers that **re-read their own array afterwards**.
    ///
    /// Java's `new Polyline(Line[])` normalises the caller's array **in place**, and six Plan 6
    /// call sites depend on it (quirk #185's neighbour — see `docs/java-quirks.md`):
    ///
    /// * `removeConsecutiveParallelLines` (Polyline.java:104-131) `return lines` — *the caller's
    ///   own array object* — for `length < 3` and when nothing is skipped;
    /// * `removeOverlaps` (Polyline.java:133-176) does the same for `length < 4` and when nothing
    ///   is skipped;
    /// * the constructor then writes `filteredLines[i] = filteredLines[i].opposite()`
    ///   (Polyline.java:97) into whatever array it was handed.
    ///
    /// So when — and only when — **neither** normaliser skipped a line, the caller sees the
    /// flipped directions, and (in this port) the **new identity tokens** those flipped lines
    /// carry. `TraceTightener.java:297` → `:311`, `TraceTightener45.java:421` → `:435` and
    /// `TraceTightenerAnyAngle.java:158` → `:186`, `:368` → `:386`, `:451` → `:465`,
    /// `:614` → `:625` all construct a `Polyline` from a local array and then read an element of
    /// that array back out; the value they read is Java's post-normalisation one. The
    /// `TraceTightenerAnyAngle.java:368` site is the strongest of the six: `:386`'s
    /// `lines = currentLines` makes the whole normalised array the loop's working state and the
    /// source of the polyline the method returns.
    ///
    /// `from_lines` consumes its `Vec` and can express none of that, which is why this method
    /// exists. Java's two early `return`s before the flip loop (either normaliser answering an
    /// empty array, or fewer than 3 lines surviving) leave the caller's array untouched, and so
    /// does this.
    pub fn from_lines_in_place(input_lines: &mut Vec<Line>) -> Result<Polyline, PolylineError> {
        let (polyline, writes_through) = Polyline::build(input_lines.clone())?;
        if writes_through {
            input_lines.clone_from(&polyline.lines);
        }
        Ok(polyline)
    }

    /// The body of `new Polyline(Line[])` (Polyline.java:73-102).
    ///
    /// The second half of the answer is Java's array aliasing: `true` when the array the flip
    /// loop wrote into **is** the one that was passed in, so a caller holding that array sees the
    /// result. See [`Polyline::from_lines_in_place`].
    fn build(input_lines: Vec<Line>) -> Result<(Polyline, bool), PolylineError> {
        let input_len = input_lines.len();
        let filtered_lines = remove_consecutive_parallel_lines(input_lines);
        let mut filtered_lines = remove_overlaps(filtered_lines)?;
        if filtered_lines.len() < 3 {
            // Java returns here, *before* the loop below, so nothing is written back.
            return Ok((Polyline { lines: Vec::new() }, false));
        }
        // Neither normaliser copied iff the array still has every line it started with: both
        // answer their input unchanged, or a strictly shorter array, or an empty one.
        let writes_through = filtered_lines.len() == input_len;

        // turn evtl the direction of the lines that they point always
        // from the previous corner to the next corner
        for i in 1..filtered_lines.len() - 1 {
            let corner = filtered_lines[i].intersection_approx(&filtered_lines[i + 1]);
            let side_of_line = filtered_lines[i - 1].side_of_float_exact(&corner);
            if side_of_line != Side::Collinear {
                let d0 = filtered_lines[i - 1].direction();
                let d1 = filtered_lines[i].direction();
                let side1 = d0.side_of(&d1);
                if side1 != side_of_line {
                    filtered_lines[i] = filtered_lines[i].opposite();
                }
            }
        }
        Ok((
            Polyline {
                lines: filtered_lines,
            },
            writes_through,
        ))
    }
}

/// Java `new Line(Point, Point)` accepts the abstract `Point` and only warns for the rational
/// case; this port's `Line` is fixed at `IntPoint`.
fn int_point_of(point: &Point) -> IntPoint {
    match point {
        Point::Int(p) => *p,
        Point::Rational(_) => panic!(
            "Polyline: only implemented for IntPoints till now (Polyline.java:42, Line.java:19-27)"
        ),
    }
}

/// Java `Line.getInstance(Point, Direction)`; a `Direction::Big` cannot be represented as an
/// offset between two `IntPoint`s (see [`Line::from_direction_any`]).
fn line_from_direction(a: IntPoint, dir: &Direction) -> Line {
    Line::from_direction_any(a, dir)
        .expect("a Direction derived from IntPoints is always an IntDirection")
}

/// Polyline.java:104-131.
fn remove_consecutive_parallel_lines(lines: Vec<Line>) -> Vec<Line> {
    if lines.len() < 3 {
        // polyline must have at least 3 lines
        return lines;
    }
    let mut tmp_arr: Vec<Line> = Vec::with_capacity(lines.len());
    tmp_arr.push(lines[0]);
    for line in lines.iter().skip(1) {
        // skip multiple lines
        if !tmp_arr[tmp_arr.len() - 1].is_parallel(line) {
            tmp_arr.push(*line);
        }
    }
    let new_length = tmp_arr.len();
    if new_length == lines.len() {
        // nothing skipped
        return lines;
    }
    // at least 1 line is skipped, adjust the array
    if new_length < 3 {
        return Vec::new();
    }
    tmp_arr
}

/// Checks if previous and next lines are equal or opposite and removes the resulting overlap
/// (Polyline.java:133-176).
///
/// Returns `Err` where Java throws `ArrayIndexOutOfBoundsException`: when the loop has already
/// decremented `newLength` to 0, `tmpArr[newLength - 1]` reads index -1. Reachable — e.g. the six
/// lines `h, v, h, v, h, v` over the same two axes, and ~11% of random line arrays drawn from a
/// small pool of equal/opposite lines.
// Java bug: Polyline.java:148 reads tmpArr[-1]; surfaced as Err rather than swallowed, because
// PolylineTrace.combine_at_end can tell an empty polyline from a thrown exception.
fn remove_overlaps(lines: Vec<Line>) -> Result<Vec<Line>, PolylineError> {
    if lines.len() < 4 {
        return Ok(lines);
    }
    let mut new_length: usize = 0;
    // Java's `new Line[lines.length]` is null-filled; the filler here is a real `Line`, so an
    // unwritten in-bounds slot silently carries a value *and an identity token* (quirk #74) where
    // Java would NPE. `newLength` bounds the truncation below, so no unwritten slot survives.
    let mut tmp_arr: Vec<Line> = vec![Line::new(IntPoint::ZERO, IntPoint::ZERO); lines.len()];
    tmp_arr[0] = lines[0];
    if !lines[0].is_equal_or_opposite(&lines[2]) {
        new_length += 1;
    }
    // else skip the first line
    tmp_arr[new_length] = lines[1];
    new_length += 1;
    for i in 2..lines.len() - 2 {
        if new_length == 0 {
            // Java reads tmpArr[-1] here and throws.
            return Err(PolylineError::NormalizationIndexUnderflow);
        }
        if tmp_arr[new_length - 1].is_equal_or_opposite(&lines[i + 1]) {
            // skip 2 lines
            new_length -= 1;
        } else {
            tmp_arr[new_length] = lines[i];
            new_length += 1;
        }
    }
    tmp_arr[new_length] = lines[lines.len() - 2];
    new_length += 1;
    // Guard: newLength must be >= 2 before accessing tmpArr[newLength - 2].
    // If the loop decremented newLength all the way to 0 the index would be -1.
    if new_length >= 2 && !lines[lines.len() - 1].is_equal_or_opposite(&tmp_arr[new_length - 2]) {
        tmp_arr[new_length] = lines[lines.len() - 1];
        new_length += 1;
    }
    // else skip the last line
    if new_length == lines.len() {
        // nothing skipped
        return Ok(lines);
    }
    // at least 1 line is skipped, adjust the array
    if new_length < 3 {
        return Ok(Vec::new());
    }
    tmp_arr.truncate(new_length);
    Ok(tmp_arr)
}

// -------------------------------------------------------------------------------------------
// Accessors (Polyline.java:178-318)
// -------------------------------------------------------------------------------------------

impl Polyline {
    /// The lines of this polyline (Java's public `lines` field, Polyline.java:22).
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Returns the number of lines minus 1 (Polyline.java:178-181).
    ///
    /// Java returns -1 for an empty polyline; `usize` saturates at 0 instead.
    // totalized: cornerCount() of an empty polyline is 0 here, -1 in Java.
    pub fn corner_count(&self) -> usize {
        self.lines.len().saturating_sub(1)
    }

    /// Polyline.java:183-185.
    pub fn is_empty(&self) -> bool {
        self.lines.len() < 3
    }

    /// Checks if this polyline is empty or if all corner points are equal
    /// (Polyline.java:187-199).
    pub fn is_point(&self) -> bool {
        if self.lines.len() < 3 {
            return true;
        }
        let first_corner = self.corner_at(0);
        for i in 1..self.lines.len() - 1 {
            if self.corner_at(i) != first_corner {
                return false;
            }
        }
        true
    }

    /// Checks if all lines of this polyline are orthogonal (Polyline.java:201-209).
    pub fn is_orthogonal(&self) -> bool {
        self.lines.iter().all(Line::is_orthogonal)
    }

    /// Checks if all lines of this polyline are multiples of 45 degrees (Polyline.java:211-219).
    pub fn is_multiple_of_45_degree(&self) -> bool {
        self.lines.iter().all(Line::is_multiple_of_45_degree)
    }

    /// Returns the intersection of the first line with the second line (Polyline.java:221-224).
    pub fn first_corner(&self) -> Option<Point> {
        self.corner(0)
    }

    /// Returns the intersection of the last line with the line before the last line
    /// (Polyline.java:226-229).
    pub fn last_corner(&self) -> Option<Point> {
        if self.lines.len() < 2 {
            return None;
        }
        self.corner(self.lines.len() - 2)
    }

    /// Returns the intersections of every two consecutive lines (Polyline.java:231-248).
    pub fn corners(&self) -> Vec<Point> {
        if self.lines.len() < 2 {
            return Vec::new();
        }
        (0..self.lines.len() - 1)
            .map(|i| self.corner_at(i))
            .collect()
    }

    /// Returns the intersections of consecutive lines, approximated by `FloatPoint` values
    /// (Polyline.java:250-265).
    pub fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        if self.lines.len() < 2 {
            return Vec::new();
        }
        (0..self.lines.len() - 1)
            .map(|i| self.corner_approx_at(i))
            .collect()
    }

    /// Returns an approximation of the intersection of the `no`-th with the `no + 1`-th line
    /// (Polyline.java:267-291).
    ///
    /// Java clamps an out-of-range index (with a warning) and would then index a negative-length
    /// array for an empty polyline; `None` stands for that crash.
    // totalized: an empty polyline answers None instead of NegativeArraySizeException.
    pub fn corner_approx(&self, no: usize) -> Option<FloatPoint> {
        if self.lines.len() < 2 {
            return None;
        }
        Some(self.corner_approx_at(no))
    }

    /// Returns the intersection of the `no`-th with the `no + 1`-th edge line
    /// (Polyline.java:293-318). `None` where Java returns `null`, i.e. for `lines.length < 2`.
    pub fn corner(&self, no: usize) -> Option<Point> {
        if self.lines.len() < 2 {
            // Java: FRLogger.trace("Polyline.corner: lines.length is < 2"); return null;
            return None;
        }
        Some(self.corner_at(no))
    }

    /// [`Polyline::corner`] for a polyline that is known to have at least 2 lines; clamps the
    /// index exactly as Java's warning branches do.
    fn corner_at(&self, no: usize) -> Point {
        let no = self.clamp_corner_index(no as i64);
        self.lines[no].intersection(&self.lines[no + 1])
    }

    /// [`Polyline::corner_approx`] for a polyline that is known to have at least 2 lines.
    fn corner_approx_at(&self, no: usize) -> FloatPoint {
        let no = self.clamp_corner_index(no as i64);
        self.lines[no].intersection_approx(&self.lines[no + 1])
    }

    /// Java's shared index clamping (Polyline.java:272-281 and 299-308): a negative index becomes
    /// 0 and an index past the last corner becomes `lines.length - 2`. Both branches log a
    /// warning, which is dropped here.
    fn clamp_corner_index(&self, corner_index: i64) -> usize {
        debug_assert!(self.lines.len() >= 2);
        if corner_index < 0 {
            0
        } else if corner_index >= self.lines.len() as i64 - 1 {
            self.lines.len() - 2
        } else {
            corner_index as usize
        }
    }
}

// -------------------------------------------------------------------------------------------
// Derived polylines and measurements (Polyline.java:320-343)
// -------------------------------------------------------------------------------------------

impl Polyline {
    /// Returns the polyline with the reversed order of lines (Polyline.java:320-327).
    ///
    /// Routes through [`Polyline::from_lines`], so it inherits its error.
    pub fn reverse(&self) -> Result<Polyline, PolylineError> {
        let reversed: Vec<Line> = self.lines.iter().rev().map(Line::opposite).collect();
        Polyline::from_lines(reversed)
    }

    /// Calculates the length of this polyline from `from_corner` to `to_corner`
    /// (Polyline.java:329-338).
    pub fn length_approx_between(&self, from_corner: usize, to_corner: usize) -> f64 {
        if self.lines.len() < 2 {
            return 0.0;
        }
        let to_corner = to_corner.min(self.lines.len() - 2);
        let mut result = 0.0;
        let mut i = from_corner;
        while i < to_corner {
            result += self
                .corner_approx_at(i + 1)
                .distance(&self.corner_approx_at(i));
            i += 1;
        }
        result
    }

    /// Calculates the cumulative distance between consecutive corners (Polyline.java:340-343).
    pub fn length_approx(&self) -> f64 {
        if self.lines.len() < 2 {
            return 0.0;
        }
        self.length_approx_between(0, self.lines.len() - 2)
    }
}

// -------------------------------------------------------------------------------------------
// Offset shapes (Polyline.java:345-534) — the trace-to-shape conversion used by the search tree.
// -------------------------------------------------------------------------------------------

impl Polyline {
    /// Calculates for each line a shape around this line where the right and left edge lines have
    /// the distance `half_width` from the center line. Returns `line_count - 2` convex shapes
    /// (Polyline.java:345-352).
    pub fn offset_shapes(&self, half_width: i32) -> Vec<TileShape> {
        if self.lines.is_empty() {
            return Vec::new();
        }
        self.offset_shapes_between(half_width, 0, self.lines.len() - 1)
    }

    /// Calculates for each line between `from_no` and `to_no` a shape around this line, where the
    /// right and left edge lines have the distance `half_width` from the center line
    /// (Polyline.java:354-511).
    pub fn offset_shapes_between(
        &self,
        half_width: i32,
        from_no: usize,
        to_no: usize,
    ) -> Vec<TileShape> {
        if self.lines.is_empty() {
            return Vec::new();
        }
        // Java: fromNo = Math.max(requestedFromNo, 0) — automatic for a usize.
        let to_no = to_no.min(self.lines.len() - 1);
        let shape_count = (to_no as i64 - from_no as i64 - 1).max(0) as usize;
        let mut shapes: Vec<TileShape> = Vec::with_capacity(shape_count);
        if shape_count == 0 {
            return shapes;
        }
        let mut prev_dir = self.lines[from_no].direction().get_vector();
        let mut current_direction = self.lines[from_no + 1].direction().get_vector();
        for i in from_no + 1..to_no {
            let next_dir = self.lines[i + 1].direction().get_vector();

            let mut offset_lines: [Line; 4] = [Line::new(IntPoint::ZERO, IntPoint::ZERO); 4];

            offset_lines[0] = self.lines[i].translate(-half_width as f64);
            // current center line translated to the right

            // create the front line of the offset shape
            let next_dir_from_curr_dir = next_dir.side_of(&current_direction);
            // left turn from currentLine to nextLine
            if next_dir_from_curr_dir == Side::OnTheLeft {
                offset_lines[1] = self.lines[i + 1].translate(-half_width as f64);
                // next right line
            } else {
                offset_lines[1] = self.lines[i + 1].opposite().translate(-half_width as f64);
                // next left line in opposite direction
            }

            offset_lines[2] = self.lines[i].opposite().translate(-half_width as f64);
            // current left line in opposite direction

            // create the back line of the offset shape
            let current_dir_from_prev_dir = current_direction.side_of(&prev_dir);
            // left turn from prevLine to currentLine
            if current_dir_from_prev_dir == Side::OnTheLeft {
                offset_lines[3] = self.lines[i - 1].translate(-half_width as f64);
                // previous line translated to the right
            } else {
                offset_lines[3] = self.lines[i - 1].opposite().translate(-half_width as f64);
                // previous left line in opposite direction
            }
            // cut off outstanding corners with following shapes
            let mut corner_to_check: Option<FloatPoint> = None;
            let mut current_line = offset_lines[1];
            let mut check_line = if next_dir_from_curr_dir == Side::OnTheLeft {
                offset_lines[2]
            } else {
                offset_lines[0]
            };
            let mut check_distance_corner = self.corner_approx_at(i);
            let check_dist_square = 2.0 * half_width as f64 * half_width as f64;
            let mut cut_dog_ear_lines: Vec<Line> = Vec::new();
            let mut tmp_curr_dir = next_dir;
            let mut direction_changed = false;
            for j in i + 2..self.lines.len() - 1 {
                if self
                    .corner_approx_at(j - 1)
                    .distance_square(&check_distance_corner)
                    > check_dist_square
                {
                    break;
                }
                if !direction_changed {
                    corner_to_check = Some(current_line.intersection_approx(&check_line));
                }
                let tmp_next_dir = self.lines[j].direction().get_vector();
                let tmp_next_dir_from_tmp_curr_dir = tmp_next_dir.side_of(&tmp_curr_dir);
                direction_changed = tmp_next_dir_from_tmp_curr_dir != next_dir_from_curr_dir;
                if !direction_changed {
                    let next_border_line = if tmp_next_dir_from_tmp_curr_dir == Side::OnTheLeft {
                        self.lines[j].translate(-half_width as f64)
                    } else {
                        self.lines[j].opposite().translate(-half_width as f64)
                    };

                    let corner = corner_to_check.expect("assigned on the first pass");
                    if next_border_line.side_of_float_exact(&corner) == Side::OnTheLeft
                        && next_border_line.side_of(&self.corner_at(i)) == Side::OnTheRight
                        && next_border_line.side_of(&self.corner_at(i - 1)) == Side::OnTheRight
                    {
                        // an outstanding corner
                        cut_dog_ear_lines.push(next_border_line);
                    }
                    tmp_curr_dir = tmp_next_dir;
                    current_line = next_border_line;
                }
            }
            // cut off outstanding corners with previous shapes
            check_distance_corner = self.corner_approx_at(i - 1);
            check_line = if current_dir_from_prev_dir == Side::OnTheLeft {
                offset_lines[2]
            } else {
                offset_lines[0]
            };
            current_line = offset_lines[3];
            tmp_curr_dir = prev_dir;
            direction_changed = false;
            let mut j = i as i64 - 2;
            while j >= 1 {
                let ju = j as usize;
                if self
                    .corner_approx_at(ju)
                    .distance_square(&check_distance_corner)
                    > check_dist_square
                {
                    break;
                }
                if !direction_changed {
                    corner_to_check = Some(current_line.intersection_approx(&check_line));
                }
                let tmp_prev_dir = self.lines[ju].direction().get_vector();
                let tmp_curr_dir_from_tmp_prev_dir = tmp_curr_dir.side_of(&tmp_prev_dir);
                direction_changed = tmp_curr_dir_from_tmp_prev_dir != current_dir_from_prev_dir;
                if !direction_changed {
                    let prev_border_line = if tmp_curr_dir.side_of(&tmp_prev_dir) == Side::OnTheLeft
                    {
                        self.lines[ju].translate(-half_width as f64)
                    } else {
                        self.lines[ju].opposite().translate(-half_width as f64)
                    };
                    let corner = corner_to_check.expect("assigned on the first pass");
                    if prev_border_line.side_of_float_exact(&corner) == Side::OnTheLeft
                        && prev_border_line.side_of(&self.corner_at(i)) == Side::OnTheRight
                        && prev_border_line.side_of(&self.corner_at(i - 1)) == Side::OnTheRight
                    {
                        // an outstanding corner
                        cut_dog_ear_lines.push(prev_border_line);
                    }
                    tmp_curr_dir = tmp_prev_dir;
                    current_line = prev_border_line;
                }
                j -= 1;
            }
            let mut s1 = TileShape::get_instance_from_lines(offset_lines.to_vec());
            if !cut_dog_ear_lines.is_empty() {
                s1 = s1.intersection(&TileShape::get_instance_from_lines(cut_dog_ear_lines));
            }
            let bounding_shape = if USE_BOUNDING_OCTAGON_FOR_OFFSET_SHAPES {
                // intersect with the bounding octagon
                let surr_oct = self.bounding_octagon_between(i - 1, i);
                TileShape::Octagon(surr_oct.offset(half_width as f64))
            } else {
                // intersect with the bounding box
                let surr_box = self.bounding_box_between(i - 1, i);
                let offset_box = surr_box.offset(half_width as f64);
                TileShape::Simplex(offset_box.to_simplex())
            };
            // Java warns when the resulting shape is empty; diagnostic only.
            shapes.push(bounding_shape.intersection_with_simplify(&s1));

            prev_dir = current_direction;
            current_direction = next_dir;
        }
        shapes
    }

    /// Calculates for the `no`-th line segment a shape around this line where the right and left
    /// edge lines have the distance `half_width` from the center line, for
    /// `0 <= no <= lines.length - 3` (Polyline.java:513-525). `None` where Java returns `null`.
    pub fn offset_shape(&self, half_width: i32, no: usize) -> Option<TileShape> {
        if no + 3 > self.lines.len() {
            // Java: FRLogger.warn("Polyline.offsetShape: no out of range")
            return None;
        }
        let result = self.offset_shapes_between(half_width, no, no + 2);
        result.into_iter().next()
    }

    /// Calculates for the `no`-th line segment a box shape around this line where the border
    /// lines have the distance `half_width` from the center line (Polyline.java:527-534).
    ///
    /// `None` where Java's `new LineSegment(this, no + 1)` is out of range: Java stores three
    /// `null` lines and then throws a `NullPointerException` in `boundingBox()`.
    // totalized: an out-of-range index answers None instead of NullPointerException.
    pub fn offset_box(&self, half_width: i32, no: usize) -> Option<IntBox> {
        let current_line_segment = LineSegment::from_polyline(self, no + 1)?;
        Some(
            current_line_segment
                .bounding_box()
                .offset(half_width as f64),
        )
    }
}

// -------------------------------------------------------------------------------------------
// Transformations (Polyline.java:536-586)
// -------------------------------------------------------------------------------------------

impl Polyline {
    /// Returns the polyline translated by `vector` (Polyline.java:536-546).
    ///
    /// # Panics
    /// For a [`Vector::Rational`] — see [`crate::simplex::Simplex::translate_by`].
    // totalized: a Vector::Rational panics here; Java's ClassCastException would come later.
    pub fn translate_by(&self, vector: &Vector) -> Result<Polyline, PolylineError> {
        if *vector == Vector::ZERO {
            return Ok(self.clone());
        }
        let v: IntVector = match vector {
            Vector::Int(v) => *v,
            Vector::Rational(_) => {
                panic!("Polyline::translate_by: only implemented for Vector::Int")
            }
        };
        Polyline::from_lines(self.lines.iter().map(|l| l.translate_by(&v)).collect())
    }

    /// Returns the polyline turned by `factor` times 90 degrees around `pole`
    /// (Polyline.java:548-555).
    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Result<Polyline, PolylineError> {
        Polyline::from_lines(
            self.lines
                .iter()
                .map(|l| l.turn_90_degree(factor, pole))
                .collect(),
        )
    }

    /// Returns an approximation of this polyline rotated around `pole` (Polyline.java:557-568).
    pub fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> Polyline {
        if angle == 0.0 {
            return self.clone();
        }
        let new_corners: Vec<Point> = (0..self.corner_count())
            .map(|i| Point::Int(self.corner_approx_at(i).rotate(angle, pole).round()))
            .collect();
        Polyline::from_points(&new_corners)
    }

    /// Mirrors this polyline at the vertical line through `pole` (Polyline.java:570-577).
    pub fn mirror_vertical(&self, pole: &IntPoint) -> Result<Polyline, PolylineError> {
        Polyline::from_lines(self.lines.iter().map(|l| l.mirror_vertical(pole)).collect())
    }

    /// Mirrors this polyline at the horizontal line through `pole` (Polyline.java:579-586).
    pub fn mirror_horizontal(&self, pole: &IntPoint) -> Result<Polyline, PolylineError> {
        Polyline::from_lines(
            self.lines
                .iter()
                .map(|l| l.mirror_horizontal(pole))
                .collect(),
        )
    }
}

// -------------------------------------------------------------------------------------------
// Bounding shapes and distances (Polyline.java:588-691)
// -------------------------------------------------------------------------------------------

impl Polyline {
    /// Returns the smallest box containing the intersection points from index `from_corner_no` to
    /// index `to_corner_no` of the lines of this polyline (Polyline.java:588-609).
    pub fn bounding_box_between(&self, from_corner_no: usize, to_corner_no: usize) -> IntBox {
        let mut llx = i32::MAX as f64;
        let mut lly = llx;
        let mut urx = i32::MIN as f64;
        let mut ury = urx;
        if self.lines.len() >= 2 {
            let to_corner_no = to_corner_no.min(self.lines.len() - 2);
            let mut i = from_corner_no;
            while i <= to_corner_no {
                let current_corner = self.corner_approx_at(i);
                llx = java_min(llx, current_corner.x);
                lly = java_min(lly, current_corner.y);
                urx = java_max(urx, current_corner.x);
                ury = java_max(ury, current_corner.y);
                i += 1;
            }
        }
        let lower_left = IntPoint::new(llx.floor() as i32, lly.floor() as i32);
        let upper_right = IntPoint::new(urx.ceil() as i32, ury.ceil() as i32);
        IntBox::new(lower_left, upper_right)
    }

    /// Returns the smallest box containing the intersection points of the lines of this polyline
    /// (Polyline.java:611-617).
    pub fn bounding_box(&self) -> IntBox {
        self.bounding_box_between(0, self.corner_count().saturating_sub(1))
    }

    /// Returns the smallest octagon containing the intersection points from index `from_corner_no`
    /// to index `to_corner_no` of the lines of this polyline (Polyline.java:619-656).
    pub fn bounding_octagon_between(
        &self,
        from_corner_no: usize,
        to_corner_no: usize,
    ) -> IntOctagon {
        let mut lx = i32::MAX as f64;
        let mut ly = i32::MAX as f64;
        let mut rx = i32::MIN as f64;
        let mut uy = i32::MIN as f64;
        let mut ulx = i32::MAX as f64;
        let mut lrx = i32::MIN as f64;
        let mut llx = i32::MAX as f64;
        let mut urx = i32::MIN as f64;
        if self.lines.len() >= 2 {
            let to_corner_no = to_corner_no.min(self.lines.len() - 2);
            let mut i = from_corner_no;
            while i <= to_corner_no {
                let current = self.corner_approx_at(i);
                lx = java_min(lx, current.x);
                ly = java_min(ly, current.y);
                rx = java_max(rx, current.x);
                uy = java_max(uy, current.y);
                let mut tmp = current.x - current.y;
                ulx = java_min(ulx, tmp);
                lrx = java_max(lrx, tmp);
                tmp = current.x + current.y;
                llx = java_min(llx, tmp);
                urx = java_max(urx, tmp);
                i += 1;
            }
        }
        IntOctagon::new(
            lx.floor() as i32,
            ly.floor() as i32,
            rx.ceil() as i32,
            uy.ceil() as i32,
            ulx.floor() as i32,
            lrx.ceil() as i32,
            llx.floor() as i32,
            urx.ceil() as i32,
        )
    }

    /// Calculates an approximation of the nearest point on this polyline to `from_point`
    /// (Polyline.java:658-686). `None` where Java returns `null`, i.e. for a polyline without
    /// corners.
    pub fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        let mut min_distance = f64::MAX;
        let mut nearest_point: Option<FloatPoint> = None;
        // calculate the nearest corner point
        let corners = self.corner_approx_arr();
        for corner in &corners {
            let current_distance = corner.distance(from_point);
            if current_distance < min_distance {
                min_distance = current_distance;
                nearest_point = Some(*corner);
            }
        }
        const CTOLERANCE: f64 = 1.0;
        for i in 1..self.lines.len().saturating_sub(1) {
            let projection = from_point.projection_approx(&self.lines[i]);
            let current_distance = projection.distance(from_point);
            if current_distance < min_distance {
                // look, if the projection is inside the segment
                let segment_length = corners[i].distance(&corners[i - 1]);
                if projection.distance(&corners[i]) + projection.distance(&corners[i - 1])
                    < segment_length + CTOLERANCE
                {
                    min_distance = current_distance;
                    nearest_point = Some(projection);
                }
            }
        }
        nearest_point
    }

    /// Calculates the distance of `from_point` to the nearest point on this polyline
    /// (Polyline.java:688-691).
    ///
    /// Java throws a `NullPointerException` for a polyline without corners; this port answers
    /// `f64::MAX`, as `TileShape::distance` does for a shape without border lines.
    // totalized: NullPointerException becomes f64::MAX.
    pub fn distance(&self, from_point: &FloatPoint) -> f64 {
        match self.nearest_point_approx(from_point) {
            Some(p) => from_point.distance(&p),
            None => f64::MAX,
        }
    }
}

// -------------------------------------------------------------------------------------------
// Combine / split / skip (Polyline.java:693-857)
// -------------------------------------------------------------------------------------------

impl Polyline {
    /// Combines the two polylines, if they have a common end corner. The order of lines in this
    /// polyline is preserved. Returns the combined polyline, or a copy of this polyline if this
    /// polyline and `other` have no common end corner. If there is something to combine at the
    /// start of this polyline, `other` is inserted in front of this polyline; if at the end, this
    /// polyline is inserted in front of `other` (Polyline.java:693-749).
    ///
    /// Java's "no common endpoint" answer is the receiver itself, not `null`, so this returns a
    /// `Polyline` rather than an `Option`.
    pub fn combine(&self, other: &Polyline) -> Result<Polyline, PolylineError> {
        if self.lines.len() < 3 || other.lines.len() < 3 {
            return Ok(self.clone());
        }
        let (combine_at_start, combine_other_at_start) =
            if self.first_corner() == other.first_corner() {
                (true, true)
            } else if self.first_corner() == other.last_corner() {
                (true, false)
            } else if self.last_corner() == other.first_corner() {
                (false, true)
            } else if self.last_corner() == other.last_corner() {
                (false, false)
            } else {
                return Ok(self.clone()); // no common endpoint
            };
        let mut new_lines: Vec<Line> = Vec::with_capacity(self.lines.len() + other.lines.len() - 2);
        if combine_at_start {
            // insert the lines of other in front
            if combine_other_at_start {
                // insert in reverse order, skip the first line of other
                for i in 0..other.lines.len() - 1 {
                    new_lines.push(other.lines[other.lines.len() - i - 1].opposite());
                }
            } else {
                // skip the last line of other
                new_lines.extend_from_slice(&other.lines[..other.lines.len() - 1]);
            }
            // append the lines of this polyline, skip the first line
            new_lines.extend_from_slice(&self.lines[1..]);
        } else {
            // insert the lines of this polyline in front, skip the last line
            new_lines.extend_from_slice(&self.lines[..self.lines.len() - 1]);
            if combine_other_at_start {
                // skip the first line of other
                new_lines.extend_from_slice(&other.lines[1..]);
            } else {
                // insert in reverse order, skip the last line of other
                for i in 1..other.lines.len() {
                    new_lines.push(other.lines[other.lines.len() - i - 1].opposite());
                }
            }
        }
        Polyline::from_lines(new_lines)
    }

    /// Splits this polyline at the line with index `line_index` into two, by inserting `end_line`
    /// as concluding line of the first split piece and as the start line of the second split
    /// piece. `end_line` and the line with index `line_index` must not be parallel. The order of
    /// the lines in the two result pieces is preserved. `line_index` must be bigger than 0 and
    /// less than `lines.length - 1`. Returns `None` if nothing was split
    /// (Polyline.java:751-835).
    pub fn split(
        &self,
        line_index: usize,
        end_line: &Line,
    ) -> Result<Option<[Polyline; 2]>, PolylineError> {
        if line_index < 1 || line_index + 2 > self.lines.len() {
            // Java: FRLogger.warn("Polyline.split: lineIndex out of range")
            return Ok(None);
        }
        if self.lines[line_index].is_parallel(end_line) {
            return Ok(None);
        }
        let new_end_corner = self.lines[line_index].intersection(end_line);
        // Java's two FRLogger.trace calls here are diagnostics only and are not ported.
        if (line_index == 1 && Some(&new_end_corner) == self.first_corner().as_ref())
            || (line_index + 2 >= self.lines.len()
                && Some(&new_end_corner) == self.last_corner().as_ref())
        {
            // No split, if endLine does not intersect, but touches
            // only this Polyline at an end point.
            return Ok(None);
        }
        let mut first_piece: Vec<Line>;
        if self.corner_at(line_index - 1) == new_end_corner {
            // skip line segment of length 0 at the end of the first piece
            first_piece = self.lines[..line_index + 1].to_vec();
        } else {
            first_piece = Vec::with_capacity(line_index + 2);
            first_piece.extend_from_slice(&self.lines[..line_index + 1]);
            first_piece.push(*end_line);
        }
        let mut second_piece: Vec<Line>;
        if self.corner_at(line_index) == new_end_corner {
            // skip line segment of length 0 at the beginning of the second piece
            second_piece = self.lines[line_index..].to_vec();
        } else {
            second_piece = Vec::with_capacity(self.lines.len() - line_index + 1);
            second_piece.push(*end_line);
            second_piece.extend_from_slice(&self.lines[line_index..]);
        }
        let result = [
            Polyline::from_lines(std::mem::take(&mut first_piece))?,
            Polyline::from_lines(std::mem::take(&mut second_piece))?,
        ];
        if result[0].is_point() || result[1].is_point() {
            return Ok(None);
        }
        Ok(Some(result))
    }

    /// Creates a new polyline by skipping lines from `from_no` to `to_no`
    /// (Polyline.java:837-846).
    pub fn skip_lines(&self, from_no: usize, to_no: usize) -> Result<Polyline, PolylineError> {
        self.skip_lines_i64(from_no as i64, to_no as i64)
    }

    /// [`Polyline::skip_lines`] for indices that Java computes as possibly negative `int`s
    /// (`shorten` passes `newLineCount - 1`, Polyline.java:922).
    fn skip_lines_i64(&self, from_no: i64, to_no: i64) -> Result<Polyline, PolylineError> {
        if from_no < 0 || to_no > self.lines.len() as i64 - 1 || from_no > to_no {
            return Ok(self.clone());
        }
        let (from_no, to_no) = (from_no as usize, to_no as usize);
        let mut new_lines: Vec<Line> = Vec::with_capacity(self.lines.len() - (to_no - from_no + 1));
        new_lines.extend_from_slice(&self.lines[..from_no]);
        new_lines.extend_from_slice(&self.lines[to_no + 1..]);
        Polyline::from_lines(new_lines)
    }

    /// Returns whether this polyline contains the given point (Polyline.java:848-857).
    pub fn contains(&self, point: &Point) -> bool {
        for i in 1..self.lines.len().saturating_sub(1) {
            if let Some(current_segment) = LineSegment::from_polyline(self, i)
                && current_segment.contains(point)
            {
                return true;
            }
        }
        false
    }
}

// -------------------------------------------------------------------------------------------
// Projection and shortening (Polyline.java:866-936)
// -------------------------------------------------------------------------------------------

impl Polyline {
    /// Creates a perpendicular line segment from `point` onto the nearest line segment of this
    /// polyline. Returns `None` if the perpendicular line does not intersect the nearest line
    /// segment inside its segment bounds, or if `point` is contained in this polyline
    /// (Polyline.java:866-910).
    ///
    /// The resulting segment runs *from* `point` *to* the polyline: its start closing line goes
    /// through `point` in the direction of the nearest polyline line (Polyline.java:908).
    ///
    /// # Panics
    /// For a [`Point::Rational`] argument — see [`Polyline::from_polygon`]; Java's
    /// `new Line(point, dir)` would only warn there and carry on with a broken line.
    // totalized: a rational query point panics mid-loop instead of producing a broken Line.
    pub fn projection_line(&self, point: &Point) -> Option<LineSegment> {
        let from_point = point.to_float();
        let mut min_distance = f64::MAX;
        let mut result_line: Option<Line> = None;
        let mut nearest_line: Option<Line> = None;
        for i in 1..self.lines.len().saturating_sub(1) {
            let projection = from_point.projection_approx(&self.lines[i]);
            let current_distance = projection.distance(&from_point);
            if current_distance < min_distance {
                let Some(direction_towards_line) = self.lines[i].perpendicular_direction(point)
                else {
                    continue;
                };
                // `Line::perpendicular_direction` only ever yields an `IntDirection`, so this
                // never falls through; the guard is here because the signature allows it.
                let Some(current_result_line) =
                    Line::from_direction_any(int_point_of(point), &direction_towards_line)
                else {
                    continue;
                };
                let prev_corner = self.corner_at(i - 1);
                let next_corner = self.corner_at(i);
                let prev_corner_side = current_result_line.side_of(&prev_corner);
                let next_corner_side = current_result_line.side_of(&next_corner);
                if prev_corner_side == next_corner_side && prev_corner_side != Side::Collinear {
                    // the projection point is outside the line segment
                    continue;
                }
                nearest_line = Some(self.lines[i]);
                min_distance = current_distance;
                result_line = Some(current_result_line);
            }
        }
        let nearest_line = nearest_line?;
        let start_line = Line::from_direction(int_point_of(point), &nearest_line.direction());
        Some(LineSegment::new(
            start_line,
            result_line.expect("set together with nearest_line"),
            nearest_line,
        ))
    }

    /// Shortens this polyline to `new_line_count` lines. Additionally, the last line segment is
    /// approximately shortened to `last_segment_length`. The last corner of the new polyline is
    /// an `IntPoint` (Polyline.java:912-936).
    pub fn shorten(
        &self,
        new_line_count: usize,
        last_segment_length: f64,
    ) -> Result<Polyline, PolylineError> {
        let last_corner = self.corner_approx_at_i64(new_line_count as i64 - 2);
        let prev_last_corner = self.corner_approx_at_i64(new_line_count as i64 - 3);
        let new_last_corner = prev_last_corner
            .change_length(&last_corner, last_segment_length)
            .round();
        if self.corner_at_i64(self.corner_count() as i64 - 2) == Point::Int(new_last_corner) {
            // skip the last line
            return self.skip_lines_i64(new_line_count as i64 - 1, new_line_count as i64 - 1);
        }
        let mut new_lines: Vec<Line> = Vec::with_capacity(new_line_count);
        new_lines.extend_from_slice(&self.lines[..new_line_count - 2]);
        // create the last 2 lines of the new polyline
        let mut first_line_point = self.lines[new_line_count - 2].a;
        if first_line_point == new_last_corner {
            first_line_point = self.lines[new_line_count - 2].b;
        }
        let new_prev_last_line = Line::new(first_line_point, new_last_corner);
        new_lines.push(new_prev_last_line);
        new_lines.push(Line::from_direction(
            new_last_corner,
            &new_prev_last_line.direction().turn_45_degree(6),
        ));
        Polyline::from_lines(new_lines)
    }

    /// [`Polyline::corner_approx_at`] for an index that Java computes as a possibly negative
    /// `int` (`shorten`, Polyline.java:917-918).
    fn corner_approx_at_i64(&self, corner_index: i64) -> FloatPoint {
        let no = self.clamp_corner_index(corner_index);
        self.lines[no].intersection_approx(&self.lines[no + 1])
    }

    /// [`Polyline::corner_at`] for an index that Java computes as a possibly negative `int`.
    fn corner_at_i64(&self, corner_index: i64) -> Point {
        let no = self.clamp_corner_index(corner_index);
        self.lines[no].intersection(&self.lines[no + 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_point::FloatPoint;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::line::Line;
    use crate::point::Point;

    fn pts(v: &[(i32, i32)]) -> Vec<Point> {
        v.iter()
            .map(|&(x, y)| Point::Int(IntPoint::new(x, y)))
            .collect()
    }
    fn l_shape() -> Polyline {
        Polyline::from_points(&pts(&[(0, 0), (10, 0), (10, 10)]))
    }

    #[test]
    fn corners_roundtrip_and_length() {
        let p = l_shape();
        assert_eq!(p.corner_count(), 3);
        assert_eq!(p.lines().len(), 4); // n corners ⇒ n+1 lines (closing lines at both ends)
        assert_eq!(p.corners(), pts(&[(0, 0), (10, 0), (10, 10)]));
        assert_eq!(p.first_corner().unwrap(), Point::Int(IntPoint::new(0, 0)));
        assert_eq!(p.last_corner().unwrap(), Point::Int(IntPoint::new(10, 10)));
        assert_eq!(p.length_approx(), 20.0);
        assert!(p.is_orthogonal());
        assert!(p.is_multiple_of_45_degree());
        assert!(!p.is_point());
        assert_eq!(
            p.reverse().unwrap().first_corner().unwrap(),
            Point::Int(IntPoint::new(10, 10))
        );
        assert_eq!(p.bounding_box(), IntBox::from_coords(0, 0, 10, 10));
    }

    #[test]
    fn the_closing_lines_are_perpendicular_to_the_end_segments() {
        // Polyline.java:44-51: lines[0] is the perpendicular through the first corner and
        // lines[n] the perpendicular through the last corner.
        let p = l_shape();
        assert_eq!(p.lines()[0], Line::from_coords(0, 0, 0, 1));
        assert_eq!(p.lines()[1], Line::from_coords(0, 0, 10, 0));
        assert_eq!(p.lines()[2], Line::from_coords(10, 0, 10, 10));
        assert_eq!(p.lines()[3], Line::from_coords(10, 10, 11, 10));
        // Polyline.java:59-71 recomputes the direction as from->to, so its closing line points
        // the other way than the Polygon constructor's would.
        let two = Polyline::from_two_points(
            &Point::Int(IntPoint::new(0, 0)),
            &Point::Int(IntPoint::new(10, 0)),
        );
        assert_eq!(two.lines().len(), 3);
        assert_eq!(two.lines()[2], Line::from_coords(10, 0, 10, 1));
        assert_eq!(
            Polyline::from_points(&pts(&[(0, 0), (10, 0)])).lines()[2],
            Line::from_coords(10, 0, 10, -1)
        );
        // equal corners produce an empty polyline
        assert!(
            Polyline::from_two_points(
                &Point::Int(IntPoint::new(1, 1)),
                &Point::Int(IntPoint::new(1, 1))
            )
            .is_empty()
        );
    }

    #[test]
    fn collinear_middle_corner_is_dropped() {
        let p = Polyline::from_points(&pts(&[(0, 0), (5, 0), (10, 0)]));
        assert_eq!(p.corner_count(), 2);
        let q = Polyline::from_points(&pts(&[(0, 0), (0, 0), (10, 0)]));
        assert_eq!(q.corner_count(), 2);
    }

    #[test]
    fn degenerate_polylines_are_empty() {
        let p = Polyline::from_points(&pts(&[(0, 0)]));
        assert!(p.is_empty());
        assert!(p.is_point());
        assert_eq!(p.corner_count(), 0);
        assert_eq!(p.corners(), Vec::<Point>::new());
        assert_eq!(p.corner(0), None);
        assert_eq!(p.first_corner(), None);
        assert_eq!(p.last_corner(), None);
        assert_eq!(p.corner_approx(0), None);
        assert_eq!(p.length_approx(), 0.0);
        assert_eq!(p.offset_shapes(3).len(), 0);
        assert_eq!(p.distance(&FloatPoint::new(0.0, 0.0)), f64::MAX);
    }

    #[test]
    fn offset_shapes_cover_segments() {
        let p = l_shape();
        let shapes = p.offset_shapes(2);
        assert_eq!(shapes.len(), 2);
        assert!(shapes[0].contains(&Point::Int(IntPoint::new(5, 1))));
        assert!(shapes[0].contains(&Point::Int(IntPoint::new(0, 0))));
        assert!(!shapes[0].contains(&Point::Int(IntPoint::new(5, 4))));
        assert_eq!(
            p.offset_box(2, 0).unwrap(),
            IntBox::from_coords(-2, -2, 12, 2)
        );
        assert_eq!(
            p.offset_box(2, 1).unwrap(),
            IntBox::from_coords(8, -2, 12, 12)
        );
        // both shapes come out as octagons because of the bounding-octagon intersection
        // (Polyline.java:491-494)
        assert_eq!(
            shapes[0],
            TileShape::Octagon(IntOctagon::new(-2, -2, 12, 2, -3, 13, -3, 13))
        );
        assert_eq!(
            shapes[1],
            TileShape::Octagon(IntOctagon::new(8, -2, 12, 12, -3, 13, 7, 23))
        );
        // offsetShape(halfWidth, no) is offsetShapes(halfWidth, no, no + 2)[0]
        assert_eq!(p.offset_shape(2, 0).unwrap(), shapes[0]);
        assert_eq!(p.offset_shape(2, 1).unwrap(), shapes[1]);
        assert_eq!(p.offset_shape(2, 2), None);
    }

    #[test]
    fn combine_and_split() {
        let a = Polyline::from_points(&pts(&[(0, 0), (10, 0)]));
        let b = Polyline::from_points(&pts(&[(10, 0), (10, 10)]));
        let c = a.combine(&b).unwrap();
        assert_eq!(c.corners(), pts(&[(0, 0), (10, 0), (10, 10)]));
        // no common end corner: Java returns the receiver (Polyline.java:718-720)
        let d = Polyline::from_points(&pts(&[(50, 50), (60, 50)]));
        assert_eq!(a.combine(&d), Ok(a));
        // split the L at its horizontal segment (line index 1) by the vertical line x = 5
        let parts = l_shape()
            .split(1, &Line::from_coords(5, 0, 5, 1))
            .unwrap()
            .expect("splits");
        assert_eq!(parts[0].corners(), pts(&[(0, 0), (5, 0)]));
        assert_eq!(parts[1].corners(), pts(&[(5, 0), (10, 0), (10, 10)]));
        // a line parallel to the split line does not split (Polyline.java:763-765)
        assert_eq!(l_shape().split(1, &Line::from_coords(5, 0, 6, 0)), Ok(None));
        // touching the polyline at its first corner does not split (Polyline.java:800-805)
        assert_eq!(l_shape().split(1, &Line::from_coords(0, 0, 0, 1)), Ok(None));
        assert_eq!(l_shape().split(0, &Line::from_coords(5, 0, 5, 1)), Ok(None));
    }

    #[test]
    fn nearest_point_distance_contains() {
        let p = l_shape();
        assert_eq!(
            p.nearest_point_approx(&FloatPoint::new(5.0, 3.0)).unwrap(),
            FloatPoint::new(5.0, 0.0)
        );
        assert_eq!(p.distance(&FloatPoint::new(5.0, 3.0)), 3.0);
        assert!(p.contains(&Point::Int(IntPoint::new(10, 4))));
        assert!(!p.contains(&Point::Int(IntPoint::new(4, 4))));
        let proj = p
            .projection_line(&Point::Int(IntPoint::new(12, 4)))
            .unwrap();
        // Java's projection segment runs *from* the queried point *onto* the polyline
        // (Polyline.java:908-909), so its start point is the query point itself.
        assert_eq!(proj.start_point(), Point::Int(IntPoint::new(12, 4)));
        assert_eq!(proj.end_point(), Point::Int(IntPoint::new(10, 4)));
    }

    #[test]
    fn transformations() {
        let p = l_shape();
        assert_eq!(
            p.translate_by(&crate::vector::Vector::new(1, 1))
                .unwrap()
                .bounding_box(),
            IntBox::from_coords(1, 1, 11, 11)
        );
        assert_eq!(
            p.turn_90_degree(1, &IntPoint::new(0, 0))
                .unwrap()
                .bounding_box(),
            IntBox::from_coords(-10, 0, 0, 10)
        );
        assert_eq!(
            p.mirror_vertical(&IntPoint::new(0, 0))
                .unwrap()
                .bounding_box(),
            IntBox::from_coords(-10, 0, 0, 10)
        );
        assert_eq!(
            p.mirror_horizontal(&IntPoint::new(0, 0))
                .unwrap()
                .bounding_box(),
            IntBox::from_coords(0, -10, 10, 0)
        );
        // skipLines(0, 0) drops one line, so 4 lines become 3 and 3 corners become 2
        // (Polyline.java:837-846).
        assert_eq!(p.skip_lines(0, 0).unwrap().corner_count(), 2);
        // out-of-range arguments return the receiver
        assert_eq!(p.skip_lines(0, 4), Ok(p.clone()));
        assert_eq!(p.skip_lines(2, 1), Ok(p.clone()));
        // translateBy(ZERO) returns the receiver (Polyline.java:538-540)
        assert_eq!(p.translate_by(&Vector::ZERO), Ok(p.clone()));
        // rotateApprox(0) returns the receiver (Polyline.java:559-561)
        assert_eq!(p.rotate_approx(0.0, &FloatPoint::new(0.0, 0.0)), p);
    }

    #[test]
    fn bounding_octagon_between_matches_java() {
        // Polyline.java:619-656 over the whole L shape.
        assert_eq!(
            l_shape().bounding_octagon_between(0, 2),
            IntOctagon::new(0, 0, 10, 10, 0, 10, 0, 20)
        );
    }

    #[test]
    fn shorten_reduces_lines() {
        let p = Polyline::from_points(&pts(&[(0, 0), (10, 0), (10, 10), (20, 10)]));
        let s = p.shorten(3, 5.0).unwrap();
        assert_eq!(s.lines().len(), 3);
        assert!(s.length_approx() < p.length_approx());
        assert_eq!(s.corners(), pts(&[(0, 0), (5, 0)]));
        assert_eq!(s.length_approx(), 5.0);
    }

    #[test]
    fn from_lines_skips_parallel_and_overlapping_lines() {
        // Polyline.java:104-131: consecutive parallel lines are dropped.
        let p = Polyline::from_lines(vec![
            Line::from_coords(0, 0, 0, 1),
            Line::from_coords(0, 0, 10, 0),
            Line::from_coords(3, 0, 13, 0), // parallel to the previous line: skipped
            Line::from_coords(10, 0, 10, 10),
            Line::from_coords(10, 10, 11, 10),
        ])
        .unwrap();
        assert_eq!(p.lines().len(), 4);
        assert_eq!(p.corners(), pts(&[(0, 0), (10, 0), (10, 10)]));
        // fewer than 3 lines after filtering: empty polyline
        assert!(
            Polyline::from_lines(vec![
                Line::from_coords(0, 0, 1, 0),
                Line::from_coords(2, 0, 3, 0),
                Line::from_coords(4, 0, 5, 0),
            ])
            .unwrap()
            .is_empty()
        );
        // Java bug: Polyline.java:148 throws ArrayIndexOutOfBoundsException: Index -1 for this
        // input; the port surfaces it as an error instead of swallowing it into an empty
        // polyline, which a caller could not tell apart from a legitimate result.
        assert_eq!(
            Polyline::from_lines(vec![
                Line::from_coords(0, 0, 1, 0),
                Line::from_coords(0, 0, 0, 1),
                Line::from_coords(0, 0, 1, 0),
                Line::from_coords(0, 0, 0, 1),
                Line::from_coords(0, 0, 1, 0),
                Line::from_coords(0, 0, 0, 1),
            ]),
            Err(PolylineError::NormalizationIndexUnderflow)
        );
    }

    /// `new Polyline(Line[])` normalises the **caller's** array: `removeConsecutiveParallelLines`
    /// (Polyline.java:118) and `removeOverlaps` (:165) both `return lines` when they skip nothing,
    /// and the constructor's `filteredLines[i] = filteredLines[i].opposite()` (:97) then writes
    /// through to it. Five Plan 6 tightener sites re-read that array (see
    /// [`Polyline::from_lines_in_place`]), and since quirk #74 the *identity* of what they read
    /// back is board-observable: a flipped line is a new `Line` object.
    #[test]
    fn from_lines_in_place_writes_the_normalised_lines_back_to_the_caller() {
        // A two-corner polyline's three lines, with the middle one handed in reversed. The
        // constructor's normalisation turns it back round, so index 1 comes back as a *different*
        // object with a different value; indices 0 and 2 are untouched.
        let base = Polyline::from_points(&pts(&[(0, 0), (10000, 0)]));
        let mut arr = vec![base.lines()[0], base.lines()[1].opposite(), base.lines()[2]];
        let handed_in = arr.clone();

        let polyline = Polyline::from_lines_in_place(&mut arr).expect("normalises");

        assert_eq!(polyline.lines().len(), 3);
        // The write-back happened, and it is Java's: the caller's array *is* the polyline's.
        assert_eq!(arr, polyline.lines());
        for (caller, built) in arr.iter().zip(polyline.lines()) {
            assert!(caller.is_same_object(built));
        }
        // Index 1 was flipped: a new object, and no longer the reversed line handed in.
        assert!(!arr[1].is_same_object(&handed_in[1]));
        assert_ne!(arr[1], handed_in[1]);
        assert_eq!(arr[1], base.lines()[1]);
        // Indices 0 and 2 keep the objects the caller put in, exactly as Java keeps the
        // references it was handed.
        assert!(arr[0].is_same_object(&handed_in[0]));
        assert!(arr[2].is_same_object(&handed_in[2]));

        // `from_lines` is the same construction with the write-back dropped, which is what every
        // caller that does not re-read its array wants.
        let mut same_input = handed_in.clone();
        assert_eq!(
            Polyline::from_lines(same_input.clone()).expect("normalises"),
            polyline
        );
        same_input.clone_from(&handed_in);
        assert!(same_input[1].is_same_object(&handed_in[1]));
    }

    /// The negative half: when either normaliser *skips* a line it returns a fresh array, so
    /// Java's write-back lands in the copy and the caller's array is untouched
    /// (Polyline.java:126-130, :168-172, and the constructor's `< 3` return at :80-83).
    #[test]
    fn from_lines_in_place_leaves_the_caller_alone_when_a_line_is_skipped() {
        // Two consecutive parallel lines: `removeConsecutiveParallelLines` skips one, two survive,
        // and the constructor returns an empty polyline before its normalisation loop.
        let mut arr = vec![
            Line::from_coords(0, 0, 0, 1),
            Line::from_coords(0, 0, 10000, 0),
            Line::from_coords(0, 100, 10000, 100),
        ];
        let handed_in = arr.clone();
        let polyline = Polyline::from_lines_in_place(&mut arr).expect("no underflow");
        assert!(polyline.lines().is_empty());
        for (after, before) in arr.iter().zip(&handed_in) {
            assert!(after.is_same_object(before));
        }
    }
}
