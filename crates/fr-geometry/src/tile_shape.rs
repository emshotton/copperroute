//! Port of `app.freerouting.geometry.planar.TileShape`: convex shapes whose border consists of
//! straight lines.
//!
//! Java has three concrete tile shapes — `IntBox`, `IntOctagon` (both `RegularTileShape`) and
//! `Simplex` — and puts roughly 900 lines of *concrete* algorithms on the shared abstract base,
//! written entirely in terms of the three abstract primitives `corner(i)`, `borderLine(i)` and
//! `borderLineCount()`. This module mirrors that split: the primitives are a `match` on the enum,
//! and everything above them is ported once, as inherent methods of [`TileShape`].
//!
//! Side convention: an interior point `p` of a tile shape satisfies
//! `border_line(i).side_of(p) == Side::OnTheRight` for every `i`. `Line::side_of` reports where
//! the *line* is as seen from the point, so those points lie geometrically to the left of each
//! directed border line — see the `simplex` module doc. `TileShape.isOutside` therefore tests for
//! `ON_THE_LEFT`, and that test is ported verbatim.
//!
//! Where Java returns `null` this port returns `Option`; where a Java array may carry trailing
//! `null`s (`nearestBorderPointsApprox`, `nearestRelativeOutsideLocations`) the `Vec` returned
//! here holds only the filled prefix, which is what Java produces.

use crate::direction::Direction;
use crate::float_line::FloatLine;
use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_direction::IntDirection;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::limits::{JAVA_DOUBLE_MIN_VALUE, java_min};
use crate::line::Line;
use crate::point::Point;
use crate::side::Side;
use crate::simplex::Simplex;
use crate::vector::Vector;

/// A convex shape whose border consists of straight lines (TileShape.java:14-15).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TileShape {
    /// Java `IntBox`.
    Box(IntBox),
    /// Java `IntOctagon`.
    Octagon(IntOctagon),
    /// Java `Simplex`.
    Simplex(Simplex),
}

impl From<IntBox> for TileShape {
    fn from(value: IntBox) -> Self {
        TileShape::Box(value)
    }
}

impl From<IntOctagon> for TileShape {
    fn from(value: IntOctagon) -> Self {
        TileShape::Octagon(value)
    }
}

impl From<Simplex> for TileShape {
    fn from(value: Simplex) -> Self {
        TileShape::Simplex(value)
    }
}

// ---------------------------------------------------------------------------------------------
// Static factories (TileShape.java:17-63)
// ---------------------------------------------------------------------------------------------

impl TileShape {
    /// Creates a Simplex as intersection of the half-planes defined by directed lines, then
    /// simplifies it to the cheapest physical representation (TileShape.java:17-21).
    pub fn get_instance_from_lines(lines: Vec<Line>) -> TileShape {
        Simplex::from_lines(lines).simplify()
    }

    /// Creates a TileShape from the corners of a convex polygon (TileShape.java:23-34). Java's
    /// parameter type is `Point[]`, but the construction goes through `new Line(Point, Point)`,
    /// which this port fixes at `IntPoint` — matching Java's own "May work only for IntPoints".
    pub fn get_instance_from_points(convex_polygon: &[IntPoint]) -> TileShape {
        Simplex::from_points(convex_polygon).simplify()
    }

    /// Creates a half-plane from a directed line (TileShape.java:36-41). Java deliberately skips
    /// the `simplify()` here.
    pub fn get_instance_from_line(line: Line) -> TileShape {
        TileShape::Simplex(Simplex::from_lines(vec![line]))
    }

    /// Creates a normalized `IntOctagon` from the eight bounds (TileShape.java:43-51). For the
    /// meaning of the parameters see [`IntOctagon`].
    #[allow(clippy::too_many_arguments)] // literal transcription of the Java factory
    pub fn get_instance_octagon(
        lx: i32,
        ly: i32,
        rx: i32,
        uy: i32,
        ulx: i32,
        lrx: i32,
        llx: i32,
        urx: i32,
    ) -> IntOctagon {
        IntOctagon::new(lx, ly, rx, uy, ulx, lrx, llx, urx).normalize()
    }

    /// Creates a box-like convex shape (TileShape.java:53-58).
    pub fn get_instance_from_box(
        lower_left_x: i32,
        lower_left_y: i32,
        upper_right_x: i32,
        upper_right_y: i32,
    ) -> IntOctagon {
        IntBox::from_coords(lower_left_x, lower_left_y, upper_right_x, upper_right_y)
            .to_int_octagon()
    }

    /// Creates the smallest box containing `point` (TileShape.java:60-63; Java's doc comment says
    /// "IntOctagon", the code returns `point.surroundingBox()`).
    pub fn get_instance_from_point(point: &Point) -> IntBox {
        point.surrounding_box()
    }
}

// ---------------------------------------------------------------------------------------------
// The abstract primitives, dispatched over the three physical representations.
// ---------------------------------------------------------------------------------------------

impl TileShape {
    /// Returns the number of border lines of the shape (`PolylineShape.borderLineCount`).
    pub fn border_line_count(&self) -> usize {
        match self {
            TileShape::Box(b) => b.border_line_count(),
            TileShape::Octagon(o) => o.border_line_count(),
            TileShape::Simplex(s) => s.border_line_count(),
        }
    }

    /// Returns the `no`-th edge line of this shape, for `no` between 0 and
    /// `border_line_count() - 1`. The edge lines are sorted in counterclock sense around the
    /// shape, starting with the edge with the smallest direction (TileShape.java:97-103).
    ///
    /// `None` only for the empty simplex, where Java returns `null`; `IntBox` and `IntOctagon`
    /// throw on an out-of-range index and so panic here.
    pub fn border_line(&self, no: usize) -> Option<Line> {
        match self {
            TileShape::Box(b) => Some(b.border_line(no)),
            TileShape::Octagon(o) => Some(o.border_line(no)),
            TileShape::Simplex(s) => s.border_line(no),
        }
    }

    /// Returns the `no`-th corner of this shape. The corners are sorted starting with the
    /// smallest y-coordinate in counterclock sense around the shape
    /// (`PolylineShape.corner`, PolylineShape.java:18-24).
    pub fn corner(&self, no: usize) -> Point {
        match self {
            TileShape::Box(b) => Point::Int(b.corner(no)),
            TileShape::Octagon(o) => Point::Int(o.corner(no)),
            TileShape::Simplex(s) => s.corner(no),
        }
    }

    /// Returns an approximation of the `no`-th corner (`PolylineShape.cornerApprox`,
    /// overridden in `Simplex`). `None` only for the empty simplex.
    pub fn corner_approx(&self, no: usize) -> Option<FloatPoint> {
        match self {
            TileShape::Box(b) => Some(b.corner(no).to_float()),
            TileShape::Octagon(o) => Some(o.corner(no).to_float()),
            TileShape::Simplex(s) => s.corner_approx(no),
        }
    }

