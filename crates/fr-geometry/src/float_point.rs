use std::fmt;

use crate::direction::Direction;
use crate::int_point::IntPoint;
use crate::rational_vector::big_sign;
use crate::side::Side;
use crate::vector::Vector;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatPoint {
    pub x: f64,
    pub y: f64,
}

fn vector_signs(v: &Vector) -> (i32, i32) {
    match v {
        Vector::Int(iv) => (iv.x.signum(), iv.y.signum()),
        Vector::Rational(rv) => (big_sign(&rv.x) as i32, big_sign(&rv.y) as i32),
    }
}

impl FloatPoint {
    pub const ZERO: FloatPoint = FloatPoint { x: 0.0, y: 0.0 };

    pub fn new(x: f64, y: f64) -> FloatPoint {
        FloatPoint { x, y }
    }

    pub fn from_int(pt: &IntPoint) -> FloatPoint {
        FloatPoint::new(pt.x as f64, pt.y as f64)
    }

    pub fn size_square(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    pub fn size(&self) -> f64 {
        self.size_square().sqrt()
    }

    pub fn distance_square(&self, other: &FloatPoint) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        dx * dx + dy * dy
    }

    pub fn distance(&self, other: &FloatPoint) -> f64 {
        self.distance_square(other).sqrt()
    }

    pub fn weighted_distance(
        &self,
        other: &FloatPoint,
        horizontal_weight: f64,
        vertical_weight: f64,
    ) -> f64 {
        let mut delta_x = self.x - other.x;
        let mut delta_y = self.y - other.y;
        delta_x *= horizontal_weight;
        delta_y *= vertical_weight;
        (delta_x * delta_x + delta_y * delta_y).sqrt()
    }

    pub fn round(&self) -> IntPoint {
        IntPoint::new(
            crate::limits::java_round(self.x) as i32,
            crate::limits::java_round(self.y) as i32,
        )
    }

    pub fn round_to_the_right(&self, dir: &Direction) -> IntPoint {
        let (dir_x, dir_y) = vector_signs(&dir.get_vector());

        let rounded_x = if dir_y > 0 {
            self.x.ceil() as i32
        } else if dir_y < 0 {
            self.x.floor() as i32
        } else {
            crate::limits::java_round(self.x) as i32
        };

        let rounded_y = if dir_x > 0 {
            self.y.floor() as i32
        } else if dir_x < 0 {
            self.y.ceil() as i32
        } else {
            crate::limits::java_round(self.y) as i32
        };
        IntPoint::new(rounded_x, rounded_y)
    }

    pub fn round_to_grid(&self, horizontal_grid: i32, vertical_grid: i32) -> IntPoint {
        let rounded_x = if horizontal_grid > 0 {
            (self.x / horizontal_grid as f64).round_ties_even() * horizontal_grid as f64
        } else {
            self.x
        };
        let rounded_y = if vertical_grid > 0 {
            (self.y / vertical_grid as f64).round_ties_even() * vertical_grid as f64
        } else {
            self.y
        };
        IntPoint::new(rounded_x as i32, rounded_y as i32)
    }

    pub fn round_to_the_left(&self, dir: &Direction) -> IntPoint {
        let (dir_x, dir_y) = vector_signs(&dir.get_vector());

        let rounded_x = if dir_y > 0 {
            self.x.floor() as i32
        } else if dir_y < 0 {
            self.x.ceil() as i32
        } else {
            crate::limits::java_round(self.x) as i32
        };

        let rounded_y = if dir_x > 0 {
            self.y.ceil() as i32
        } else if dir_x < 0 {
            self.y.floor() as i32
        } else {
            crate::limits::java_round(self.y) as i32
        };
        IntPoint::new(rounded_x, rounded_y)
    }

    pub fn add(&self, other: &FloatPoint) -> FloatPoint {
        FloatPoint::new(self.x + other.x, self.y + other.y)
    }

    pub fn subtract(&self, other: &FloatPoint) -> FloatPoint {
        FloatPoint::new(self.x - other.x, self.y - other.y)
    }

    pub fn projection_approx(&self, line: &crate::line::Line) -> FloatPoint {
        let float_line = crate::float_line::FloatLine::new(line.a.to_float(), line.b.to_float());
        float_line.perpendicular_projection(self)
    }

