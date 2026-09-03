use crate::float_point::FloatPoint;
use crate::int_direction::IntDirection;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::limits::{CRIT_INT, java_max, java_round};
use crate::line::Line;
use crate::side::Side;
use crate::simplex::Simplex;
use crate::vector::Vector;

/// Implements functionality of orthogonal rectangles in the plane with integer coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntBox {
    /// Stores the coordinates of the lower-left corner.
    pub ll: IntPoint,
    /// Stores the coordinates of the upper-right corner.
    pub ur: IntPoint,
}

impl IntBox {
    pub const EMPTY: IntBox = IntBox {
        ll: IntPoint {
            x: CRIT_INT,
            y: CRIT_INT,
        },
        ur: IntPoint {
            x: -CRIT_INT,
            y: -CRIT_INT,
        },
    };

    /// Creates an IntBox from its lower left and upper right corners.
    pub fn new(ll: IntPoint, ur: IntPoint) -> IntBox {
        IntBox { ll, ur }
    }

    /// Creates an IntBox from the coordinates of its lower-left and upper-right corners.
    pub fn from_coords(
        lower_left_x: i32,
        lower_left_y: i32,
        upper_right_x: i32,
        upper_right_y: i32,
    ) -> IntBox {
        IntBox {
            ll: IntPoint::new(lower_left_x, lower_left_y),
            ur: IntPoint::new(upper_right_x, upper_right_y),
        }
    }

    pub fn is_int_octagon(&self) -> bool {
        true
    }

    /// Returns true, if the box is empty.
    pub fn is_empty(&self) -> bool {
        self.ll.x > self.ur.x || self.ll.y > self.ur.y
    }

    /// Returns the horizontal extension of the box.
    pub fn width(&self) -> i32 {
        self.ur.x - self.ll.x
    }

    /// Returns the vertical extension of the box.
    pub fn height(&self) -> i32 {
        self.ur.y - self.ll.y
    }

    pub fn max_width(&self) -> f64 {
        (self.ur.x - self.ll.x).max(self.ur.y - self.ll.y) as f64
    }

    pub fn min_width(&self) -> f64 {
        (self.ur.x - self.ll.x).min(self.ur.y - self.ll.y) as f64
    }

    /// Returns the area of the box.
    pub fn area(&self) -> f64 {
        ((self.ur.x - self.ll.x) as f64) * ((self.ur.y - self.ll.y) as f64)
    }

    pub fn circumference(&self) -> f64 {
        (2 * ((self.ur.x - self.ll.x) + (self.ur.y - self.ll.y))) as f64
    }

    pub fn corner(&self, no: usize) -> IntPoint {
        match no {
            0 => self.ll,
            1 => IntPoint::new(self.ur.x, self.ll.y),
            2 => self.ur,
            3 => IntPoint::new(self.ll.x, self.ur.y),
            _ => panic!("IntBox.corner: no out of range"),
        }
    }

    /// Returns the dimension of the box: -1 if empty, 0 if a point, 1 if a line segment, else 2.
    pub fn dimension(&self) -> i32 {
        if self.is_empty() {
            return -1;
        }
        if self.ll == self.ur {
            return 0;
        }
        if self.ur.x == self.ll.x || self.ll.y == self.ur.y {
            return 1;
        }
        2
    }

    /// Checks, if point is located in the interior of this box.
    pub fn contains_inside(&self, point: &IntPoint) -> bool {
        point.x > self.ll.x && point.x < self.ur.x && point.y > self.ll.y && point.y < self.ur.y
    }

    pub fn is_int_box(&self) -> bool {
        true
    }

    /// Calculates the nearest point of this box to from_point.
    pub fn nearest_point(&self, from_point: &FloatPoint) -> FloatPoint {
        let x = if from_point.x <= self.ll.x as f64 {
            self.ll.x as f64
        } else if from_point.x >= self.ur.x as f64 {
            self.ur.x as f64
        } else {
            from_point.x
        };
        let y = if from_point.y <= self.ll.y as f64 {
            self.ll.y as f64
        } else if from_point.y >= self.ur.y as f64 {
            self.ur.y as f64
        } else {
            from_point.y
        };
        FloatPoint::new(x, y)
    }