    /// Approximations of all corners of this shape (`PolylineShape.cornerApproxArr`).
    pub fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        match self {
            TileShape::Simplex(s) => s.corner_approx_arr(),
            _ => (0..self.border_line_count())
                .map(|i| self.corner_approx_at(i))
                .collect(),
        }
    }

    /// Returns true if the shape has no infinite part at this corner
    /// (`PolylineShape.cornerIsBounded`).
    pub fn corner_is_bounded(&self, no: usize) -> bool {
        match self {
            TileShape::Box(b) => b.corner_is_bounded(no),
            TileShape::Octagon(o) => o.corner_is_bounded(no),
            TileShape::Simplex(s) => s.corner_is_bounded(no),
        }
    }

    /// Returns true if the shape is contained in a sufficiently large box.
    pub fn is_bounded(&self) -> bool {
        match self {
            TileShape::Box(b) => b.is_bounded(),
            TileShape::Octagon(o) => o.is_bounded(),
            TileShape::Simplex(s) => s.is_bounded(),
        }
    }

    /// Returns true if this shape is empty.
    pub fn is_empty(&self) -> bool {
        match self {
            TileShape::Box(b) => b.is_empty(),
            TileShape::Octagon(o) => o.is_empty(),
            TileShape::Simplex(s) => s.is_empty(),
        }
    }

    /// Returns the dimension of this shape: 2, 1, 0, or -1 if it is empty.
    pub fn dimension(&self) -> i32 {
        match self {
            TileShape::Box(b) => b.dimension(),
            TileShape::Octagon(o) => o.dimension(),
            TileShape::Simplex(s) => s.dimension(),
        }
    }

    /// Converts the physical instance of this shape to a simpler physical instance, if possible
    /// (TileShape.java:74-75; IntBox.java:118-121, IntOctagon.java:1050-1057,
    /// Simplex.java:59-70).
    pub fn simplify(&self) -> TileShape {
        match self {
            TileShape::Box(b) => b.simplify(),
            TileShape::Octagon(o) => o.simplify(),
            TileShape::Simplex(s) => s.simplify(),
        }
    }

    /// A deterministic tie-breaking id for this shape (TileShape.java:77-78).
    pub fn get_id(&self) -> i32 {
        match self {
            TileShape::Box(b) => b.get_id(),
            TileShape::Octagon(o) => o.get_id(),
            TileShape::Simplex(s) => s.get_id(),
        }
    }

    /// Checks if this TileShape is an `IntBox` or can be converted into one (TileShape.java:80-81).
    pub fn is_int_box(&self) -> bool {
        match self {
            TileShape::Box(b) => b.is_int_box(),
            TileShape::Octagon(o) => o.is_int_box(),
            TileShape::Simplex(s) => s.is_int_box(),
        }
    }

    /// Checks if this TileShape is an `IntOctagon` or can be converted into one
    /// (TileShape.java:83-84).
    pub fn is_int_octagon(&self) -> bool {
        match self {
            TileShape::Box(b) => b.is_int_octagon(),
            TileShape::Octagon(o) => o.is_int_octagon(),
            TileShape::Simplex(s) => s.is_int_octagon(),
        }
    }

    /// Converts the internal representation of this TileShape to a `Simplex`
    /// (TileShape.java:108-109).
    pub fn to_simplex(&self) -> Simplex {
        match self {
            TileShape::Box(b) => b.to_simplex(),
            TileShape::Octagon(o) => o.to_simplex(),
            TileShape::Simplex(s) => s.to_simplex(),
        }
    }

    /// Returns the edge number if `line` is a border line of this shape (TileShape.java:105-106).
    ///
    /// Only `Simplex` implements this for real (Simplex.java:664-673); `IntBox.borderLineIndex`
    /// and `IntOctagon.borderLineIndex` are Java stubs that warn "not yet implemented" and return
    /// `-1`, which is `None` here.
    pub fn border_line_index(&self, line: &Line) -> Option<usize> {
        match self {
            TileShape::Box(b) => b.border_line_index(line),
            TileShape::Octagon(o) => o.border_line_index(line),
            TileShape::Simplex(s) => s.border_line_index(line),
        }
    }

    /// Returns the smallest axis-parallel box containing this shape.
    pub fn bounding_box(&self) -> IntBox {
        match self {
            TileShape::Box(b) => b.bounding_box(),
            TileShape::Octagon(o) => o.bounding_box(),
            TileShape::Simplex(s) => s.bounding_box(),
        }
    }

    /// Returns the smallest 45-degree octagon containing this shape; `None` for an unbounded
    /// simplex, where Java returns `null`.
    pub fn bounding_octagon(&self) -> Option<IntOctagon> {
        match self {
            TileShape::Box(b) => Some(b.bounding_octagon()),
            TileShape::Octagon(o) => Some(o.bounding_octagon()),
            TileShape::Simplex(s) => s.bounding_octagon(),
        }
    }

    /// Java `boundingTile()`: every tile shape returns itself
    /// (IntBox.java:261-264, IntOctagon.java:121-124, Simplex.java:541-544).
    pub fn bounding_tile(&self) -> TileShape {
        self.clone()
    }

    /// Returns the affine translation of this shape by `vector`.
    pub fn translate_by(&self, vector: &Vector) -> TileShape {
        match self {
            TileShape::Box(b) => TileShape::Box(b.translate_by(vector)),
            TileShape::Octagon(o) => TileShape::Octagon(o.translate_by(vector)),
            TileShape::Simplex(s) => TileShape::Simplex(s.translate_by(vector)),
        }
    }

    /// Returns this shape offsetted by `dist`. If `dist > 0` the offset is to the outside, else
    /// to the inside. The physical representation is preserved
    /// (IntBox.java:443-455, IntOctagon.java:298-316, Simplex.java:555-574).
    pub fn offset(&self, dist: f64) -> TileShape {
        match self {
            TileShape::Box(b) => TileShape::Box(b.offset(dist)),
            TileShape::Octagon(o) => TileShape::Octagon(o.offset(dist)),
            TileShape::Simplex(s) => TileShape::Simplex(s.offset(dist)),
        }
    }

    /// Enlarges this shape by `offset`. Contrary to [`TileShape::offset`], an enlarged `IntBox`
    /// becomes an `IntOctagon` (IntBox.java:387-393).
    pub fn enlarge(&self, offset: f64) -> TileShape {
        match self {
            TileShape::Box(b) => TileShape::Octagon(b.enlarge(offset)),
            TileShape::Octagon(o) => TileShape::Octagon(o.enlarge(offset)),
            TileShape::Simplex(s) => TileShape::Simplex(s.enlarge(offset)),
        }
    }

    /// The maximum of the edge widths of this shape.
    pub fn max_width(&self) -> f64 {
        match self {
            TileShape::Box(b) => b.max_width(),
            TileShape::Octagon(o) => o.max_width(),
            TileShape::Simplex(s) => s.max_width(),
        }
    }

    /// The minimum of the edge widths of this shape.
    pub fn min_width(&self) -> f64 {
        match self {
            TileShape::Box(b) => b.min_width(),
            TileShape::Octagon(o) => o.min_width(),
            TileShape::Simplex(s) => s.min_width(),
        }
    }

    /// The cumulative border line length of this shape; `i32::MAX` if it is unbounded
    /// (`PolylineShape.circumference`, PolylineShape.java:99-117, overridden by `IntBox`).
    pub fn circumference(&self) -> f64 {
        match self {
            TileShape::Box(b) => b.circumference(),
            _ => {
                if !self.is_bounded() {
                    return i32::MAX as f64;
                }
                let corner_count = self.border_line_count();
                if corner_count == 0 {
                    // Java assigns `prevCorner = cornerApprox(cornerCount - 1)` = `cornerApprox(-1)`,
                    // which `Simplex.cornerApprox` answers with `null` for a line-less simplex
                    // (Simplex.java:172-175 — the `null` check precedes the index clamp). The loop
                    // then runs zero times and the sum stays 0.
                    return 0.0;
                }
                let mut result = 0.0;
                let mut prev_corner = self.corner_approx_at(corner_count - 1);
                for i in 0..corner_count {
                    let current_corner = self.corner_approx_at(i);
                    result += current_corner.distance(&prev_corner);
                    prev_corner = current_corner;
                }
                result
            }
        }
    }

    /// The arithmetic middle of the corners of this shape (`PolylineShape.centreOfGravity`,
    /// PolylineShape.java:119-133).
    pub fn centre_of_gravity(&self) -> FloatPoint {
        let corner_count = self.border_line_count();
        let mut x = 0.0;
        let mut y = 0.0;
        for i in 0..corner_count {
            let current_point = self.corner_approx_at(i);
            x += current_point.x;
            y += current_point.y;
        }
        x /= corner_count as f64;
        y /= corner_count as f64;
        FloatPoint::new(x, y)
    }

    /// The `no`-th border line, for an index that is known to be in range. Panics on the empty
    /// simplex, where every loop below has zero iterations anyway.
    fn border_line_at(&self, no: usize) -> Line {
        self.border_line(no)
            .expect("border line index is below border_line_count()")
    }

    /// The `no`-th corner approximation, for an index that is known to be in range.
    fn corner_approx_at(&self, no: usize) -> FloatPoint {
        self.corner_approx(no)
            .expect("corner index is below border_line_count()")
    }
}

// ---------------------------------------------------------------------------------------------
// Intersection, containment and metrics (the concrete part of TileShape.java).
// ---------------------------------------------------------------------------------------------

impl TileShape {
    /// Tries to simplify the result shape to a simpler shape. Simplifying always in the
    /// intersection function may cause performance problems (TileShape.java:65-72).
    pub fn intersection_with_simplify(&self, other: &TileShape) -> TileShape {
        self.intersection(other).simplify()
    }

    /// Returns the intersection of this shape with `other` (TileShape.java:86-95).
    ///
    /// The physical representation of the result follows Java's overload resolution: box ∩ box is
    /// a box, any pairing with an octagon (and no simplex) is an octagon, and any pairing with a
    /// simplex is a simplex.
    pub fn intersection(&self, other: &TileShape) -> TileShape {
        match (self, other) {
            (TileShape::Box(a), TileShape::Box(b)) => TileShape::Box(b.intersection(a)),
            (TileShape::Box(a), TileShape::Octagon(b)) => TileShape::Octagon(b.intersection_box(a)),
            (TileShape::Box(a), TileShape::Simplex(b)) => TileShape::Simplex(b.intersection_box(a)),
            (TileShape::Octagon(a), TileShape::Box(b)) => {
                TileShape::Octagon(b.intersection_octagon(a))
            }
            (TileShape::Octagon(a), TileShape::Octagon(b)) => TileShape::Octagon(b.intersection(a)),
            (TileShape::Octagon(a), TileShape::Simplex(b)) => {
                TileShape::Simplex(b.intersection_octagon(a))
            }
            (TileShape::Simplex(a), TileShape::Box(b)) => TileShape::Simplex(a.intersection_box(b)),
            (TileShape::Simplex(a), TileShape::Octagon(b)) => {
                TileShape::Simplex(a.intersection_octagon(b))
            }
            (TileShape::Simplex(a), TileShape::Simplex(b)) => TileShape::Simplex(b.intersection(a)),
        }
    }

    /// Checks whether this shape and `other` have a nonempty intersection (`Shape.intersects`,
    /// implemented per pair in IntBox.java:329-357, IntOctagon.java:627-676,
    /// Simplex.java:638-658).
    pub fn intersects(&self, other: &TileShape) -> bool {
        // Java: `other.intersects(this)`, so the receiver is the *argument* shape.
        match self {
            TileShape::Box(b) => other.intersects_box(b),
            TileShape::Octagon(o) => other.intersects_octagon(o),
            TileShape::Simplex(s) => other.intersects_simplex(s),
        }
    }

