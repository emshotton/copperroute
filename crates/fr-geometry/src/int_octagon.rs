use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_point::IntPoint;
use crate::limits::{CRIT_INT, SQRT2, java_max, java_min, java_round};
use crate::line::Line;
use crate::side::Side;
use crate::simplex::Simplex;
use crate::vector::Vector;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntOctagon {
    pub left_x: i32,
    pub bottom_y: i32,
    pub right_x: i32,
    pub top_y: i32,
    pub upper_left_diagonal_x: i32,
    pub lower_right_diagonal_x: i32,
    pub lower_left_diagonal_x: i32,
    pub upper_right_diagonal_x: i32,
}

impl IntOctagon {
    pub const EMPTY: IntOctagon = IntOctagon {
        left_x: CRIT_INT,
        bottom_y: CRIT_INT,
        right_x: -CRIT_INT,
        top_y: -CRIT_INT,
        upper_left_diagonal_x: CRIT_INT,
        lower_right_diagonal_x: -CRIT_INT,
        lower_left_diagonal_x: CRIT_INT,
        upper_right_diagonal_x: -CRIT_INT,
    };

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        left_x: i32,
        bottom_y: i32,
        right_x: i32,
        top_y: i32,
        upper_left_diagonal_x: i32,
        lower_right_diagonal_x: i32,
        lower_left_diagonal_x: i32,
        upper_right_diagonal_x: i32,
    ) -> IntOctagon {
        IntOctagon {
            left_x,
            bottom_y,
            right_x,
            top_y,
            upper_left_diagonal_x,
            lower_right_diagonal_x,
            lower_left_diagonal_x,
            upper_right_diagonal_x,
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == IntOctagon::EMPTY
    }

    pub fn is_int_octagon(&self) -> bool {
        true
    }

    pub fn is_bounded(&self) -> bool {
        true
    }

    pub fn corner_is_bounded(&self, _no: usize) -> bool {
        true
    }

    pub fn bounding_box(&self) -> IntBox {
        IntBox::from_coords(self.left_x, self.bottom_y, self.right_x, self.top_y)
    }

    pub fn bounding_octagon(&self) -> IntOctagon {
        *self
    }

    pub fn dimension(&self) -> i32 {
        if self.is_empty() {
            return -1;
        }
        if self.right_x > self.left_x
            && self.top_y > self.bottom_y
            && self.lower_right_diagonal_x > self.upper_left_diagonal_x
            && self.upper_right_diagonal_x > self.lower_left_diagonal_x
        {
            2
        } else if self.right_x == self.left_x && self.top_y == self.bottom_y {
            0
        } else {
            1
        }
    }

    pub fn corner(&self, no: usize) -> IntPoint {
        match no {
            0 => IntPoint::new(self.lower_left_diagonal_x - self.bottom_y, self.bottom_y),
            1 => IntPoint::new(self.lower_right_diagonal_x + self.bottom_y, self.bottom_y),
            2 => IntPoint::new(self.right_x, self.right_x - self.lower_right_diagonal_x),
            3 => IntPoint::new(self.right_x, self.upper_right_diagonal_x - self.right_x),
            4 => IntPoint::new(self.upper_right_diagonal_x - self.top_y, self.top_y),
            5 => IntPoint::new(self.upper_left_diagonal_x + self.top_y, self.top_y),
            6 => IntPoint::new(self.left_x, self.left_x - self.upper_left_diagonal_x),
            7 => IntPoint::new(self.left_x, self.lower_left_diagonal_x - self.left_x),
            _ => panic!("IntOctagon.corner: no out of range"),
        }
    }

    pub fn get_id(&self) -> i32 {
        let mut result = self.left_x;
        result = 31i32.wrapping_mul(result).wrapping_add(self.right_x);
        result = 31i32.wrapping_mul(result).wrapping_add(self.bottom_y);
        result = 31i32.wrapping_mul(result).wrapping_add(self.top_y);
        result = 31i32
            .wrapping_mul(result)
            .wrapping_add(self.lower_left_diagonal_x);
        result = 31i32
            .wrapping_mul(result)
            .wrapping_add(self.upper_right_diagonal_x);
        result = 31i32
            .wrapping_mul(result)
            .wrapping_add(self.upper_left_diagonal_x);
        31i32
            .wrapping_mul(result)
            .wrapping_add(self.lower_right_diagonal_x)
    }

    pub fn corner_y(&self, no: usize) -> i32 {
        match no {
            0 | 1 => self.bottom_y,
            2 => self.right_x - self.lower_right_diagonal_x,
            3 => self.upper_right_diagonal_x - self.right_x,
            4 | 5 => self.top_y,
            6 => self.left_x - self.upper_left_diagonal_x,
            7 => self.lower_left_diagonal_x - self.left_x,
            _ => panic!("IntOctagon.corner: no out of range"),
        }
    }

    pub fn corner_x(&self, no: usize) -> i32 {
        match no {
            0 => self.lower_left_diagonal_x - self.bottom_y,
            1 => self.lower_right_diagonal_x + self.bottom_y,
            2 | 3 => self.right_x,
            4 => self.upper_right_diagonal_x - self.top_y,
            5 => self.upper_left_diagonal_x + self.top_y,
            6 | 7 => self.left_x,
            _ => panic!("IntOctagon.corner: no out of range"),
        }
    }

    pub fn area(&self) -> f64 {
        let mut result = (self.lower_left_diagonal_x - self.bottom_y) as f64
            * (self.bottom_y - self.lower_left_diagonal_x + self.left_x) as f64;
        result += (self.lower_right_diagonal_x + self.bottom_y) as f64
            * (self.right_x - self.lower_right_diagonal_x - self.bottom_y) as f64;
        result += self.right_x as f64
            * (self.upper_right_diagonal_x - 2 * self.right_x - self.bottom_y
                + self.top_y
                + self.lower_right_diagonal_x) as f64;
        result += (self.upper_right_diagonal_x - self.top_y) as f64
            * (self.top_y - self.upper_right_diagonal_x + self.right_x) as f64;
        result += (self.upper_left_diagonal_x + self.top_y) as f64
            * (self.left_x - self.upper_left_diagonal_x - self.top_y) as f64;
        result += self.left_x as f64
            * (self.lower_left_diagonal_x - 2 * self.left_x - self.top_y
                + self.bottom_y
                + self.upper_left_diagonal_x) as f64;

        0.5 * result.abs()
    }

    pub fn border_line_count(&self) -> usize {
        8
    }

    pub fn border_line(&self, no: usize) -> Line {
        match no {
            0 => Line::from_coords(0, self.bottom_y, 1, self.bottom_y),
            1 => Line::from_coords(
                self.lower_right_diagonal_x,
                0,
                self.lower_right_diagonal_x + 1,
                1,
            ),
            2 => Line::from_coords(self.right_x, 0, self.right_x, 1),
            3 => Line::from_coords(
                self.upper_right_diagonal_x,
                0,
                self.upper_right_diagonal_x - 1,
                1,
            ),
            4 => Line::from_coords(0, self.top_y, -1, self.top_y),
            5 => Line::from_coords(
                self.upper_left_diagonal_x,
                0,
                self.upper_left_diagonal_x - 1,
                -1,
            ),
            6 => Line::from_coords(self.left_x, 0, self.left_x, -1),
            7 => Line::from_coords(
                self.lower_left_diagonal_x,
                0,
                self.lower_left_diagonal_x + 1,
                -1,
            ),
            _ => panic!("IntOctagon.borderLine: no out of range"),
        }
    }

    pub fn translate_by(&self, rel_coor: &Vector) -> IntOctagon {
        if *rel_coor == Vector::ZERO {
            return *self;
        }
        let v = match rel_coor {
            Vector::Int(v) => v,
            Vector::Rational(_) => panic!(
                "IntOctagon::translate_by: only implemented for Vector::Int (IntOctagon.java:258-277)"
            ),
        };
        IntOctagon::new(
            self.left_x + v.x,
            self.bottom_y + v.y,
            self.right_x + v.x,
            self.top_y + v.y,
            self.upper_left_diagonal_x + v.x - v.y,
            self.lower_right_diagonal_x + v.x - v.y,
            self.lower_left_diagonal_x + v.x + v.y,
            self.upper_right_diagonal_x + v.x + v.y,
        )
    }

    pub fn max_width(&self) -> f64 {
        let width1 = (self.right_x - self.left_x).max(self.top_y - self.bottom_y) as f64;
        let width2 = (self.upper_right_diagonal_x - self.lower_left_diagonal_x)
            .max(self.lower_right_diagonal_x - self.upper_left_diagonal_x)
            as f64;
        java_max(width1, width2 / SQRT2)
    }

    pub fn min_width(&self) -> f64 {
        let width1 = (self.right_x - self.left_x).min(self.top_y - self.bottom_y) as f64;
        let width2 = (self.upper_right_diagonal_x - self.lower_left_diagonal_x)
            .min(self.lower_right_diagonal_x - self.upper_left_diagonal_x)
            as f64;
        java_min(width1, width2 / SQRT2)
    }

    pub fn offset(&self, distance: f64) -> IntOctagon {
        let width = java_round(distance) as i32;
        if width == 0 {
            return *self;
        }
        let dia_width = java_round(SQRT2 * distance) as i32;
        let result = IntOctagon::new(
            self.left_x - width,
            self.bottom_y - width,
            self.right_x + width,
            self.top_y + width,
            self.upper_left_diagonal_x - dia_width,
            self.lower_right_diagonal_x + dia_width,
            self.lower_left_diagonal_x - dia_width,
            self.upper_right_diagonal_x + dia_width,
        );
        result.normalize()
    }

    pub fn enlarge(&self, offset: f64) -> IntOctagon {
        self.offset(offset)
    }

    pub fn contains_float(&self, point: &FloatPoint) -> bool {
        if self.left_x as f64 > point.x
            || self.bottom_y as f64 > point.y
            || (self.right_x as f64) < point.x
            || (self.top_y as f64) < point.y
        {
            return false;
        }
        let tmp1 = point.x - point.y;
        let tmp2 = point.x + point.y;
        self.upper_left_diagonal_x as f64 <= tmp1
            && self.lower_right_diagonal_x as f64 >= tmp1
            && self.lower_left_diagonal_x as f64 <= tmp2
            && self.upper_right_diagonal_x as f64 >= tmp2
    }

    pub fn union(&self, other: &IntOctagon) -> IntOctagon {
        IntOctagon::new(
            self.left_x.min(other.left_x),
            self.bottom_y.min(other.bottom_y),
            self.right_x.max(other.right_x),
            self.top_y.max(other.top_y),
            self.upper_left_diagonal_x.min(other.upper_left_diagonal_x),
            self.lower_right_diagonal_x
                .max(other.lower_right_diagonal_x),
            self.lower_left_diagonal_x.min(other.lower_left_diagonal_x),
            self.upper_right_diagonal_x
                .max(other.upper_right_diagonal_x),
        )
    }

    pub fn union_box(&self, other: &IntBox) -> IntOctagon {
        self.union(&other.to_int_octagon())
    }

    pub fn intersection(&self, other: &IntOctagon) -> IntOctagon {
        let result = IntOctagon::new(
            self.left_x.max(other.left_x),
            self.bottom_y.max(other.bottom_y),
            self.right_x.min(other.right_x),
            self.top_y.min(other.top_y),
            self.upper_left_diagonal_x.max(other.upper_left_diagonal_x),
            self.lower_right_diagonal_x
                .min(other.lower_right_diagonal_x),
            self.lower_left_diagonal_x.max(other.lower_left_diagonal_x),
            self.upper_right_diagonal_x
                .min(other.upper_right_diagonal_x),
        );
        result.normalize()
    }

    pub fn intersection_box(&self, other: &IntBox) -> IntOctagon {
        self.intersection(&other.to_int_octagon())
    }

    pub fn normalize(&self) -> IntOctagon {
        if self.left_x > self.right_x
            || self.bottom_y > self.top_y
            || self.lower_left_diagonal_x > self.upper_right_diagonal_x
            || self.upper_left_diagonal_x > self.lower_right_diagonal_x
        {
            return IntOctagon::EMPTY;
        }
        let mut new_lx = self.left_x;
        let mut new_rx = self.right_x;
        let mut new_ly = self.bottom_y;
        let mut new_uy = self.top_y;
        let mut new_llx = self.lower_left_diagonal_x;
        let mut new_ulx = self.upper_left_diagonal_x;
        let mut new_lrx = self.lower_right_diagonal_x;
        let mut new_urx = self.upper_right_diagonal_x;

        if new_lx < new_llx - new_uy {
            new_lx = new_llx - new_uy;
        }

        if new_lx < new_ulx + new_ly {
            new_lx = new_ulx + new_ly;
        }

        if new_rx > new_urx - new_ly {
            new_rx = new_urx - new_ly;
        }

        if new_rx > new_lrx + new_uy {
            new_rx = new_lrx + new_uy;
        }

        if new_ly < new_lx - new_lrx {
            new_ly = new_lx - new_lrx;
        }

        if new_ly < new_llx - new_rx {
            new_ly = new_llx - new_rx;
        }

        if new_uy > new_urx - new_lx {
            new_uy = new_urx - new_lx;
        }

        if new_uy > new_rx - new_ulx {
            new_uy = new_rx - new_ulx;
        }

        if new_llx - new_lx < new_ly {
            new_llx = new_lx + new_ly;
        }

        if new_rx - new_lrx < new_ly {
            new_lrx = new_rx - new_ly;
        }

        if new_urx - new_rx > new_uy {
            new_urx = new_uy + new_rx;
        }

        if new_lx - new_ulx > new_uy {
            new_ulx = new_lx - new_uy;
        }

        let diag_upper_y = (((new_urx - new_ulx) as f64) / 2.0).ceil() as i32;

        if new_uy > diag_upper_y {
            new_uy = diag_upper_y;
        }

        let diag_lower_y = (((new_llx - new_lrx) as f64) / 2.0).floor() as i32;

        if new_ly < diag_lower_y {
            new_ly = diag_lower_y;
        }

        let diag_right_x = (((new_urx + new_lrx) as f64) / 2.0).ceil() as i32;

        if new_rx > diag_right_x {
            new_rx = diag_right_x;
        }

        let diag_left_x = (((new_llx + new_ulx) as f64) / 2.0).floor() as i32;

        if new_lx < diag_left_x {
            new_lx = diag_left_x;
        }
        if new_lx > new_rx || new_ly > new_uy || new_llx > new_urx || new_ulx > new_lrx {
            return IntOctagon::EMPTY;
        }
        IntOctagon::new(
            new_lx, new_ly, new_rx, new_uy, new_ulx, new_lrx, new_llx, new_urx,
        )
    }

    pub fn is_normalized(&self) -> bool {
        *self == self.normalize()
    }

    pub fn side_of_border_line(&self, x: i32, y: i32, border_line_no: usize) -> Side {
        debug_assert!(
            border_line_no < 8,
            "IntOctagon.sideOfBorderLine: borderLineNo out of range"
        );
        let tmp = match border_line_no {
            0 => self.bottom_y - y,
            1 => x - y - self.lower_right_diagonal_x,
            2 => x - self.right_x,
            3 => x + y - self.upper_right_diagonal_x,
            4 => y - self.top_y,
            5 => self.upper_left_diagonal_x + y - x,
            6 => self.left_x - x,
            7 => self.lower_left_diagonal_x - x - y,
            _ => 0,
        };
        if tmp < 0 {
            Side::OnTheLeft
        } else if tmp > 0 {
            Side::OnTheRight
        } else {
            Side::Collinear
        }
    }

    pub fn is_contained_in(&self, box_: &IntBox) -> bool {
        self.left_x >= box_.ll.x
            && self.bottom_y >= box_.ll.y
            && self.right_x <= box_.ur.x
            && self.top_y <= box_.ur.y
    }

    pub fn is_contained_in_octagon(&self, other: &IntOctagon) -> bool {
        self.left_x >= other.left_x
            && self.bottom_y >= other.bottom_y
            && self.right_x <= other.right_x
            && self.top_y <= other.top_y
            && self.lower_left_diagonal_x >= other.lower_left_diagonal_x
            && self.upper_left_diagonal_x >= other.upper_left_diagonal_x
            && self.lower_right_diagonal_x <= other.lower_right_diagonal_x
            && self.upper_right_diagonal_x <= other.upper_right_diagonal_x
    }

    pub fn intersects_box(&self, other: &IntBox) -> bool {
        self.intersects_octagon(&other.to_int_octagon())
    }

    pub fn intersects_octagon(&self, other: &IntOctagon) -> bool {
        let is_lx = other.left_x.max(self.left_x);
        let is_rx = other.right_x.min(self.right_x);
        if is_lx > is_rx {
            return false;
        }

        let is_ly = other.bottom_y.max(self.bottom_y);
        let is_uy = other.top_y.min(self.top_y);
        if is_ly > is_uy {
            return false;
        }

        let is_llx = other.lower_left_diagonal_x.max(self.lower_left_diagonal_x);
        let is_urx = other
            .upper_right_diagonal_x
            .min(self.upper_right_diagonal_x);
        if is_llx > is_urx {
            return false;
        }

        let is_ulx = other.upper_left_diagonal_x.max(self.upper_left_diagonal_x);
        let is_lrx = other
            .lower_right_diagonal_x
            .min(self.lower_right_diagonal_x);
        is_ulx <= is_lrx
    }

    pub fn overlaps(&self, other: &IntOctagon) -> bool {
        let is_lx = other.left_x.max(self.left_x);
        let is_rx = other.right_x.min(self.right_x);
        if is_lx >= is_rx {
            return false;
        }

        let is_ly = other.bottom_y.max(self.bottom_y);
        let is_uy = other.top_y.min(self.top_y);
        if is_ly >= is_uy {
            return false;
        }

        let is_llx = other.lower_left_diagonal_x.max(self.lower_left_diagonal_x);
        let is_urx = other
            .upper_right_diagonal_x
            .min(self.upper_right_diagonal_x);
        if is_llx >= is_urx {
            return false;
        }

        let is_ulx = other.upper_left_diagonal_x.max(self.upper_left_diagonal_x);
        let is_lrx = other
            .lower_right_diagonal_x
            .min(self.lower_right_diagonal_x);
        is_ulx < is_lrx
    }

    pub fn left_x_value(&self, y: i32) -> i32 {
        let result = self.left_x.max(self.upper_left_diagonal_x + y);
        result.max(self.lower_left_diagonal_x - y)
    }

    pub fn right_x_value(&self, y: i32) -> i32 {
        let result = self.right_x.min(self.upper_right_diagonal_x - y);
        result.min(self.lower_right_diagonal_x + y)
    }

    pub fn lower_y_value(&self, x: i32) -> i32 {
        let result = self.bottom_y.max(self.lower_left_diagonal_x - x);
        result.max(x - self.lower_right_diagonal_x)
    }

    pub fn upper_y_value(&self, x: i32) -> i32 {
        let result = self.top_y.min(x - self.upper_left_diagonal_x);
        result.min(self.upper_right_diagonal_x - x)
    }

    pub fn compare_octagon(&self, other: &IntOctagon, edge_index: usize) -> Side {
        match edge_index {
            0 => {
                if self.bottom_y > other.bottom_y {
                    Side::OnTheLeft
                } else if self.bottom_y < other.bottom_y {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            1 => {
                if self.lower_right_diagonal_x < other.lower_right_diagonal_x {
                    Side::OnTheLeft
                } else if self.lower_right_diagonal_x > other.lower_right_diagonal_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            2 => {
                if self.right_x < other.right_x {
                    Side::OnTheLeft
                } else if self.right_x > other.right_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            3 => {
                if self.upper_right_diagonal_x < other.upper_right_diagonal_x {
                    Side::OnTheLeft
                } else if self.upper_right_diagonal_x > other.upper_right_diagonal_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            4 => {
                if self.top_y < other.top_y {
                    Side::OnTheLeft
                } else if self.top_y > other.top_y {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            5 => {
                if self.upper_left_diagonal_x > other.upper_left_diagonal_x {
                    Side::OnTheLeft
                } else if self.upper_left_diagonal_x < other.upper_left_diagonal_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            6 => {
                if self.left_x > other.left_x {
                    Side::OnTheLeft
                } else if self.left_x < other.left_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            7 => {
                if self.lower_left_diagonal_x > other.lower_left_diagonal_x {
                    Side::OnTheLeft
                } else if self.lower_left_diagonal_x < other.lower_left_diagonal_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            _ => panic!("IntBox.compare: edgeIndex out of range"),
        }
    }

    pub fn compare_box(&self, other: &IntBox, edge_index: usize) -> Side {
        self.compare_octagon(&other.to_int_octagon(), edge_index)
    }

    pub fn border_line_index(&self, _line: &Line) -> Option<usize> {
        None
    }

    pub fn border_line_side_of(
        &self,
        point: &FloatPoint,
        line_index: usize,
        tolerance: f64,
    ) -> Side {
        debug_assert!(
            line_index < 8,
            "IntOctagon.borderLineSideOf: lineIndex out of range"
        );
        match line_index {
            0 => {
                if point.y > self.bottom_y as f64 + tolerance {
                    Side::OnTheRight
                } else if point.y < self.bottom_y as f64 - tolerance {
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            2 => {
                if point.x < self.right_x as f64 - tolerance {
                    Side::OnTheRight
                } else if point.x > self.right_x as f64 + tolerance {
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            4 => {
                if point.y < self.top_y as f64 - tolerance {
                    Side::OnTheRight
                } else if point.y > self.top_y as f64 + tolerance {
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            6 => {
                if point.x > self.left_x as f64 + tolerance {
                    Side::OnTheRight
                } else if point.x < self.left_x as f64 - tolerance {
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            1 => {
                let tmp = point.y - point.x + self.lower_right_diagonal_x as f64;
                if tmp > tolerance {
                    Side::OnTheRight
                } else if tmp < -tolerance {
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            3 => {
                let tmp = point.x + point.y - self.upper_right_diagonal_x as f64;
                if tmp < -tolerance {
                    Side::OnTheRight
                } else if tmp > tolerance {
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            5 => {
                let tmp = point.y - point.x + self.upper_left_diagonal_x as f64;
                if tmp < -tolerance {
                    Side::OnTheRight
                } else if tmp > tolerance {
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            7 => {
                let tmp = point.x + point.y - self.lower_left_diagonal_x as f64;
                if tmp > tolerance {
                    Side::OnTheRight
                } else if tmp < -tolerance {
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            _ => Side::Collinear,
        }
    }

    pub fn is_int_box(&self) -> bool {
        if self.lower_left_diagonal_x != self.left_x + self.bottom_y {
            return false;
        }
        if self.lower_right_diagonal_x != self.right_x - self.bottom_y {
            return false;
        }
        if self.upper_right_diagonal_x != self.right_x + self.top_y {
            return false;
        }
        self.upper_left_diagonal_x == self.left_x - self.top_y
    }

    #[allow(clippy::too_many_lines)]
    pub fn cutout_from_box(&self, d: &IntBox) -> Vec<IntOctagon> {
        let c = self.intersection_box(d);

        if self.is_empty() || c.dimension() < self.dimension() {
            return vec![d.to_int_octagon()];
        }

        let mut boxes: [IntBox; 4] = [
            IntBox::from_coords(
                d.ll.x,
                c.lower_left_diagonal_x - c.left_x,
                c.left_x,
                c.left_x - c.upper_left_diagonal_x,
            ),
            IntBox::from_coords(
                c.right_x,
                c.right_x - c.lower_right_diagonal_x,
                d.ur.x,
                c.upper_right_diagonal_x - c.right_x,
            ),
            IntBox::from_coords(
                c.lower_left_diagonal_x - c.bottom_y,
                d.ll.y,
                c.lower_right_diagonal_x + c.bottom_y,
                c.bottom_y,
            ),
            IntBox::from_coords(
                c.upper_left_diagonal_x + c.top_y,
                c.top_y,
                c.upper_right_diagonal_x - c.top_y,
                d.ur.y,
            ),
        ];

        let mut octagons: [IntOctagon; 4] = [IntOctagon::EMPTY; 4];

        let mut current_oct = IntOctagon::new(
            d.ll.x,
            boxes[0].ur.y,
            boxes[3].ll.x,
            d.ur.y,
            -CRIT_INT,
            c.upper_left_diagonal_x,
            -CRIT_INT,
            CRIT_INT,
        );
        octagons[0] = current_oct.normalize();

        current_oct = IntOctagon::new(
            d.ll.x,
            d.ll.y,
            boxes[2].ll.x,
            boxes[0].ll.y,
            -CRIT_INT,
            CRIT_INT,
            -CRIT_INT,
            c.lower_left_diagonal_x,
        );
        octagons[1] = current_oct.normalize();

        current_oct = IntOctagon::new(
            boxes[2].ur.x,
            d.ll.y,
            d.ur.x,
            boxes[1].ll.y,
            c.lower_right_diagonal_x,
            CRIT_INT,
            -CRIT_INT,
            CRIT_INT,
        );
        octagons[2] = current_oct.normalize();

        current_oct = IntOctagon::new(
            boxes[3].ur.x,
            boxes[1].ur.y,
            d.ur.x,
            d.ur.y,
            -CRIT_INT,
            CRIT_INT,
            c.upper_right_diagonal_x,
            CRIT_INT,
        );
        octagons[3] = current_oct.normalize();

        let mut b = boxes[0];
        let mut o = octagons[0];
        if b.ur.x - b.ll.x > o.top_y - o.bottom_y {
            boxes[0] = IntBox::from_coords(b.ll.x, b.ll.y, b.ur.x, o.top_y);
            current_oct = IntOctagon::new(
                b.ur.x,
                o.bottom_y,
                o.right_x,
                o.top_y,
                o.upper_left_diagonal_x,
                o.lower_right_diagonal_x,
                o.lower_left_diagonal_x,
                o.upper_right_diagonal_x,
            );
            octagons[0] = current_oct.normalize();
        }

        b = boxes[3];
        o = octagons[0];
        if b.ur.y - b.ll.y > o.right_x - o.left_x {
            boxes[3] = IntBox::from_coords(o.left_x, b.ll.y, b.ur.x, b.ur.y);
            current_oct = IntOctagon::new(
                o.left_x,
                o.bottom_y,
                o.right_x,
                b.ll.y,
                o.upper_left_diagonal_x,
                o.lower_right_diagonal_x,
                o.lower_left_diagonal_x,
                o.upper_right_diagonal_x,
            );
            octagons[0] = current_oct.normalize();
        }
        b = boxes[3];
        o = octagons[3];
        if b.ur.y - b.ll.y > o.right_x - o.left_x {
            boxes[3] = IntBox::from_coords(b.ll.x, b.ll.y, o.right_x, b.ur.y);
            current_oct = IntOctagon::new(
                o.left_x,
                o.bottom_y,
                o.right_x,
                o.top_y,
                o.upper_left_diagonal_x,
                o.lower_right_diagonal_x,
                o.lower_left_diagonal_x,
                o.upper_right_diagonal_x,
            );
            octagons[3] = current_oct.normalize();
        }
        b = boxes[1];
        o = octagons[3];
        if b.ur.x - b.ll.x > o.top_y - o.bottom_y {
            boxes[1] = IntBox::from_coords(b.ll.x, b.ll.y, b.ur.x, o.top_y);
            current_oct = IntOctagon::new(
                o.left_x,
                o.bottom_y,
                b.ll.x,
                o.top_y,
                o.upper_left_diagonal_x,
                o.lower_right_diagonal_x,
                o.lower_left_diagonal_x,
                o.upper_right_diagonal_x,
            );
            octagons[3] = current_oct.normalize();
        }
        b = boxes[1];
        o = octagons[2];
        if b.ur.x - b.ll.x > o.top_y - o.bottom_y {
            boxes[1] = IntBox::from_coords(b.ll.x, o.bottom_y, b.ur.x, b.ur.y);
            current_oct = IntOctagon::new(
                o.left_x,
                o.bottom_y,
                b.ll.x,
                o.top_y,
                o.upper_left_diagonal_x,
                o.lower_right_diagonal_x,
                o.lower_left_diagonal_x,
                o.upper_right_diagonal_x,
            );
            octagons[2] = current_oct.normalize();
        }
        b = boxes[2];
        o = octagons[2];
        if b.ur.y - b.ll.y > o.right_x - o.left_x {
            boxes[2] = IntBox::from_coords(b.ll.x, b.ll.y, o.right_x, b.ur.y);
            current_oct = IntOctagon::new(
                o.left_x,
                b.ur.y,
                o.right_x,
                o.top_y,
                o.upper_left_diagonal_x,
                o.lower_right_diagonal_x,
                o.lower_left_diagonal_x,
                o.upper_right_diagonal_x,
            );
            octagons[2] = current_oct.normalize();
        }
        b = boxes[2];
        o = octagons[1];
        if b.ur.y - b.ll.y > o.right_x - o.left_x {
            boxes[2] = IntBox::from_coords(o.left_x, b.ll.y, b.ur.x, b.ur.y);
            current_oct = IntOctagon::new(
                o.left_x,
                b.ur.y,
                o.right_x,
                o.top_y,
                o.upper_left_diagonal_x,
                o.lower_right_diagonal_x,
                o.lower_left_diagonal_x,
                o.upper_right_diagonal_x,
            );
            octagons[1] = current_oct.normalize();
        }
        b = boxes[0];
        o = octagons[1];
        if b.ur.x - b.ll.x > o.top_y - o.bottom_y {
            boxes[0] = IntBox::from_coords(b.ll.x, o.bottom_y, b.ur.x, b.ur.y);
            current_oct = IntOctagon::new(
                b.ur.x,
                o.bottom_y,
                o.right_x,
                o.top_y,
                o.upper_left_diagonal_x,
                o.lower_right_diagonal_x,
                o.lower_left_diagonal_x,
                o.upper_right_diagonal_x,
            );
            octagons[1] = current_oct.normalize();
        }

        let mut result = Vec::with_capacity(8);
        for current_box in &boxes {
            result.push(current_box.to_int_octagon());
        }
        result.extend_from_slice(&octagons);
        result
    }

    #[allow(clippy::too_many_lines)]
    pub fn cutout_from_octagon(&self, d: &IntOctagon) -> Vec<IntOctagon> {
        let c = self.intersection(d);

        if self.is_empty() || c.dimension() < self.dimension() {
            return vec![*d];
        }

        let mut result: [IntOctagon; 8] = [IntOctagon::EMPTY; 8];

        let mut tmp = c.lower_left_diagonal_x - c.left_x;

        result[0] = IntOctagon::new(
            d.left_x,
            tmp,
            c.left_x,
            c.left_x - c.upper_left_diagonal_x,
            d.upper_left_diagonal_x,
            d.lower_right_diagonal_x,
            d.lower_left_diagonal_x,
            d.upper_right_diagonal_x,
        );

        let mut tmp2 = c.lower_left_diagonal_x - c.bottom_y;

        result[1] = IntOctagon::new(
            d.left_x,
            d.bottom_y,
            tmp2,
            tmp,
            d.upper_left_diagonal_x,
            d.lower_right_diagonal_x,
            d.lower_left_diagonal_x,
            c.lower_left_diagonal_x,
        );

        tmp = c.lower_right_diagonal_x + c.bottom_y;

        result[2] = IntOctagon::new(
            tmp2,
            d.bottom_y,
            tmp,
            c.bottom_y,
            d.upper_left_diagonal_x,
            d.lower_right_diagonal_x,
            d.lower_left_diagonal_x,
            d.upper_right_diagonal_x,
        );

        tmp2 = c.right_x - c.lower_right_diagonal_x;

        result[3] = IntOctagon::new(
            tmp,
            d.bottom_y,
            d.right_x,
            tmp2,
            c.lower_right_diagonal_x,
            d.lower_right_diagonal_x,
            d.lower_left_diagonal_x,
            d.upper_right_diagonal_x,
        );

        tmp = c.upper_right_diagonal_x - c.right_x;

        result[4] = IntOctagon::new(
            c.right_x,
            tmp2,
            d.right_x,
            tmp,
            d.upper_left_diagonal_x,
            d.lower_right_diagonal_x,
            d.lower_left_diagonal_x,
            d.upper_right_diagonal_x,
        );

        tmp2 = c.upper_right_diagonal_x - c.top_y;

        result[5] = IntOctagon::new(
            tmp2,
            tmp,
            d.right_x,
            d.top_y,
            d.upper_left_diagonal_x,
            d.lower_right_diagonal_x,
            c.upper_right_diagonal_x,
            d.upper_right_diagonal_x,
        );

        tmp = c.upper_left_diagonal_x + c.top_y;

        result[6] = IntOctagon::new(
            tmp,
            c.top_y,
            tmp2,
            d.top_y,
            d.upper_left_diagonal_x,
            d.lower_right_diagonal_x,
            d.lower_left_diagonal_x,
            d.upper_right_diagonal_x,
        );

        tmp2 = c.left_x - c.upper_left_diagonal_x;

        result[7] = IntOctagon::new(
            d.left_x,
            tmp2,
            tmp,
            d.top_y,
            d.upper_left_diagonal_x,
            c.upper_left_diagonal_x,
            d.lower_left_diagonal_x,
            d.upper_right_diagonal_x,
        );

        for piece in result.iter_mut() {
            *piece = piece.normalize();
        }

        let mut curr1 = result[0];
        let mut curr2 = result[7];

        if !(curr1.is_empty() || curr2.is_empty())
            && curr1.right_x - curr1.left_x_value(curr1.top_y)
                > curr2.upper_y_value(curr1.right_x) - curr2.bottom_y
        {
            curr1 = IntOctagon::new(
                curr1.left_x.min(curr2.left_x),
                curr1.bottom_y,
                curr1.right_x,
                curr2.top_y,
                curr2.upper_left_diagonal_x,
                curr1.lower_right_diagonal_x,
                curr1.lower_left_diagonal_x,
                curr2.upper_right_diagonal_x,
            );

            curr2 = IntOctagon::new(
                curr1.right_x,
                curr2.bottom_y,
                curr2.right_x,
                curr2.top_y,
                curr2.upper_left_diagonal_x,
                curr2.lower_right_diagonal_x,
                curr2.lower_left_diagonal_x,
                curr2.upper_right_diagonal_x,
            );

            result[0] = curr1.normalize();
            result[7] = curr2.normalize();
        }
        curr1 = result[7];
        curr2 = result[6];
        if !(curr1.is_empty() || curr2.is_empty())
            && curr2.upper_y_value(curr1.right_x) - curr2.bottom_y
                > curr1.right_x - curr1.left_x_value(curr2.bottom_y)
        {
            curr2 = IntOctagon::new(
                curr1.left_x,
                curr2.bottom_y,
                curr2.right_x,
                curr2.top_y.max(curr1.top_y),
                curr1.upper_left_diagonal_x,
                curr2.lower_right_diagonal_x,
                curr1.lower_left_diagonal_x,
                curr2.upper_right_diagonal_x,
            );

            curr1 = IntOctagon::new(
                curr1.left_x,
                curr1.bottom_y,
                curr1.right_x,
                curr2.bottom_y,
                curr1.upper_left_diagonal_x,
                curr1.lower_right_diagonal_x,
                curr1.lower_left_diagonal_x,
                curr1.upper_right_diagonal_x,
            );

            result[7] = curr1.normalize();
            result[6] = curr2.normalize();
        }
        curr1 = result[6];
        curr2 = result[5];
        if !(curr1.is_empty() || curr2.is_empty())
            && curr2.upper_y_value(curr1.right_x) - curr1.bottom_y
                > curr2.right_x_value(curr1.bottom_y) - curr2.left_x
        {
            curr1 = IntOctagon::new(
                curr1.left_x,
                curr1.bottom_y,
                curr2.right_x,
                curr2.top_y.max(curr1.top_y),
                curr1.upper_left_diagonal_x,
                curr2.lower_right_diagonal_x,
                curr1.lower_left_diagonal_x,
                curr2.upper_right_diagonal_x,
            );

            curr2 = IntOctagon::new(
                curr2.left_x,
                curr2.bottom_y,
                curr2.right_x,
                curr1.bottom_y,
                curr2.upper_left_diagonal_x,
                curr2.lower_right_diagonal_x,
                curr2.lower_left_diagonal_x,
                curr2.upper_right_diagonal_x,
            );

            result[6] = curr1.normalize();
            result[5] = curr2.normalize();
        }
        curr1 = result[5];
        curr2 = result[4];
        if !(curr1.is_empty() || curr2.is_empty())
            && curr2.right_x_value(curr2.top_y) - curr2.left_x
                > curr1.upper_y_value(curr2.left_x) - curr2.top_y
        {
            curr2 = IntOctagon::new(
                curr2.left_x,
                curr2.bottom_y,
                curr2.right_x.max(curr1.right_x),
                curr1.top_y,
                curr1.upper_left_diagonal_x,
                curr2.lower_right_diagonal_x,
                curr2.lower_left_diagonal_x,
                curr1.upper_right_diagonal_x,
            );

            curr1 = IntOctagon::new(
                curr1.left_x,
                curr1.bottom_y,
                curr2.left_x,
                curr1.top_y,
                curr1.upper_left_diagonal_x,
                curr1.lower_right_diagonal_x,
                curr1.lower_left_diagonal_x,
                curr1.upper_right_diagonal_x,
            );

            result[5] = curr1.normalize();
            result[4] = curr2.normalize();
        }
        curr1 = result[4];
        curr2 = result[3];
        if !(curr1.is_empty() || curr2.is_empty())
            && curr1.right_x_value(curr1.bottom_y) - curr1.left_x
                > curr1.bottom_y - curr2.lower_y_value(curr1.left_x)
        {
            curr1 = IntOctagon::new(
                curr1.left_x,
                curr2.bottom_y,
                curr2.right_x.max(curr1.right_x),
                curr1.top_y,
                curr1.upper_left_diagonal_x,
                curr2.lower_right_diagonal_x,
                curr2.lower_left_diagonal_x,
                curr1.upper_right_diagonal_x,
            );

            curr2 = IntOctagon::new(
                curr2.left_x,
                curr2.bottom_y,
                curr1.left_x,
                curr2.top_y,
                curr2.upper_left_diagonal_x,
                curr2.lower_right_diagonal_x,
                curr2.lower_left_diagonal_x,
                curr2.upper_right_diagonal_x,
            );

            result[4] = curr1.normalize();
            result[3] = curr2.normalize();
        }

        curr1 = result[3];
        curr2 = result[2];

        if !(curr1.is_empty() || curr2.is_empty())
            && curr2.top_y - curr2.lower_y_value(curr2.right_x)
                > curr1.right_x_value(curr2.top_y) - curr2.right_x
        {
            curr2 = IntOctagon::new(
                curr2.left_x,
                curr1.bottom_y.min(curr2.bottom_y),
                curr1.right_x,
                curr2.top_y,
                curr2.upper_left_diagonal_x,
                curr1.lower_right_diagonal_x,
                curr2.lower_left_diagonal_x,
                curr1.upper_right_diagonal_x,
            );

            curr1 = IntOctagon::new(
                curr1.left_x,
                curr2.top_y,
                curr1.right_x,
                curr1.top_y,
                curr1.upper_left_diagonal_x,
                curr1.lower_right_diagonal_x,
                curr1.lower_left_diagonal_x,
                curr1.upper_right_diagonal_x,
            );

            result[3] = curr1.normalize();
            result[2] = curr2.normalize();
        }

        curr1 = result[2];
        curr2 = result[1];

        if !(curr1.is_empty() || curr2.is_empty())
            && curr1.top_y - curr1.lower_y_value(curr1.left_x)
                > curr1.left_x - curr2.left_x_value(curr1.top_y)
        {
            curr1 = IntOctagon::new(
                curr2.left_x,
                curr1.bottom_y.min(curr2.bottom_y),
                curr1.right_x,
                curr1.top_y,
                curr2.upper_left_diagonal_x,
                curr1.lower_right_diagonal_x,
                curr2.lower_left_diagonal_x,
                curr1.upper_right_diagonal_x,
            );

            curr2 = IntOctagon::new(
                curr2.left_x,
                curr1.top_y,
                curr2.right_x,
                curr2.top_y,
                curr2.upper_left_diagonal_x,
                curr2.lower_right_diagonal_x,
                curr2.lower_left_diagonal_x,
                curr2.upper_right_diagonal_x,
            );

            result[2] = curr1.normalize();
            result[1] = curr2.normalize();
        }

        curr1 = result[1];
        curr2 = result[0];

        if !(curr1.is_empty() || curr2.is_empty())
            && curr2.right_x - curr2.left_x_value(curr2.bottom_y)
                > curr2.bottom_y - curr1.lower_y_value(curr2.right_x)
        {
            curr2 = IntOctagon::new(
                curr2.left_x.min(curr1.left_x),
                curr1.bottom_y,
                curr2.right_x,
                curr2.top_y,
                curr2.upper_left_diagonal_x,
                curr1.lower_right_diagonal_x,
                curr1.lower_left_diagonal_x,
                curr2.upper_right_diagonal_x,
            );

            curr1 = IntOctagon::new(
                curr2.right_x,
                curr1.bottom_y,
                curr1.right_x,
                curr1.top_y,
                curr1.upper_left_diagonal_x,
                curr1.lower_right_diagonal_x,
                curr1.lower_left_diagonal_x,
                curr1.upper_right_diagonal_x,
            );

            result[1] = curr1.normalize();
            result[0] = curr2.normalize();
        }

        result.to_vec()
    }

    pub fn to_simplex(&self) -> Simplex {
        if self.is_empty() {
            return Simplex::EMPTY;
        }
        let lines: Vec<Line> = (0..8).map(|i| self.border_line(i)).collect();
        Simplex::new(lines).remove_redundant_lines()
    }

    pub fn intersection_simplex(&self, other: &Simplex) -> Simplex {
        other.intersection_octagon(self)
    }

    pub fn intersects_simplex(&self, other: &Simplex) -> bool {
        other.intersects_octagon(self)
    }

    pub fn cutout_from_simplex(&self, simplex: &Simplex) -> Option<Vec<Simplex>> {
        self.to_simplex().cutout_from(simplex)
    }
}

impl fmt::Display for IntOctagon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IntOctagon(leftX={}, bottomY={}, rightX={}, topY={}, upperLeftDiagonalX={}, \
             lowerRightDiagonalX={}, lowerLeftDiagonalX={}, upperRightDiagonalX={})",
            self.left_x,
            self.bottom_y,
            self.right_x,
            self.top_y,
            self.upper_left_diagonal_x,
            self.lower_right_diagonal_x,
            self.lower_left_diagonal_x,
            self.upper_right_diagonal_x
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_point::FloatPoint;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;

    fn from_box(b: IntBox) -> IntOctagon {
        b.to_int_octagon()
    }

    #[test]
    fn surrounding_octagon_of_point() {
        assert_eq!(
            IntPoint::new(3, 5).surrounding_octagon(),
            IntOctagon::new(3, 5, 3, 5, -2, -2, 8, 8)
        );
    }

    #[test]
    fn box_conversion_and_normalize_roundtrip() {
        let b = IntBox::from_coords(0, 0, 10, 10);
        let o = from_box(b);
        assert_eq!(o.left_x, 0);
        assert_eq!(o.right_x, 10);
        assert_eq!(o.upper_left_diagonal_x, -10);
        assert_eq!(o.lower_right_diagonal_x, 10);
        assert_eq!(o.lower_left_diagonal_x, 0);
        assert_eq!(o.upper_right_diagonal_x, 20);
        assert!(o.is_normalized());
        assert!(o.is_int_box());
        assert_eq!(o.bounding_box(), b);
        assert_eq!(o.area(), 100.0);
        assert_eq!(o.border_line_count(), 8);
    }

    #[test]
    fn normalize_tightens_loose_diagonals() {
        let loose = IntOctagon::new(0, 0, 10, 10, -50, 50, -50, 50);
        assert!(!loose.is_normalized());
        assert_eq!(
            loose.normalize(),
            from_box(IntBox::from_coords(0, 0, 10, 10))
        );
    }

    #[test]
    fn diamond_octagon() {
        let d = IntOctagon::new(-10, -10, 10, 10, -10, 10, -10, 10).normalize();
        assert!(d.contains_float(&FloatPoint::new(4.0, 4.0)));
        assert!(!d.contains_float(&FloatPoint::new(8.0, 8.0)));
        assert_eq!(d.area(), 200.0);
        assert!(!d.is_int_box());
        assert_eq!(d.left_x_value(0), -10);
        assert_eq!(d.left_x_value(5), -5);
        assert_eq!(d.upper_y_value(5), 5);
    }

    #[test]
    fn union_intersection_and_containment() {
        let a = from_box(IntBox::from_coords(0, 0, 10, 10));
        let b = from_box(IntBox::from_coords(5, 5, 20, 20));
        assert_eq!(
            a.intersection(&b),
            from_box(IntBox::from_coords(5, 5, 10, 10))
        );
        assert_eq!(
            a.union(&b).bounding_box(),
            IntBox::from_coords(0, 0, 20, 20)
        );
        assert!(a.intersects_octagon(&b));
        assert!(a.intersects_box(&IntBox::from_coords(10, 10, 12, 12)));
        assert!(!a.overlaps(&from_box(IntBox::from_coords(10, 10, 12, 12))));
        assert!(from_box(IntBox::from_coords(1, 1, 2, 2)).is_contained_in_octagon(&a));
        assert!(
            a.intersection(&from_box(IntBox::from_coords(50, 50, 60, 60)))
                .is_empty()
        );
    }

    #[test]
    fn corners_and_border_lines_are_consistent() {
        let d = IntOctagon::new(-10, -10, 10, 10, -10, 10, -10, 10).normalize();
        for i in 0..8 {
            let line = d.border_line(i);
            assert_eq!(d.border_line_index(&line), None);
            for j in 0..8 {
                let c = crate::point::Point::Int(d.corner(j));
                assert_ne!(
                    line.side_of(&c),
                    crate::Side::OnTheLeft,
                    "corner {j} outside line {i}"
                );
            }
        }
    }

    #[test]
    fn offset_and_translate() {
        let a = from_box(IntBox::from_coords(0, 0, 10, 10));
        let grown = a.offset(2.0);
        assert_eq!(grown.bounding_box(), IntBox::from_coords(-2, -2, 12, 12));
        assert!(a.is_contained_in_octagon(&grown));
        let moved = a.translate_by(&crate::vector::Vector::new(3, 4));
        assert_eq!(moved.bounding_box(), IntBox::from_coords(3, 4, 13, 14));
    }

    #[test]
    fn cutout_pieces_cover_difference() {
        let outer = from_box(IntBox::from_coords(0, 0, 20, 20));
        let inner = IntOctagon::new(5, 5, 15, 15, -5, 5, 15, 25).normalize();
        let pieces = inner.cutout_from_octagon(&outer);
        assert!(!pieces.is_empty());
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert!((total - (outer.area() - inner.intersection(&outer).area())).abs() < 1e-6);
        for p in &pieces {
            assert!(!p.overlaps(&inner));
            assert!(p.is_contained_in_octagon(&outer));
        }
    }

    #[test]
    fn empty_and_dimension() {
        assert!(IntOctagon::EMPTY.is_empty());
        assert_eq!(IntOctagon::EMPTY.dimension(), -1);
        assert_eq!(IntOctagon::new(3, 3, 3, 3, 0, 0, 6, 6).dimension(), 0);
        assert_eq!(IntOctagon::new(3, 3, 9, 3, 0, 6, 6, 12).dimension(), 1);
        assert_eq!(from_box(IntBox::from_coords(0, 0, 10, 10)).dimension(), 2);
        assert!(IntOctagon::EMPTY.is_bounded());
        assert!(IntOctagon::EMPTY.corner_is_bounded(0));
        assert!(IntOctagon::EMPTY.is_int_octagon());
    }

    #[test]
    fn corner_accessors_agree_with_corner() {
        let d = IntOctagon::new(-10, -10, 10, 10, -10, 10, -10, 10).normalize();
        for i in 0..8 {
            assert_eq!(d.corner(i), IntPoint::new(d.corner_x(i), d.corner_y(i)));
        }
    }

    #[test]
    fn get_id_wraps_like_java() {
        assert_eq!(IntOctagon::EMPTY.get_id(), -268_435_456);
    }

    #[test]
    fn widths_and_enlarge() {
        let a = from_box(IntBox::from_coords(0, 0, 10, 4));
        assert_eq!(a.max_width(), 10.0);
        assert_eq!(a.min_width(), 4.0);
        assert_eq!(a.enlarge(3.0), a.offset(3.0));
        let loose = IntOctagon::new(0, 0, 10, 10, -50, 50, -50, 50);
        assert_eq!(loose.offset(0.0), loose);
    }

    #[test]
    fn side_of_border_line_puts_interior_on_the_left() {
        let d = IntOctagon::new(-10, -10, 10, 10, -10, 10, -10, 10).normalize();
        for i in 0..8 {
            assert_eq!(d.side_of_border_line(0, 0, i), Side::OnTheLeft);
        }
        assert_eq!(d.side_of_border_line(0, -10, 0), Side::Collinear);
        assert_eq!(d.side_of_border_line(0, -11, 0), Side::OnTheRight);
    }

    #[test]
    fn border_line_side_of_uses_a_tolerance() {
        let d = IntOctagon::new(-10, -10, 10, 10, -10, 10, -10, 10).normalize();
        assert_eq!(
            d.border_line_side_of(&FloatPoint::new(0.0, 0.0), 0, 0.0),
            Side::OnTheRight
        );
        assert_eq!(
            d.border_line_side_of(&FloatPoint::new(0.0, -10.0), 0, 0.5),
            Side::Collinear
        );
        assert_eq!(
            d.border_line_side_of(&FloatPoint::new(0.0, -11.0), 0, 0.5),
            Side::OnTheLeft
        );
        assert_eq!(
            d.border_line_side_of(&FloatPoint::new(0.0, 10.0), 3, 0.0),
            Side::Collinear
        );
        assert_eq!(
            d.border_line_side_of(&FloatPoint::new(0.0, 0.0), 3, 0.0),
            Side::OnTheRight
        );
    }

    #[test]
    fn compare_and_boxes() {
        let a = from_box(IntBox::from_coords(0, 0, 10, 10));
        let b = from_box(IntBox::from_coords(1, 1, 10, 10));
        assert_eq!(a.compare_octagon(&b, 0), Side::OnTheRight);
        assert_eq!(a.compare_octagon(&b, 6), Side::OnTheRight);
        assert_eq!(a.compare_octagon(&a, 2), Side::Collinear);
        assert_eq!(
            a.compare_box(&IntBox::from_coords(1, 1, 10, 10), 0),
            Side::OnTheRight
        );
        assert_eq!(a.bounding_octagon(), a);
        assert!(a.is_contained_in(&IntBox::from_coords(-1, -1, 11, 11)));
        assert!(!a.is_contained_in(&IntBox::from_coords(1, 1, 11, 11)));
        let big = IntBox::from_coords(5, 5, 20, 20);
        assert_eq!(a.union_box(&big), a.union(&from_box(big)));
        assert_eq!(a.union_box(&big), big.union_octagon(&a));
        assert_eq!(
            a.intersection_box(&IntBox::from_coords(5, 5, 20, 20)),
            from_box(IntBox::from_coords(5, 5, 10, 10))
        );
    }

    #[test]
    fn x_and_y_values_on_the_diamond() {
        let d = IntOctagon::new(-10, -10, 10, 10, -10, 10, -10, 10).normalize();
        assert_eq!(d.right_x_value(0), 10);
        assert_eq!(d.right_x_value(5), 5);
        assert_eq!(d.lower_y_value(5), -5);
        assert_eq!(d.lower_y_value(0), -10);
    }

    #[test]
    fn cutout_from_box_covers_the_difference() {
        let outer = IntBox::from_coords(0, 0, 20, 20);
        let inner = IntOctagon::new(5, 5, 15, 15, -5, 5, 15, 25).normalize();
        let pieces = inner.cutout_from_box(&outer);
        assert_eq!(pieces.len(), 8);
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        let expected = outer.area() - inner.intersection_box(&outer).area();
        assert!((total - expected).abs() < 1e-6, "{total} vs {expected}");
        for p in &pieces {
            assert!(!p.overlaps(&inner));
            assert!(p.is_contained_in(&outer));
        }
    }

    #[test]
    fn cutout_of_a_disjoint_shape_returns_the_original() {
        let outer = from_box(IntBox::from_coords(0, 0, 20, 20));
        let far = from_box(IntBox::from_coords(50, 50, 60, 60));
        assert_eq!(far.cutout_from_octagon(&outer), vec![outer]);
        assert_eq!(
            far.cutout_from_box(&IntBox::from_coords(0, 0, 20, 20)),
            vec![outer]
        );
    }

    #[test]
    fn display_matches_java_to_string() {
        assert_eq!(
            IntOctagon::new(1, 2, 3, 4, 5, 6, 7, 8).to_string(),
            "IntOctagon(leftX=1, bottomY=2, rightX=3, topY=4, upperLeftDiagonalX=5, \
             lowerRightDiagonalX=6, lowerLeftDiagonalX=7, upperRightDiagonalX=8)"
        );
    }
}
