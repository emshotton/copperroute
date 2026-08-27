//! Port of `app.freerouting.geometry.planar.Simplex`: a convex shape defined as the intersection
//! of half-planes, where a half-plane is the positive side of a directed line.
//!
//! The half-plane of a directed line `l` is `{ p : l.side_of(p) == Side::OnTheRight }` — every
//! interior point of a simplex satisfies that for every one of its border lines. Read the
//! predicate literally: [`Line::side_of`] answers where the *line* lies as seen from the point,
//! so the half-plane collects the points that have the line on their right, which are the points
//! lying *geometrically to the left* of the directed line. For the upward line `x = 0` (from
//! `(0, 0)` towards `(0, 1)`) the half-plane is therefore `x <= 0`, not `x >= 0`. That convention
//! is fixed by `Line::side_of` (Task 10) and is used verbatim by every side test below.
//!
//! Java's `Simplex` extends `TileShape` → `PolylineShape`; the methods inherited from those two
//! (and the ones whose signature mentions `TileShape`, `Shape` or `Circle`) arrive in Task 14 —
//! see the marker at the end of the `impl` block.

use crate::direction::Direction;
use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_direction::IntDirection;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::limits::CRIT_INT;
use crate::line::Line;
use crate::point::Point;
use crate::side::Side;
use crate::vector::Vector;

/// Convex shape defined as intersection of half-planes. A half-plane is defined as the positive
/// side of a directed line.
///
/// The border lines of a *normalized* simplex (one built through [`Simplex::from_lines`] or
/// [`Simplex::from_points`]) are sorted in ascending direction, and `corner(i)` is the
/// intersection of line `i - 1` with line `i`. [`Simplex::new`] keeps the lines exactly as given.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Simplex {
    lines: Vec<Line>,
}

// not ported: the four transient memo fields `precalculatedCorners`,
// `precalculatedFloatCorners`, `precalculatedBoundingBox` and `precalculatedBoundingOctagon`
// (Simplex.java:21-26). They are pure caches of `corner`/`cornerApprox`/`boundingBox`/
// `boundingOctagon`; keeping them would make `Simplex` non-`Hash` and non-`Eq` for no behavioral
// gain, so every accessor recomputes instead.

impl Simplex {
    /// Standard implementation for an empty Simplex (Simplex.java:17).
    pub const EMPTY: Simplex = Simplex { lines: Vec::new() };

    /// Constructs a Simplex from the directed lines in `lines`. The simplex will **not** be
    /// normalized; use [`Simplex::from_lines`] for the normalizing factory (Java
    /// `TileShape.getInstance` additionally calls `simplify()`, which arrives in Task 14).
    pub fn new(lines: Vec<Line>) -> Simplex {
        Simplex { lines }
    }

    /// The border lines of this simplex, in their stored order.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Creates a Simplex as intersection of the half-planes defined by directed lines
    /// (Java `Simplex.getInstance(Line[])`, Simplex.java:37-46): sorts the lines in ascending
    /// direction and removes the redundant ones.
    ///
    /// Java sorts with `Arrays.sort`, a *stable* merge sort; `slice::sort_by` is stable too, so
    /// lines with equal direction keep their input order in both.
    pub fn from_lines(lines: Vec<Line>) -> Simplex {
        if lines.is_empty() {
            return Simplex::EMPTY;
        }
        let mut current_arr = lines;
        // sort the lines in ascending direction
        current_arr.sort_by(|a, b| a.compare_to(b));
        let current_simplex = Simplex::new(current_arr);
        current_simplex.remove_redundant_lines()
    }

    /// Creates a Simplex from a point array, which forms the corners of the shape of a convex
    /// polygon (Java `TileShape.getInstance(Point[])`, TileShape.java:23-34, minus the trailing
    /// `simplify()` — that only swaps the physical representation and arrives in Task 14).
    pub fn from_points(convex_polygon: &[IntPoint]) -> Simplex {
        let n = convex_polygon.len();
        let mut lines: Vec<Line> = Vec::with_capacity(n);
        for j in 0..n.saturating_sub(1) {
            lines.push(Line::new(convex_polygon[j], convex_polygon[j + 1]));
        }
        if n > 0 {
            lines.push(Line::new(convex_polygon[n - 1], convex_polygon[0]));
        }
        Simplex::from_lines(lines)
    }

    /// Return true, if this simplex is empty.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Java `getId()`: `result = 31 * result + current.getId()` over the border lines, starting
    /// from 0. Java `int` arithmetic wraps silently on overflow; this is a hash-shaped value, so
    /// `wrapping_*` reproduces that.
    pub fn get_id(&self) -> i32 {
        let mut result: i32 = 0;
        for current in &self.lines {
            result = 31i32.wrapping_mul(result).wrapping_add(current.get_id());
        }
        result
    }

    /// Returns true, if the determinant of the direction of index no -1 and the direction of
    /// index no is > 0.
    ///
    /// Java clamps an out-of-range index (after a warning); the negative half of that clamp
    /// cannot happen for a `usize`. On an *empty* simplex the clamp makes Java index `lines[-2]`
    /// and throw `ArrayIndexOutOfBoundsException`; the `false` returned here is this port's
    /// totalization of that, not observed Java behaviour.
    pub fn corner_is_bounded(&self, corner_index: usize) -> bool {
        if self.lines.is_empty() {
            return false;
        }
        let no = if corner_index >= self.lines.len() {
            self.lines.len() - 1
        } else {
            corner_index
        };
        if self.lines.len() == 1 {
            return false;
        }
        let prev_no = if no == 0 {
            self.lines.len() - 1
        } else {
            no - 1
        };
        let prev_dir = self.lines[prev_no].direction().get_vector();
        let current_direction = self.lines[no].direction().get_vector();
        prev_dir.determinant(&current_direction) > 0
    }

    /// Returns true, if the shape of this simplex is contained in a sufficiently large box.
    pub fn is_bounded(&self) -> bool {
        if self.lines.is_empty() {
            return true;
        }
        if self.lines.len() < 3 {
            return false;
        }
        (0..self.lines.len()).all(|i| self.corner_is_bounded(i))
    }

    /// Returns the number of edge lines defining this simplex.
    pub fn border_line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns the intersection of the no -1-th with the no-th line of this simplex. If the
    /// simplex is not bounded at this corner, `Point::is_infinite` will hold for the result.
    ///
    /// Java's doc claims the coordinates are then set to `Integer.MAX_VALUE`; that is only true
    /// of [`Simplex::corner_approx`] — the exact `Line.intersection` returns a `RationalPoint`
    /// with denominator 0 for parallel lines.
    ///
    /// # Panics
    /// On an empty simplex, exactly as Java does (`lines[lines.length - 1]` with length 0).
    pub fn corner(&self, corner_index: usize) -> Point {
        let no = if corner_index >= self.lines.len() {
            self.lines.len() - 1
        } else {
            corner_index
        };
        let prev = if no == 0 {
            self.lines[self.lines.len() - 1]
        } else {
            self.lines[no - 1]
        };
        self.lines[no].intersection(&prev)
    }

