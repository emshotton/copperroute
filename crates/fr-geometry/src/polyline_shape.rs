//! Port of `app.freerouting.geometry.planar.PolylineShape` — "Abstract class with functions for
//! shapes whose borders consist of straight lines" (PolylineShape.java:9-10).
//!
//! Java's abstract class collapses into the [`PolylineShapeOps`] trait: the `abstract` members
//! become required methods, the concrete ones become provided methods with the Java body
//! transcribed verbatim. It is implemented for [`TileShape`] (and therefore for `IntBox`,
//! `IntOctagon` and `Simplex` through it) and for [`crate::polygon_shape::PolygonShape`], which
//! are Java's only two `PolylineShape` subclasses.
//!
//! Java's `IntBox` overrides `circumference()` and `Simplex` overrides `cornerApprox` /
//! `cornerApproxArr`; both overrides already live on the `TileShape` enum, so the
//! `impl PolylineShapeOps for TileShape` simply forwards to the inherent methods rather than
//! taking the trait defaults.

use crate::float_line::FloatLine;
use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::line::Line;
use crate::point::Point;
use crate::polygon_shape::PolygonShape;
use crate::side::Side;
use crate::tile_shape::TileShape;
use crate::vector::Vector;

/// The functionality of Java's abstract `PolylineShape`.
///
/// Object safe: [`crate::line_segment::LineSegment::from_polyline_shape`] takes
/// `&dyn PolylineShapeOps`, mirroring Java's `LineSegment(PolylineShape, int)`.
pub trait PolylineShapeOps {
    // -- the `abstract` members of PolylineShape.java, plus the `Shape`/`Area` members that its
    //    concrete methods below call.

    /// Returns true if the shape has no infinite part at this corner (PolylineShape.java:12-13).
    fn corner_is_bounded(&self, no: usize) -> bool;

    /// Returns the number of border lines of the shape (PolylineShape.java:15-16).
    fn border_line_count(&self) -> usize;

    /// Returns the `no`-th corner of this shape for `no` between 0 and `border_line_count() - 1`.
    /// The corners are sorted starting with the smallest y-coordinate in counterclock sense
    /// around the shape. If there are several corners with the smallest y-coordinate, the corner
    /// with the smallest x-coordinate comes first. Consecutive corners may be equal
    /// (PolylineShape.java:18-24).
    fn corner(&self, no: usize) -> Point;

    /// Returns the `no`-th border line of this shape (PolylineShape.java:202-203).
    ///
    /// `Option` because both implementations answer `null` for an out-of-range index
    /// (`PolygonShape.borderLine`, PolygonShape.java:456-459) resp. for the empty simplex.
    fn border_line(&self, no: usize) -> Option<Line>;

    /// Returns true if the shape is contained in a sufficiently large box (`Area.isBounded`).
    fn is_bounded(&self) -> bool;

    /// Returns true if the area is empty (`Area.isEmpty`).
    fn is_empty(&self) -> bool;

    /// 2 for two-dimensional shapes, 1 for curves, 0 for a point and -1 for the empty shape
    /// (`Area.dimension`).
    fn dimension(&self) -> i32;

    /// The smallest surrounding box of the shape (`Area.boundingBox`).
    fn bounding_box(&self) -> IntBox;

    // -- the concrete members of PolylineShape.java.

    /// Return all bounded corners of this shape (PolylineShape.java:46-61). Java's doc comment
    /// says "unbounded"; the body keeps the corners for which `cornerIsBounded` holds.
    fn bounded_corners(&self) -> Vec<Point> {
        let corner_count = self.border_line_count();
        let mut result = Vec::new();
        for i in 0..corner_count {
            if self.corner_is_bounded(i) {
                result.push(self.corner(i));
            }
        }
        result
    }

    /// An approximation of the `no`-th corner of this shape. If the shape is not bounded at this
    /// corner, the coordinates of the result are `i32::MAX` (PolylineShape.java:63-70).
    ///
    /// `Option` because `Simplex.cornerApprox` answers `null` for the line-less simplex.
    fn corner_approx(&self, no: usize) -> Option<FloatPoint> {
        Some(self.corner(no).to_float())
    }