    pub fn scalar_product(&self, p1: &FloatPoint, p2: &FloatPoint) -> f64 {
        let dx1 = p1.x - self.x;
        let dx2 = p2.x - self.x;
        let dy1 = p1.y - self.y;
        let dy2 = p2.y - self.y;
        dx1 * dx2 + dy1 * dy2
    }

    pub fn change_size(&self, new_size: f64) -> FloatPoint {
        if self.x == 0.0 && self.y == 0.0 {
            return *self;
        }
        let length = (self.x * self.x + self.y * self.y).sqrt();
        let new_x = (self.x * new_size) / length;
        let new_y = (self.y * new_size) / length;
        FloatPoint::new(new_x, new_y)
    }

    pub fn change_length(&self, to_point: &FloatPoint, new_length: f64) -> FloatPoint {
        let dx = to_point.x - self.x;
        let dy = to_point.y - self.y;
        if dx == 0.0 && dy == 0.0 {
            return *to_point;
        }
        let length = (dx * dx + dy * dy).sqrt();
        let new_x = self.x + (dx * new_length) / length;
        let new_y = self.y + (dy * new_length) / length;
        FloatPoint::new(new_x, new_y)
    }

    pub fn middle_point(&self, to_point: &FloatPoint) -> FloatPoint {
        let middle_x = 0.5 * (self.x + to_point.x);
        let middle_y = 0.5 * (self.y + to_point.y);
        FloatPoint::new(middle_x, middle_y)
    }

    pub fn side_of(&self, p1: &FloatPoint, p2: &FloatPoint) -> Side {
        let d21x = p2.x - p1.x;
        let d21y = p2.y - p1.y;
        let d01x = self.x - p1.x;
        let d01y = self.y - p1.y;
        let determinant = d21x * d01y - d21y * d01x;
        Side::of_f64(determinant)
    }

    pub fn rotate(&self, angle: f64, pole: &FloatPoint) -> FloatPoint {
        if angle == 0.0 {
            return *self;
        }
        let dx = self.x - pole.x;
        let dy = self.y - pole.y;
        let sin_angle = angle.sin();
        let cos_angle = angle.cos();
        let new_dx = dx * cos_angle - dy * sin_angle;
        let new_dy = dx * sin_angle + dy * cos_angle;
        FloatPoint::new(pole.x + new_dx, pole.y + new_dy)
    }

    pub fn turn_90_degree(&self, factor: i32) -> FloatPoint {
        match factor.rem_euclid(4) {
            0 => FloatPoint::new(self.x, self.y),
            1 => FloatPoint::new(-self.y, self.x),
            2 => FloatPoint::new(-self.x, -self.y),
            3 => FloatPoint::new(self.y, -self.x),
            _ => FloatPoint::ZERO,
        }
    }

    pub fn turn_90_degree_pole(&self, factor: i32, pole: &FloatPoint) -> FloatPoint {
        let v = self.subtract(pole);
        let v = v.turn_90_degree(factor);
        pole.add(&v)
    }

    pub fn is_contained_in_box(&self, p1: &FloatPoint, p2: &FloatPoint, tolerance: f64) -> bool {
        let (min_x, max_x) = if p1.x < p2.x {
            (p1.x, p2.x)
        } else {
            (p2.x, p1.x)
        };
        if self.x < min_x - tolerance || self.x > max_x + tolerance {
            return false;
        }
        let (min_y, max_y) = if p1.y < p2.y {
            (p1.y, p2.y)
        } else {
            (p2.y, p1.y)
        };
        self.y >= min_y - tolerance && self.y <= max_y + tolerance
    }

    pub fn bounding_box(&self) -> crate::int_box::IntBox {
        crate::int_box::IntBox::from_coords(
            self.x.floor() as i32,
            self.y.floor() as i32,
            self.x.ceil() as i32,
            self.y.ceil() as i32,
        )
    }