    /// Returns an approximation of the intersection of the no -1-th with the no-th line of this
    /// simplex by a FloatPoint. If the simplex is not bounded at this corner, the coordinates of
    /// the result will be `i32::MAX` (Java's `Integer.MAX_VALUE` sentinel, kept verbatim).
    ///
    /// Java returns `null` for the empty simplex; ported as `None`.
    pub fn corner_approx(&self, corner_index: usize) -> Option<FloatPoint> {
        if self.lines.is_empty() {
            return None;
        }
        let no = if corner_index >= self.lines.len() {
            self.lines.len() - 1
        } else {
            corner_index
        };
        let prev = if no == 0 {
            self.lines[self.lines.len() - 1]
        } else {
            self.lines[no - 1]
        };
        Some(self.lines[no].intersection_approx(&prev))
    }

    /// All corners of this simplex, approximated by `FloatPoint`s.
    pub fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        let count = self.lines.len();
        (0..count)
            .map(|i| {
                let prev = if i == 0 {
                    self.lines[count - 1]
                } else {
                    self.lines[i - 1]
                };
                self.lines[i].intersection_approx(&prev)
            })
            .collect()
    }

    /// Returns the no-th edge line of this simplex. The edge lines are sorted in ascending
    /// direction. Java warns and returns `null` for the empty simplex; ported as `None`, and the
    /// out-of-range index is clamped exactly as Java does.
    pub fn border_line(&self, edge_index: usize) -> Option<Line> {
        if self.lines.is_empty() {
            return None;
        }
        let no = if edge_index >= self.lines.len() {
            self.lines.len() - 1
        } else {
            edge_index
        };
        Some(self.lines[no])
    }

    /// Returns the dimension of this simplex. The result may be 2, 1, 0, or -1 (if the simplex is
    /// empty).
    pub fn dimension(&self) -> i32 {
        let lines = &self.lines;
        if lines.is_empty() {
            return -1;
        }
        if lines.len() > 4 {
            return 2;
        }
        if lines.len() == 1 {
            // we have a half plane
            return 2;
        }
        if lines.len() == 2 {
            if lines[0].overlaps(&lines[1]) {
                return 1;
            }
            return 2;
        }
        if lines.len() == 3 {
            if lines[0].overlaps(&lines[1])
                || lines[0].overlaps(&lines[2])
                || lines[1].overlaps(&lines[2])
            {
                // simplex is 1 dimensional and unbounded at one side
                return 1;
            }
            let intersection = lines[1].intersection(&lines[2]);
            let side_of_line0 = lines[0].side_of(&intersection);
            if side_of_line0 == Side::OnTheRight {
                return 2;
            }
            if side_of_line0 == Side::OnTheLeft {
                // Java logs "empty Simplex not normalized" here (dropped: no logger in
                // fr-geometry).
                return -1;
            }
            // now the 3 lines intersect in the same point
            return 0;
        }
        // now the simplex has 4 edge lines
        // check if opposing lines are collinear
        let collinear02 = lines[0].overlaps(&lines[2]);
        let collinear13 = lines[1].overlaps(&lines[3]);
        if collinear02 && collinear13 {
            return 0;
        }
        if collinear02 || collinear13 {
            return 1;
        }
        2
    }

    /// Returns the arithmetic middle of the corners of this shape (Java
    /// `PolylineShape.centreOfGravity()`, PolylineShape.java:120-133, inherited by `Simplex`).
    ///
    /// The public trait form arrives with `PolylineShapeOps` in Task 14; it lives here now
    /// because [`Simplex::max_width`] and [`Simplex::min_width`] need it.
    pub fn centre_of_gravity(&self) -> FloatPoint {
        let corner_count = self.border_line_count();
        let mut x = 0.0;
        let mut y = 0.0;
        for current_point in self.corner_approx_arr() {
            x += current_point.x;
            y += current_point.y;
        }
        x /= corner_count as f64;
        y /= corner_count as f64;
        FloatPoint::new(x, y)
    }

    /// The sum of the two biggest distances of the border lines from the centre of gravity;
    /// `i32::MAX` if this simplex is not bounded.
    pub fn max_width(&self) -> f64 {
        if !self.is_bounded() {
            return i32::MAX as f64;
        }
        let mut max_distance = i32::MIN as f64;
        let mut max_distance2 = i32::MIN as f64;
        let gravity_point = self.centre_of_gravity();

        for i in 0..self.border_line_count() {
            let current_distance = self.lines[i].signed_distance(&gravity_point).abs();
            if current_distance > max_distance {
                max_distance2 = max_distance;
                max_distance = current_distance;
            } else if current_distance > max_distance2 {
                max_distance2 = current_distance;
            }
        }
        max_distance + max_distance2
    }

    /// The sum of the two smallest distances of the border lines from the centre of gravity;
    /// `i32::MAX` if this simplex is not bounded.
    pub fn min_width(&self) -> f64 {
        if !self.is_bounded() {
            return i32::MAX as f64;
        }
        let mut min_distance = i32::MAX as f64;
        let mut min_distance2 = i32::MAX as f64;
        let gravity_point = self.centre_of_gravity();

        for i in 0..self.border_line_count() {
            let current_distance = self.lines[i].signed_distance(&gravity_point).abs();
            if current_distance < min_distance {
                min_distance2 = min_distance;
                min_distance = current_distance;
            } else if current_distance < min_distance2 {
                min_distance2 = current_distance;
            }
        }
        min_distance + min_distance2
    }

    /// Checks if this simplex can be converted into an IntBox.
    ///
    /// Java additionally tests `line.a instanceof IntPoint && line.b instanceof IntPoint`; this
    /// port's `Line` always holds `IntPoint`s (see the note at the top of `line.rs`), so that
    /// half of the test is statically true and dropped.
    pub fn is_int_box(&self) -> bool {
        (0..self.lines.len()).all(|i| self.lines[i].is_orthogonal() && self.corner_is_bounded(i))
    }

    /// Checks if this simplex can be converted into an IntOctagon. See [`Simplex::is_int_box`]
    /// for the dropped `instanceof IntPoint` test.
    pub fn is_int_octagon(&self) -> bool {
        (0..self.lines.len())
            .all(|i| self.lines[i].is_multiple_of_45_degree() && self.corner_is_bounded(i))
    }

    /// Converts this Simplex to an IntOctagon. Returns `None` (Java: `null`), if that is not
    /// possible, because not all lines of this Simplex are 45 degrees.
    pub fn to_int_octagon(&self) -> Option<IntOctagon> {
        // this function is at the moment only implemented for lines
        // consisting of IntPoints.
        // The general implementation is still missing.
        if !self.is_int_octagon() {
            return None;
        }
        if self.is_empty() {
            return Some(IntOctagon::EMPTY);
        }

        // initialise to the biggest octagon values

        let mut rx = CRIT_INT;
        let mut uy = CRIT_INT;
        let mut lrx = CRIT_INT;
        let mut urx = CRIT_INT;
        let mut lx = -CRIT_INT;
        let mut ly = -CRIT_INT;
        let mut llx = -CRIT_INT;
        let mut ulx = -CRIT_INT;
        for current_line in &self.lines {
            let a = current_line.a;
            let b = current_line.b;
            if a.y == b.y {
                if b.x >= a.x {
                    // lower boundary line
                    ly = a.y;
                }
                if b.x <= a.x {
                    // upper boundary line
                    uy = a.y;
                }
            }
            if a.x == b.x {
                if b.y >= a.y {
                    // right boundary line
                    rx = a.x;
                }
                if b.y <= a.y {
                    // left boundary line
                    lx = a.x;
                }
            }
            if a.y < b.y {
                if a.x < b.x {
                    // lower right boundary line
                    lrx = a.x - a.y;
                } else if a.x > b.x {
                    // upper right boundary line
                    urx = a.x + a.y;
                }
            } else if a.y > b.y {
                if a.x < b.x {
                    // lower left boundary line
                    llx = a.x + a.y;
                } else if a.x > b.x {
                    // upper left boundary line
                    ulx = a.x - a.y;
                }
            }
        }
        let result = IntOctagon::new(lx, ly, rx, uy, ulx, lrx, llx, urx);
        Some(result.normalize())
    }

    /// Returns the simplex that results from translating its lines by vector.
    ///
    /// Java takes the abstract `Vector`; a `Vector::Rational` would turn the end points into
    /// `RationalPoint`s, which this port's `Line` cannot hold and which every later cast in Java
    /// would reject with a `ClassCastException` — ported as a panic, as in
    /// `IntOctagon::translate_by`.
    pub fn translate_by(&self, vector: &Vector) -> Simplex {
        if *vector == Vector::ZERO {
            return self.clone();
        }
        let v = match vector {
            Vector::Int(v) => v,
            Vector::Rational(_) => {
                panic!("Simplex::translate_by: only implemented for Vector::Int")
            }
        };
        Simplex::new(self.lines.iter().map(|l| l.translate_by(v)).collect())
    }

    /// Returns the smallest box with int coordinates containing all corners of this simplex. The
    /// coordinates of the result will be `i32::MAX` if the simplex is not bounded.
    pub fn bounding_box(&self) -> IntBox {
        if self.lines.is_empty() {
            return IntBox::EMPTY;
        }
        let mut llx = i32::MAX as f64;
        let mut lly = i32::MAX as f64;
        let mut urx = i32::MIN as f64;
        let mut ury = i32::MIN as f64;
        for current in self.corner_approx_arr() {
            llx = llx.min(current.x);
            lly = lly.min(current.y);
            urx = urx.max(current.x);
            ury = ury.max(current.y);
        }
        let lower_left = IntPoint::new(llx.floor() as i32, lly.floor() as i32);
        let upper_right = IntPoint::new(urx.ceil() as i32, ury.ceil() as i32);
        IntBox::new(lower_left, upper_right)
    }

    /// Calculates a bounding octagon of the Simplex. Returns `None` (Java: `null`), if the
    /// Simplex is not bounded.
    pub fn bounding_octagon(&self) -> Option<IntOctagon> {
        let mut lx = i32::MAX as f64;
        let mut ly = i32::MAX as f64;
        let mut rx = i32::MIN as f64;
        let mut uy = i32::MIN as f64;
        let mut ulx = i32::MAX as f64;
        let mut lrx = i32::MIN as f64;
        let mut llx = i32::MAX as f64;
        let mut urx = i32::MIN as f64;
        for current in self.corner_approx_arr() {
            lx = lx.min(current.x);
            ly = ly.min(current.y);
            rx = rx.max(current.x);
            uy = uy.max(current.y);

            let tmp = current.x - current.y;
            ulx = ulx.min(tmp);
            lrx = lrx.max(tmp);

            let tmp = current.x + current.y;
            llx = llx.min(tmp);
            urx = urx.max(tmp);
        }
        if lx.min(ly) < -(CRIT_INT as f64)
            || rx.max(uy) > CRIT_INT as f64
            || ulx.min(llx) < -(CRIT_INT as f64)
            || lrx.max(urx) > CRIT_INT as f64
        {
            // result is not bounded
            return None;
        }
        Some(IntOctagon::new(
            lx.floor() as i32,
            ly.floor() as i32,
            rx.ceil() as i32,
            uy.ceil() as i32,
            ulx.floor() as i32,
            lrx.ceil() as i32,
            llx.floor() as i32,
            urx.ceil() as i32,
        ))
    }

    /// Java `boundingTile()`: `return this;`
    pub fn bounding_tile(&self) -> Simplex {
        self.clone()
    }

    /// Returns the simplex offsetted by `width`. If `width > 0`, the offset is to the outer, else
    /// to the inner.
    pub fn offset(&self, width: f64) -> Simplex {
        if width == 0.0 {
            return self.clone();
        }
        let new_lines: Vec<Line> = self.lines.iter().map(|l| l.translate(-width)).collect();
        let offset_simplex = Simplex::new(new_lines);
        if width < 0.0 {
            return offset_simplex.remove_redundant_lines();
        }
        offset_simplex
    }

    /// Returns this simplex enlarged by offset. The result simplex is intersected with the by
    /// offset enlarged bounding octagon of this simplex.
    pub fn enlarge(&self, offset: f64) -> Simplex {
        if offset == 0.0 {
            return self.clone();
        }
        let offset_simplex = self.offset(offset);
        let Some(bounding_oct) = self.bounding_octagon() else {
            return Simplex::EMPTY;
        };
        let offset_oct = bounding_oct.offset(offset);
        offset_simplex.intersection(&offset_oct.to_simplex())
    }

    /// Returns the number of the rightmost corner seen from `from_point`. No other point of this
    /// simplex may be to the right of the line from `from_point` to the result corner.
    pub fn index_of_right_most_corner(&self, from_point: &Point) -> usize {
        let pole = from_point;
        let mut right_most_corner = self.corner(0);
        let mut result = 0;
        for i in 1..self.lines.len() {
            let current_corner = self.corner(i);
            if current_corner.side_of(pole, &right_most_corner) == Side::OnTheRight {
                right_most_corner = current_corner;
                result = i;
            }
        }
        result
    }

    /// Returns the intersection of `box_` with this simplex.
    pub fn intersection_box(&self, box_: &IntBox) -> Simplex {
        self.intersection(&box_.to_simplex())
    }

    /// Returns the intersection of `other` with this simplex.
    pub fn intersection_octagon(&self, other: &IntOctagon) -> Simplex {
        self.intersection(&other.to_simplex())
    }

    /// Returns the intersection of this simplex and other.
    pub fn intersection(&self, other: &Simplex) -> Simplex {
        if self.is_empty() || other.is_empty() {
            return Simplex::EMPTY;
        }
        let mut new_arr: Vec<Line> = Vec::with_capacity(self.lines.len() + other.lines.len());
        new_arr.extend_from_slice(&self.lines);
        new_arr.extend_from_slice(&other.lines);
        new_arr.sort_by(|a, b| a.compare_to(b));
        Simplex::new(new_arr).remove_redundant_lines()
    }

    /// Java `intersects(Simplex other)`: `!intersection(other).isEmpty()`.
    pub fn intersects(&self, other: &Simplex) -> bool {
        !self.intersection(other).is_empty()
    }

    /// Java `intersects(IntBox box)`: `intersects(box.toSimplex())`.
    pub fn intersects_box(&self, box_: &IntBox) -> bool {
        self.intersects(&box_.to_simplex())
    }

    /// Java `intersects(IntOctagon octagon)`: `intersects(octagon.toSimplex())`.
    pub fn intersects_octagon(&self, octagon: &IntOctagon) -> bool {
        self.intersects(&octagon.to_simplex())
    }

    /// Returns the edge number if `line` is a border line of this simplex, otherwise `None`
    /// (Java: -1). Uses Java's *geometric* `Line.equals`, not structural equality — see
    /// [`Line::equals_geometric`].
    pub fn border_line_index(&self, line: &Line) -> Option<usize> {
        (0..self.lines.len()).find(|&i| line.equals_geometric(&self.lines[i]))
    }

    /// Enlarges the simplex by removing the edge line with index `no`. The result simplex may get
    /// unbounded. An out-of-range index returns the simplex unchanged, as in Java.
    pub fn remove_border_line(&self, no: usize) -> Simplex {
        if no >= self.lines.len() {
            return self.clone();
        }
        let mut new_lines = self.lines.clone();
        new_lines.remove(no);
        Simplex::new(new_lines)
    }

    /// Java `toSimplex()`: `return this;`
    pub fn to_simplex(&self) -> Simplex {
        self.clone()
    }

    /// Java `cutoutFrom(IntOctagon oct)`: `cutoutFrom(oct.toSimplex())`.
    pub fn cutout_from_octagon(&self, oct: &IntOctagon) -> Option<Vec<Simplex>> {
        self.cutout_from(&oct.to_simplex())
    }

    /// Java `cutoutFrom(IntBox box)`: `cutoutFrom(box.toSimplex())`.
    pub fn cutout_from_box(&self, box_: &IntBox) -> Option<Vec<Simplex>> {
        self.cutout_from(&box_.to_simplex())
    }

    /// Cuts this simplex out of `outer_simplex`. Divides the resulting shape into simplices along
    /// the minimal distance lines from the vertices of the inner simplex to the outer simplex;
    /// returns the convex pieces constructed by this division.
    ///
    /// Returns `None` (Java: `null` after a warning) when this simplex is not 2-dimensional.
    #[allow(clippy::too_many_lines)] // literal transcription of Simplex.java:706-868
    pub fn cutout_from(&self, outer_simplex: &Simplex) -> Option<Vec<Simplex>> {
        if self.dimension() < 2 {
            // Java warns "Simplex.cutout_from only implemented for 2-dim simplex" and returns
            // null.
            return None;
        }
        let inner_simplex = self.intersection(outer_simplex);
        if inner_simplex.dimension() < 2 {
            // nothing to cutout from outerSimplex
            return Some(vec![outer_simplex.clone()]);
        }
        let inner_corner_count = inner_simplex.lines.len();
        let mut division_line_arr: Vec<Vec<Line>> = Vec::with_capacity(inner_corner_count);
        for inner_corner_no in 0..inner_corner_count {
            match inner_simplex.calc_division_lines(inner_corner_no, outer_simplex) {
                Some(lines) => division_line_arr.push(lines),
                // Java warns "Simplex.cutout_from: division line is null" and returns
                // { outerSimplex }.
                None => return Some(vec![outer_simplex.clone()]),
            }
        }
        let mut check_cross_first_line = false;
        // Java declares `prevDivisionLine` here and never assigns it inside the loop: the only
        // assignment in the whole method is the reversed `nextDivisionLine = prevDivisionLine;`
        // at the very end of the loop body (Simplex.java:860), which writes the *other* way
        // round and is itself dead (`nextDivisionLine` is recomputed at the top of the next
        // iteration). `prevDivisionLine` therefore stays null for every iteration and both
        // `mergePrevDivisionLine` flags below are always false. Ported verbatim, bug included;
        // the dead `nextDivisionLine = prevDivisionLine;` is not ported.
        let prev_division_line: Option<Line> = None;
        let first_division_line = division_line_arr[0][0];
        let first_direction = first_division_line.direction();
        let mut result_list: Vec<Simplex> = Vec::new();

        for inner_corner_no in 0..inner_corner_count {
            let next_corner_no = (inner_corner_no + 1) % inner_corner_count;
            let next_division_line = division_line_arr[next_corner_no][0];
            let current_division_lines = division_line_arr[inner_corner_no].clone();
            if current_division_lines.len() == 2 {
                // 2 division lines are necessary (sharp corner).
                // Construct an unbounded simplex from
                // currentDivisionLines[1] and currentDivisionLines[0]
                // and intersect it with the outer simplex
                let current_direction = current_division_lines[0].direction();
                // Java's `mergePrevDivisionLine` boolean plus the line it would append; kept as
                // one `Option` because `prevDivisionLine` is itself an `Option` here.
                let mut merge_prev_division_line: Option<Line> = None;
                let mut merge_first_division_line = false;
                if let Some(prev) = prev_division_line {
                    let prev_dir = prev.direction();
                    if current_direction.determinant(&prev_dir) > 0 {
                        // the previous division line may intersect
                        //  currentDivisionLines[0] inside divideSimplex
                        merge_prev_division_line = Some(prev);
                    }
                }
                if !check_cross_first_line {
                    check_cross_first_line =
                        inner_corner_no > 0 && current_direction.determinant(&first_direction) > 0;
                }
                if check_cross_first_line {
                    let current_dir2 = current_division_lines[1].direction();
                    if current_dir2.determinant(&first_direction) < 0 {
                        // The current piece has an intersection area with the first piece.
                        // Add a line to tmpPolyline to prevent this
                        merge_first_division_line = true;
                    }
                }
                let mut piece_lines: Vec<Line> = Vec::with_capacity(4);
                piece_lines.push(Line::new(
                    current_division_lines[1].b,
                    current_division_lines[1].a,
                ));
                piece_lines.push(current_division_lines[0]);
                if let Some(prev) = merge_prev_division_line {
                    piece_lines.push(prev);
                }
                if merge_first_division_line {
                    piece_lines.push(Line::new(first_division_line.b, first_division_line.a));
                }
                let current_piece = Simplex::new(piece_lines);
                result_list.push(current_piece.intersection(outer_simplex));
            }
            // construct an unbounded simplex from nextDivisionLine,
            // innerSimplex.line [innerCornerNo] and the last current division line
            // and intersect it with the outer simplex
            let merge_next_division_line = next_division_line.b != next_division_line.a;
            let last_curr_division_line = current_division_lines[current_division_lines.len() - 1];
            let last_curr_dir = last_curr_division_line.direction();
            let merge_last_curr_division_line =
                last_curr_division_line.b != last_curr_division_line.a;
            let mut merge_prev_division_line: Option<Line> = None;
            let mut merge_first_division_line = false;
            if let Some(prev) = prev_division_line {
                let prev_dir = prev.direction();
                if last_curr_dir.determinant(&prev_dir) > 0 {
                    // the previous division line may intersect
                    //  the last current division line inside divideSimplex
                    merge_prev_division_line = Some(prev);
                }
            }
            if !check_cross_first_line {
                check_cross_first_line = inner_corner_no > 0
                    && last_curr_dir.determinant(&first_direction) > 0
                    && last_curr_dir
                        .get_vector()
                        .scalar_product(&first_direction.get_vector())
                        < 0.0;
                // scalarProduct checked to ignore backcrossing at
                // small innerCornerNo
            }
            if check_cross_first_line {
                let next_dir = next_division_line.direction();
                if next_dir.determinant(&first_direction) < 0 {
                    // The current piece has an intersection area with the first piece.
                    // Add a line to tmpPolyline to prevent this
                    merge_first_division_line = true;
                }
            }
            let mut piece_lines: Vec<Line> = Vec::with_capacity(5);
            let current_line = inner_simplex.lines[inner_corner_no];
            piece_lines.push(Line::new(current_line.b, current_line.a));
            if merge_next_division_line {
                piece_lines.push(Line::new(next_division_line.b, next_division_line.a));
            }
            if merge_last_curr_division_line {
                piece_lines.push(last_curr_division_line);
            }
            if let Some(prev) = merge_prev_division_line {
                piece_lines.push(prev);
            }
            if merge_first_division_line {
                piece_lines.push(Line::new(first_division_line.b, first_division_line.a));
            }
            let current_piece = Simplex::new(piece_lines);
            result_list.push(current_piece.intersection(outer_simplex));
        }
        Some(result_list)
    }

    /// Removes lines, which are redundant in the definition of the shape of this simplex. Assumes
    /// that the lines of this simplex are sorted.
    #[allow(clippy::too_many_lines)] // literal transcription of Simplex.java:884-1020
    pub fn remove_redundant_lines(&self) -> Simplex {
        if self.lines.is_empty() {
            // Java indexes `this.lines[0]` unconditionally below and would throw
            // ArrayIndexOutOfBoundsException; every Java caller guards the empty simplex first
            // (Simplex.getInstance, IntOctagon.toSimplex). Returning EMPTY keeps this total.
            return Simplex::EMPTY;
        }
        let original_len = self.lines.len();
        let mut lines: Vec<Line> = self.lines.clone();
        // copy the sorted lines of arr into lines while skipping
        // multiple lines
        let mut new_length: usize = 1;
        let mut prev = self.lines[0];
        for i in 1..original_len {
            if !self.lines[i].fast_equals(&prev) {
                lines[new_length] = self.lines[i];
                prev = lines[new_length];
                new_length += 1;
            }
        }

        // precalculated array, on which side of this line the previous and the next line do
        // intersect
        let mut intersection_sides: Vec<Option<Side>> = vec![None; new_length];

        let mut try_again = new_length > 2;
        let mut index_of_last_removed_line: isize = new_length as isize;
        while try_again {
            try_again = false;
            let mut prev_ind: isize = new_length as isize - 1;
            let mut next_ind: isize;
            let mut prev_line = lines[prev_ind as usize];
            let mut current_line = lines[0];
            let mut ind: isize = 0;
            while ind < new_length as isize {
                if ind == new_length as isize - 1 {
                    next_ind = 0;
                } else {
                    next_ind = ind + 1;
                }
                let next_line = lines[next_ind as usize];

                let mut remove_line = false;
                let prev_dir = prev_line.direction();
                let next_dir = next_line.direction();
                let det = prev_dir.determinant(&next_dir);
                if det != 0 {
                    // prevLine and nextLine are not parallel
                    if intersection_sides[ind as usize].is_none() {
                        // intersectionSides [ind] not precalculated
                        intersection_sides[ind as usize] =
                            Some(current_line.side_of_intersection(&prev_line, &next_line));
                    }
                    if det > 0 {
                        // direction of nextLine is bigger than direction of prevLine
                        // if the intersection of prevLine and nextLine
                        // is on the left of currentLine, currentLine does not
                        // contribute to the shape of the simplex
                        remove_line = intersection_sides[ind as usize] != Some(Side::OnTheLeft);
                    } else {
                        // direction of nextLine is smaller than direction of prevLine
                        if intersection_sides[ind as usize] == Some(Side::OnTheLeft) {
                            let current_direction = current_line.direction();
                            if prev_dir.determinant(&current_direction) > 0 {
                                // direction of currentLine is bigger than direction of prevLine
                                // the halfplane defined by currentLine does not intersect
                                // with the simplex defined by prevLine and nex_line,
                                // hence this simplex must be empty
                                new_length = 0;
                                try_again = false;
                                break;
                            }
                        }
                    }
                } else {
                    // prevLine and nextLine are parallel
                    if prev_line.side_of(&Point::Int(next_line.a)) == Side::OnTheLeft {
                        // prevLine is to the left of nextLine; the half-planes defined by
                        // prevLine and nextLine do not intersect.
                        new_length = 0;
                        try_again = false;
                        break;
                    }
                }
                if remove_line {
                    try_again = true;
                    new_length -= 1;
                    for i in ind as usize..new_length {
                        lines[i] = lines[i + 1];
                        intersection_sides[i] = intersection_sides[i + 1];
                    }

                    if new_length < 3 {
                        try_again = false;
                        break;
                    }
                    // reset 3 precalculated intersectionSides
                    if ind == 0 {
                        prev_ind = new_length as isize - 1;
                    }
                    intersection_sides[prev_ind as usize] = None;
                    if ind >= new_length as isize {
                        next_ind = 0;
                    } else {
                        next_ind = ind;
                    }
                    intersection_sides[next_ind as usize] = None;
                    ind -= 1;
                    index_of_last_removed_line = ind;
                } else {
                    prev_line = current_line;
                    prev_ind = ind;
                }
                current_line = next_line;
                if !try_again && ind >= index_of_last_removed_line {
                    // tried all lines without removing one
                    break;
                }
                ind += 1;
            }
        }

        if new_length == 2 && lines[0].is_parallel(&lines[1]) {
            if lines[0].direction() == lines[1].direction() {
                // one of the two remaining lines is redundant
                if lines[1].side_of(&Point::Int(lines[0].a)) == Side::OnTheLeft {
                    lines[0] = lines[1];
                }
                new_length -= 1;
            } else {
                // the two remaining lines have opposite direction; the simplex may be empty.
                if lines[1].side_of(&Point::Int(lines[0].a)) == Side::OnTheLeft {
                    new_length = 0;
                }
            }
        }
        if new_length == original_len {
            return self.clone(); // nothing removed
        }
        if new_length == 0 {
            return Simplex::EMPTY;
        }
        lines.truncate(new_length);
        Simplex::new(lines)
    }

    /// For each corner of this inner simplex 1 or 2 perpendicular projections onto lines of the
    /// outer simplex are constructed, so that the resulting pieces after cutting out the inner
    /// simplex are convex. 2 projections may be necessary at sharp angle corners. Used in the
    /// method [`Simplex::cutout_from`] (Simplex.java:1028-1146).
    ///
    /// Returns `None` (Java: `null` after a warning) when no division could be found.
    fn calc_division_lines(
        &self,
        inner_corner_no: usize,
        outer_simplex: &Simplex,
    ) -> Option<Vec<Line>> {
        let current_inner_line = self.lines[inner_corner_no];
        let prev_inner_line = if inner_corner_no != 0 {
            self.lines[inner_corner_no - 1]
        } else {
            self.lines[self.lines.len() - 1]
        };
        let intersection = current_inner_line.intersection_approx(&prev_inner_line);
        if intersection.x >= i32::MAX as f64 {
            // Java warns "Simplex.calc_division_lines: intersection expected" and returns null.
            return None;
        }
        let inner_corner = intersection.round();
        let ctolerance = 0.0001;
        let is_exact = (inner_corner.x as f64 - intersection.x).abs() < ctolerance
            && (inner_corner.y as f64 - intersection.y).abs() < ctolerance;

        if !is_exact {
            // it is assumed, that the corners of the original inner simplex are
            // exact and the not exact corners come from the intersection of
            // the inner simplex with the outer simplex.
            // Because these corners lie on the border of the outer simplex,
            // no division is necessary
            return Some(vec![prev_inner_line]);
        }
        let mut first_projection_dir = IntDirection::NULL;
        let mut second_projection_dir = IntDirection::NULL;
        let prev_inner_dir = prev_inner_line.direction().opposite();
        let next_inner_dir = current_inner_line.direction();
        let mut outer_line_no = 0usize;

        // search the first outer line, so that
        // the perpendicular projection of the inner corner onto this
        // line is visible from innerCorner to the left of prevInnerLine.

        let mut min_distance = i32::MAX as f64;
        let last_outer = outer_simplex.lines.len() - 1;

        for _ind in 0..outer_simplex.lines.len() {
            let outer_line = outer_simplex.lines[outer_line_no];
            let current_projection_dir = perpendicular_int_direction(inner_corner, &outer_line);
            if current_projection_dir == IntDirection::NULL {
                return Some(vec![Line::new(inner_corner, inner_corner)]);
            }
            let projection_visible = prev_inner_dir.determinant(&current_projection_dir) >= 0;
            if projection_visible {
                let mut current_distance =
                    outer_line.signed_distance(&inner_corner.to_float()).abs();
                let second_division_necessary =
                    current_projection_dir.determinant(&next_inner_dir) < 0;
                // may occur at a sharp angle
                let mut current_second_projection_dir = current_projection_dir;

                if second_division_necessary {
                    // search the first projection_dir between currentProjectionDir
                    // and nextInnerDir, that is visible from next_inner_line
                    let mut second_projection_visible = false;
                    let mut tmp_outer_line_no = outer_line_no;
                    while !second_projection_visible {
                        if tmp_outer_line_no == last_outer {
                            tmp_outer_line_no = 0;
                        } else {
                            tmp_outer_line_no += 1;
                        }
                        current_second_projection_dir = perpendicular_int_direction(
                            inner_corner,
                            &outer_simplex.lines[tmp_outer_line_no],
                        );

                        if current_second_projection_dir == IntDirection::NULL {
                            // inner corner is on outerLine
                            return Some(vec![Line::new(inner_corner, inner_corner)]);
                        }
                        if current_projection_dir.determinant(&current_second_projection_dir) < 0 {
                            // currentSecondProjectionDir not found;
                            // the angle between currentProjectionDir and
                            // currentSecondProjectionDir would be already bigger
                            // than 180 degree
                            current_distance = i32::MAX as f64;
                            break;
                        }

                        second_projection_visible =
                            current_second_projection_dir.determinant(&next_inner_dir) >= 0;
                    }
                    current_distance += outer_simplex.lines[tmp_outer_line_no]
                        .signed_distance(&inner_corner.to_float())
                        .abs();
                }
                if current_distance < min_distance {
                    min_distance = current_distance;
                    first_projection_dir = current_projection_dir;
                    second_projection_dir = current_second_projection_dir;
                }
            }
            if outer_line_no == last_outer {
                outer_line_no = 0;
            } else {
                outer_line_no += 1;
            }
        }
        if min_distance == i32::MAX as f64 {
            // Java warns "Simplex.calc_division_lines: division not found" and returns null.
            return None;
        }
        if first_projection_dir == second_projection_dir {
            Some(vec![Line::from_direction(
                inner_corner,
                &first_projection_dir,
            )])
        } else {
            Some(vec![
                Line::from_direction(inner_corner, &first_projection_dir),
                Line::from_direction(inner_corner, &second_projection_dir),
            ])
        }
    }

    // added in Task 14 (TileShape / Shape / Circle / ShapeBoundingDirections): simplify(),
    // boundingShape(dirs), intersection(TileShape), cutout(TileShape), intersects(Shape),
    // intersects(Circle), plus the concrete methods `Simplex` inherits from `TileShape` and
    // `PolylineShape` (area(), circumference(), contains(...), borderDistance(...),
    // nearestPoint(...), divideIntoSections(), ...).
}