    #[allow(unused_assignments)]
    pub fn nearest_border_projections(
        &self,
        point: &IntPoint,
        max_result_points: usize,
    ) -> Vec<IntPoint> {
        if max_result_points == 0 {
            return Vec::new();
        }
        let max_result_points = max_result_points.min(2);

        let lower_horizontal_difference = point.x - self.ll.x;
        let upper_horizontal_difference = self.ur.x - point.x;
        let lower_vertical_difference = point.y - self.ll.y;
        let upper_vertical_difference = self.ur.y - point.y;

        let mut min_diff;
        let mut second_min_diff;

        let mut nearest_projection_x = point.x;
        let mut nearest_projection_y = point.y;
        let mut second_nearest_projection_x = point.x;
        let mut second_nearest_projection_y = point.y;

        if lower_horizontal_difference <= upper_horizontal_difference {
            min_diff = lower_horizontal_difference;
            second_min_diff = upper_horizontal_difference;
            nearest_projection_x = self.ll.x;
            second_nearest_projection_x = self.ur.x;
        } else {
            min_diff = upper_horizontal_difference;
            second_min_diff = lower_horizontal_difference;
            nearest_projection_x = self.ur.x;
            second_nearest_projection_x = self.ll.x;
        }

        if lower_vertical_difference < min_diff {
            second_min_diff = min_diff;
            min_diff = lower_vertical_difference;
            second_nearest_projection_x = nearest_projection_x;
            second_nearest_projection_y = nearest_projection_y;
            nearest_projection_x = point.x;
            nearest_projection_y = self.ll.y;
        } else if lower_vertical_difference < second_min_diff {
            second_min_diff = lower_vertical_difference;
            second_nearest_projection_x = point.x;
            second_nearest_projection_y = self.ll.y;
        }

        if upper_vertical_difference < min_diff {
            second_min_diff = min_diff;
            min_diff = upper_vertical_difference;
            second_nearest_projection_x = nearest_projection_x;
            second_nearest_projection_y = nearest_projection_y;
            nearest_projection_x = point.x;
            nearest_projection_y = self.ur.y;
        } else if upper_vertical_difference < second_min_diff {
            second_min_diff = upper_vertical_difference;
            second_nearest_projection_x = point.x;
            second_nearest_projection_y = self.ur.y;
        }

        let mut result = Vec::with_capacity(max_result_points);
        result.push(IntPoint::new(nearest_projection_x, nearest_projection_y));
        if max_result_points > 1 {
            result.push(IntPoint::new(
                second_nearest_projection_x,
                second_nearest_projection_y,
            ));
        }
        result
    }

    /// Calculates distance of this box to from_point.
    pub fn distance(&self, from_point: &FloatPoint) -> f64 {
        from_point.distance(&self.nearest_point(from_point))
    }

    /// Computes the weighted distance to the box other.
    pub fn weighted_distance(
        &self,
        other: &IntBox,
        horizontal_weight: f64,
        vertical_weight: f64,
    ) -> f64 {
        let max_ll_x = self.ll.x.max(other.ll.x) as f64;
        let max_ll_y = self.ll.y.max(other.ll.y) as f64;
        let min_ur_x = self.ur.x.min(other.ur.x) as f64;
        let min_ur_y = self.ur.y.min(other.ur.y) as f64;

        if min_ur_x >= max_ll_x {
            java_max(vertical_weight * (max_ll_y - min_ur_y), 0.0)
        } else if min_ur_y >= max_ll_y {
            java_max(horizontal_weight * (max_ll_x - min_ur_x), 0.0)
        } else {
            let delta_x = (max_ll_x - min_ur_x) * horizontal_weight;
            let delta_y = (max_ll_y - min_ur_y) * vertical_weight;
            (delta_x * delta_x + delta_y * delta_y).sqrt()
        }
    }

    pub fn bounding_box(&self) -> IntBox {
        *self
    }

    pub fn get_id(&self) -> i32 {
        31i32
            .wrapping_mul(self.ll.get_id())
            .wrapping_add(self.ur.get_id())
    }

    pub fn is_bounded(&self) -> bool {
        true
    }

    pub fn corner_is_bounded(&self, _no: usize) -> bool {
        true
    }

