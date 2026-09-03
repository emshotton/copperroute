//! Port of `app.freerouting.geometry.planar.IntOctagon`: octagons with integer coordinates whose
//! eight border lines run at multiples of 45 degrees.
//!
//! Java's `IntOctagon` extends `RegularTileShape` → `TileShape` → `PolylineShape`. This port
//! carries the octagon- and box-typed methods; the `TileShape`/`RegularTileShape`/`Simplex`/
//! `Circle`/`ShapeBoundingDirections`/`FortyfiveDegreeDirection`-typed ones are added in
//! Tasks 13-14 (see the marker at the end of this file).

use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_point::IntPoint;
use crate::limits::{CRIT_INT, SQRT2, java_max, java_min, java_round};
use crate::line::Line;
use crate::side::Side;
use crate::simplex::Simplex;
use crate::vector::Vector;
use std::fmt;

/// Implements functionality of octagons in the plane with integer coordinates and 45 degree
/// angle constraints.
///
/// The four "diagonal" fields are the x-axis intersections of the four diagonal border lines:
/// `upper_left_diagonal_x` / `lower_right_diagonal_x` bound `x - y`, and
/// `lower_left_diagonal_x` / `upper_right_diagonal_x` bound `x + y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntOctagon {
    /// The smallest x value of the shape (x-coordinate of the left vertical border).
    pub left_x: i32,
    /// The smallest y value of the shape (y-coordinate of the bottom horizontal border).
    pub bottom_y: i32,
    /// The biggest x value of the shape (x-coordinate of the right vertical border).
    pub right_x: i32,
    /// The biggest y value of the shape (y-coordinate of the top horizontal border).
    pub top_y: i32,
    /// The intersection of the upper left diagonal boundary line with the x axis.
    pub upper_left_diagonal_x: i32,
    /// The intersection of the lower right diagonal boundary line with the x axis.
    pub lower_right_diagonal_x: i32,
    /// The intersection of the lower left diagonal boundary line with the x axis.
    pub lower_left_diagonal_x: i32,
    /// The intersection of the upper right diagonal boundary line with the x axis.
    pub upper_right_diagonal_x: i32,
}

impl IntOctagon {
    /// Reusable instance of an empty octagon. IntOctagon.java:14-23.
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

    /// Creates an `IntOctagon` from 8 integer boundary values, in Java's parameter order
    /// (IntOctagon.java:72-89).
    ///
    /// * `left_x` — the smallest x value of the shape
    /// * `bottom_y` — the smallest y value of the shape
    /// * `right_x` — the biggest x value of the shape
    /// * `top_y` — the biggest y value of the shape
    /// * `upper_left_diagonal_x` — x-axis intersection of the upper left diagonal boundary line
    /// * `lower_right_diagonal_x` — x-axis intersection of the lower right diagonal boundary line
    /// * `lower_left_diagonal_x` — x-axis intersection of the lower left diagonal boundary line
    /// * `upper_right_diagonal_x` — x-axis intersection of the upper right diagonal boundary line
    #[allow(clippy::too_many_arguments)] // Java's 8-parameter constructor, ported verbatim.
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

    /// Java `isEmpty()` is `this == EMPTY`, i.e. reference identity with the shared `EMPTY`
    /// singleton (IntOctagon.java:91-94). `IntOctagon` is a plain value type here, so this is
    /// ported as value equality: `EMPTY` is the only octagon `normalize()` ever produces with
    /// those eight coordinates, so the two agree in practice while value equality additionally
    /// catches a hand-built copy of `EMPTY` (Java would report `false` for that one).
    pub fn is_empty(&self) -> bool {
        *self == IntOctagon::EMPTY
    }

    /// Java `isIntOctagon()`: always true.
    pub fn is_int_octagon(&self) -> bool {
        true
    }

    /// Java `isBounded()`: always true for an `IntOctagon`.
    pub fn is_bounded(&self) -> bool {
        true
    }

    /// Java `cornerIsBounded(int no)`: always true, `no` unused (as in Java).
    pub fn corner_is_bounded(&self, _no: usize) -> bool {
        true
    }

    /// Returns the smallest `IntBox` containing this octagon.
    #[inline]
    pub fn bounding_box(&self) -> IntBox {
        IntBox::from_coords(self.left_x, self.bottom_y, self.right_x, self.top_y)
    }

