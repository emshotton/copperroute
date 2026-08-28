//! Port of `app.freerouting.geometry.planar.Circle`: "Describes functionality of a circle shape
//! in the plane" (Circle.java:8).
//!
//! Java's `Circle implements ConvexShape`, so it carries `offset`, `shrink`, `maxWidth` and
//! `minWidth` next to the `Shape` members.

use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::int_vector::IntVector;
use crate::limits::{java_max, java_round};
use crate::line::Line;
use crate::point::Point;
use crate::polyline::Polyline;
use crate::regular_tile_shape::RegularTileShape;
use crate::shape::ShapeOps;
use crate::simplex::Simplex;
use crate::tile_shape::TileShape;
use crate::vector::Vector;
use std::fmt;

/// A circle with an integer centre and an integer radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Circle {
    /// The centre of the circle.
    pub center: IntPoint,
    /// The radius of the circle; never negative.
    pub radius: i32,
}

impl Circle {
    /// Creates a new instance of `Circle` (Circle.java:14-23). A negative radius is negated;
    /// Java additionally warns "Circle: unexpected negative radius".
    pub fn new(center: IntPoint, radius: i32) -> Circle {
        Circle {
            center,
            radius: if radius < 0 { -radius } else { radius },
        }
    }

    /// A circle is never empty (Circle.java:25-28).
    pub fn is_empty(&self) -> bool {
        false
    }

    /// A circle is always bounded (Circle.java:30-33).
    pub fn is_bounded(&self) -> bool {
        true
    }

    /// 0 if the circle is reduced to a point, 2 otherwise (Circle.java:35-42).
    pub fn dimension(&self) -> i32 {
        if self.radius == 0 {
            // circle is reduced to a point
            return 0;
        }
        2
    }

    /// The length of the border of this circle (Circle.java:44-47).
    pub fn circumference(&self) -> f64 {
        2.0 * std::f64::consts::PI * self.radius as f64
    }

    /// The content of the area of this circle (Circle.java:49-52). Java evaluates
    /// `(Math.PI * radius) * radius`, which is left-associative — kept, because the two
    /// groupings differ in the last bit.
    pub fn area(&self) -> f64 {
        (std::f64::consts::PI * self.radius as f64) * self.radius as f64
    }

    /// The gravity point of this circle (Circle.java:54-57).
    pub fn centre_of_gravity(&self) -> FloatPoint {
        self.center.to_float()
    }

    /// Returns true if `point` is neither inside nor on the boundary (Circle.java:59-63).
    pub fn is_outside(&self, point: &Point) -> bool {
        let fp = point.to_float();
        fp.distance_square(&self.center.to_float()) > self.radius as f64 * self.radius as f64
    }

    /// Returns true if `point` is inside or on the border (Circle.java:65-68).
    pub fn contains(&self, point: &Point) -> bool {
        !self.is_outside(point)
    }

    /// Returns true if `point` is inside or on the border (Circle.java:70-73).
    pub fn contains_float(&self, point: &FloatPoint) -> bool {
        point.distance_square(&self.center.to_float()) <= self.radius as f64 * self.radius as f64
    }

    /// Returns true if `point` is inside, but not on the border (Circle.java:75-79).
    pub fn contains_inside(&self, point: &Point) -> bool {
        let fp = point.to_float();
        fp.distance_square(&self.center.to_float()) < self.radius as f64 * self.radius as f64
    }

    /// Returns true if `point` lies exactly on the boundary (Circle.java:81-85).
    pub fn contains_on_border(&self, point: &Point) -> bool {
        let fp = point.to_float();
        fp.distance_square(&self.center.to_float()) == self.radius as f64 * self.radius as f64
    }

    /// The distance between `point` and its nearest point on this circle; 0 inside
    /// (Circle.java:87-91).
    pub fn distance(&self, point: &FloatPoint) -> f64 {
        let d = point.distance(&self.center.to_float()) - self.radius as f64;
        java_max(d, 0.0)
    }

    /// The smallest distance from the centre of gravity to the border (Circle.java:93-96).
    pub fn smallest_radius(&self) -> f64 {
        self.radius as f64
    }