/// Java `innerCorner.perpendicularDirection(outerLine)` (Point.java:101-113) narrowed back to the
/// `IntDirection` that `Simplex.calcDivisionLines` casts it to. `Point::perpendicular_direction`
/// returns either `Direction::NULL` or a 45-degree turn of the line's own `IntDirection`, so the
/// `Direction::Big` arm is unreachable.
fn perpendicular_int_direction(point: IntPoint, line: &Line) -> IntDirection {
    match Point::Int(point).perpendicular_direction(line) {
        Direction::Int(d) => d,
        Direction::Big(_) => {
            unreachable!("Point::perpendicular_direction always returns a Direction::Int")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::line::Line;
    use crate::point::Point;

    fn unit_box() -> IntBox {
        IntBox::from_coords(0, 0, 10, 10)
    }

    #[test]
    fn box_to_simplex_has_four_lines_and_same_corners() {
        let s = unit_box().to_simplex();
        assert_eq!(s.border_line_count(), 4);
        assert!(s.is_bounded());
        assert!(s.is_int_box());
        assert!(s.is_int_octagon());
        assert_eq!(s.bounding_box(), unit_box());
        let corners: Vec<Point> = (0..4).map(|i| s.corner(i)).collect();
        for c in [(0, 0), (10, 0), (10, 10), (0, 10)] {
            assert!(corners.contains(&Point::Int(IntPoint::new(c.0, c.1))));
        }
        assert_eq!(s.to_int_octagon(), Some(unit_box().to_int_octagon()));
    }

    #[test]
    fn triangle_from_points() {
        let t = Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
            IntPoint::new(0, 10),
        ]);
        assert_eq!(t.border_line_count(), 3);
        assert!(!t.is_int_box());
        assert_eq!(t.bounding_box(), unit_box());
        assert_eq!(t.dimension(), 2);
        assert!(
            t.corner_approx_arr()
                .iter()
                .any(|p| (p.x - 10.0).abs() < 1e-9 && p.y.abs() < 1e-9)
        );
    }

    #[test]
    fn intersection_of_box_and_triangle() {
        let t = Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(20, 0),
            IntPoint::new(0, 20),
        ]);
        let i = t.intersection_box(&IntBox::from_coords(5, 5, 30, 30));
        assert!(!i.is_empty());
        // triangle (5,5),(15,5),(5,15)
        assert_eq!(i.bounding_box(), IntBox::from_coords(5, 5, 15, 15));
        assert_eq!(i.border_line_count(), 3);
        assert!(t.intersects_box(&IntBox::from_coords(5, 5, 30, 30)));
        assert!(!t.intersects_box(&IntBox::from_coords(15, 15, 30, 30)));
        assert!(
            t.intersection_box(&IntBox::from_coords(15, 15, 30, 30))
                .is_empty()
        );
    }

    #[test]
    fn redundant_lines_are_removed() {
        let mut lines: Vec<Line> = unit_box().to_simplex().lines().to_vec();
        // An extra line far away whose half-plane still contains the whole box, so it does not
        // contribute to the shape. The half-plane of a directed line is the set of points `p`
        // with `line.side_of(p) == Side::OnTheRight` (Simplex.java:276-281 reads the same test),
        // i.e. for this rightwards line everything with `y >= -100`.
        lines.push(Line::from_coords(0, -100, 10, -100));
        let s = Simplex::from_lines(lines);
        assert_eq!(s.border_line_count(), 4);
        assert_eq!(s.bounding_box(), unit_box());
    }

    #[test]
    fn a_far_line_on_the_other_side_makes_the_simplex_empty() {
        // The brief's guess used `Line::from_coords(0, 100, 10, 100)` as a line "that does not cut
        // the box". It does cut it: the half-plane of that rightwards line is `y >= 100`, which is
        // disjoint from the box, so Java's `removeRedundantLines` reaches the parallel-lines
        // emptiness branch (Simplex.java:949-955, `prevLine.sideOf(nextLine.a) == ON_THE_LEFT`)
        // and returns `Simplex.EMPTY`.
        let mut lines: Vec<Line> = unit_box().to_simplex().lines().to_vec();
        lines.push(Line::from_coords(0, 100, 10, 100));
        let s = Simplex::from_lines(lines);
        assert!(s.is_empty());
        assert_eq!(s.border_line_count(), 0);
        assert_eq!(s.dimension(), -1);
    }

    #[test]
    fn unbounded_simplex() {
        // The half-plane of the upward line x = 0, i.e. `x <= 0`: `Line::side_of` reports where
        // the line is as seen from the point, so the half-plane is geometrically on the *left*
        // of the directed line (see the module doc).
        let s = Simplex::from_lines(vec![Line::from_coords(0, 0, 0, 1)]);
        assert!(!s.is_bounded());
        assert_eq!(s.dimension(), 2);
        assert!(!s.corner_is_bounded(0));
    }

    /// The `cutout_from` pieces must tile `outer \ inner` exactly: three convex pieces whose
    /// shoelace areas sum to `area(outer) - area(inner) = 400 - 50`. (`TileShape.area()` itself
    /// is a Task 14 method, so the shoelace is computed here.)
    #[test]
    fn cutout_from_pieces_tile_the_difference() {
        let outer = IntBox::from_coords(0, 0, 20, 20).to_simplex();
        let inner = Simplex::from_points(&[
            IntPoint::new(5, 5),
            IntPoint::new(15, 5),
            IntPoint::new(10, 15),
        ]);
        let pieces = inner.cutout_from(&outer).expect("inner is 2-dimensional");
        assert_eq!(pieces.len(), 3);
        let total: f64 = pieces.iter().map(shoelace_area).sum();
        assert!((total - 350.0).abs() < 1e-9, "total area {total}");
    }

    fn shoelace_area(s: &Simplex) -> f64 {
        let c = s.corner_approx_arr();
        let mut a = 0.0;
        for i in 0..c.len() {
            let j = (i + 1) % c.len();
            a += c[i].x * c[j].y - c[j].x * c[i].y;
        }
        a / 2.0
    }

    /// Java returns `{ outerSimplex }` when the intersection is not 2-dimensional, and `null`
    /// when *this* simplex is not 2-dimensional (Simplex.java:706-716).
    #[test]
    fn cutout_from_edge_cases() {
        let outer = IntBox::from_coords(0, 0, 20, 20).to_simplex();
        let disjoint = Simplex::from_points(&[
            IntPoint::new(50, 50),
            IntPoint::new(60, 50),
            IntPoint::new(50, 60),
        ]);
        assert_eq!(disjoint.cutout_from(&outer), Some(vec![outer.clone()]));
        // A half plane is 2-dimensional, but `Simplex::EMPTY` is not.
        assert_eq!(Simplex::EMPTY.cutout_from(&outer), None);
    }

    #[test]
    fn widths_and_centre_of_gravity() {
        let s = unit_box().to_simplex();
        assert_eq!(s.centre_of_gravity(), FloatPoint::new(5.0, 5.0));
        // The two biggest / two smallest of the four distances (all 5) from the centre.
        assert_eq!(s.max_width(), 10.0);
        assert_eq!(s.min_width(), 10.0);
        // Java returns Integer.MAX_VALUE for an unbounded simplex.
        let half_plane = Simplex::from_lines(vec![Line::from_coords(0, 0, 0, 1)]);
        assert_eq!(half_plane.max_width(), i32::MAX as f64);
        assert_eq!(half_plane.min_width(), i32::MAX as f64);
    }

    #[test]
    fn offset_and_enlarge() {
        let s = unit_box().to_simplex();
        assert_eq!(s.offset(0.0), s);
        let outer = s.offset(2.0);
        assert_eq!(outer.border_line_count(), 4);
        assert_eq!(outer.bounding_box(), IntBox::from_coords(-2, -2, 12, 12));
        let inner = s.offset(-2.0);
        assert_eq!(inner.border_line_count(), 4);
        assert_eq!(inner.bounding_box(), IntBox::from_coords(2, 2, 8, 8));
        // `enlarge` intersects the offsetted simplex with the offsetted bounding octagon, so the
        // four corners of the box get cut off by the four diagonal octagon lines.
        let enlarged = s.enlarge(2.0);
        assert_eq!(enlarged.border_line_count(), 8);
        assert_eq!(enlarged.bounding_box(), IntBox::from_coords(-2, -2, 12, 12));
        assert_eq!(s.enlarge(0.0), s);
        assert_eq!(
            s.bounding_octagon(),
            Some(IntOctagon::new(0, 0, 10, 10, -10, 10, 0, 20))
        );
    }

    #[test]
    fn border_lines_and_removal() {
        let s = unit_box().to_simplex();
        let line0 = s.border_line(0).expect("not empty");
        assert_eq!(
            line0,
            Line::from_direction(IntPoint::ZERO, &IntDirection::RIGHT)
        );
        // Out-of-range indices are clamped to the last line, as in Java.
        assert_eq!(s.border_line(99), s.border_line(3));
        assert_eq!(Simplex::EMPTY.border_line(0), None);
        // `border_line_index` uses the geometric `Line::equals_geometric`: a different pair of
        // points on the same directed line still matches.
        assert_eq!(s.border_line_index(&line0), Some(0));
        assert_eq!(
            s.border_line_index(&Line::from_coords(-7, 0, 12, 0)),
            Some(0)
        );
        assert_eq!(s.border_line_index(&Line::from_coords(0, 3, 1, 3)), None);
        let opened = s.remove_border_line(0);
        assert_eq!(opened.border_line_count(), 3);
        assert!(!opened.is_bounded());
        assert_eq!(s.remove_border_line(4), s);
    }

    #[test]
    fn translate_by_and_identities() {
        let s = unit_box().to_simplex();
        assert_eq!(s.translate_by(&Vector::ZERO), s);
        let moved = s.translate_by(&Vector::new(3, -4));
        assert_eq!(moved.bounding_box(), IntBox::from_coords(3, -4, 13, 6));
        assert_eq!(s.to_simplex(), s);
        assert_eq!(s.bounding_tile(), s);
        assert_eq!(Simplex::new(s.lines().to_vec()), s);
    }

    #[test]
    fn index_of_right_most_corner() {
        let t = Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
            IntPoint::new(0, 10),
        ]);
        // Corners in stored order are (0,0), (10,0), (0,10); seen from (100, 0) the rightmost is
        // (0, 10).
        assert_eq!(
            t.index_of_right_most_corner(&Point::Int(IntPoint::new(100, 0))),
            2
        );
    }

    #[test]
    fn empty_simplex_behaviour() {
        let e = Simplex::EMPTY;
        assert!(e.is_empty());
        assert_eq!(e.dimension(), -1);
        // Java: `isBounded` is true for 0 lines. `cornerIsBounded(0)` on the other hand *throws*
        // in Java (`no` is clamped to `lines.length - 1 == -1`, then `lines[-2]` raises
        // ArrayIndexOutOfBoundsException, Simplex.java:86-109); `false` here is this port's
        // totalization of that, not observed Java behaviour.
        assert!(e.is_bounded());
        assert!(!e.corner_is_bounded(0));
        assert_eq!(e.corner_approx(0), None);
        assert!(e.corner_approx_arr().is_empty());
        assert_eq!(e.bounding_box(), IntBox::EMPTY);
        assert_eq!(e.get_id(), 0);
        assert!(!e.intersects(&unit_box().to_simplex()));
        assert!(e.intersection(&unit_box().to_simplex()).is_empty());
        // `isIntOctagon` vacuously holds for 0 lines, so `toIntOctagon` returns IntOctagon.EMPTY.
        assert_eq!(e.to_int_octagon(), Some(IntOctagon::EMPTY));
        assert_eq!(Simplex::from_lines(Vec::new()), Simplex::EMPTY);
        assert_eq!(Simplex::from_points(&[]), Simplex::EMPTY);
        assert_eq!(e.remove_redundant_lines(), Simplex::EMPTY);
    }

    #[test]
    fn to_int_octagon_rejects_a_non_45_degree_simplex() {
        let t = Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
            IntPoint::new(0, 20),
        ]);
        assert!(!t.is_int_octagon());
        assert_eq!(t.to_int_octagon(), None);
        // A 45-degree triangle does convert.
        let d = Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
            IntPoint::new(0, 10),
        ]);
        assert!(d.is_int_octagon());
        assert_eq!(
            d.to_int_octagon(),
            Some(IntOctagon::new(0, 0, 10, 10, -10, 10, 0, 10))
        );
    }

    #[test]
    fn dimension_of_degenerate_simplices() {
        // Two opposite collinear lines: a 1-dimensional strip of zero width.
        let a = Line::from_coords(0, 0, 1, 0);
        let strip = Simplex::new(vec![a, a.opposite()]);
        assert_eq!(strip.dimension(), 1);
        // Two crossing half-planes: 2-dimensional wedge.
        assert_eq!(
            Simplex::new(vec![a, Line::from_coords(0, 0, 0, 1)]).dimension(),
            2
        );
        // Four lines with both opposing pairs collinear: a single point.
        let b = Line::from_coords(0, 0, 0, 1);
        assert_eq!(
            Simplex::new(vec![a, b, a.opposite(), b.opposite()]).dimension(),
            0
        );
        // More than four lines is always reported as 2-dimensional, without any check.
        let oct = IntOctagon::new(0, 0, 10, 10, -5, 15, 5, 15).normalize();
        assert!(oct.to_simplex().border_line_count() >= 5);
        assert_eq!(oct.to_simplex().dimension(), 2);
    }

    #[test]
    fn octagon_round_trip_through_simplex() {
        let oct = IntOctagon::new(0, 0, 10, 10, -5, 15, 5, 15).normalize();
        let s = oct.to_simplex();
        // The two redundant border lines of this octagon (the ones that do not bite) are gone.
        assert_eq!(s.border_line_count(), 5);
        assert!(s.is_int_octagon());
        assert_eq!(s.to_int_octagon(), Some(oct));
        assert_eq!(s.bounding_box(), oct.bounding_box());
        assert_eq!(IntOctagon::EMPTY.to_simplex(), Simplex::EMPTY);
    }

    #[test]
    fn simplex_typed_methods_on_int_box_and_int_octagon() {
        let t = Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(20, 0),
            IntPoint::new(0, 20),
        ]);
        let b = IntBox::from_coords(5, 5, 30, 30);
        assert_eq!(b.intersection_simplex(&t), t.intersection_box(&b));
        assert!(b.intersects_simplex(&t));
        assert!(!IntBox::from_coords(15, 15, 30, 30).intersects_simplex(&t));

        let oct = IntBox::from_coords(5, 5, 30, 30).to_int_octagon();
        assert_eq!(oct.intersection_simplex(&t), t.intersection_octagon(&oct));
        assert!(oct.intersects_simplex(&t));
        assert!(t.intersects_octagon(&oct));

        // `cutout_from_*` all funnel through the simplex version.
        let outer_box = IntBox::from_coords(0, 0, 20, 20);
        let inner = Simplex::from_points(&[
            IntPoint::new(5, 5),
            IntPoint::new(15, 5),
            IntPoint::new(10, 15),
        ]);
        assert_eq!(
            inner.cutout_from_box(&outer_box),
            inner.cutout_from(&outer_box.to_simplex())
        );
        assert_eq!(
            inner.cutout_from_octagon(&outer_box.to_int_octagon()),
            inner.cutout_from(&outer_box.to_int_octagon().to_simplex())
        );
        assert_eq!(
            outer_box.cutout_from_simplex(&inner),
            outer_box.to_simplex().cutout_from(&inner)
        );
        assert_eq!(
            outer_box.to_int_octagon().cutout_from_simplex(&inner),
            outer_box.to_int_octagon().to_simplex().cutout_from(&inner)
        );
    }

    #[test]
    fn get_id_is_stable_and_wraps() {
        let s = unit_box().to_simplex();
        assert_eq!(s.get_id(), s.clone().get_id());
        assert_ne!(s.get_id(), s.translate_by(&Vector::new(1, 1)).get_id());
        // Java `int` arithmetic wraps silently; huge coordinates must not panic.
        let big = Simplex::new(vec![Line::from_coords(
            crate::CRIT_INT,
            crate::CRIT_INT,
            crate::CRIT_INT - 1,
            crate::CRIT_INT,
        )]);
        let _ = big.get_id();
    }

    #[test]
    fn cutout_from_box() {
        let outer = IntBox::from_coords(0, 0, 20, 20).to_simplex();
        let inner = Simplex::from_points(&[
            IntPoint::new(5, 5),
            IntPoint::new(15, 5),
            IntPoint::new(10, 15),
        ]);
        let pieces = inner.cutout_from(&outer).expect("inner is 2-dimensional");
        assert!(pieces.len() >= 3);
        for p in &pieces {
            assert!(p.intersection(&inner).dimension() < 2);
        }
    }
}