    /// Java `boundingOctagon()`: an `IntOctagon` is its own bounding octagon.
    pub fn bounding_octagon(&self) -> IntOctagon {
        *self
    }

    /// Returns the dimension of this octagon: -1 if empty, 0 if a point, 1 if a segment, else 2.
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

    /// Returns the corner with the given number, counterclockwise starting at the lower left
    /// corner of the bottom horizontal border. Panics for `no` outside `0..8` (Java throws
    /// `IllegalArgumentException`).
    pub fn corner(&self, no: usize) -> IntPoint {
        match no {
            // lower-left (bottom horizontal)
            0 => IntPoint::new(self.lower_left_diagonal_x - self.bottom_y, self.bottom_y),
            // lower-right (bottom horizontal)
            1 => IntPoint::new(self.lower_right_diagonal_x + self.bottom_y, self.bottom_y),
            // bottom-right vertical
            2 => IntPoint::new(self.right_x, self.right_x - self.lower_right_diagonal_x),
            // top-right vertical
            3 => IntPoint::new(self.right_x, self.upper_right_diagonal_x - self.right_x),
            // upper-right (top horizontal)
            4 => IntPoint::new(self.upper_right_diagonal_x - self.top_y, self.top_y),
            // upper-left (top horizontal)
            5 => IntPoint::new(self.upper_left_diagonal_x + self.top_y, self.top_y),
            // top-left vertical
            6 => IntPoint::new(self.left_x, self.left_x - self.upper_left_diagonal_x),
            // bottom-left vertical
            7 => IntPoint::new(self.left_x, self.lower_left_diagonal_x - self.left_x),
            _ => panic!("IntOctagon.corner: no out of range"),
        }
    }

    /// Returns a stable identifier for this octagon. Java `int` arithmetic wraps silently on
    /// overflow and this is a hash-shaped value rather than a magnitude, so `wrapping_*`
    /// reproduces it (plain `*`/`+` would panic in a debug build, e.g. for `IntOctagon::EMPTY`).
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

    /// Additional to `corner()` for performance reasons to avoid allocation of an `IntPoint`.
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

    /// Additional to `corner()` for performance reasons to avoid allocation of an `IntPoint`.
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

    /// Returns the area of this octagon.
    ///
    /// Java calculates half of the absolute value of
    /// `x0 (y1 - y7) + x1 (y2 - y0) + x2 (y3 - y1) + ... + x7 (y0 - y6)`, where `xi, yi` are the
    /// coordinates of the i-th corner. It overwrites the same implementation in `TileShape` for
    /// performance reasons to avoid `Point` allocation. Each factor is an `int` expression that
    /// Java widens to `double` only at the multiplication, so the inner arithmetic stays `i32`.
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

    /// Java `borderLineCount()`: an octagon always has 8 border lines.
    pub fn border_line_count(&self) -> usize {
        8
    }

    /// Returns the infinite line coincident with border edge `no`. **Not** the segment from
    /// `corner(no)` to `corner((no + 1) % 8)` — as in `IntBox.borderLine`, Java fixes each line
    /// with two arbitrary points that only pin down its position and direction
    /// (IntOctagon.java:239-256). Panics for `no` outside `0..8`.
    pub fn border_line(&self, no: usize) -> Line {
        match no {
            // lower boundary line
            0 => Line::from_coords(0, self.bottom_y, 1, self.bottom_y),
            // lower right boundary line
            1 => Line::from_coords(
                self.lower_right_diagonal_x,
                0,
                self.lower_right_diagonal_x + 1,
                1,
            ),
            // right boundary line
            2 => Line::from_coords(self.right_x, 0, self.right_x, 1),
            // upper right boundary line
            3 => Line::from_coords(
                self.upper_right_diagonal_x,
                0,
                self.upper_right_diagonal_x - 1,
                1,
            ),
            // upper boundary line
            4 => Line::from_coords(0, self.top_y, -1, self.top_y),
            // upper left boundary line
            5 => Line::from_coords(
                self.upper_left_diagonal_x,
                0,
                self.upper_left_diagonal_x - 1,
                -1,
            ),
            // left boundary line
            6 => Line::from_coords(self.left_x, 0, self.left_x, -1),
            // lower left boundary line
            7 => Line::from_coords(
                self.lower_left_diagonal_x,
                0,
                self.lower_left_diagonal_x + 1,
                -1,
            ),
            _ => panic!("IntOctagon.borderLine: no out of range"),
        }
    }