    /// The smallest surrounding box of this circle (Circle.java:98-105).
    pub fn bounding_box(&self) -> IntBox {
        let lower_left_x = self.center.x - self.radius;
        let upper_right_x = self.center.x + self.radius;
        let lower_left_y = self.center.y - self.radius;
        let upper_right_y = self.center.y + self.radius;
        IntBox::from_coords(lower_left_x, lower_left_y, upper_right_x, upper_right_y)
    }

    /// The smallest surrounding octagon of this circle (Circle.java:107-131).
    ///
    /// The four diagonal bounds read oddly because Java folds the centre in through `leftX` /
    /// `rightX` and then corrects with `center.y`: `upperLeftDiagonalX` works out to
    /// `cx - cy - (r + floor((sqrt(2) - 1) r))`, i.e. `x - y` at the upper-left tangent, and the
    /// other three are its mirrors. Transcribed exactly, floor/ceil included.
    pub fn bounding_octagon(&self) -> IntOctagon {
        let left_x = self.center.x - self.radius;
        let right_x = self.center.x + self.radius;
        let bottom_y = self.center.y - self.radius;
        let top_y = self.center.y + self.radius;

        let sqrt2_minus_1 = std::f64::consts::SQRT_2 - 1.0;
        let ceil_corner_value = (sqrt2_minus_1 * self.radius as f64).ceil() as i32;
        let floor_corner_value = (sqrt2_minus_1 * self.radius as f64).floor() as i32;

        let upper_left_diagonal_x = left_x - (self.center.y + floor_corner_value);
        let lower_right_diagonal_x = right_x - (self.center.y - ceil_corner_value);
        let lower_left_diagonal_x = left_x + (self.center.y - floor_corner_value);
        let upper_right_diagonal_x = right_x + (self.center.y + ceil_corner_value);
        IntOctagon::new(
            left_x,
            bottom_y,
            right_x,
            top_y,
            upper_left_diagonal_x,
            lower_right_diagonal_x,
            lower_left_diagonal_x,
            upper_right_diagonal_x,
        )
    }

    /// A bounding `TileShape` of this circle (Circle.java:133-142). Java returns the bounding
    /// octagon; the commented-out alternative "caused problems with the spring_over algorithm in
    /// routing".
    pub fn bounding_tile(&self) -> TileShape {
        TileShape::Octagon(self.bounding_octagon())
    }

    /// Creates a bounding tile shape around this circle, so that the length of the line segments
    /// of the tile is at most `max_segment_length` (Circle.java:144-175).
    ///
    /// # Panics
    /// For `max_segment_length == 0`, where Java throws `ArithmeticException: / by zero`.
    pub fn bounding_tile_max_seg(&self, max_segment_length: i32) -> TileShape {
        let quadrant_division_count = self.radius / max_segment_length + 1;
        if quadrant_division_count <= 2 {
            return TileShape::Octagon(self.bounding_octagon());
        }
        let count = quadrant_division_count as usize;
        // Java fills a `Line[quadrantDivisionCount * 4]` out of order; `Option` stands in for the
        // `null` slots until every index has been written.
        let mut tangent_line_arr: Vec<Option<Line>> = vec![None; count * 4];
        for i in 0..count {
            // calculate the tangential points in the first quadrant
            let border_delta = if i == 0 {
                IntVector::new(self.radius, 0)
            } else {
                let current_angle =
                    i as f64 * std::f64::consts::PI / (2.0 * quadrant_division_count as f64);
                let current_x = (current_angle.sin() * self.radius as f64).ceil() as i32;
                let current_y = (current_angle.cos() * self.radius as f64).ceil() as i32;
                IntVector::new(current_x, current_y)
            };
            let current_a = self.center.translate_by(&border_delta);
            let current_b = current_a.turn_90_degree(1, &self.center);
            let current_direction = current_b
                .difference_by(&self.center)
                .to_normalized_direction();
            let current_tangent = Line::from_direction(current_a, &current_direction);
            tangent_line_arr[count + i] = Some(current_tangent);
            tangent_line_arr[2 * count + i] = Some(current_tangent.turn_90_degree(1, &self.center));
            tangent_line_arr[3 * count + i] = Some(current_tangent.turn_90_degree(2, &self.center));
            tangent_line_arr[i] = Some(current_tangent.turn_90_degree(3, &self.center));
        }
        let lines: Vec<Line> = tangent_line_arr
            .into_iter()
            .map(|l| l.expect("every slot of the tangent array is written"))
            .collect();
        TileShape::get_instance_from_lines(lines)
    }