    /// Returns the union of this box with an `IntBox`.
    pub fn union(&self, other: &IntBox) -> IntBox {
        let lower_left_x = self.ll.x.min(other.ll.x);
        let lower_left_y = self.ll.y.min(other.ll.y);
        let upper_right_x = self.ur.x.max(other.ur.x);
        let upper_right_y = self.ur.y.max(other.ur.y);
        IntBox::from_coords(lower_left_x, lower_left_y, upper_right_x, upper_right_y)
    }

    /// Returns the intersection of this box with an `IntBox`.
    pub fn intersection(&self, other: &IntBox) -> IntBox {
        if other.ll.x > self.ur.x {
            return IntBox::EMPTY;
        }
        if other.ll.y > self.ur.y {
            return IntBox::EMPTY;
        }
        if self.ll.x > other.ur.x {
            return IntBox::EMPTY;
        }
        if self.ll.y > other.ur.y {
            return IntBox::EMPTY;
        }
        let lower_left_x = self.ll.x.max(other.ll.x);
        let upper_right_x = self.ur.x.min(other.ur.x);
        let lower_left_y = self.ll.y.max(other.ll.y);
        let upper_right_y = self.ur.y.min(other.ur.y);
        IntBox::from_coords(lower_left_x, lower_left_y, upper_right_x, upper_right_y)
    }

    /// Returns true, if this box intersects with other (touching counts as intersecting).
    #[inline]
    pub fn intersects(&self, other: &IntBox) -> bool {
        if other.ll.x > self.ur.x {
            return false;
        }
        if other.ll.y > self.ur.y {
            return false;
        }
        if self.ll.x > other.ur.x {
            return false;
        }
        self.ll.y <= other.ur.y
    }

    /// Returns true, if this box intersects with other and the intersection is 2-dimensional.
    pub fn overlaps(&self, other: &IntBox) -> bool {
        if other.ll.x >= self.ur.x {
            return false;
        }
        if other.ll.y >= self.ur.y {
            return false;
        }
        if self.ll.x >= other.ur.x {
            return false;
        }
        self.ll.y < other.ur.y
    }

    pub fn contains(&self, other: &IntBox) -> bool {
        other.is_contained_in(self)
    }

    /// Return true, if other is contained in the interior of this box.
    pub fn contains_in_interior(&self, other: &IntBox) -> bool {
        if other.is_empty() {
            return true;
        }
        other.ll.x > self.ll.x
            && other.ll.y > self.ll.y
            && other.ur.x < self.ur.x
            && other.ur.y < self.ur.y
    }

    pub fn is_contained_in(&self, other: &IntBox) -> bool {
        if self.is_empty() || self == other {
            return true;
        }
        self.ll.x >= other.ll.x
            && self.ll.y >= other.ll.y
            && self.ur.x <= other.ur.x
            && self.ur.y <= other.ur.y
    }

    pub fn translate_by(&self, rel_coor: &Vector) -> IntBox {
        if *rel_coor == Vector::ZERO {
            return *self;
        }
        match rel_coor {
            Vector::Int(v) => IntBox::new(self.ll.translate_by(v), self.ur.translate_by(v)),
            Vector::Rational(_) => panic!(
                "IntBox::translate_by: only implemented for Vector::Int (IntBox.java:395-407)"
            ),
        }
    }

    /// Turns this box by factor times 90 degree around pole.
    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> IntBox {
        let p1 = self.ll.turn_90_degree(factor, pole);
        let p2 = self.ur.turn_90_degree(factor, pole);

        let lower_left_x = p1.x.min(p2.x);
        let lower_left_y = p1.y.min(p2.y);
        let upper_right_x = p1.x.max(p2.x);
        let upper_right_y = p1.y.max(p2.y);
        IntBox::from_coords(lower_left_x, lower_left_y, upper_right_x, upper_right_y)
    }

    pub fn border_line(&self, no: usize) -> Line {
        match no {
            0 => Line::from_coords(0, self.ll.y, 1, self.ll.y), // lower boundary line
            1 => Line::from_coords(self.ur.x, 0, self.ur.x, 1), // right boundary line
            2 => Line::from_coords(0, self.ur.y, -1, self.ur.y), // upper boundary line
            3 => Line::from_coords(self.ll.x, 0, self.ll.x, -1), // left boundary line
            _ => panic!("IntBox.borderLine: no out of range"),
        }
    }

    pub fn border_line_index(&self, line: &Line) -> Option<usize> {
        (0..4).find(|&i| line.equals_geometric(&self.border_line(i)))
    }