    /// Returns the translation of this octagon by `rel_coor`.
    ///
    /// Java: "This function is at the moment only implemented for Vectors with integer
    /// coordinates. The general implementation is still missing." (IntOctagon.java:258-277) A
    /// `Vector::Rational` would hit Java's unchecked `(IntVector) relCoor` cast and throw
    /// `ClassCastException`; ported as a panic.
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

    /// Java `maxWidth()`: the two `Math.max` of `int` differences, the diagonal one scaled down
    /// by `sqrt(2)`.
    pub fn max_width(&self) -> f64 {
        let width1 = (self.right_x - self.left_x).max(self.top_y - self.bottom_y) as f64;
        let width2 = (self.upper_right_diagonal_x - self.lower_left_diagonal_x)
            .max(self.lower_right_diagonal_x - self.upper_left_diagonal_x)
            as f64;
        java_max(width1, width2 / SQRT2)
    }

    /// Java `minWidth()`: the two `Math.min` of `int` differences, the diagonal one scaled down
    /// by `sqrt(2)`.
    pub fn min_width(&self) -> f64 {
        let width1 = (self.right_x - self.left_x).min(self.top_y - self.bottom_y) as f64;
        let width2 = (self.upper_right_diagonal_x - self.lower_left_diagonal_x)
            .min(self.lower_right_diagonal_x - self.upper_left_diagonal_x)
            as f64;
        java_min(width1, width2 / SQRT2)
    }

    /// Returns this octagon offsetted by `distance`. If `distance > 0` the offset is to the
    /// outside, else to the inside. The diagonal border lines move by `sqrt(2) * distance`.
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

    /// Java `enlarge(double offset)`: `return offset(offset);`
    pub fn enlarge(&self, offset: f64) -> IntOctagon {
        self.offset(offset)
    }

    /// Returns true if `point` is contained in this octagon. Because of the parameter type
    /// `FloatPoint`, the function may not be exact close to the border.
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

    /// Returns the smallest octagon containing this octagon and `other`.
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

    /// Java `union(IntBox other)`: `return union(other.toIntOctagon());`
    #[inline]
    pub fn union_box(&self, other: &IntBox) -> IntOctagon {
        self.union(&other.to_int_octagon())
    }

    /// Returns the intersection of this octagon with `other`, normalized.
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

    /// Java `intersection(IntBox other)`: `return intersection(other.toIntOctagon());`
    pub fn intersection_box(&self, other: &IntBox) -> IntOctagon {
        self.intersection(&other.to_int_octagon())
    }