    /// Checks if this circle is completely contained in `b` (Circle.java:177-189).
    pub fn is_contained_in(&self, b: &IntBox) -> bool {
        if b.ll.x > self.center.x - self.radius {
            return false;
        }
        if b.ll.y > self.center.y - self.radius {
            return false;
        }
        if b.ur.x < self.center.x + self.radius {
            return false;
        }
        b.ur.y >= self.center.y + self.radius
    }

    /// Turns this circle by `factor` times 90 degree around `pole` (Circle.java:191-195).
    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Circle {
        Circle::new(self.center.turn_90_degree(factor, pole), self.radius)
    }

    /// Rotates this circle around `pole` by `angle` (Circle.java:197-201).
    pub fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> Circle {
        Circle::new(
            self.center.to_float().rotate(angle, pole).round(),
            self.radius,
        )
    }

    /// Mirrors this circle at the vertical line through `pole` (Circle.java:203-207).
    pub fn mirror_vertical(&self, pole: &IntPoint) -> Circle {
        Circle::new(self.center.mirror_vertical(pole), self.radius)
    }

    /// Mirrors this circle at the horizontal line through `pole` (Circle.java:209-213).
    pub fn mirror_horizontal(&self, pole: &IntPoint) -> Circle {
        Circle::new(self.center.mirror_horizontal(pole), self.radius)
    }

    /// The maximum diameter of this circle (Circle.java:215-218).
    pub fn max_width(&self) -> f64 {
        2.0 * self.radius as f64
    }

    /// The minimum diameter of this circle (Circle.java:220-223).
    pub fn min_width(&self) -> f64 {
        2.0 * self.radius as f64
    }

    /// The bounding `RegularTileShape` with the fixed directions `dirs` (Circle.java:225-228).
    pub fn bounding_shape(
        &self,
        dirs: crate::bounding_directions::ShapeBoundingDirections,
    ) -> RegularTileShape {
        dirs.bounds_circle(self)
    }

    /// The offset shape by `offset` (Circle.java:230-235).
    pub fn offset(&self, offset: f64) -> Circle {
        let new_radius = self.radius as f64 + offset;
        Circle::new(self.center, java_round(new_radius) as i32)
    }

    /// Shrinks this circle by `offset`; the result shape will not be empty
    /// (Circle.java:237-242).
    pub fn shrink(&self, offset: f64) -> Circle {
        let new_radius = self.radius as f64 - offset;
        Circle::new(self.center, (java_round(new_radius) as i32).max(1))
    }

    /// The affine translation of this circle by `vector` (Circle.java:244-255).
    ///
    /// Java warns "Circle.translate_by only implemented for IntVectors till now" and returns
    /// `this` unchanged for a rational vector; ported as-is, because the Java caller sees a
    /// value, not a crash.
    pub fn translate_by(&self, vector: &Vector) -> Circle {
        if *vector == Vector::ZERO {
            return *self;
        }
        let Vector::Int(int_vector) = vector else {
            // Java: FRLogger.warn("Circle.translate_by only implemented for IntVectors till now")
            return *self;
        };
        Circle::new(self.center.translate_by(int_vector), self.radius)
    }

    /// Java stub: warns "Circle.nearest_point_approx not yet implemented" and returns `null`
    /// (Circle.java:257-261).
    pub fn nearest_point_approx(&self, _point: &FloatPoint) -> Option<FloatPoint> {
        None
    }

    /// The distance between `point` and its nearest point on the border (Circle.java:263-267).
    pub fn border_distance(&self, point: &FloatPoint) -> f64 {
        let d = point.distance(&self.center.to_float()) - self.radius as f64;
        d.abs()
    }

    /// The offset shape of this circle by `offset` (Circle.java:269-276).
    pub fn enlarge(&self, offset: f64) -> Circle {
        if offset == 0.0 {
            return *self;
        }
        Circle::new(self.center, self.radius + java_round(offset) as i32)
    }

    /// Checks if this circle and `other` have a nonempty intersection (Circle.java:278-281;
    /// Java double-dispatches through `other.intersects(this)`).
    pub fn intersects(&self, other: &crate::shape::Shape) -> bool {
        other.intersects_circle(self)
    }

