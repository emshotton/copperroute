use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::int_vector::IntVector;
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
    pub fn new(center: IntPoint, radius: i32) -> Circle {
        Circle {
            center,
            radius: if radius < 0 { -radius } else { radius },
        }
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn is_bounded(&self) -> bool {
        true
    }

    pub fn dimension(&self) -> i32 {
        if self.radius == 0 {
            // circle is reduced to a point
            return 0;
        }
        2
    }

    pub fn circumference(&self) -> f64 {
        2.0 * std::f64::consts::PI * self.radius as f64
    }

    pub fn area(&self) -> f64 {
        (std::f64::consts::PI * self.radius as f64) * self.radius as f64
    }

    pub fn centre_of_gravity(&self) -> FloatPoint {
        self.center.to_float()
    }

    pub fn is_outside(&self, point: &Point) -> bool {
        let fp = point.to_float();
        fp.distance_square(&self.center.to_float()) > self.radius as f64 * self.radius as f64
    }

    pub fn contains(&self, point: &Point) -> bool {
        !self.is_outside(point)
    }

    pub fn contains_float(&self, point: &FloatPoint) -> bool {
        point.distance_square(&self.center.to_float()) <= self.radius as f64 * self.radius as f64
    }

    pub fn contains_inside(&self, point: &Point) -> bool {
        let fp = point.to_float();
        fp.distance_square(&self.center.to_float()) < self.radius as f64 * self.radius as f64
    }

    pub fn contains_on_border(&self, point: &Point) -> bool {
        let fp = point.to_float();
        fp.distance_square(&self.center.to_float()) == self.radius as f64 * self.radius as f64
    }

    pub fn distance(&self, point: &FloatPoint) -> f64 {
        let d = point.distance(&self.center.to_float()) - self.radius as f64;
        (d).max(0.0)
    }

    pub fn smallest_radius(&self) -> f64 {
        self.radius as f64
    }

    pub fn bounding_box(&self) -> IntBox {
        let lower_left_x = self.center.x - self.radius;
        let upper_right_x = self.center.x + self.radius;
        let lower_left_y = self.center.y - self.radius;
        let upper_right_y = self.center.y + self.radius;
        IntBox::from_coords(lower_left_x, lower_left_y, upper_right_x, upper_right_y)
    }

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

    pub fn bounding_tile(&self) -> TileShape {
        TileShape::Octagon(self.bounding_octagon())
    }

    pub fn bounding_tile_max_seg(&self, max_segment_length: i32) -> TileShape {
        let quadrant_division_count = self.radius / max_segment_length + 1;
        if quadrant_division_count <= 2 {
            return TileShape::Octagon(self.bounding_octagon());
        }
        let count = quadrant_division_count as usize;
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

    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Circle {
        Circle::new(self.center.turn_90_degree(factor, pole), self.radius)
    }

    pub fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> Circle {
        Circle::new(
            self.center.to_float().rotate(angle, pole).round(),
            self.radius,
        )
    }

    pub fn mirror_vertical(&self, pole: &IntPoint) -> Circle {
        Circle::new(self.center.mirror_vertical(pole), self.radius)
    }

    pub fn mirror_horizontal(&self, pole: &IntPoint) -> Circle {
        Circle::new(self.center.mirror_horizontal(pole), self.radius)
    }

    pub fn max_width(&self) -> f64 {
        2.0 * self.radius as f64
    }

    pub fn min_width(&self) -> f64 {
        2.0 * self.radius as f64
    }

    pub fn bounding_shape(
        &self,
        dirs: crate::bounding_directions::ShapeBoundingDirections,
    ) -> RegularTileShape {
        dirs.bounds_circle(self)
    }

    pub fn offset(&self, offset: f64) -> Circle {
        let new_radius = self.radius as f64 + offset;
        Circle::new(self.center, (new_radius).round() as i64 as i32)
    }

    pub fn shrink(&self, offset: f64) -> Circle {
        let new_radius = self.radius as f64 - offset;
        Circle::new(self.center, ((new_radius).round() as i64 as i32).max(1))
    }

    pub fn translate_by(&self, vector: &Vector) -> Circle {
        if *vector == Vector::ZERO {
            return *self;
        }
        let Vector::Int(int_vector) = vector else {
            panic!(
                "Circle.translateBy is not implemented for a RationalVector (Circle.java:249-252); \
                 a circle's centre is an IntPoint, so a rational translation has no representable \
                 result. Java returned the untranslated circle here, which no caller could detect."
            );
        };
        Circle::new(self.center.translate_by(int_vector), self.radius)
    }

    pub fn nearest_point_approx(&self, point: &FloatPoint) -> Option<FloatPoint> {
        let center = self.center.to_float();
        let dx = point.x - center.x;
        let dy = point.y - center.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length == 0.0 {
            return Some(FloatPoint::new(center.x + self.radius as f64, center.y));
        }
        let scale = self.radius as f64 / length;
        Some(FloatPoint::new(
            center.x + dx * scale,
            center.y + dy * scale,
        ))
    }

    pub fn border_distance(&self, point: &FloatPoint) -> f64 {
        let d = point.distance(&self.center.to_float()) - self.radius as f64;
        d.abs()
    }

    pub fn enlarge(&self, offset: f64) -> Circle {
        if offset == 0.0 {
            return *self;
        }
        Circle::new(self.center, self.radius + (offset).round() as i64 as i32)
    }

    pub fn intersects(&self, other: &crate::shape::Shape) -> bool {
        other.intersects_circle(self)
    }

    pub fn intersects_circle(&self, other: &Circle) -> bool {
        let mut radius_sum_square = (self.radius + other.radius) as f64;
        radius_sum_square *= radius_sum_square;
        self.center.distance_square(&other.center) <= radius_sum_square
    }

    pub fn intersects_box(&self, b: &IntBox) -> bool {
        b.distance(&self.center.to_float()) <= self.radius as f64
    }

    pub fn intersects_octagon(&self, oct: &IntOctagon) -> bool {
        TileShape::Octagon(*oct).distance(&self.center.to_float()) <= self.radius as f64
    }

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

    pub fn cutout(&self, polyline: &Polyline) -> Option<Vec<Polyline>> {
        let max_segment_length = (self.radius / 32).max(1);
        self.bounding_tile_max_seg(max_segment_length)
            .cutout_polyline(polyline)
            .ok()
    }

    pub fn split_to_convex(&self) -> Vec<TileShape> {
        vec![self.bounding_tile()]
    }

    pub fn get_border(&self) -> Circle {
        *self
    }

    pub fn get_holes(&self) -> Vec<crate::shape::Shape> {
        Vec::new()
    }

    pub fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        Vec::new()
    }
}

impl fmt::Display for Circle {
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
        let nearest = c.nearest_point_approx(&FloatPoint::new(0.0, 0.0)).unwrap();
        assert!((nearest.distance(&c.center.to_float()) - c.radius as f64).abs() < 1e-9);
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