    /// Java `intersects(IntBox other)`.
    pub fn intersects_box(&self, other: &IntBox) -> bool {
        match self {
            TileShape::Box(b) => b.intersects(other),
            TileShape::Octagon(o) => o.intersects_box(other),
            TileShape::Simplex(s) => s.intersects_box(other),
        }
    }

    /// Java `intersects(IntOctagon other)`.
    pub fn intersects_octagon(&self, other: &IntOctagon) -> bool {
        match self {
            TileShape::Box(b) => b.intersects_octagon(other),
            TileShape::Octagon(o) => o.intersects_octagon(other),
            TileShape::Simplex(s) => s.intersects_octagon(other),
        }
    }

    /// Java `intersects(Simplex other)`.
    pub fn intersects_simplex(&self, other: &Simplex) -> bool {
        match self {
            TileShape::Box(b) => b.intersects_simplex(other),
            TileShape::Octagon(o) => o.intersects_simplex(other),
            TileShape::Simplex(s) => s.intersects(other),
        }
    }

    /// Returns the content of the area of the shape; `f64::MAX` if the shape is unbounded
    /// (TileShape.java:111-139). `IntBox` and `IntOctagon` override the shoelace sum with a
    /// closed form, so only a `Simplex` runs the loop.
    pub fn area(&self) -> f64 {
        match self {
            TileShape::Box(b) => b.area(),
            TileShape::Octagon(o) => o.area(),
            TileShape::Simplex(_) => {
                if !self.is_bounded() {
                    return f64::MAX;
                }
                if self.dimension() < 2 {
                    return 0.0;
                }
                // calculate half of the absolute value of
                // x0 (y1 - yn-1) + x1 (y2 - y0) + x2 (y3 - y1) + ...+ xn-1( y0 - yn-2)
                // where xi, yi are the coordinates of the i-th corner of this TileShape.
                let corner_count = self.border_line_count();
                // A bounded 2-dimensional simplex always has at least 3 border lines, so the two
                // indices below never underflow.
                debug_assert!(corner_count >= 3);
                let mut result = 0.0;
                let mut prev_corner = self.corner_approx_at(corner_count - 2);
                let mut current_corner = self.corner_approx_at(corner_count - 1);
                for i in 0..corner_count {
                    let next_corner = self.corner_approx_at(i);
                    result += current_corner.x * (next_corner.y - prev_corner.y);
                    prev_corner = current_corner;
                    current_corner = next_corner;
                }
                0.5 * result.abs()
            }
        }
    }