    /// Checks if this circle and `other` have a nonempty intersection (Circle.java:283-288).
    pub fn intersects_circle(&self, other: &Circle) -> bool {
        let mut radius_sum_square = (self.radius + other.radius) as f64;
        radius_sum_square *= radius_sum_square;
        self.center.distance_square(&other.center) <= radius_sum_square
    }

    /// Checks if this circle and `b` have a nonempty intersection (Circle.java:290-293).
    pub fn intersects_box(&self, b: &IntBox) -> bool {
        b.distance(&self.center.to_float()) <= self.radius as f64
    }

    /// Checks if this circle and `oct` have a nonempty intersection (Circle.java:295-298).
    pub fn intersects_octagon(&self, oct: &IntOctagon) -> bool {
        TileShape::Octagon(*oct).distance(&self.center.to_float()) <= self.radius as f64
    }

    /// Checks if this circle and `simplex` have a nonempty intersection (Circle.java:300-303).
    pub fn intersects_simplex(&self, simplex: &Simplex) -> bool {
        TileShape::Simplex(simplex.clone()).distance(&self.center.to_float()) <= self.radius as f64
    }

    /// `intersects(IntBox|IntOctagon|Simplex)` dispatched over the `TileShape` enum, for
    /// `PolygonShape.intersects(Circle)`.
    pub fn intersects_tile(&self, tile: &TileShape) -> bool {
        match tile {
            TileShape::Box(b) => self.intersects_box(b),
            TileShape::Octagon(o) => self.intersects_octagon(o),
            TileShape::Simplex(s) => self.intersects_simplex(s),
        }
    }

    /// Java stub: warns "Circle.cutout not yet implemented" and returns `null`
    /// (Circle.java:305-309).
    pub fn cutout(&self, _polyline: &Polyline) -> Option<Vec<Polyline>> {
        None
    }

    /// A division of this circle into convex pieces: the single bounding tile
    /// (Circle.java:311-316).
    pub fn split_to_convex(&self) -> Vec<TileShape> {
        vec![self.bounding_tile()]
    }

    /// The border shape of this area: the circle itself (Circle.java:318-321).
    pub fn get_border(&self) -> Circle {
        *self
    }

    /// A circle has no holes (Circle.java:323-326).
    pub fn get_holes(&self) -> Vec<crate::shape::Shape> {
        Vec::new()
    }

    /// A circle has no corners (Circle.java:328-331).
    pub fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        Vec::new()
    }
}

impl fmt::Display for Circle {
    /// `Circle.toString(Locale.ENGLISH)` (Circle.java:333-349). The centre is omitted when it is
    /// the origin, and Java concatenates the two parts without a separator.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Circle: ")?;
        if self.center != IntPoint::new(0, 0) {
            write!(f, "center {}", self.center)?;
        }
        write!(f, "radius {}", grouped(self.radius))
    }
}