    /// Returns the box offsetted by dist. If dist > 0, the offset is to the outside, else to the
    /// inside.
    pub fn offset(&self, dist: f64) -> IntBox {
        if dist == 0.0 || self.is_empty() {
            return *self;
        }
        let rounded_distance = java_round(dist) as i32;
        IntBox::from_coords(
            self.ll.x - rounded_distance,
            self.ll.y - rounded_distance,
            self.ur.x + rounded_distance,
            self.ur.y + rounded_distance,
        )
    }

    /// Returns the box, where the horizontal boundary is offsetted by dist. If dist > 0, the
    /// offset is to the outside, else to the inside.
    pub fn horizontal_offset(&self, dist: f64) -> IntBox {
        if dist == 0.0 || self.is_empty() {
            return *self;
        }
        let rounded_distance = java_round(dist) as i32;
        IntBox::from_coords(
            self.ll.x - rounded_distance,
            self.ll.y,
            self.ur.x + rounded_distance,
            self.ur.y,
        )
    }

    /// Returns the box, where the vertical boundary is offsetted by dist. If dist > 0, the offset
    /// is to the outside, else to the inside.
    pub fn vertical_offset(&self, dist: f64) -> IntBox {
        if dist == 0.0 || self.is_empty() {
            return *self;
        }
        let rounded_distance = java_round(dist) as i32;
        IntBox::from_coords(
            self.ll.x,
            self.ll.y - rounded_distance,
            self.ur.x,
            self.ur.y + rounded_distance,
        )
    }

    /// Shrinks the width and height of the box by the input width. The box will not vanish
    /// completely — it collapses to the centre point instead.
    pub fn shrink(&self, width: i32) -> IntBox {
        let (lower_left_x, upper_right_x) = if 2 * width <= self.ur.x - self.ll.x {
            (self.ll.x + width, self.ur.x - width)
        } else {
            let mid = (self.ll.x + self.ur.x) / 2;
            (mid, mid)
        };
        let (lower_left_y, upper_right_y) = if 2 * width <= self.ur.y - self.ll.y {
            (self.ll.y + width, self.ur.y - width)
        } else {
            let mid = (self.ll.y + self.ur.y) / 2;
            (mid, mid)
        };
        IntBox::from_coords(lower_left_x, lower_left_y, upper_right_x, upper_right_y)
    }