    /// Returns true, if `point` is not contained in the inside or the edge of the shape
    /// (TileShape.java:141-154).
    pub fn is_outside(&self, point: &Point) -> bool {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return true;
        }
        for i in 0..line_count {
            if self.border_line_at(i).side_of(point) == Side::OnTheLeft {
                return true;
            }
        }
        false
    }

    /// Returns true, if `point` is contained in this shape, its border included
    /// (TileShape.java:156-159).
    pub fn contains(&self, point: &Point) -> bool {
        !self.is_outside(point)
    }

    /// Returns true, if `point` is contained in this shape (TileShape.java:161-165).
    /// `IntOctagon` overrides this with an inclusive coordinate test that also accepts border
    /// points (IntOctagon.java:327-343); the generic version below requires the point to be
    /// strictly inside.
    pub fn contains_float(&self, point: &FloatPoint) -> bool {
        match self {
            TileShape::Octagon(o) => o.contains_float(point),
            _ => self.contains_float_tol(point, 0.0),
        }
    }

    /// Returns true, if `point` is contained in this shape with tolerance `tolerance`.
    /// `tolerance` is used when determining if a point is on the left side of a border line. It is
    /// used there in calculating a determinant and is not the distance of point to the border
    /// (TileShape.java:167-183).
    pub fn contains_float_tol(&self, point: &FloatPoint, tolerance: f64) -> bool {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return false;
        }
        for i in 0..line_count {
            if self.border_line_at(i).side_of_float(point, tolerance) != Side::OnTheRight {
                return false;
            }
        }
        true
    }

    /// Returns true, if this shape contains `other` completely (TileShape.java:185-193).
    pub fn contains_tile(&self, other: &TileShape) -> bool {
        for i in 0..other.border_line_count() {
            if !self.contains(&other.corner(i)) {
                return false;
            }
        }
        true
    }

    /// Returns true, if `point` is contained in this shape, but not on an edge line
    /// (TileShape.java:195-208).
    pub fn contains_inside(&self, point: &Point) -> bool {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return false;
        }
        for i in 0..line_count {
            if self.border_line_at(i).side_of(point) != Side::OnTheRight {
                return false;
            }
        }
        true
    }

    /// Returns `Side::Collinear` if `point` is on the border of this shape within `tolerance`,
    /// `Side::OnTheLeft` if it is outside, and `Side::OnTheRight` if it is inside
    /// (TileShape.java:210-232).
    pub fn side_of_border(&self, point: &FloatPoint, tolerance: f64) -> Side {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return Side::Collinear;
        }
        let mut result = Side::OnTheRight; // point is inside
        for i in 0..line_count {
            let current_side = self.border_line_at(i).side_of_float(point, tolerance);
            if current_side == Side::OnTheLeft {
                return Side::OnTheLeft; // point is outside
            } else if current_side == Side::Collinear {
                result = current_side;
            }
        }
        result
    }

    /// If `point` lies on the border of this shape, the number of the edge line segment containing
    /// it is returned (TileShape.java:234-255; Java's `-1` is `None`).
    pub fn contains_on_border_line_no(&self, point: &Point) -> Option<usize> {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return None;
        }
        let mut containing_line_no = None;
        for i in 0..line_count {
            let side_of = self.border_line_at(i).side_of(point);
            if side_of == Side::OnTheLeft {
                // point outside the convex shape
                return None;
            }
            if side_of == Side::Collinear {
                containing_line_no = Some(i);
            }
        }
        containing_line_no
    }

    /// Returns true, if `point` lies exactly on the boundary of the shape
    /// (TileShape.java:257-261).
    pub fn contains_on_border(&self, point: &Point) -> bool {
        self.contains_on_border_line_no(point).is_some()
    }

    /// Returns true, if this shape contains `other` completely. There may be some numerical
    /// inaccuracy (TileShape.java:263-274).
    pub fn contains_approx(&self, other: &TileShape) -> bool {
        for current_corner in other.corner_approx_arr() {
            if !self.contains_float(&current_corner) {
                return false;
            }
        }
        true
    }

    /// Returns the distance between `point` and its nearest point on the shape; 0 if `point` is
    /// contained in this shape (TileShape.java:276-284). `IntBox` overrides it with a closed form
    /// (IntBox.java:213-216).
    ///
    /// Java dereferences a possibly-`null` nearest point here; on the empty shape, where that
    /// throws in Java, this port returns `f64::MAX`.
    pub fn distance(&self, point: &FloatPoint) -> f64 {
        match self {
            TileShape::Box(b) => b.distance(point),
            _ => match self.nearest_point_approx(point) {
                Some(nearest_point) => nearest_point.distance(point),
                None => f64::MAX,
            },
        }
    }

    /// Returns the distance between `point` and its nearest point on the edge of the shape
    /// (TileShape.java:286-291). See [`TileShape::distance`] for the empty-shape totalization.
    pub fn border_distance(&self, point: &FloatPoint) -> f64 {
        match self.nearest_border_point_approx(point) {
            Some(nearest_point) => nearest_point.distance(point),
            None => f64::MAX,
        }
    }

    /// The smallest distance from the centre of gravity to the border of the shape
    /// (TileShape.java:293-296).
    pub fn smallest_radius(&self) -> f64 {
        self.border_distance(&self.centre_of_gravity())
    }

    /// Returns the point in this shape which has the smallest distance to `from_point`, or
    /// `from_point` itself if that is contained in this shape (TileShape.java:298-307). `None`
    /// only where Java returns `null`, i.e. for a shape without border lines.
    pub fn nearest_point(&self, from_point: &Point) -> Option<Point> {
        if !self.is_outside(from_point) {
            return Some(from_point.clone());
        }
        self.nearest_border_point(from_point)
    }

    /// Java `nearestPointApprox(FloatPoint)` (TileShape.java:309-315).
    pub fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        if self.contains_float(from_point) {
            return Some(*from_point);
        }
        self.nearest_border_point_approx(from_point)
    }

    /// Returns the nearest point to `from_point` on the edge of the shape
    /// (TileShape.java:317-362).
    pub fn nearest_border_point(&self, from_point: &Point) -> Option<Point> {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return None;
        }
        let from_point_f = from_point.to_float();
        if line_count == 1 {
            return Some(self.border_line_at(0).perpendicular_projection(from_point));
        }
        let mut min_dist = f64::MAX;
        let mut min_dist_ind = 0;

        // calculate the distance to the nearest corner first
        for i in 0..line_count {
            let current_corner_f = self.corner_approx_at(i);
            let current_distance = current_corner_f.distance_square(&from_point_f);
            if current_distance < min_dist {
                min_dist = current_distance;
                min_dist_ind = i;
            }
        }

        let mut nearest_point = self.corner(min_dist_ind);

        let mut prev_ind = line_count - 2;
        let mut current_ind = line_count - 1;

        for next_ind in 0..line_count {
            let projection = self
                .border_line_at(current_ind)
                .perpendicular_projection(from_point);
            if (!self.corner_is_bounded(current_ind)
                || self.border_line_at(prev_ind).side_of(&projection) == Side::OnTheRight)
                && (!self.corner_is_bounded(next_ind)
                    || self.border_line_at(next_ind).side_of(&projection) == Side::OnTheRight)
            {
                let projection_f = projection.to_float();
                let current_distance = projection_f.distance_square(&from_point_f);
                if current_distance < min_dist {
                    min_dist = current_distance;
                    nearest_point = projection;
                }
            }
            prev_ind = current_ind;
            current_ind = next_ind;
        }
        Some(nearest_point)
    }

    /// Returns an approximation of the nearest point to `from_point` on the border of this shape
    /// (TileShape.java:364-371).
    pub fn nearest_border_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        self.nearest_border_points_approx(from_point, 1)
            .first()
            .copied()
    }

    /// Returns an approximation of the `count` nearest points to `from_point` on the border of
    /// this shape. The result points must be located on different border lines and are sorted in
    /// ascending order, the nearest point first (TileShape.java:373-446).
    ///
    /// Java allocates `min(count, borderLineCount())` slots and leaves the unfilled tail `null`;
    /// the filled slots always form a prefix, and only that prefix is returned here.
    ///
    /// Java's insertion shift runs *upward* (`minDists[k] = minDists[k - 1]` for increasing `k`),
    /// which copies the displaced entry into every later slot instead of moving each entry down
    /// by one. That is kept verbatim — it is observable in the result for `count > 1`.
    pub fn nearest_border_points_approx(
        &self,
        from_point: &FloatPoint,
        count: usize,
    ) -> Vec<FloatPoint> {
        if count == 0 {
            return Vec::new();
        }
        let line_count = self.border_line_count();
        if line_count == 0 {
            return Vec::new();
        }
        if line_count == 1 {
            return vec![from_point.projection_approx(&self.border_line_at(0))];
        }
        if self.dimension() == 0 {
            return vec![self.corner_approx_at(0)];
        }
        let result_count = count.min(line_count);
        let mut nearest_points: Vec<Option<FloatPoint>> = vec![None; result_count];
        let mut min_dists = vec![f64::MAX; result_count];

        // calculate the distances to the nearest corners first
        for i in 0..line_count {
            if self.corner_is_bounded(i) {
                let current_corner = self.corner_approx_at(i);
                let current_distance = current_corner.distance_square(from_point);
                insert_sorted(
                    &mut min_dists,
                    &mut nearest_points,
                    current_distance,
                    current_corner,
                );
            }
        }

        let mut prev_ind = line_count - 2;
        let mut current_ind = line_count - 1;

        for next_ind in 0..line_count {
            let projection = from_point.projection_approx(&self.border_line_at(current_ind));
            if (!self.corner_is_bounded(current_ind)
                || self
                    .border_line_at(prev_ind)
                    .side_of_float_exact(&projection)
                    == Side::OnTheRight)
                && (!self.corner_is_bounded(next_ind)
                    || self
                        .border_line_at(next_ind)
                        .side_of_float_exact(&projection)
                        == Side::OnTheRight)
            {
                let current_distance = projection.distance_square(from_point);
                insert_sorted(
                    &mut min_dists,
                    &mut nearest_points,
                    current_distance,
                    projection,
                );
            }
            prev_ind = current_ind;
            current_ind = next_ind;
        }
        nearest_points.into_iter().flatten().collect()
    }

    /// Returns the number of the nearest corner of the shape to `from_point`
    /// (TileShape.java:448-462).
    ///
    /// Java initializes the running minimum with `Double.MIN_VALUE`, the smallest *positive*
    /// double, instead of `Double.MAX_VALUE`, so `currentDistance < minDist` can only fire for a
    /// distance of exactly 0. The method therefore returns 0 unless `from_point` coincides with a
    /// corner, in which case it returns that corner's index. That bug is ported verbatim.
    pub fn index_of_nearest_corner(&self, from_point: &Point) -> usize {
        let from_point_f = from_point.to_float();
        let mut result = 0;
        let corner_count = self.border_line_count();
        let mut min_dist = JAVA_DOUBLE_MIN_VALUE;
        for i in 0..corner_count {
            let current_distance = self.corner_approx_at(i).distance(&from_point_f);
            if current_distance < min_dist {
                min_dist = current_distance;
                result = i;
            }
        }
        result
    }

    /// Returns a line segment consisting of approximations of the corners with index 0 and
    /// `border_line_count() / 2` (TileShape.java:464-475). `None` for an empty shape, where Java
    /// returns `null`.
    pub fn diagonal_corner_segment(&self) -> Option<FloatLine> {
        if self.is_empty() {
            return None;
        }
        let first_corner = self.corner_approx_at(0);
        let last_corner = self.corner_approx_at(self.border_line_count() / 2);
        Some(FloatLine::new(first_corner, last_corner))
    }

    /// Returns an approximation of the `count` nearest relative outside locations of `shape` in
    /// the direction of different border lines of this shape, sorted in ascending order — the
    /// shortest first (TileShape.java:477-527).
    pub fn nearest_relative_outside_locations(
        &self,
        shape: &TileShape,
        count: usize,
    ) -> Vec<FloatPoint> {
        let line_count = self.border_line_count();
        if count == 0 || line_count < 3 || !self.intersects(shape) {
            return Vec::new();
        }

        let result_count = count.min(line_count);

        let mut translate_coors: Vec<Option<FloatPoint>> = vec![None; result_count];
        let mut min_dists = vec![f64::MAX; result_count];

        let mut current_ind = line_count - 1;

        let other_line_count = shape.border_line_count();

        for next_ind in 0..line_count {
            let mut current_max_dist = 0.0;
            let mut current_translate_coor = FloatPoint::ZERO;
            for corner_index in 0..other_line_count {
                let current_corner = shape.corner_approx_at(corner_index);
                if self
                    .border_line_at(current_ind)
                    .side_of_float_exact(&current_corner)
                    == Side::OnTheRight
                {
                    let projection =
                        current_corner.projection_approx(&self.border_line_at(current_ind));
                    let current_distance = projection.distance_square(&current_corner);
                    if current_distance > current_max_dist {
                        current_max_dist = current_distance;
                        current_translate_coor = projection.subtract(&current_corner);
                    }
                }
            }

            insert_sorted(
                &mut min_dists,
                &mut translate_coors,
                current_max_dist,
                current_translate_coor,
            );
            current_ind = next_ind;
        }
        translate_coors.into_iter().flatten().collect()
    }

    /// Shrinks this shape by `offset`; if the offset shape is empty, the intersection with the
    /// bounding box of the centre of gravity is returned instead (TileShape.java:529-537).
    pub fn shrink(&self, offset: f64) -> TileShape {
        let result = self.offset(-offset);
        if result.is_empty() {
            let centre_box = self.centre_of_gravity().bounding_box();
            return self.intersection(&TileShape::Box(centre_box));
        }
        result
    }

    /// Returns the maximum of the edge widths of the shape. Only defined when the shape is
    /// bounded (TileShape.java:539-567).
    pub fn length(&self) -> f64 {
        if !self.is_bounded() {
            return i32::MAX as f64;
        }
        let dimension = self.dimension();
        if dimension <= 0 {
            return 0.0;
        }
        if dimension == 1 {
            return self.circumference() / 2.0;
        }
        // now the shape is 2-dimensional
        let mut max_distance = -1.0;
        let mut max_distance2 = -1.0;
        let gravity_point = self.centre_of_gravity();
        for i in 0..self.border_line_count() {
            let current_distance = self.border_line_at(i).signed_distance(&gravity_point).abs();
            if current_distance > max_distance {
                max_distance2 = max_distance;
                max_distance = current_distance;
            } else if current_distance > max_distance2 {
                max_distance2 = current_distance;
            }
        }
        max_distance + max_distance2
    }

    /// Calculates whether this shape and `other` have a common border piece, and returns the
    /// indices in this shape and in `other` of the touching edge lines if so
    /// (TileShape.java:569-615). Java returns an `int[0]` when there is none; that is `None` here.
    ///
    /// Used when the intersection shape is 1-dimensional.
    pub fn touching_sides(&self, other: &TileShape) -> Option<[usize; 2]> {
        // search the first edge line of other with reverse direction >= right

        let mut side_no2 = 0;
        let mut dir2: Option<IntDirection> = None;
        for i in 0..other.border_line_count() {
            let current_direction = other.border_line_at(i).direction();
            if current_direction.compare_to(&IntDirection::LEFT) != std::cmp::Ordering::Less {
                side_no2 = i;
                dir2 = Some(current_direction.opposite());
                break;
            }
        }
        // Java logs "touching_side : dir2 not found" here (dropped: no logger in fr-geometry).
        let mut dir2 = dir2?;
        let mut side_no1 = 0;
        let mut dir1 = self.border_line_at(0).direction();
        let max_ind = self.border_line_count() + other.border_line_count();

        for _ in 0..max_ind {
            let compare = dir2.compare_to(&dir1);
            if compare == std::cmp::Ordering::Equal
                && self
                    .border_line_at(side_no1)
                    .is_equal_or_opposite(&other.border_line_at(side_no2))
            {
                return Some([side_no1, side_no2]);
            }
            if compare != std::cmp::Ordering::Less {
                // dir2 is bigger than dir1
                side_no1 = (side_no1 + 1) % self.border_line_count();
                dir1 = self.border_line_at(side_no1).direction();
            } else {
                // dir1 is bigger than dir2
                side_no2 = (side_no2 + 1) % other.border_line_count();
                dir2 = other.border_line_at(side_no2).direction().opposite();
            }
        }
        None
    }

    /// Calculates the minimal distance of `line` to this shape, assuming that `line` is on the
    /// left of this shape. Returns -1 if `line` is on the right of this shape or intersects with
    /// its interior (TileShape.java:617-638).
    pub fn distance_to_the_left(&self, line: &Line) -> f64 {
        let mut result = i32::MAX as f64;
        for i in 0..self.border_line_count() {
            let current_corner = self.corner_approx_at(i);
            let mut line_side = line.side_of_float(&current_corner, 1.0);
            if line_side == Side::Collinear {
                line_side = line.side_of(&self.corner(i));
            }
            if line_side == Side::OnTheRight {
                // currentPoint would be outside the result shape
                result = -1.0;
                break;
            }
            result = java_min(result, line.signed_distance(&current_corner));
        }
        result
    }

    /// Returns `Side::Collinear` if `line` intersects with the interior of this shape,
    /// `Side::OnTheLeft` if this shape is completely on the left of `line`, or
    /// `Side::OnTheRight` if it is completely on the right (TileShape.java:640-666).
    pub fn side_of_line(&self, line: &Line) -> Side {
        let mut on_the_left = false;
        let mut on_the_right = false;
        for i in 0..self.border_line_count() {
            let current_side = line.side_of(&self.corner(i));
            if current_side == Side::OnTheLeft {
                on_the_right = true;
            } else if current_side == Side::OnTheRight {
                on_the_left = true;
            }
            if on_the_left && on_the_right {
                return Side::Collinear;
            }
        }
        if on_the_left {
            Side::OnTheLeft
        } else {
            Side::OnTheRight
        }
    }

    /// Turns this shape by `factor` times 90 degree around `pole` (TileShape.java:668-675).
    /// `IntBox` overrides it to stay an `IntBox` (IntBox.java:409-420).
    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> TileShape {
        if let TileShape::Box(b) = self {
            return TileShape::Box(b.turn_90_degree(factor, pole));
        }
        let new_lines: Vec<Line> = (0..self.border_line_count())
            .map(|i| self.border_line_at(i).turn_90_degree(factor, pole))
            .collect();
        TileShape::get_instance_from_lines(new_lines)
    }

    // added in Task 16: rotateApprox(double angle, FloatPoint pole) (TileShape.java:677-702) — it
    // builds a `Polygon` from the rounded rotated corners and falls back to a `Polyline` /
    // `LineSegment` when the polygon degenerates, so it needs `LineSegment` (Task 15) plus
    // `Polygon` and `Polyline` (Task 16).

    /// Mirrors this shape at the vertical line through `pole` (TileShape.java:704-711).
    pub fn mirror_vertical(&self, pole: &IntPoint) -> TileShape {
        let new_lines: Vec<Line> = (0..self.border_line_count())
            .map(|i| self.border_line_at(i).mirror_vertical(pole))
            .collect();
        TileShape::get_instance_from_lines(new_lines)
    }

    /// Mirrors this shape at the horizontal line through `pole` (TileShape.java:713-720).
    pub fn mirror_horizontal(&self, pole: &IntPoint) -> TileShape {
        let new_lines: Vec<Line> = (0..self.border_line_count())
            .map(|i| self.border_line_at(i).mirror_horizontal(pole))
            .collect();
        TileShape::get_instance_from_lines(new_lines)
    }

    /// Calculates the border line of this shape intersecting the ray from `point` into `direction`
    /// (TileShape.java:722-753). `point` is assumed to be inside this shape, otherwise `None` is
    /// returned.
    ///
    /// Java builds the ray with `new Line(point, direction)`, which this port's `Line` can only
    /// represent for an `IntPoint` and an `IntDirection`; the other cases (where Java only warns
    /// and then misbehaves) also yield `None`.
    pub fn intersecting_border_line_no(
        &self,
        point: &Point,
        direction: &Direction,
    ) -> Option<usize> {
        if !self.contains(point) {
            return None;
        }
        let Point::Int(int_point) = point else {
            return None;
        };
        let from_point = point.to_float();
        let intersection_line = Line::from_direction_any(*int_point, direction)?;
        let second_line_point = intersection_line.b.to_float();
        let mut result = None;
        // Java initializes this with `Float.MAX_VALUE`, not `Double.MAX_VALUE`.
        let mut min_distance = f32::MAX as f64;
        for i in 0..self.border_line_count() {
            let current_border_line = self.border_line_at(i);
            let current_intersection = current_border_line.intersection_approx(&intersection_line);
            if current_intersection.x >= i32::MAX as f64 {
                continue; // lines are parallel
            }
            let current_distance = current_intersection.distance_square(&from_point);
            if current_distance < min_distance {
                let direction_ok = current_border_line.side_of_float_exact(&second_line_point)
                    == Side::OnTheLeft
                    || second_line_point.distance_square(&current_intersection) < current_distance;
                if direction_ok {
                    result = Some(i);
                    min_distance = current_distance;
                }
            }
        }
        result
    }

    /// Splits this shape into convex pieces; a tile shape is already convex
    /// (TileShape.java:889-894).
    pub fn split_to_convex(&self) -> Vec<TileShape> {
        vec![self.clone()]
    }

    /// Divides this shape into sections with width and height at most `max_section_width`, of
    /// about equal size (TileShape.java:896-920).
    ///
    /// `IntBox` overrides this with a covariant `IntBox[]` return (IntBox.java:645-683) that grids
    /// the box directly, without the `dimension() == 2` filter below, so a box dispatches there.
    pub fn divide_into_sections(&self, max_section_width: f64) -> Vec<TileShape> {
        if let TileShape::Box(b) = self {
            return b
                .divide_into_sections(max_section_width)
                .into_iter()
                .map(TileShape::Box)
                .collect();
        }
        if self.is_empty() {
            return vec![self.clone()];
        }
        let section_boxes = self.bounding_box().divide_into_sections(max_section_width);
        let mut section_list = Vec::new();
        for section_box in section_boxes {
            let current_section = self.intersection_with_simplify(&TileShape::Box(section_box));
            if current_section.dimension() == 2 {
                section_list.push(current_section);
            }
        }
        section_list
    }

    // added in Task 15: isIntersectedInteriorBy(LineSegment) (TileShape.java:922-926) — a
    // one-line forward to `is_intersected_interior_by_points`, waiting on `LineSegment`.

    /// Checks if the line segment defined by `start_point`, `end_point` and `line` has a common
    /// point with the interior of this shape (TileShape.java:928-1012).
    pub fn is_intersected_interior_by_points(
        &self,
        start_point: &Point,
        end_point: &Point,
        line: &Line,
    ) -> bool {
        let float_start_point = start_point.to_float();
        let float_end_point = end_point.to_float();

        let line_count = self.border_line_count();
        let mut border_line_side_of_start_point_arr: Vec<Side> = Vec::with_capacity(line_count);
        let mut border_line_side_of_end_point_arr: Vec<Side> = Vec::with_capacity(line_count);
        for i in 0..line_count {
            let current_border_line = self.border_line_at(i);
            let mut border_line_side_of_start_point =
                current_border_line.side_of_float(&float_start_point, 1.0);
            if border_line_side_of_start_point == Side::Collinear {
                border_line_side_of_start_point = current_border_line.side_of(start_point);
            }
            let mut border_line_side_of_end_point =
                current_border_line.side_of_float(&float_end_point, 1.0);
            if border_line_side_of_end_point == Side::Collinear {
                border_line_side_of_end_point = current_border_line.side_of(end_point);
            }
            if border_line_side_of_start_point != Side::OnTheRight
                && border_line_side_of_end_point != Side::OnTheRight
            {
                // both endpoints are outside the borderLine, no intersection possible
                return false;
            }
            border_line_side_of_start_point_arr.push(border_line_side_of_start_point);
            border_line_side_of_end_point_arr.push(border_line_side_of_end_point);
        }
        let start_point_is_inside = border_line_side_of_start_point_arr
            .iter()
            .all(|s| *s == Side::OnTheRight);
        if start_point_is_inside {
            return true;
        }
        let end_point_is_inside = border_line_side_of_end_point_arr
            .iter()
            .all(|s| *s == Side::OnTheRight);
        if end_point_is_inside {
            return true;
        }
        let segment_line = line;
        // Check, if this line segments intersect a border line of shape.
        for i in 0..line_count {
            let border_line_side_of_start_point = border_line_side_of_start_point_arr[i];
            let border_line_side_of_end_point = border_line_side_of_end_point_arr[i];
            if border_line_side_of_start_point != border_line_side_of_end_point {
                if border_line_side_of_start_point == Side::Collinear
                    && border_line_side_of_end_point == Side::OnTheLeft
                    || border_line_side_of_end_point == Side::Collinear
                        && border_line_side_of_start_point == Side::OnTheLeft
                {
                    // the interior of shape is not intersected.
                    continue;
                }
                let mut prev_corner_side =
                    segment_line.side_of_float(&self.corner_approx_at(i), 1.0);
                if prev_corner_side == Side::Collinear {
                    prev_corner_side = segment_line.side_of(&self.corner(i));
                }
                let next_corner_index = if i == line_count - 1 { 0 } else { i + 1 };
                let mut next_corner_side =
                    segment_line.side_of_float(&self.corner_approx_at(next_corner_index), 1.0);
                if next_corner_side == Side::Collinear {
                    next_corner_side = segment_line.side_of(&self.corner(next_corner_index));
                }
                if prev_corner_side == Side::OnTheLeft && next_corner_side == Side::OnTheRight
                    || prev_corner_side == Side::OnTheRight && next_corner_side == Side::OnTheLeft
                {
                    // this line segment crosses a border line of shape
                    return true;
                }
            }
        }
        false
    }

    /// Cuts `shape` out of this shape and divides the result into convex pieces
    /// (TileShape.java:755-756; IntBox.java:687-696, IntOctagon.java:1058-1062,
    /// Simplex.java:694-698).
    ///
    /// `IntBox.cutout` additionally simplifies every piece; the other two do not. Returns `None`
    /// where Java returns `null`, i.e. when `shape` is a simplex of dimension < 2.
    pub fn cutout(&self, shape: &TileShape) -> Option<Vec<TileShape>> {
        let pieces = shape.cutout_from(self)?;
        if matches!(self, TileShape::Box(_)) {
            return Some(pieces.iter().map(|p| p.simplify()).collect());
        }
        Some(pieces)
    }

    /// Cuts this shape out of `outer` (Java's auxiliary `cutoutFrom(IntBox|IntOctagon|Simplex)`,
    /// TileShape.java:1014-1021). `None` where Java returns `null`:
    /// `Simplex.cutoutFrom` refuses a `this` of dimension < 2 (Simplex.java:706-710).
    pub fn cutout_from(&self, outer: &TileShape) -> Option<Vec<TileShape>> {
        match (self, outer) {
            (TileShape::Box(a), TileShape::Box(d)) => {
                Some(a.cutout_from(d).into_iter().map(TileShape::Box).collect())
            }
            (TileShape::Box(a), TileShape::Octagon(d)) => Some(
                a.cutout_from_octagon(d)
                    .into_iter()
                    .map(TileShape::Octagon)
                    .collect(),
            ),
            (TileShape::Box(a), TileShape::Simplex(d)) => Some(
                a.cutout_from_simplex(d)?
                    .into_iter()
                    .map(TileShape::Simplex)
                    .collect(),
            ),
            (TileShape::Octagon(a), TileShape::Box(d)) => Some(
                a.cutout_from_box(d)
                    .into_iter()
                    .map(TileShape::Octagon)
                    .collect(),
            ),
            (TileShape::Octagon(a), TileShape::Octagon(d)) => Some(
                a.cutout_from_octagon(d)
                    .into_iter()
                    .map(TileShape::Octagon)
                    .collect(),
            ),
            (TileShape::Octagon(a), TileShape::Simplex(d)) => Some(
                a.cutout_from_simplex(d)?
                    .into_iter()
                    .map(TileShape::Simplex)
                    .collect(),
            ),
            (TileShape::Simplex(a), TileShape::Box(d)) => Some(
                a.cutout_from_box(d)?
                    .into_iter()
                    .map(TileShape::Simplex)
                    .collect(),
            ),
            (TileShape::Simplex(a), TileShape::Octagon(d)) => Some(
                a.cutout_from_octagon(d)?
                    .into_iter()
                    .map(TileShape::Simplex)
                    .collect(),
            ),
            (TileShape::Simplex(a), TileShape::Simplex(d)) => Some(
                a.cutout_from(d)?
                    .into_iter()
                    .map(TileShape::Simplex)
                    .collect(),
            ),
        }
    }

    // added in Task 16: cutout(Polyline) (TileShape.java:758-859) and entrancePoints(Polyline)
    // (TileShape.java:861-887) — both need `Polyline` (Task 16) and `LineSegment` (Task 15).
    // added in Task 17 (Circle / Shape / PolygonShape): intersects(Circle), the `Shape`-typed
    // intersects(Shape), boundingShape's `ConvexShape` overload, and the `ShapeOps` /
    // `PolylineShapeOps` traits. The latter also picks up the `PolylineShape` members that no
    // `TileShape.java` algorithm needs: boundedCorners(), equalsCorner(Point),
    // isContainedIn(IntBox), indexOfLeftMostCorner(FloatPoint),
    // indexOfRightMostCorner(FloatPoint), polarLineSegment(FloatPoint), prevNo(int), nextNo(int),
    // getBorder(), getHoles(), intersects(Line), leftMostCorner(Point), rightMostCorner(Point).
}