    /// Returns an equivalent octagon with all redundant bounds tightened.
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
            // the point new_lx, new_uy is the lower left border line of this octagon
            // change new_lx, that the lower left border line runs through this point
            new_lx = new_llx - new_uy;
        }

        if new_lx < new_ulx + new_ly {
            // the point new_lx, new_ly is above the upper left border line of this octagon
            // change new_lx, that the upper left border line runs through this point
            new_lx = new_ulx + new_ly;
        }

        if new_rx > new_urx - new_ly {
            // the point new_rx, new_ly is above the upper right border line of this octagon
            // change new_rx, that the upper right border line runs through this point
            new_rx = new_urx - new_ly;
        }

        if new_rx > new_lrx + new_uy {
            // the point new_rx, new_uy is below the lower right border line of this octagon
            // change rx, that the lower right border line runs through this point
            new_rx = new_lrx + new_uy;
        }

        if new_ly < new_lx - new_lrx {
            // the point lx, ly is below the lower right border line of this octagon
            // change ly, so that the lower right border line runs through this point
            new_ly = new_lx - new_lrx;
        }

        if new_ly < new_llx - new_rx {
            // the point rx, ly is below the lower left border line of this octagon.
            // change ly, so that the lower left border line runs through this point
            new_ly = new_llx - new_rx;
        }

        if new_uy > new_urx - new_lx {
            // the point lx, uy is above the upper right border line of this octagon.
            // Change the uy, so that the upper right border line runs through this point.
            new_uy = new_urx - new_lx;
        }

        if new_uy > new_rx - new_ulx {
            // the point rx, uy is above the upper left border line of this octagon.
            // Change the uy, so that the upper left border line runs through this point.
            new_uy = new_rx - new_ulx;
        }

        if new_llx - new_lx < new_ly {
            // The point lx, ly is above the lower left border line of this octagon.
            // Change the lower left line, so that it runs through this point.
            new_llx = new_lx + new_ly;
        }

        if new_rx - new_lrx < new_ly {
            // the point rx, ly is above the lower right border line of this octagon.
            // Change the lower right line, so that it runs through this point.
            new_lrx = new_rx - new_ly;
        }

        if new_urx - new_rx > new_uy {
            // the point rx, uy is below the upper right border line of oct.
            // Change the upper right line, so that it runs through this point.
            new_urx = new_uy + new_rx;
        }

        if new_lx - new_ulx > new_uy {
            // the point lx, uy is below the upper left border line of this octagon.
            // Change the upper left line, so that it runs through this point.
            new_ulx = new_lx - new_uy;
        }

        let diag_upper_y = (((new_urx - new_ulx) as f64) / 2.0).ceil() as i32;

        if new_uy > diag_upper_y {
            // the intersection of the upper right and the upper left border line is below
            // new_uy. Adjust new_uy to diag_upper_y.
            new_uy = diag_upper_y;
        }

        let diag_lower_y = (((new_llx - new_lrx) as f64) / 2.0).floor() as i32;

        if new_ly < diag_lower_y {
            // the intersection of the lower right and the lower left border line is above
            // new_ly. Adjust new_ly to diag_lower_y.
            new_ly = diag_lower_y;
        }

        let diag_right_x = (((new_urx + new_lrx) as f64) / 2.0).ceil() as i32;

        if new_rx > diag_right_x {
            // the intersection of the upper right and the lower right border line is to the left
            // of right x. Adjust new_rx to diag_right_x.
            new_rx = diag_right_x;
        }

        let diag_left_x = (((new_llx + new_ulx) as f64) / 2.0).floor() as i32;

        if new_lx < diag_left_x {
            // the intersection of the lower left and the upper left border line is to the right
            // of left x. Adjust new_lx to diag_left_x.
            new_lx = diag_left_x;
        }
        if new_lx > new_rx || new_ly > new_uy || new_llx > new_urx || new_ulx > new_lrx {
            return IntOctagon::EMPTY;
        }
        IntOctagon::new(
            new_lx, new_ly, new_rx, new_uy, new_ulx, new_lrx, new_llx, new_urx,
        )
    }

    /// Checks if this `IntOctagon` is normalized.
    pub fn is_normalized(&self) -> bool {
        *self == self.normalize()
    }

    /// Calculates the side of the point (x, y) of the border line with index `border_line_no`.
    /// The border lines are located in counterclock sense around this octagon.
    ///
    /// Note that this uses the *opposite* sign convention to `border_line(no).side_of(point)`:
    /// here the interior of the octagon is `Side::OnTheLeft`, while `Line::side_of` puts it on
    /// the right (see `TileShape.isOutside`). Both are ported as Java has them.
    ///
    /// Java logs a warning and yields 0 (`Side::Collinear`) for an out-of-range index; ported as
    /// a `debug_assert!` plus the same `Collinear` fallback (`FRLogger` dropped per conventions).
    pub fn side_of_border_line(&self, x: i32, y: i32, border_line_no: usize) -> Side {
        debug_assert!(
            border_line_no < 8,
            "IntOctagon.sideOfBorderLine: borderLineNo out of range"
        );
        let tmp = match border_line_no {
            0 => self.bottom_y - y,                   // lower boundary line
            1 => x - y - self.lower_right_diagonal_x, // lower-right diagonal line
            2 => x - self.right_x,                    // right boundary line
            3 => x + y - self.upper_right_diagonal_x, // upper-right diagonal line
            4 => y - self.top_y,                      // upper boundary line
            5 => self.upper_left_diagonal_x + y - x,  // upper-left diagonal line
            6 => self.left_x - x,                     // left boundary line
            7 => self.lower_left_diagonal_x - x - y,  // lower-left diagonal line
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

    /// Checks if this normalized octagon is contained in `box_`.
    pub fn is_contained_in(&self, box_: &IntBox) -> bool {
        self.left_x >= box_.ll.x
            && self.bottom_y >= box_.ll.y
            && self.right_x <= box_.ur.x
            && self.top_y <= box_.ur.y
    }

    /// Checks if this normalized octagon is contained in `other`.
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

    /// Java `intersects(IntBox other)`: `return intersects(other.toIntOctagon());`
    #[inline]
    pub fn intersects_box(&self, other: &IntBox) -> bool {
        self.intersects_octagon(&other.to_int_octagon())
    }

    /// Checks if two normalized octagons intersect (touching counts as intersecting).
    #[inline]
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

    /// Returns true if this octagon intersects with `other` and the intersection is
    /// 2-dimensional.
    #[inline]
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

    /// Computes the x value of the left boundary of this octagon at `y`.
    pub fn left_x_value(&self, y: i32) -> i32 {
        let result = self.left_x.max(self.upper_left_diagonal_x + y);
        result.max(self.lower_left_diagonal_x - y)
    }

    /// Computes the x value of the right boundary of this octagon at `y`.
    pub fn right_x_value(&self, y: i32) -> i32 {
        let result = self.right_x.min(self.upper_right_diagonal_x - y);
        result.min(self.lower_right_diagonal_x + y)
    }

    /// Computes the y value of the lower boundary of this octagon at `x`.
    pub fn lower_y_value(&self, x: i32) -> i32 {
        let result = self.bottom_y.max(self.lower_left_diagonal_x - x);
        result.max(x - self.lower_right_diagonal_x)
    }

    /// Computes the y value of the upper boundary of this octagon at `x`.
    pub fn upper_y_value(&self, x: i32) -> i32 {
        let result = self.top_y.min(x - self.upper_left_diagonal_x);
        result.min(self.upper_right_diagonal_x - x)
    }

    /// Compares this octagon against `other` along `edge_index` (0: lower, 1: lower right,
    /// 2: right, 3: upper right, 4: upper, 5: upper left, 6: left, 7: lower left). Panics for
    /// `edge_index` outside `0..8` (Java throws `IllegalArgumentException`).
    pub fn compare_octagon(&self, other: &IntOctagon, edge_index: usize) -> Side {
        match edge_index {
            0 => {
                // compare the lower edge line
                if self.bottom_y > other.bottom_y {
                    Side::OnTheLeft
                } else if self.bottom_y < other.bottom_y {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            1 => {
                // compare the lower right edge line
                if self.lower_right_diagonal_x < other.lower_right_diagonal_x {
                    Side::OnTheLeft
                } else if self.lower_right_diagonal_x > other.lower_right_diagonal_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            2 => {
                // compare the right edge line
                if self.right_x < other.right_x {
                    Side::OnTheLeft
                } else if self.right_x > other.right_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            3 => {
                // compare the upper right edge line
                if self.upper_right_diagonal_x < other.upper_right_diagonal_x {
                    Side::OnTheLeft
                } else if self.upper_right_diagonal_x > other.upper_right_diagonal_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            4 => {
                // compare the upper edge line
                if self.top_y < other.top_y {
                    Side::OnTheLeft
                } else if self.top_y > other.top_y {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            5 => {
                // compare the upper left edge line
                if self.upper_left_diagonal_x > other.upper_left_diagonal_x {
                    Side::OnTheLeft
                } else if self.upper_left_diagonal_x < other.upper_left_diagonal_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            6 => {
                // compare the left edge line
                if self.left_x > other.left_x {
                    Side::OnTheLeft
                } else if self.left_x < other.left_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            7 => {
                // compare the lower left edge line
                if self.lower_left_diagonal_x > other.lower_left_diagonal_x {
                    Side::OnTheLeft
                } else if self.lower_left_diagonal_x < other.lower_left_diagonal_x {
                    Side::OnTheRight
                } else {
                    Side::Collinear
                }
            }
            // Java's message really does say "IntBox.compare" here (IntOctagon.java:832).
            _ => panic!("IntBox.compare: edgeIndex out of range"),
        }
    }

    /// Java `compare(IntBox other, int edgeIndex)`:
    /// `return compare(other.toIntOctagon(), edgeIndex);`
    pub fn compare_box(&self, other: &IntBox, edge_index: usize) -> Side {
        self.compare_octagon(&other.to_int_octagon(), edge_index)
    }

    /// Java `borderLineIndex(Line line)` (IntOctagon.java:842-846) is an unfinished stub in
    /// upstream freerouting: it unconditionally logs "edge_index_of_line not yet implemented for
    /// octagons" and returns -1, regardless of whether `line` actually is one of this octagon's
    /// border lines. Ported faithfully as always `None` (`FRLogger.warn` dropped per conventions
    /// — `fr-geometry` has no `tracing` dependency, and the log carried no information beyond
    /// "not implemented").
    pub fn border_line_index(&self, _line: &Line) -> Option<usize> {
        None
    }

    /// Returns the side of `point` with respect to border line `line_index`, with `tolerance`.
    ///
    /// Java logs a warning and yields `COLLINEAR` for an out-of-range index; ported as a
    /// `debug_assert!` plus the same fallback.
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
                    // the point is above the lower right border line of this octagon
                    Side::OnTheRight
                } else if tmp < -tolerance {
                    // the point is below the lower right border line of this octagon
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            3 => {
                let tmp = point.x + point.y - self.upper_right_diagonal_x as f64;
                if tmp < -tolerance {
                    // the point is below the upper right border line of this octagon
                    Side::OnTheRight
                } else if tmp > tolerance {
                    // the point is above the upper right border line of this octagon
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            5 => {
                let tmp = point.y - point.x + self.upper_left_diagonal_x as f64;
                if tmp < -tolerance {
                    // the point is below the upper left border line of this octagon
                    Side::OnTheRight
                } else if tmp > tolerance {
                    // the point is above the upper left border line of this octagon
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            7 => {
                let tmp = point.x + point.y - self.lower_left_diagonal_x as f64;
                if tmp > tolerance {
                    // the point is above the lower left border line of this octagon
                    Side::OnTheRight
                } else if tmp < -tolerance {
                    // the point is below the lower left border line of this octagon
                    Side::OnTheLeft
                } else {
                    Side::Collinear
                }
            }
            _ => Side::Collinear,
        }
    }

    /// Checks if this octagon can be converted to an `IntBox`.
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

    /// Divide `d` minus this octagon into 8 convex pieces, from which 4 have cut off a corner.
    /// Java `IntOctagon.cutoutFrom(IntBox d)` (IntOctagon.java:1063-1314), transcribed literally.
    #[allow(clippy::too_many_lines)] // literal transcription of Java's case analysis
    pub fn cutout_from_box(&self, d: &IntBox) -> Vec<IntOctagon> {
        let c = self.intersection_box(d);

        if self.is_empty() || c.dimension() < self.dimension() {
            // there is only an overlap at the border
            return vec![d.to_int_octagon()];
        }

        let mut boxes: [IntBox; 4] = [
            // construct left box
            IntBox::from_coords(
                d.ll.x,
                c.lower_left_diagonal_x - c.left_x,
                c.left_x,
                c.left_x - c.upper_left_diagonal_x,
            ),
            // construct right box
            IntBox::from_coords(
                c.right_x,
                c.right_x - c.lower_right_diagonal_x,
                d.ur.x,
                c.upper_right_diagonal_x - c.right_x,
            ),
            // construct lower box
            IntBox::from_coords(
                c.lower_left_diagonal_x - c.bottom_y,
                d.ll.y,
                c.lower_right_diagonal_x + c.bottom_y,
                c.bottom_y,
            ),
            // construct upper box
            IntBox::from_coords(
                c.upper_left_diagonal_x + c.top_y,
                c.top_y,
                c.upper_right_diagonal_x - c.top_y,
                d.ur.y,
            ),
        ];

        let mut octagons: [IntOctagon; 4] = [IntOctagon::EMPTY; 4];

        // construct upper left octagon
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

        // construct lower left octagon
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

        // construct lower right octagon
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

        // construct upper right octagon
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

        // optimise the result to minimum cumulative circumference

        let mut b = boxes[0];
        let mut o = octagons[0];
        if b.ur.x - b.ll.x > o.top_y - o.bottom_y {
            // switch the horizontal upper left divide line to vertical
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
            // switch the vertical upper left divide line to horizontal
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
            // switch the vertical upper right divide line to horizontal
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
            // switch the horizontal upper right divide line to vertical
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
            // switch the horizontal lower right divide line to vertical
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
            // switch the vertical lower right divide line to horizontal
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
            // switch the vertical lower left divide line to horizontal
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
            // switch the horizontal lower left divide line to vertical
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
        // add the 4 boxes to the result
        for current_box in &boxes {
            result.push(current_box.to_int_octagon());
        }
        // add the 4 octagons to the result
        result.extend_from_slice(&octagons);
        result
    }

    /// Divide `d` minus this octagon into 8 convex pieces without sharp angles. Java
    /// `IntOctagon.cutoutFrom(IntOctagon d)` (IntOctagon.java:1316-1695), transcribed literally.
    #[allow(clippy::too_many_lines)] // literal transcription of Java's case analysis
    pub fn cutout_from_octagon(&self, d: &IntOctagon) -> Vec<IntOctagon> {
        let c = self.intersection(d);

        if self.is_empty() || c.dimension() < self.dimension() {
            // there is only an overlap at the border
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
            // switch the horizontal upper left divide line to vertical
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
            // switch the vertical upper left divide line to horizontal
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
            // switch the vertical upper right divide line to horizontal
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
            // switch the horizontal upper right divide line to vertical
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
            // switch the horizontal lower right divide line to vertical
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
            // switch the vertical lower right divide line to horizontal
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
            // switch the vertical lower left divide line to horizontal
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
            // switch the horizontal lower left divide line to vertical
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

    /// Returns an object of class `Simplex` defining the same shape (IntOctagon.java:556-570).
    /// The eight border lines are already sorted in ascending direction; the redundant ones (a
    /// non-square octagon whose diagonal or orthogonal bounds do not bite) are removed.
    pub fn to_simplex(&self) -> Simplex {
        if self.is_empty() {
            return Simplex::EMPTY;
        }
        let lines: Vec<Line> = (0..8).map(|i| self.border_line(i)).collect();
        Simplex::new(lines).remove_redundant_lines()
    }

    /// Java `intersection(Simplex other)`: `other.intersection(this)`, which dispatches to
    /// `Simplex.intersection(IntOctagon)`.
    pub fn intersection_simplex(&self, other: &Simplex) -> Simplex {
        other.intersection_octagon(self)
    }

    /// Java `intersects(Simplex other)`: `other.intersects(this)`, which dispatches to
    /// `Simplex.intersects(IntOctagon)`.
    pub fn intersects_simplex(&self, other: &Simplex) -> bool {
        other.intersects_octagon(self)
    }

    /// Java `cutoutFrom(Simplex simplex)`: `this.toSimplex().cutoutFrom(simplex)`.
    pub fn cutout_from_simplex(&self, simplex: &Simplex) -> Option<Vec<Simplex>> {
        self.to_simplex().cutout_from(simplex)
    }

    // not ported: the private `precalculatedToSimplex` memo field — it is a pure cache of
    // `toSimplex()` and would make `IntOctagon` non-`Copy` for no behavioral gain.
    // ported in Task 14, but in the modules that own the types they mention:
    // `tile_shape.rs` has `boundingTile()`, `simplify() -> TileShape` and the
    // `TileShape`-/`RegularTileShape`-typed `contains`, `union`, `intersection`, `intersects`,
    // `compare` and `cutout` (all on the `TileShape` / `RegularTileShape` enums, where Java's
    // double dispatch collapses into one `match`); `bounding_directions.rs` has
    // `boundingShape(dirs)`, `borderPoint(IntPoint, FortyfiveDegreeDirection)` and
    // `nearestBorderProjections(IntPoint, int)`.
    // ported in Task 17, on the `TileShape` enum: intersects(Circle) (`intersects_circle`) and
    // the `Shape`-typed `intersects(Shape)` (the `ShapeOps` impl in `shape.rs`).
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

    /// Octagon from a box: diagonals are the extreme x-y / x+y values.
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
        assert_eq!(o.upper_left_diagonal_x, -10); // min of x - y
        assert_eq!(o.lower_right_diagonal_x, 10); // max of x - y
        assert_eq!(o.lower_left_diagonal_x, 0); // min of x + y
        assert_eq!(o.upper_right_diagonal_x, 20); // max of x + y
        assert!(o.is_normalized());
        assert!(o.is_int_box());
        assert_eq!(o.bounding_box(), b);
        assert_eq!(o.area(), 100.0);
        assert_eq!(o.border_line_count(), 8);
    }

    #[test]
    fn normalize_tightens_loose_diagonals() {
        // A box-shaped octagon with slack diagonals must normalise to the tight ones.
        let loose = IntOctagon::new(0, 0, 10, 10, -50, 50, -50, 50);
        assert!(!loose.is_normalized());
        assert_eq!(
            loose.normalize(),
            from_box(IntBox::from_coords(0, 0, 10, 10))
        );
    }

    #[test]
    fn diamond_octagon() {
        // |x| + |y| <= 10 : left=-10,bottom=-10,right=10,top=10, x-y in [-10,10], x+y in [-10,10]
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
            // `IntOctagon.borderLineIndex` (IntOctagon.java:842-846) is an unfinished stub in
            // upstream freerouting: it logs "edge_index_of_line not yet implemented for octagons"
            // and returns -1 for *every* line, including this octagon's own border lines. Ported
            // as always `None`; the brief's `Some(i)` expectation does not match the Java source.
            assert_eq!(d.border_line_index(&line), None);
            // Freerouting's actual side convention (TileShape.java:140-157, `isOutside` /
            // `contains(Point)`): a point is outside the shape as soon as
            // `borderLine(i).sideOf(point) == ON_THE_LEFT`, i.e. the shape lies on the *right* of
            // its border lines under `Line::side_of`. (`Line.sideOf(Point)` negates
            // `Point.sideOf(Line)`, which itself negates a determinant, so the naming reads
            // backwards; `IntOctagon::side_of_border_line` uses the opposite convention again.)
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
        let inner = IntOctagon::new(5, 5, 15, 15, -5, 5, 15, 25).normalize(); // diamond-ish inside
        let pieces = inner.cutout_from_octagon(&outer);
        assert!(!pieces.is_empty());
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert!((total - (outer.area() - inner.intersection(&outer).area())).abs() < 1e-6);
        for p in &pieces {
            assert!(!p.overlaps(&inner));
            assert!(p.is_contained_in_octagon(&outer));
        }
    }

    // ---- additional coverage beyond the brief's tests ----

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
        // Java `int` hash arithmetic wraps silently; `IntOctagon::EMPTY` overflows i32 several
        // times over, so plain `*`/`+` would panic in a debug build. Hard-coded rather than
        // recomputed with the same formula, so the test cannot agree with a wrong
        // implementation. Hand-derived from IntOctagon.java:165-174, which folds the eight
        // fields in the order leftX, rightX, bottomY, topY, lowerLeftDiagonalX,
        // upperRightDiagonalX, upperLeftDiagonalX, lowerRightDiagonalX; for `EMPTY` those are
        // C, -C, C, -C, C, -C, C, -C with C = CRIT_INT = 2^25 = 33_554_432:
        //   r0 =                 C          =        33_554_432
        //   r1 = 31 * r0 + (-C)             =     1_006_632_960
        //   r2 = 31 * r1 +   C   (wraps)    =     1_174_405_120
        //   r3 = 31 * r2 + (-C)  (wraps)    =     2_013_265_920
        //   r4 = 31 * r3 +   C   (wraps)    =    -1_979_711_488
        //   r5 = 31 * r4 + (-C)  (wraps)    =    -1_275_068_416
        //   r6 = 31 * r5 +   C   (wraps)    =      -838_860_800
        //   r7 = 31 * r6 + (-C)  (wraps)    =      -268_435_456
        assert_eq!(IntOctagon::EMPTY.get_id(), -268_435_456);
    }

    #[test]
    fn widths_and_enlarge() {
        let a = from_box(IntBox::from_coords(0, 0, 10, 4));
        assert_eq!(a.max_width(), 10.0);
        assert_eq!(a.min_width(), 4.0);
        // enlarge delegates to offset (IntOctagon.java:317-320)
        assert_eq!(a.enlarge(3.0), a.offset(3.0));
        // offset(0) returns the octagon unchanged, even unnormalized
        let loose = IntOctagon::new(0, 0, 10, 10, -50, 50, -50, 50);
        assert_eq!(loose.offset(0.0), loose);
    }

    #[test]
    fn side_of_border_line_puts_interior_on_the_left() {
        // IntOctagon.sideOfBorderLine (IntOctagon.java:581-607) uses the opposite sign
        // convention to Line::side_of: the interior comes out ON_THE_LEFT here.
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
        // upper right diagonal: the top corner (0, 10) is on it
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
        assert_eq!(a.compare_octagon(&b, 0), Side::OnTheRight); // a.bottomY < b.bottomY
        assert_eq!(a.compare_octagon(&b, 6), Side::OnTheRight); // a.leftX < b.leftX
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