    /// An approximation of all corners of this shape (PolylineShape.java:72-84).
    fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        (0..self.border_line_count())
            .map(|i| self.corner_approx_at(i))
            .collect()
    }

    /// If `point` is equal to a corner of this shape, the number of that corner is returned;
    /// `None` (Java: -1) otherwise (PolylineShape.java:86-97).
    fn equals_corner(&self, point: &Point) -> Option<usize> {
        (0..self.border_line_count()).find(|&i| *point == self.corner(i))
    }

    /// The cumulative border line length of the shape. If the shape is unbounded, `i32::MAX` is
    /// returned (PolylineShape.java:99-117).
    fn circumference(&self) -> f64 {
        if !self.is_bounded() {
            return i32::MAX as f64;
        }
        let corner_count = self.border_line_count();
        if corner_count == 0 {
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

    /// The arithmetic middle of the corners of this shape (PolylineShape.java:119-133).
    fn centre_of_gravity(&self) -> FloatPoint {
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

    /// Checks if this shape is completely contained in `b` (PolylineShape.java:135-139).
    fn is_contained_in(&self, b: &IntBox) -> bool {
        b.contains(&self.bounding_box())
    }

    /// The index of the corner of the shape, so that all other points of the shape are to the
    /// right of the line from `from_point` to this corner (PolylineShape.java:141-157).
    fn index_of_left_most_corner(&self, from_point: &FloatPoint) -> usize {
        let corner_count = self.border_line_count();
        if corner_count == 0 {
            // Java leaves `leftMostCorner` at the `null` of `cornerApprox(0)` and returns 0.
            return 0;
        }
        let mut left_most_corner = self.corner_approx_at(0);
        let mut result = 0;
        for i in 1..corner_count {
            let current_corner = self.corner_approx_at(i);
            if current_corner.side_of(from_point, &left_most_corner) == Side::OnTheLeft {
                left_most_corner = current_corner;
                result = i;
            }
        }
        result
    }

    /// The index of the corner of the shape, so that all other points of the shape are to the
    /// left of the line from `from_point` to this corner (PolylineShape.java:159-175).
    fn index_of_right_most_corner(&self, from_point: &FloatPoint) -> usize {
        let corner_count = self.border_line_count();
        if corner_count == 0 {
            return 0;
        }
        let mut right_most_corner = self.corner_approx_at(0);
        let mut result = 0;
        for i in 1..corner_count {
            let current_corner = self.corner_approx_at(i);
            if current_corner.side_of(from_point, &right_most_corner) == Side::OnTheRight {
                right_most_corner = current_corner;
                result = i;
            }
        }
        result
    }

    /// A [`FloatLine`] whose `a` is an approximation of the left most corner of this shape when
    /// viewed from `from_point`, and whose `b` is an approximation of the right most corner
    /// (PolylineShape.java:177-200). `None` where Java warns and returns `null`, i.e. for an
    /// empty shape.
    fn polar_line_segment(&self, from_point: &FloatPoint) -> Option<FloatLine> {
        if self.is_empty() {
            // Java: FRLogger.warn("PolylineShape.polarLineSegment: shape is empty")
            return None;
        }
        let mut left_most_corner = self.corner_approx_at(0);
        let mut right_most_corner = left_most_corner;
        let corner_count = self.border_line_count();
        for i in 1..corner_count {
            let current_corner = self.corner_approx_at(i);
            if current_corner.side_of(from_point, &right_most_corner) == Side::OnTheRight {
                right_most_corner = current_corner;
            }
            if current_corner.side_of(from_point, &left_most_corner) == Side::OnTheLeft {
                left_most_corner = current_corner;
            }
        }
        Some(FloatLine::new(left_most_corner, right_most_corner))
    }

    /// The previous border line or corner number of this shape (PolylineShape.java:205-214).
    ///
    /// # Panics
    /// For `no == 0` on a shape with no border lines, where Java hands back -1 and every caller
    /// then indexes out of range.
    fn prev_no(&self, no: usize) -> usize {
        if no == 0 {
            self.border_line_count() - 1
        } else {
            no - 1
        }
    }

    /// The next border line or corner number of this shape (PolylineShape.java:216-219).
    ///
    /// # Panics
    /// On a shape with no border lines, where Java throws `ArithmeticException: / by zero`.
    fn next_no(&self, no: usize) -> usize {
        (no + 1) % self.border_line_count()
    }

    /// Checks if this shape and `line` have a common point (PolylineShape.java:231-243).
    fn intersects_line(&self, line: &Line) -> bool {
        let side_of_first_corner = line.side_of(&self.corner(0));
        if side_of_first_corner == Side::Collinear {
            return true;
        }
        for i in 1..self.border_line_count() {
            if line.side_of(&self.corner(i)) != side_of_first_corner {
                return true;
            }
        }
        false
    }

    /// Calculates the left most corner of this shape, when looked at from `from_point`
    /// (PolylineShape.java:245-259).
    fn left_most_corner(&self, from_point: &Point) -> Point {
        if self.is_empty() {
            return from_point.clone();
        }
        let mut result = self.corner(0);
        let corner_count = self.border_line_count();
        for i in 1..corner_count {
            let current_corner = self.corner(i);
            if current_corner.side_of(from_point, &result) == Side::OnTheLeft {
                result = current_corner;
            }
        }
        result
    }

    /// Calculates the right most corner of this shape, when looked at from `from_point`
    /// (PolylineShape.java:261-275; Java's doc comment repeats "left most").
    fn right_most_corner(&self, from_point: &Point) -> Point {
        if self.is_empty() {
            return from_point.clone();
        }
        let mut result = self.corner(0);
        let corner_count = self.border_line_count();
        for i in 1..corner_count {
            let current_corner = self.corner(i);
            if current_corner.side_of(from_point, &result) == Side::OnTheRight {
                result = current_corner;
            }
        }
        result
    }

    /// The `no`-th corner approximation, for an index that is known to be in range. Java would
    /// hand a `null` to the arithmetic that follows.
    fn corner_approx_at(&self, no: usize) -> FloatPoint {
        self.corner_approx(no)
            .expect("corner index is below border_line_count()")
    }
}

impl PolylineShapeOps for TileShape {
    fn corner_is_bounded(&self, no: usize) -> bool {
        TileShape::corner_is_bounded(self, no)
    }
    fn border_line_count(&self) -> usize {
        TileShape::border_line_count(self)
    }
    fn corner(&self, no: usize) -> Point {
        TileShape::corner(self, no)
    }
    fn border_line(&self, no: usize) -> Option<Line> {
        TileShape::border_line(self, no)
    }
    fn is_bounded(&self) -> bool {
        TileShape::is_bounded(self)
    }
    fn is_empty(&self) -> bool {
        TileShape::is_empty(self)
    }
    fn dimension(&self) -> i32 {
        TileShape::dimension(self)
    }
    fn bounding_box(&self) -> IntBox {
        TileShape::bounding_box(self)
    }
    // `Simplex` overrides `cornerApprox` / `cornerApproxArr` and `IntBox` overrides
    // `circumference`; all three overrides already live on the `TileShape` enum.
    fn corner_approx(&self, no: usize) -> Option<FloatPoint> {
        TileShape::corner_approx(self, no)
    }
    fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        TileShape::corner_approx_arr(self)
    }
    fn circumference(&self) -> f64 {
        TileShape::circumference(self)
    }
    fn centre_of_gravity(&self) -> FloatPoint {
        TileShape::centre_of_gravity(self)
    }
}

/// Java's `PolylineShape`-typed reference, as it is stored by
/// [`crate::polyline_area::PolylineArea`] for its border and its holes.
///
/// A two-variant enum rather than a plain `PolygonShape` because the Java tree really does
/// construct both: `DrillPage.java:102` and `BoardOutline.java:186` pass an `IntBox` border
/// (`DrillPage` also passes `TileShape[]` holes), while `io/specctra/parser/Shape.java:552,591`
/// and `HoleConstructionState.java:127` pass `PolygonShape`s.
#[derive(Debug, Clone, PartialEq)]
pub enum PolylineShapeRef {
    /// A convex tile: `IntBox`, `IntOctagon` or `Simplex`.
    Tile(TileShape),
    /// A closed polygon of corner points.
    Polygon(PolygonShape),
}

impl From<TileShape> for PolylineShapeRef {
    fn from(value: TileShape) -> Self {
        PolylineShapeRef::Tile(value)
    }
}

impl From<IntBox> for PolylineShapeRef {
    fn from(value: IntBox) -> Self {
        PolylineShapeRef::Tile(TileShape::Box(value))
    }
}

impl From<IntOctagon> for PolylineShapeRef {
    fn from(value: IntOctagon) -> Self {
        PolylineShapeRef::Tile(TileShape::Octagon(value))
    }
}

impl From<PolygonShape> for PolylineShapeRef {
    fn from(value: PolygonShape) -> Self {
        PolylineShapeRef::Polygon(value)
    }
}

impl PolylineShapeRef {
    /// `PolylineShape.contains(FloatPoint)` (`Area.contains(FloatPoint)`).
    pub fn contains_float(&self, point: &FloatPoint) -> bool {
        match self {
            PolylineShapeRef::Tile(t) => t.contains_float(point),
            PolylineShapeRef::Polygon(p) => p.contains_float(point),
        }
    }

    /// `PolylineShape.contains(Point)` (`Area.contains(Point)`).
    pub fn contains(&self, point: &Point) -> bool {
        match self {
            PolylineShapeRef::Tile(t) => t.contains(point),
            PolylineShapeRef::Polygon(p) => p.contains(point),
        }
    }

    /// `Shape.containsInside(Point)`.
    pub fn contains_inside(&self, point: &Point) -> bool {
        match self {
            PolylineShapeRef::Tile(t) => t.contains_inside(point),
            PolylineShapeRef::Polygon(p) => p.contains_inside(point),
        }
    }

    /// `Area.boundingOctagon()`; `None` for an unbounded simplex, where Java returns `null`.
    pub fn bounding_octagon(&self) -> Option<IntOctagon> {
        match self {
            PolylineShapeRef::Tile(t) => t.bounding_octagon(),
            PolylineShapeRef::Polygon(p) => Some(p.bounding_octagon()),
        }
    }

    /// `Area.splitToConvex()`; `None` where Java returns `null` (a self-intersecting polygon or
    /// a `Simplex` of dimension < 2).
    pub fn split_to_convex(&self) -> Option<Vec<TileShape>> {
        match self {
            PolylineShapeRef::Tile(t) => Some(t.split_to_convex()),
            PolylineShapeRef::Polygon(p) => p.split_to_convex(),
        }
    }

    /// `PolylineShape.translateBy(Vector)`.
    pub fn translate_by(&self, vector: &Vector) -> PolylineShapeRef {
        match self {
            PolylineShapeRef::Tile(t) => PolylineShapeRef::Tile(t.translate_by(vector)),
            PolylineShapeRef::Polygon(p) => PolylineShapeRef::Polygon(p.translate_by(vector)),
        }
    }

    /// `PolylineShape.turn90Degree(int, IntPoint)`.
    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> PolylineShapeRef {
        match self {
            PolylineShapeRef::Tile(t) => PolylineShapeRef::Tile(t.turn_90_degree(factor, pole)),
            PolylineShapeRef::Polygon(p) => {
                PolylineShapeRef::Polygon(p.turn_90_degree(factor, pole))
            }
        }
    }

    /// `PolylineShape.rotateApprox(double, FloatPoint)`.
    pub fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> PolylineShapeRef {
        match self {
            PolylineShapeRef::Tile(t) => PolylineShapeRef::Tile(t.rotate_approx(angle, pole)),
            PolylineShapeRef::Polygon(p) => PolylineShapeRef::Polygon(p.rotate_approx(angle, pole)),
        }
    }

    /// `PolylineShape.mirrorVertical(IntPoint)`.
    pub fn mirror_vertical(&self, pole: &IntPoint) -> PolylineShapeRef {
        match self {
            PolylineShapeRef::Tile(t) => PolylineShapeRef::Tile(t.mirror_vertical(pole)),
            PolylineShapeRef::Polygon(p) => PolylineShapeRef::Polygon(p.mirror_vertical(pole)),
        }
    }

    /// `PolylineShape.mirrorHorizontal(IntPoint)`.
    pub fn mirror_horizontal(&self, pole: &IntPoint) -> PolylineShapeRef {
        match self {
            PolylineShapeRef::Tile(t) => PolylineShapeRef::Tile(t.mirror_horizontal(pole)),
            PolylineShapeRef::Polygon(p) => PolylineShapeRef::Polygon(p.mirror_horizontal(pole)),
        }
    }

    /// This shape as one of the three [`crate::shape::Shape`] variants, for
    /// `PolylineShape.getBorder()` / `PolylineArea.getHoles()`.
    pub fn to_shape(&self) -> crate::shape::Shape {
        match self {
            PolylineShapeRef::Tile(t) => crate::shape::Shape::Tile(t.clone()),
            PolylineShapeRef::Polygon(p) => crate::shape::Shape::Polygon(p.clone()),
        }
    }

    /// This shape as a `&dyn PolylineShapeOps`, for the shared `PolylineShape` methods.
    pub fn as_ops(&self) -> &dyn PolylineShapeOps {
        match self {
            PolylineShapeRef::Tile(t) => t,
            PolylineShapeRef::Polygon(p) => p,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;

    fn unit_box() -> TileShape {
        TileShape::Box(IntBox::from_coords(0, 0, 10, 10))
    }

    #[test]
    fn tile_shape_implements_the_polyline_shape_members() {
        let b = unit_box();
        assert_eq!(PolylineShapeOps::border_line_count(&b), 4);
        assert_eq!(b.bounded_corners().len(), 4);
        assert_eq!(
            b.equals_corner(&Point::Int(IntPoint::new(10, 10))),
            Some(2) // IntBox.corner: (ll), (ur.x, ll.y), (ur), (ll.x, ur.y)
        );
        assert_eq!(b.equals_corner(&Point::Int(IntPoint::new(3, 3))), None);
        assert_eq!(PolylineShapeOps::circumference(&b), 40.0);
        assert!(b.is_contained_in(&IntBox::from_coords(-1, -1, 11, 11)));
        assert!(!b.is_contained_in(&IntBox::from_coords(1, 1, 11, 11)));
        assert_eq!(b.prev_no(0), 3);
        assert_eq!(b.next_no(3), 0);
        // Every corner of the box lies to the right of the ray from (-10, 5) to (10, 10)? No —
        // the polar segment simply reports the extreme corners seen from that point.
        let seg = b
            .polar_line_segment(&FloatPoint::new(-10.0, 5.0))
            .expect("box is not empty");
        assert_eq!(seg.a, FloatPoint::new(0.0, 10.0));
        assert_eq!(seg.b, FloatPoint::new(0.0, 0.0));
        assert!(b.intersects_line(&Line::from_coords(5, -5, 5, 15)));
        assert!(!b.intersects_line(&Line::from_coords(20, -5, 20, 15)));
    }

    #[test]
    fn polyline_shape_ref_dispatches_to_both_variants() {
        let tile: PolylineShapeRef = IntBox::from_coords(0, 0, 10, 10).into();
        assert!(tile.contains(&Point::Int(IntPoint::new(5, 5))));
        assert_eq!(tile.split_to_convex().expect("box splits").len(), 1);
        let poly: PolylineShapeRef = PolygonShape::from_points(&[
            Point::Int(IntPoint::new(0, 0)),
            Point::Int(IntPoint::new(10, 0)),
            Point::Int(IntPoint::new(10, 10)),
        ])
        .into();
        assert!(poly.contains(&Point::Int(IntPoint::new(8, 4))));
        assert!(!poly.contains(&Point::Int(IntPoint::new(2, 8))));
    }
}