/// The insertion step shared by `nearestBorderPointsApprox` and
/// `nearestRelativeOutsideLocations` (TileShape.java:406-416, 430-440 and 513-523).
///
/// Java's inner shift loop runs upward — `values[k] = values[k - 1]` for `k` from `j + 1` — so it
/// copies the entry displaced at `j` into *every* later slot instead of moving each entry one slot
/// down. Kept verbatim: it changes which points come back for `count > 1`.
fn insert_sorted(
    min_dists: &mut [f64],
    values: &mut [Option<FloatPoint>],
    current_distance: f64,
    current_value: FloatPoint,
) {
    let result_count = min_dists.len();
    for j in 0..result_count {
        if current_distance < min_dists[j] {
            for k in (j + 1)..result_count {
                min_dists[k] = min_dists[k - 1];
                values[k] = values[k - 1];
            }
            min_dists[j] = current_distance;
            values[j] = Some(current_value);
            break;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The `TileShape`-typed methods left over on the three concrete shapes.
// ---------------------------------------------------------------------------------------------

impl IntBox {
    /// Java `IntBox.simplify()`: `return this;` (IntBox.java:118-121).
    pub fn simplify(&self) -> TileShape {
        TileShape::Box(*self)
    }

    /// Java `IntBox.boundingTile()`: `return this;` (IntBox.java:261-264).
    pub fn bounding_tile(&self) -> IntBox {
        *self
    }
}

impl IntOctagon {
    /// Java `IntOctagon.simplify()`: an octagon that is really a box becomes its bounding box
    /// (IntOctagon.java:1050-1057).
    pub fn simplify(&self) -> TileShape {
        if self.is_int_box() {
            return TileShape::Box(self.bounding_box());
        }
        TileShape::Octagon(*self)
    }

    /// Java `IntOctagon.boundingTile()`: `return this;` (IntOctagon.java:121-124).
    pub fn bounding_tile(&self) -> IntOctagon {
        *self
    }
}

impl Simplex {
    /// Java `Simplex.simplify()`: converts the physical instance of this shape to a simpler one,
    /// if possible — for example a `Simplex` to an `IntOctagon` (Simplex.java:55-70).
    pub fn simplify(&self) -> TileShape {
        if self.is_empty() {
            TileShape::Simplex(Simplex::EMPTY)
        } else if self.is_int_box() {
            TileShape::Box(self.bounding_box())
        } else if self.is_int_octagon() {
            match self.to_int_octagon() {
                Some(oct) => TileShape::Octagon(oct),
                // `isIntOctagon()` is exactly `toIntOctagon()`'s precondition, so this is
                // unreachable; Java would NPE on the `null`.
                None => TileShape::Simplex(self.clone()),
            }
        } else {
            TileShape::Simplex(self.clone())
        }
    }
}

impl Line {
    /// Looks if all interior points of `tile` are on the left side of this line
    /// (Line.java:165-174).
    pub fn is_on_the_left(&self, tile: &TileShape) -> bool {
        for i in 0..tile.border_line_count() {
            if self.side_of(&tile.corner(i)) == Side::OnTheRight {
                return false;
            }
        }
        true
    }

    /// Looks if all interior points of `tile` are on the right side of this line
    /// (Line.java:176-184; Java's doc comment says "left", the code tests the other side).
    pub fn is_on_the_right(&self, tile: &TileShape) -> bool {
        for i in 0..tile.border_line_count() {
            if self.side_of(&tile.corner(i)) == Side::OnTheLeft {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_point::FloatPoint;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::simplex::Simplex;

    fn bx() -> TileShape {
        TileShape::Box(IntBox::from_coords(0, 0, 10, 10))
    }

    fn tri() -> TileShape {
        TileShape::Simplex(Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
            IntPoint::new(0, 10),
        ]))
    }

    #[test]
    fn containment_family() {
        let b = bx();
        assert!(b.contains(&Point::Int(IntPoint::new(5, 5))));
        assert!(b.contains(&Point::Int(IntPoint::new(0, 5)))); // border counts as contained
        assert!(!b.contains_inside(&Point::Int(IntPoint::new(0, 5))));
        assert!(b.contains_on_border(&Point::Int(IntPoint::new(0, 5))));
        assert_eq!(
            b.contains_on_border_line_no(&Point::Int(IntPoint::new(5, 5))),
            None
        );
        assert!(b.is_outside(&Point::Int(IntPoint::new(11, 5))));
        assert!(b.contains_float(&FloatPoint::new(9.9, 0.1)));
        assert!(b.contains_tile(&TileShape::Box(IntBox::from_coords(2, 2, 3, 3))));
        assert!(!b.contains_tile(&TileShape::Box(IntBox::from_coords(2, 2, 30, 3))));
    }

    #[test]
    fn area_and_intersection_across_variants() {
        assert_eq!(bx().area(), 100.0);
        assert!((tri().area() - 50.0).abs() < 1e-9);
        let i = bx().intersection(&tri());
        assert!((i.area() - 50.0).abs() < 1e-9);
        let oct = TileShape::Octagon(IntBox::from_coords(5, 5, 20, 20).to_int_octagon());
        let j = bx().intersection(&oct);
        assert_eq!(j.area(), 25.0);
        assert!(bx().intersects(&oct));
        assert!(!tri().intersects(&TileShape::Box(IntBox::from_coords(8, 8, 9, 9))));
        // simplify folds a box-shaped simplex back into IntBox
        let s = TileShape::Simplex(IntBox::from_coords(0, 0, 10, 10).to_simplex());
        assert!(matches!(s.simplify(), TileShape::Box(_)));
        assert!(matches!(
            bx().intersection_with_simplify(&oct),
            TileShape::Box(_)
        ));
    }

    #[test]
    fn distances_and_nearest() {
        let b = bx();
        assert_eq!(b.distance(&FloatPoint::new(13.0, 14.0)), 5.0);
        assert_eq!(b.distance(&FloatPoint::new(5.0, 5.0)), 0.0);
        assert_eq!(b.border_distance(&FloatPoint::new(5.0, 5.0)), 5.0);
        assert_eq!(b.smallest_radius(), 5.0);
        assert_eq!(
            b.nearest_point(&Point::Int(IntPoint::new(-4, 5))),
            Some(Point::Int(IntPoint::new(0, 5)))
        );
        assert_eq!(
            b.nearest_border_point(&Point::Int(IntPoint::new(1, 5))),
            Some(Point::Int(IntPoint::new(0, 5)))
        );
        assert_eq!(b.max_width(), 10.0);
    }

    #[test]
    fn touching_sides_and_side_of_line() {
        let a = bx();
        let b = TileShape::Box(IntBox::from_coords(10, 0, 20, 10));
        let ts = a.touching_sides(&b).expect("boxes share an edge");
        assert_eq!(a.border_line(ts[0]).unwrap().a.x, 10);
        assert_eq!(b.border_line(ts[1]).unwrap().a.x, 10);
        assert!(
            a.touching_sides(&TileShape::Box(IntBox::from_coords(30, 0, 40, 10)))
                .is_none()
        );
        let far = crate::line::Line::from_coords(50, 0, 50, 1);
        assert_ne!(a.side_of_line(&far), crate::Side::Collinear);
        assert!(far.is_on_the_right(&a) || far.is_on_the_left(&a));
    }

    #[test]
    fn transformations() {
        let t = tri().turn_90_degree(1, &IntPoint::new(0, 0));
        assert_eq!(t.bounding_box(), IntBox::from_coords(-10, 0, 0, 10));
        let m = tri().mirror_vertical(&IntPoint::new(0, 0));
        assert_eq!(m.bounding_box(), IntBox::from_coords(-10, 0, 0, 10));
        let s = bx().shrink(2.0);
        assert_eq!(s.bounding_box(), IntBox::from_coords(2, 2, 8, 8));
        let parts = bx().divide_into_sections(4.0);
        assert!(parts.len() >= 4);
        assert!((parts.iter().map(|p| p.area()).sum::<f64>() - 100.0).abs() < 1e-9);
    }

    /// The generic `TileShape.divideIntoSections` grids the bounding box and keeps only the
    /// 2-dimensional intersections; the fourth section of the triangle degenerates to the single
    /// point (5, 5) and is dropped.
    #[test]
    fn divide_into_sections_drops_degenerate_pieces() {
        let parts = tri().divide_into_sections(6.0);
        assert_eq!(parts.len(), 3);
        assert!((parts.iter().map(|p| p.area()).sum::<f64>() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn cutout_from_tile() {
        let outer = TileShape::Box(IntBox::from_coords(0, 0, 20, 20));
        let pieces = tri()
            .cutout_from(&outer)
            .expect("the triangle is 2-dimensional");
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert!((total - (400.0 - 50.0)).abs() < 1e-6);
    }
    #[test]
    fn static_factories() {
        let square = TileShape::get_instance_from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
            IntPoint::new(10, 10),
            IntPoint::new(0, 10),
        ]);
        assert_eq!(square, bx()); // simplify() folds the simplex back into an IntBox
        assert_eq!(
            TileShape::get_instance_from_box(0, 0, 10, 10),
            IntBox::from_coords(0, 0, 10, 10).to_int_octagon()
        );
        assert_eq!(
            TileShape::get_instance_octagon(0, 0, 10, 10, -10, 10, 0, 20),
            IntOctagon::new(0, 0, 10, 10, -10, 10, 0, 20).normalize()
        );
        assert_eq!(
            TileShape::get_instance_from_point(&Point::Int(IntPoint::new(3, 5))),
            IntPoint::new(3, 5).surrounding_box()
        );
        // a half-plane keeps its single line and is unbounded (Java skips simplify() here)
        let half_plane = TileShape::get_instance_from_line(Line::from_coords(0, 0, 0, 1));
        assert_eq!(half_plane.border_line_count(), 1);
        assert!(!half_plane.is_bounded());
        assert_eq!(half_plane.area(), f64::MAX);
    }

    #[test]
    fn representation_dispatch() {
        let oct = TileShape::Octagon(IntBox::from_coords(0, 0, 10, 10).to_int_octagon());
        assert_eq!(bx().border_line_count(), 4);
        assert_eq!(oct.border_line_count(), 8);
        assert_eq!(tri().border_line_count(), 3);
        assert!(bx().is_int_box() && bx().is_int_octagon());
        assert!(oct.is_int_box());
        assert!(!tri().is_int_box()); // the hypotenuse is not orthogonal
        assert!(tri().is_int_octagon()); // but every edge is a multiple of 45 degree
        // an octagon that really is a box simplifies to one
        assert_eq!(oct.simplify(), bx());
        assert_eq!(
            bx().to_simplex(),
            IntBox::from_coords(0, 0, 10, 10).to_simplex()
        );
        assert_eq!(bx().bounding_tile(), bx());
        assert_eq!(bx().get_id(), IntBox::from_coords(0, 0, 10, 10).get_id());
        assert_eq!(
            bx().bounding_octagon(),
            Some(IntBox::from_coords(0, 0, 10, 10).to_int_octagon())
        );
        assert_eq!(bx().dimension(), 2);
        assert_eq!(tri().dimension(), 2);
        assert!(bx().corner_is_bounded(0) && tri().corner_is_bounded(0));
        assert_eq!(bx().corner(2), Point::Int(IntPoint::new(10, 10)));
        assert_eq!(bx().corner_approx_arr().len(), 4);
        // borderLineIndex is a Java stub for IntBox/IntOctagon and real only for Simplex
        let tri_line = tri().border_line(0).unwrap();
        assert_eq!(tri().border_line_index(&tri_line), Some(0));
        assert_eq!(bx().border_line_index(&bx().border_line(0).unwrap()), None);
        assert_eq!(oct.border_line_index(&oct.border_line(0).unwrap()), None);
    }

    #[test]
    fn offsets_translation_and_widths() {
        let v = Vector::Int(crate::int_vector::IntVector::new(3, 4));
        assert_eq!(
            bx().translate_by(&v),
            TileShape::Box(IntBox::from_coords(3, 4, 13, 14))
        );
        assert_eq!(
            bx().offset(2.0),
            TileShape::Box(IntBox::from_coords(-2, -2, 12, 12))
        );
        // enlarging an IntBox widens it to an IntOctagon (Java IntBox.enlarge)
        assert!(matches!(bx().enlarge(2.0), TileShape::Octagon(_)));
        assert_eq!(bx().min_width(), 10.0);
        assert_eq!(bx().circumference(), 40.0);
        // the generic PolylineShape.circumference for a simplex: 10 + 10 + sqrt(200)
        assert!((tri().circumference() - (20.0 + 200.0_f64.sqrt())).abs() < 1e-9);
        assert_eq!(bx().centre_of_gravity(), FloatPoint::new(5.0, 5.0));
        assert_eq!(bx().length(), 10.0);
        assert_eq!(
            bx().diagonal_corner_segment(),
            Some(crate::float_line::FloatLine::new(
                FloatPoint::new(0.0, 0.0),
                FloatPoint::new(10.0, 10.0)
            ))
        );
        assert_eq!(bx().split_to_convex(), vec![bx()]);
        let m = tri().mirror_horizontal(&IntPoint::new(0, 0));
        assert_eq!(m.bounding_box(), IntBox::from_coords(0, -10, 10, 0));
    }

    /// Java initializes the running minimum of `indexOfNearestCorner` with `Double.MIN_VALUE`
    /// (the smallest *positive* double) instead of `Double.MAX_VALUE`, so the comparison only
    /// fires at distance 0: for any point that is not itself a corner the answer is 0, however
    /// near corner 2 is (TileShape.java:448-462).
    #[test]
    fn index_of_nearest_corner_only_moves_off_zero_at_distance_zero() {
        assert_eq!(
            bx().index_of_nearest_corner(&Point::Int(IntPoint::new(9, 9))),
            0
        );
        assert_eq!(
            bx().index_of_nearest_corner(&Point::Int(IntPoint::new(10, 10))),
            2
        );
    }

    #[test]
    fn side_of_border_and_contains_approx() {
        let b = bx();
        assert_eq!(
            b.side_of_border(&FloatPoint::new(5.0, 5.0), 0.0),
            crate::Side::OnTheRight
        );
        assert_eq!(
            b.side_of_border(&FloatPoint::new(0.0, 5.0), 0.0),
            crate::Side::Collinear
        );
        assert_eq!(
            b.side_of_border(&FloatPoint::new(20.0, 5.0), 0.0),
            crate::Side::OnTheLeft
        );
        // contains(FloatPoint) is the *strict* test for a box and a simplex ...
        assert!(b.contains_approx(&TileShape::Box(IntBox::from_coords(2, 2, 3, 3))));
        assert!(!b.contains_approx(&b));
        assert!(!b.contains_float(&FloatPoint::new(0.0, 5.0)));
        assert!(!b.contains_float_tol(&FloatPoint::new(0.0, 5.0), 0.0));
        // ... but IntOctagon overrides it with an inclusive coordinate test that accepts the
        // border (IntOctagon.java:327-343).
        let oct = TileShape::Octagon(IntBox::from_coords(0, 0, 10, 10).to_int_octagon());
        assert!(oct.contains_float(&FloatPoint::new(0.0, 5.0)));
        assert!(!oct.contains_float_tol(&FloatPoint::new(0.0, 5.0), 0.0));
    }

    #[test]
    fn distance_to_the_left_and_ray_intersection() {
        let b = bx();
        // the upward line x = -5 has the whole box on its right, at distance 5
        assert_eq!(
            b.distance_to_the_left(&Line::from_coords(-5, 0, -5, 1)),
            5.0
        );
        // the upward line x = 15 has the box on its left, so the result is the -1 sentinel
        assert_eq!(
            b.distance_to_the_left(&Line::from_coords(15, 0, 15, 1)),
            -1.0
        );
        // the ray from the centre to the right leaves through border line 1 (x = 10)
        assert_eq!(
            b.intersecting_border_line_no(
                &Point::Int(IntPoint::new(5, 5)),
                &Direction::Int(IntDirection::RIGHT)
            ),
            Some(1)
        );
        // a point outside gives None (Java: -1)
        assert_eq!(
            b.intersecting_border_line_no(
                &Point::Int(IntPoint::new(50, 5)),
                &Direction::Int(IntDirection::RIGHT)
            ),
            None
        );
    }

    /// The one production-reachable NaN site in this module. A degenerate `Line` (`a == b`) makes
    /// `Line::signed_distance` compute `det / length` = `0.0 / 0.0` = NaN, and Java's
    /// `Math.min(result, line.signedDistance(currentCorner))` (TileShape.java:635) *propagates*
    /// that NaN into the result. Rust's `f64::min` would absorb it instead and leave the
    /// `Integer.MAX_VALUE` seed, so `distance_to_the_left` goes through `limits::java_min`.
    ///
    /// The degenerate line reaches the `java_min` call because every corner tests `Collinear`:
    /// `side_of_float` sees `det == 0`, and the exact `side_of` fallback sees `0` as well, so the
    /// `OnTheRight` early-out with its `-1.0` sentinel never fires.
    #[test]
    fn distance_to_the_left_propagates_nan_for_degenerate_line() {
        let degenerate = Line::from_coords(5, 5, 5, 5);
        assert!(
            degenerate
                .signed_distance(&FloatPoint::new(0.0, 0.0))
                .is_nan()
        );
        assert!(bx().distance_to_the_left(&degenerate).is_nan());
        assert!(tri().distance_to_the_left(&degenerate).is_nan());
    }

    #[test]
    fn is_intersected_interior_by_points_crossings() {
        let b = bx();
        let start = Point::Int(IntPoint::new(-5, 5));
        let end = Point::Int(IntPoint::new(15, 5));
        let line = Line::from_coords(-5, 5, 15, 5);
        assert!(b.is_intersected_interior_by_points(&start, &end, &line));
        // a segment well above the box touches nothing
        let start = Point::Int(IntPoint::new(-5, 20));
        let end = Point::Int(IntPoint::new(15, 20));
        let line = Line::from_coords(-5, 20, 15, 20);
        assert!(!b.is_intersected_interior_by_points(&start, &end, &line));
        // a segment running along the lower border only touches the border, not the interior
        let start = Point::Int(IntPoint::new(-5, 0));
        let end = Point::Int(IntPoint::new(15, 0));
        let line = Line::from_coords(-5, 0, 15, 0);
        assert!(!b.is_intersected_interior_by_points(&start, &end, &line));
    }

    #[test]
    fn nearest_border_points_and_relative_outside_locations() {
        let b = bx();
        assert_eq!(
            b.nearest_border_points_approx(&FloatPoint::new(5.0, 5.0), 2),
            vec![FloatPoint::new(0.0, 5.0), FloatPoint::new(5.0, 0.0)]
        );
        assert!(
            b.nearest_border_points_approx(&FloatPoint::new(5.0, 5.0), 0)
                .is_empty()
        );
        // a half-plane has a single border line, so the projection is the only answer
        let half_plane = TileShape::get_instance_from_line(Line::from_coords(0, 0, 0, 1));
        assert_eq!(
            half_plane.nearest_border_point_approx(&FloatPoint::new(4.0, 7.0)),
            Some(FloatPoint::new(0.0, 7.0))
        );
        assert_eq!(
            half_plane.nearest_border_point(&Point::Int(IntPoint::new(4, 7))),
            Some(Point::Int(IntPoint::new(0, 7)))
        );
        // the shortest way out of the overlap with box (8,8)-(12,12) is 2 units to the right
        assert_eq!(
            b.nearest_relative_outside_locations(
                &TileShape::Box(IntBox::from_coords(8, 8, 12, 12)),
                1
            ),
            vec![FloatPoint::new(2.0, 0.0)]
        );
        // no overlap, no locations
        assert!(
            b.nearest_relative_outside_locations(
                &TileShape::Box(IntBox::from_coords(80, 80, 120, 120)),
                1
            )
            .is_empty()
        );
    }

    #[test]
    fn cutout_simplifies_only_for_a_box_receiver() {
        let outer = TileShape::Box(IntBox::from_coords(0, 0, 20, 20));
        let pieces = outer.cutout(&tri()).expect("the triangle is 2-dimensional");
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert!((total - 350.0).abs() < 1e-6);
        // cutting a box out of a box stays in IntBox land
        let inner = TileShape::Box(IntBox::from_coords(5, 5, 15, 15));
        let pieces = outer.cutout(&inner).expect("boxes always cut out");
        assert!(pieces.iter().all(|p| matches!(p, TileShape::Box(_))));
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert!((total - 300.0).abs() < 1e-9);
        // Java returns null when the shape to cut out is a simplex of dimension < 2
        let degenerate = TileShape::Simplex(Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
        ]));
        assert_eq!(degenerate.cutout_from(&outer), None);
    }

    #[test]
    fn empty_shape_behaviour() {
        let e = TileShape::Simplex(Simplex::EMPTY);
        assert!(e.is_empty());
        assert_eq!(e.border_line_count(), 0);
        assert_eq!(e.border_line(0), None);
        assert_eq!(e.corner_approx(0), None);
        assert!(e.is_outside(&Point::Int(IntPoint::new(0, 0))));
        assert!(!e.contains(&Point::Int(IntPoint::new(0, 0))));
        assert!(!e.contains_inside(&Point::Int(IntPoint::new(0, 0))));
        assert!(!e.contains_float(&FloatPoint::ZERO));
        assert_eq!(
            e.side_of_border(&FloatPoint::ZERO, 0.0),
            crate::Side::Collinear
        );
        assert_eq!(
            e.contains_on_border_line_no(&Point::Int(IntPoint::ZERO)),
            None
        );
        assert_eq!(e.nearest_point(&Point::Int(IntPoint::ZERO)), None);
        assert_eq!(e.nearest_border_point(&Point::Int(IntPoint::ZERO)), None);
        assert_eq!(e.nearest_border_point_approx(&FloatPoint::ZERO), None);
        assert!(
            e.nearest_border_points_approx(&FloatPoint::ZERO, 3)
                .is_empty()
        );
        assert_eq!(e.diagonal_corner_segment(), None);
        assert_eq!(e.area(), 0.0); // bounded, but dimension() == -1 < 2
        assert_eq!(e.length(), 0.0);
        // Java's PolylineShape.circumference assigns `prevCorner = cornerApprox(-1)`, which
        // `Simplex.cornerApprox` answers with null for 0 lines (Simplex.java:172-175); the loop
        // then runs zero times and 0 is returned. It does not throw.
        assert_eq!(e.circumference(), 0.0);
        assert_eq!(e.divide_into_sections(4.0), vec![e.clone()]);
        // Java dereferences a null nearest point here; this port totalizes to f64::MAX.
        assert_eq!(e.distance(&FloatPoint::ZERO), f64::MAX);
        assert_eq!(e.border_distance(&FloatPoint::ZERO), f64::MAX);
        assert_eq!(e.smallest_radius(), f64::MAX);
    }
}