    pub fn tangential_points(
        &self,
        to_point: &FloatPoint,
        distance: f64,
    ) -> Option<[FloatPoint; 2]> {
        let mut dx = (self.x - to_point.x).abs();
        let dy_abs = (self.y - to_point.y).abs();
        let situation_turned = dy_abs > dx;

        let (pole, circle_center) = if situation_turned {
            (
                FloatPoint::new(-self.y, self.x),
                FloatPoint::new(-to_point.y, to_point.x),
            )
        } else {
            (*self, *to_point)
        };

        dx = pole.x - circle_center.x;
        let dy = pole.y - circle_center.y;
        let dx_square = dx * dx;
        let dy_square = dy * dy;
        let dist_square = dx_square + dy_square;
        let radius_square = distance * distance;
        let discriminant = radius_square * dy_square - (radius_square - dx_square) * dist_square;

        if discriminant <= 0.0 {
            return None;
        }
        let square_root = discriminant.sqrt();

        let a1 = radius_square * dy;
        let dy1 = (a1 + distance * square_root) / dist_square;
        let dy2 = (a1 - distance * square_root) / dist_square;

        let first_point_y = dy1 + circle_center.y;
        let first_point_x = (radius_square - dy * dy1) / dx + circle_center.x;
        let second_point_y = dy2 + circle_center.y;
        let second_point_x = (radius_square - dy * dy2) / dx + circle_center.x;

        if situation_turned {
            Some([
                FloatPoint::new(first_point_y, -first_point_x),
                FloatPoint::new(second_point_y, -second_point_x),
            ])
        } else {
            Some([
                FloatPoint::new(first_point_x, first_point_y),
                FloatPoint::new(second_point_x, second_point_y),
            ])
        }
    }

    pub fn left_tangential_point(
        &self,
        to_point: &FloatPoint,
        distance: f64,
    ) -> Option<FloatPoint> {
        let tangent_points = self.tangential_points(to_point, distance)?;
        Some(
            if to_point.side_of(self, &tangent_points[0]) == Side::OnTheRight {
                tangent_points[0]
            } else {
                tangent_points[1]
            },
        )
    }

    pub fn right_tangential_point(
        &self,
        to_point: &FloatPoint,
        distance: f64,
    ) -> Option<FloatPoint> {
        let tangent_points = self.tangential_points(to_point, distance)?;
        Some(
            if to_point.side_of(self, &tangent_points[0]) == Side::OnTheLeft {
                tangent_points[0]
            } else {
                tangent_points[1]
            },
        )
    }

    pub fn circle_center(&self, p1: &FloatPoint, p2: &FloatPoint) -> Option<FloatPoint> {
        let slope1 = (p1.y - self.y) / (p1.x - self.x);
        let slope2 = (p2.y - p1.y) / (p2.x - p1.x);
        let center_x = (slope1 * slope2 * (self.y - p2.y) + slope2 * (self.x + p1.x)
            - slope1 * (p1.x + p2.x))
            / (2.0 * (slope2 - slope1));
        let center_y = (0.5 * (self.x + p1.x) - center_x) / slope1 + 0.5 * (self.y + p1.y);
        if center_x.is_finite() && center_y.is_finite() {
            Some(FloatPoint::new(center_x, center_y))
        } else {
            None
        }
    }

    pub fn inside_circle(&self, p1: &FloatPoint, p2: &FloatPoint, p3: &FloatPoint) -> bool {
        match p1.circle_center(p2, p3) {
            Some(center) => {
                let radius_square = center.distance_square(p1);
                self.distance_square(&center) < radius_square - 1.0
            }
            None => false,
        }
    }

    fn format_component(value: f64) -> String {
        if value.is_nan() {
            return "NaN".to_string();
        }
        if value.is_infinite() {
            return if value > 0.0 { "\u{221e}" } else { "-\u{221e}" }.to_string();
        }

        let negative = value.is_sign_negative();
        let formatted = format!("{:.4}", value.abs());
        let (int_part, frac_part) = formatted
            .split_once('.')
            .expect("fixed precision always has a '.'");
        let frac_trimmed = frac_part.trim_end_matches('0');

        let mut result = String::with_capacity(int_part.len() + frac_trimmed.len() + 6);
        if negative {
            result.push('-');
        }
        result.push_str(&group_thousands(int_part));
        if !frac_trimmed.is_empty() {
            result.push('.');
            result.push_str(frac_trimmed);
        }
        result
    }
}

fn group_thousands(digits: &str) -> String {
    let reversed: Vec<char> = digits.chars().rev().collect();
    let mut out: Vec<char> = Vec::with_capacity(reversed.len() + reversed.len() / 3);
    for (i, c) in reversed.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(*c);
    }
    out.reverse();
    out.into_iter().collect()
}