    pub fn compare(&self, other: &IntBox, edge_index: usize) -> Side {
        match edge_index {
            0 => {
                // compare the lower edge line
                if self.ll.y > other.ll.y {
                    Side::OnTheLeft
                } else if self.ll.y < other.ll.y {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            1 => {
                // compare the right edge line
                if self.ur.x < other.ur.x {
                    Side::OnTheLeft
                } else if self.ur.x > other.ur.x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            2 => {
                // compare the upper edge line
                if self.ur.y < other.ur.y {
                    Side::OnTheLeft
                } else if self.ur.y > other.ur.y {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            3 => {
                // compare the left edge line
                if self.ll.x > other.ll.x {
                    Side::OnTheLeft
                } else if self.ll.x < other.ll.x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            _ => panic!("IntBox.compare: edgeIndex out of range"),
        }
    }

    /// Calculates the part of from_box, which has minimal distance to this box.
    pub fn nearest_part(&self, from_box: &IntBox) -> IntBox {
        let ll_x = if from_box.ll.x >= self.ll.x {
            from_box.ll.x
        } else {
            from_box.ur.x.min(self.ll.x)
        };
        let ur_x = if from_box.ur.x <= self.ur.x {
            from_box.ur.x
        } else {
            from_box.ll.x.max(self.ur.x)
        };
        let ll_y = if from_box.ll.y >= self.ll.y {
            from_box.ll.y
        } else {
            from_box.ur.y.min(self.ll.y)
        };
        let ur_y = if from_box.ur.y <= self.ur.y {
            from_box.ur.y
        } else {
            from_box.ll.y.max(self.ur.y)
        };
        IntBox::from_coords(ll_x, ll_y, ur_x, ur_y)
    }

    pub fn divide_into_sections(&self, max_section_width: f64) -> Vec<IntBox> {
        if max_section_width <= 0.0 {
            return Vec::new();
        }
        let length = (self.ur.x - self.ll.x) as f64;
        let height = (self.ur.y - self.ll.y) as f64;
        let xcount = (length / max_section_width).ceil() as i32;
        let ycount = (height / max_section_width).ceil() as i32;
        let section_length_x = (length / xcount as f64).ceil() as i32;
        let section_length_y = (height / ycount as f64).ceil() as i32;

        let mut result = Vec::new();
        for j in 0..ycount {
            let current_lower_left_y = self.ll.y + j * section_length_y;
            let current_upper_right_y = if j == ycount - 1 {
                self.ur.y
            } else {
                current_lower_left_y + section_length_y
            };
            for i in 0..xcount {
                let current_lower_left_x = self.ll.x + i * section_length_x;
                let current_upper_right_x = if i == xcount - 1 {
                    self.ur.x
                } else {
                    current_lower_left_x + section_length_x
                };
                let section = IntBox::from_coords(
                    current_lower_left_x,
                    current_lower_left_y,
                    current_upper_right_x,
                    current_upper_right_y,
                );
                if section.dimension() == 2 {
                    result.push(section);
                }
            }
        }
        result
    }

    pub fn cutout_from(&self, d: &IntBox) -> Vec<IntBox> {
        let c = self.intersection(d);
        if self.is_empty() || c.dimension() < self.dimension() {
            // there is only an overlap at the border
            return vec![*d];
        }

        let mut result = [
            IntBox::from_coords(d.ll.x, d.ll.y, c.ur.x, c.ll.y),
            IntBox::from_coords(d.ll.x, c.ll.y, c.ll.x, d.ur.y),
            IntBox::from_coords(c.ur.x, d.ll.y, d.ur.x, c.ur.y),
            IntBox::from_coords(c.ll.x, c.ur.y, d.ur.x, d.ur.y),
        ];

        // now the division will be optimised, so that the cumulative circumference will be
        // minimal.

        if c.ll.x - d.ll.x > c.ll.y - d.ll.y {
            // switch left dividing line to lower
            let b = result[0];
            result[0] = IntBox::from_coords(c.ll.x, b.ll.y, b.ur.x, b.ur.y);
            let b = result[1];
            result[1] = IntBox::from_coords(b.ll.x, d.ll.y, b.ur.x, b.ur.y);
        }
        if d.ur.y - c.ur.y > c.ll.x - d.ll.x {
            // switch upper dividing line to the left
            let b = result[1];
            result[1] = IntBox::from_coords(b.ll.x, b.ll.y, b.ur.x, c.ur.y);
            let b = result[3];
            result[3] = IntBox::from_coords(d.ll.x, b.ll.y, b.ur.x, b.ur.y);
        }
        if d.ur.x - c.ur.x > d.ur.y - c.ur.y {
            // switch right dividing line to upper
            let b = result[2];
            result[2] = IntBox::from_coords(b.ll.x, b.ll.y, b.ur.x, d.ur.y);
            let b = result[3];
            result[3] = IntBox::from_coords(b.ll.x, b.ll.y, c.ur.x, b.ur.y);
        }
        if c.ll.y - d.ll.y > d.ur.x - c.ur.x {
            // switch lower dividing line to the left
            let b = result[0];
            result[0] = IntBox::from_coords(b.ll.x, b.ll.y, d.ur.x, b.ur.y);
            let b = result[2];
            result[2] = IntBox::from_coords(b.ll.x, c.ll.y, b.ur.x, b.ur.y);
        }
        result.to_vec()
    }

    #[inline]
    pub fn to_int_octagon(&self) -> IntOctagon {
        IntOctagon::new(
            self.ll.x,
            self.ll.y,
            self.ur.x,
            self.ur.y,
            self.ll.x - self.ur.y,
            self.ur.x - self.ll.y,
            self.ll.x + self.ll.y,
            self.ur.x + self.ur.y,
        )
    }

    #[inline]
    pub fn bounding_octagon(&self) -> IntOctagon {
        self.to_int_octagon()
    }

    pub fn union_octagon(&self, other: &IntOctagon) -> IntOctagon {
        other.union(&self.to_int_octagon())
    }

    pub fn intersection_octagon(&self, other: &IntOctagon) -> IntOctagon {
        other.intersection(&self.to_int_octagon())
    }

    #[inline]
    pub fn intersects_octagon(&self, other: &IntOctagon) -> bool {
        other.intersects_octagon(&self.to_int_octagon())
    }

    pub fn is_contained_in_octagon(&self, other: &IntOctagon) -> bool {
        self.to_int_octagon().is_contained_in_octagon(other)
    }

    pub fn enlarge(&self, offset: f64) -> IntOctagon {
        self.bounding_octagon().offset(offset)
    }

    pub fn compare_octagon(&self, other: &IntOctagon, edge_index: usize) -> Side {
        self.to_int_octagon().compare_octagon(other, edge_index)
    }

    pub fn cutout_from_octagon(&self, oct: &IntOctagon) -> Vec<IntOctagon> {
        self.to_int_octagon().cutout_from_octagon(oct)
    }

    pub fn to_simplex(&self) -> Simplex {
        let lines = if self.is_empty() {
            Vec::new()
        } else {
            vec![
                Line::from_direction(self.ll, &IntDirection::RIGHT),
                Line::from_direction(self.ur, &IntDirection::UP),
                Line::from_direction(self.ur, &IntDirection::LEFT),
                Line::from_direction(self.ll, &IntDirection::DOWN),
            ]
        };
        Simplex::new(lines)
    }

    pub fn intersection_simplex(&self, other: &Simplex) -> Simplex {
        other.intersection(&self.to_simplex())
    }

    pub fn intersects_simplex(&self, other: &Simplex) -> bool {
        other.intersects(&self.to_simplex())
    }

    pub fn cutout_from_simplex(&self, simplex: &Simplex) -> Option<Vec<Simplex>> {
        self.to_simplex().cutout_from(simplex)
    }

    pub fn border_line_count(&self) -> usize {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_point::FloatPoint;
    use crate::int_point::IntPoint;
    use crate::int_vector::IntVector;
    use crate::vector::Vector;

    fn b(a: i32, b_: i32, c: i32, d: i32) -> IntBox {
        IntBox::from_coords(a, b_, c, d)
    }

    #[test]
    fn metrics() {
        let x = b(0, 0, 10, 4);
        assert_eq!(x.width(), 10);
        assert_eq!(x.height(), 4);
        assert_eq!(x.area(), 40.0);
        assert_eq!(x.circumference(), 28.0);
        assert_eq!(x.max_width(), 10.0);
        assert_eq!(x.min_width(), 4.0);
        assert!(!x.is_empty());
        assert!(b(5, 5, 4, 9).is_empty());
        assert_eq!(x.dimension(), 2);
        assert_eq!(b(3, 3, 3, 3).dimension(), 0);
        assert_eq!(b(3, 3, 9, 3).dimension(), 1);
    }

    #[test]
    fn set_operations() {
        let x = b(0, 0, 10, 10);
        let y = b(5, 5, 20, 20);
        assert_eq!(x.intersection(&y), b(5, 5, 10, 10));
        assert_eq!(x.union(&y), b(0, 0, 20, 20));
        assert!(x.intersects(&y));
        assert!(x.intersects(&b(10, 10, 12, 12))); // touching counts as intersecting
        assert!(!x.overlaps(&b(10, 10, 12, 12))); // overlaps requires interior overlap
        assert!(x.contains(&b(1, 1, 2, 2)));
        assert!(!x.contains_in_interior(&b(0, 1, 2, 2)));
        assert!(b(1, 1, 2, 2).is_contained_in(&x));
        assert!(x.intersection(&b(50, 50, 60, 60)).is_empty());
    }

    #[test]
    fn corners_and_border_lines() {
        let x = b(0, 0, 10, 4);
        let corners: Vec<IntPoint> = (0..4).map(|i| x.corner(i)).collect();
        assert!(corners.contains(&IntPoint::new(0, 0)));
        assert!(corners.contains(&IntPoint::new(10, 4)));

        assert_eq!(x.border_line(0), Line::from_coords(0, x.ll.y, 1, x.ll.y));
        assert_eq!(x.border_line(1), Line::from_coords(x.ur.x, 0, x.ur.x, 1));
        assert_eq!(x.border_line(2), Line::from_coords(0, x.ur.y, -1, x.ur.y));
        assert_eq!(x.border_line(3), Line::from_coords(x.ll.x, 0, x.ll.x, -1));

        for i in 0..4 {
            assert_eq!(x.border_line_index(&x.border_line(i)), Some(i));
        }
        assert_eq!(x.border_line_index(&Line::from_coords(0, 0, 1, 1)), None);
    }

    #[test]
    fn offsets_and_translation() {
        let x = b(0, 0, 10, 10);
        assert_eq!(x.offset(2.0), b(-2, -2, 12, 12));
        assert_eq!(x.offset(1.4), b(-1, -1, 11, 11));
        assert_eq!(x.horizontal_offset(3.0), b(-3, 0, 13, 10));
        assert_eq!(x.shrink(2), b(2, 2, 8, 8));
        assert_eq!(x.shrink(50), b(5, 5, 5, 5)); // collapses to the centre
        assert_eq!(
            x.translate_by(&Vector::Int(IntVector::new(1, -1))),
            b(1, -1, 11, 9)
        );
        assert_eq!(x.turn_90_degree(1, &IntPoint::new(0, 0)), b(-10, 0, 0, 10));
    }

    #[test]
    fn nearest_point_and_distance() {
        let x = b(0, 0, 10, 10);
        assert_eq!(
            x.nearest_point(&FloatPoint::new(-3.0, 4.0)),
            FloatPoint::new(0.0, 4.0)
        );
        assert_eq!(
            x.nearest_point(&FloatPoint::new(13.0, 14.0)),
            FloatPoint::new(10.0, 10.0)
        );
        assert_eq!(x.distance(&FloatPoint::new(13.0, 14.0)), 5.0);
        assert_eq!(x.distance(&FloatPoint::new(5.0, 5.0)), 0.0);
    }

    #[test]
    fn nearest_border_projections_and_weighted_distance() {
        let x = b(0, 0, 10, 10);
        // point (3,3): equidistant (3) from the left and bottom borders.
        assert_eq!(
            x.nearest_border_projections(&IntPoint::new(3, 3), 2),
            vec![IntPoint::new(0, 3), IntPoint::new(3, 0)]
        );
        assert_eq!(
            x.nearest_border_projections(&IntPoint::new(3, 3), 0),
            vec![]
        );
        assert_eq!(
            x.nearest_border_projections(&IntPoint::new(3, 3), 5).len(),
            2
        );
        assert_eq!(x.weighted_distance(&b(20, 0, 30, 10), 1.0, 2.0), 10.0);
        assert_eq!(x.weighted_distance(&b(0, 0, 5, 5), 1.0, 2.0), 0.0);
    }

    #[test]
    fn cutout_from_returns_surrounding_pieces() {
        let outer = b(0, 0, 10, 10);
        let inner = b(4, 4, 6, 6);
        let pieces = inner.cutout_from(&outer);
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert_eq!(total, outer.area() - inner.area());
        for p in &pieces {
            assert!(!p.overlaps(&inner));
            assert!(outer.contains(p));
        }
        assert!(b(20, 20, 30, 30).cutout_from(&outer).len() == 1);
    }

    #[test]
    fn divide_into_sections_covers_area() {
        let x = b(0, 0, 100, 10);
        let parts = x.divide_into_sections(30.0);
        assert_eq!(parts.iter().map(|p| p.area()).sum::<f64>(), 1000.0);
        assert!(parts.iter().all(|p| p.width() <= 30 && p.height() <= 30));
    }

    #[test]
    fn point_helpers() {
        assert_eq!(IntPoint::new(3, 4).surrounding_box(), b(3, 4, 3, 4));
        assert!(IntPoint::new(3, 4).is_contained_in(&b(0, 0, 3, 4)));
        assert_eq!(FloatPoint::new(1.2, -1.2).bounding_box(), b(1, -2, 2, -1));
    }

    #[test]
    fn misc_accessors() {
        let x = b(0, 0, 10, 10);
        assert!(x.is_bounded());
        assert!(x.corner_is_bounded(0));
        assert!(x.is_int_box());
        assert!(x.is_int_octagon());
        assert!(x.contains_inside(&IntPoint::new(5, 5)));
        assert!(!x.contains_inside(&IntPoint::new(0, 5)));
        assert_eq!(x.bounding_box(), x);
        assert_eq!(x.nearest_part(&b(20, 20, 30, 30)), b(20, 20, 20, 20));
        assert!(IntBox::EMPTY.is_empty());
    }

    #[test]
    fn get_id_wraps_like_java() {
        assert_eq!(IntBox::EMPTY.get_id(), i32::MIN);
    }
}