/// `NumberFormat.getInstance(Locale.ENGLISH).format(int)`: decimal digits grouped in threes by
/// commas. The radius is never negative after the constructor.
fn grouped(value: i32) -> String {
    let digits = value.to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_point::FloatPoint;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::point::Point;

    #[test]
    fn metrics_and_containment() {
        let c = Circle::new(IntPoint::new(0, 0), 10);
        assert!((c.area() - std::f64::consts::PI * 100.0).abs() < 1e-9);
        assert_eq!(c.bounding_box(), IntBox::from_coords(-10, -10, 10, 10));
        assert!(c.contains(&Point::Int(IntPoint::new(5, 5))));
        assert!(!c.contains(&Point::Int(IntPoint::new(8, 8))));
        assert!(c.contains_on_border(&Point::Int(IntPoint::new(10, 0))));
        assert_eq!(c.distance(&FloatPoint::new(13.0, 0.0)), 3.0);
        assert_eq!(c.smallest_radius(), 10.0);
        assert!(c.bounding_tile().contains(&Point::Int(IntPoint::new(7, 7))));
        assert!(c.bounding_octagon().is_normalized());
        // Pinned against Java: `IntBox.distance((0,0))` for the box (9,9)-(20,20) is
        // sqrt(2) * 9 = 12.7 > 10, so the two do *not* intersect even though the box's corner
        // lies inside the bounding octagon.
        assert!(!c.intersects_box(&IntBox::from_coords(9, 9, 20, 20)));
        assert!(!c.intersects_box(&IntBox::from_coords(11, 11, 20, 20)));
        assert!(c.intersects_circle(&Circle::new(IntPoint::new(15, 0), 6)));
        assert!(!c.intersects_circle(&Circle::new(IntPoint::new(20, 0), 6)));
    }

    #[test]
    fn bounding_octagon_matches_java() {
        let c = Circle::new(IntPoint::new(0, 0), 10);
        assert_eq!(
            c.bounding_octagon(),
            IntOctagon::new(-10, -10, 10, 10, -14, 15, -14, 15)
        );
    }

    #[test]
    fn negative_radius_is_negated_and_degenerate_circles_have_dimension_zero() {
        assert_eq!(Circle::new(IntPoint::new(1, 2), -7).radius, 7);
        assert_eq!(Circle::new(IntPoint::new(0, 0), 0).dimension(), 0);
        assert_eq!(Circle::new(IntPoint::new(0, 0), 3).dimension(), 2);
        assert!(!Circle::new(IntPoint::new(0, 0), 0).is_empty());
    }

    #[test]
    fn offset_shrink_and_enlarge() {
        let c = Circle::new(IntPoint::new(0, 0), 10);
        assert_eq!(c.offset(2.4).radius, 12);
        assert_eq!(c.shrink(2.4).radius, 8);
        // "The result shape will not be empty": the radius is clamped to 1.
        assert_eq!(c.shrink(100.0).radius, 1);
        assert_eq!(c.enlarge(0.0), c);
        assert_eq!(c.enlarge(2.5).radius, 13); // Math.round is half-up
        assert_eq!(c.max_width(), 20.0);
        assert_eq!(c.min_width(), 20.0);
    }

    #[test]
    fn bounding_tile_with_a_maximum_segment_length() {
        let c = Circle::new(IntPoint::new(0, 0), 1000);
        // radius / maxSegmentLength + 1 <= 2 falls back to the bounding octagon.
        assert_eq!(
            c.bounding_tile_max_seg(1000),
            TileShape::Octagon(c.bounding_octagon())
        );
        let tile = c.bounding_tile_max_seg(100);
        assert!(tile.contains(&Point::Int(IntPoint::new(0, 0))));
        assert!(tile.contains(&Point::Int(IntPoint::new(999, 0))));
        assert!(!tile.contains(&Point::Int(IntPoint::new(1100, 0))));
        // The tile bounds the circle, so it is at least as large as it.
        assert!(tile.area() >= c.area());
    }

    #[test]
    fn transformations_and_containment_in_a_box() {
        let c = Circle::new(IntPoint::new(3, 4), 5);
        assert!(c.is_contained_in(&IntBox::from_coords(-2, -1, 8, 9)));
        assert!(!c.is_contained_in(&IntBox::from_coords(-1, -1, 8, 9)));
        assert_eq!(c.turn_90_degree(4, &IntPoint::new(0, 0)), c);
        assert_eq!(
            c.turn_90_degree(1, &IntPoint::new(0, 0)).center,
            IntPoint::new(-4, 3)
        );
        assert_eq!(
            c.mirror_vertical(&IntPoint::new(0, 0)).center,
            IntPoint::new(-3, 4)
        );
        assert_eq!(
            c.mirror_horizontal(&IntPoint::new(0, 0)).center,
            IntPoint::new(3, -4)
        );
        assert_eq!(c.rotate_approx(0.0, &FloatPoint::new(0.0, 0.0)), c);
        assert_eq!(c.translate_by(&Vector::ZERO), c);
        assert_eq!(
            c.translate_by(&Vector::Int(IntVector::new(1, 1))).center,
            IntPoint::new(4, 5)
        );
        assert_eq!(c.split_to_convex().len(), 1);
        assert!(c.get_holes().is_empty());
        assert!(c.corner_approx_arr().is_empty());
        assert!(c.nearest_point_approx(&FloatPoint::new(0.0, 0.0)).is_none());
    }

    #[test]
    fn display_matches_the_java_english_locale() {
        assert_eq!(
            Circle::new(IntPoint::new(0, 0), 1000).to_string(),
            "Circle: radius 1,000"
        );
        assert_eq!(
            Circle::new(IntPoint::new(2, 3), 7).to_string(),
            "Circle: center (2,3)radius 7"
        );
    }
}