impl fmt::Display for FloatPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({} , {})",
            FloatPoint::format_component(self.x),
            FloatPoint::format_component(self.y)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direction::Direction;
    use crate::int_direction::IntDirection;
    use crate::int_point::IntPoint;

    #[test]
    fn round_uses_java_semantics() {
        assert_eq!(FloatPoint::new(1.5, -1.5).round(), IntPoint::new(2, -1));
        assert_eq!(FloatPoint::new(2.4, 2.6).round(), IntPoint::new(2, 3));
    }

    #[test]
    fn round_to_the_right_of_direction() {
        let p = FloatPoint::new(1.2, 3.7);
        let r = p.round_to_the_right(&Direction::Int(IntDirection::UP));
        assert_eq!(r, IntPoint::new(2, 4));
        let l = p.round_to_the_left(&Direction::Int(IntDirection::UP));
        assert_eq!(l, IntPoint::new(1, 4));
    }

    #[test]
    fn round_to_grid() {
        assert_eq!(
            FloatPoint::new(17.0, 26.0).round_to_grid(10, 10),
            IntPoint::new(20, 30)
        );
        assert_eq!(
            FloatPoint::new(17.0, 26.0).round_to_grid(0, 10),
            IntPoint::new(17, 30)
        );
    }

    #[test]
    fn size_change_and_middle() {
        let p = FloatPoint::new(3.0, 4.0);
        assert_eq!(p.size(), 5.0);
        let q = p.change_size(10.0);
        assert!((q.x - 6.0).abs() < 1e-12 && (q.y - 8.0).abs() < 1e-12);
        assert_eq!(
            FloatPoint::new(0.0, 0.0).middle_point(&p),
            FloatPoint::new(1.5, 2.0)
        );
    }

    #[test]
    fn circle_center_and_inside_circle() {
        let a = FloatPoint::new(1.0, 1.0);
        let b = FloatPoint::new(4.0, 2.0);
        let c = FloatPoint::new(2.0, 5.0);
        let center = a.circle_center(&b, &c).unwrap();
        assert!((center.x - 2.045_454_545_454_545_5).abs() < 1e-9);
        assert!((center.y - 2.863_636_363_636_363_8).abs() < 1e-9);

        assert!(
            FloatPoint::new(0.0, 0.0)
                .circle_center(&FloatPoint::new(2.0, 0.0), &FloatPoint::new(4.0, 0.0))
                .is_none()
        );

        assert!(FloatPoint::new(2.0, 2.0).inside_circle(&a, &b, &c));
        assert!(!FloatPoint::new(20.0, 20.0).inside_circle(&a, &b, &c));
    }

    #[test]
    fn tangential_points_none_when_inside() {
        let origin = FloatPoint::new(0.0, 0.0);
        assert!(
            origin
                .tangential_points(&FloatPoint::new(1.0, 0.0), 5.0)
                .is_none()
        );
        let to_point = FloatPoint::new(10.0, 0.0);
        let t = origin.tangential_points(&to_point, 5.0).unwrap();
        for p in t {
            assert!((p.distance(&to_point) - 5.0).abs() < 1e-9);
        }
    }

    #[test]
    fn display_matches_java_number_format() {
        assert_eq!(FloatPoint::new(1.5, -2.0).to_string(), "(1.5 , -2)");
        assert_eq!(
            FloatPoint::new(1_234_567.891_234, -0.0).to_string(),
            "(1,234,567.8912 , -0)"
        );
        assert_eq!(FloatPoint::new(100.0, 0.1).to_string(), "(100 , 0.1)");
    }

    #[test]
    fn display_of_non_finite_coordinates_matches_java() {
        assert_eq!(
            FloatPoint::new(f64::NAN, f64::INFINITY).to_string(),
            "(NaN , \u{221e})"
        );
        assert_eq!(
            FloatPoint::new(f64::NEG_INFINITY, 0.0).to_string(),
            "(-\u{221e} , 0)"
        );
    }

    #[test]
    fn projection_approx_onto_a_line() {
        use crate::line::Line;
        let line = Line::from_coords(0, 0, 10, 0);
        assert_eq!(
            FloatPoint::new(3.0, 4.0).projection_approx(&line),
            FloatPoint::new(3.0, 0.0)
        );
    }
}
