//! Port of `app.freerouting.geometry.planar.LineSegment`: implements functionality for line
//! segments. The difference between a `LineSegment` and a [`Line`] is that a `Line` is infinite
//! and a `LineSegment` has a start and an end point (LineSegment.java:7-10).
//!
//! A segment is stored as three lines: it starts at the intersection of `start` with `middle` and
//! ends at the intersection of `middle` with `end`; `start` and `end` must not be parallel to
//! `middle`.
//!
//! **Caching.** Java memoises the two end points in the transient fields
//! `precalculatedStartPoint` / `precalculatedEndPoint` (LineSegment.java:16-17). This port drops
//! the cache and recomputes, as [`crate::line::Line::direction`] already does: the end points are
//! a pure function of the three lines. That has one observable consequence, in
//! [`LineSegment::start_point_approx`] / [`LineSegment::end_point_approx`] — see their docs.
//!
//! **Equality.** Java does not override `equals`, so Java `LineSegment`s compare by identity.
//! This port derives structural equality over the three lines, matching the treatment of
//! [`Line`] itself.

use std::cmp::Ordering;

use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::int_vector::IntVector;
use crate::limits::{java_max, java_min, java_round};
use crate::line::Line;
use crate::point::Point;
use crate::polyline::{Polyline, PolylineError};
use crate::polyline_shape::PolylineShapeOps;
use crate::side::Side;
use crate::signum::Signum;
use crate::simplex::Simplex;
use crate::tile_shape::TileShape;

/// A finite piece of a directed line, delimited by a start and an end closing line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineSegment {
    start: Line,
    middle: Line,
    end: Line,
}

impl LineSegment {
    /// Creates a line segment from the 3 input lines. It starts at the intersection of
    /// `start_line` and `middle_line` and ends at the intersection of `middle_line` and
    /// `end_line`. `start_line` and `end_line` must not be parallel to `middle_line`
    /// (LineSegment.java:19-28).
    pub fn new(start_line: Line, middle_line: Line, end_line: Line) -> LineSegment {
        LineSegment {
            start: start_line,
            middle: middle_line,
            end: end_line,
        }
    }

    /// Creates the `no`-th line segment of `polyline`, for `no` between 1 and
    /// `polyline.lines().len() - 2` (LineSegment.java:30-42).
    ///
    /// Java stores three `null` lines when `no` is out of range (and warns), so every later call
    /// on the result throws; this port returns `None` instead.
    pub fn from_polyline(polyline: &Polyline, no: usize) -> Option<LineSegment> {
        let lines = polyline.lines();
        if no == 0 || no + 1 >= lines.len() {
            // Java: FRLogger.warn("LineSegment from Polyline: no out of range")
            return None;
        }
        Some(LineSegment {
            start: lines[no - 1],
            middle: lines[no],
            end: lines[no + 1],
        })
    }

    /// Transforms this `LineSegment` into a polyline of length 3 (LineSegment.java:125-132).
    ///
    /// Routes through [`Polyline::from_lines`], so it inherits its error.
    pub fn to_polyline(&self) -> Result<Polyline, PolylineError> {
        Polyline::from_lines(vec![self.start, self.middle, self.end])
    }

    /// Creates the `no`-th line segment of `shape`, for `no` between 0 and
    /// `shape.border_line_count() - 1` (LineSegment.java:44-65).
    ///
    /// Java takes the abstract `PolylineShape` and returns an object with three `null` lines when
    /// `no` is out of range; this port takes the concrete [`TileShape`] and returns `None`. The
    /// `no < 0` half of Java's range check is unrepresentable with a `usize` index.
    pub fn from_tile_shape(shape: &TileShape, no: usize) -> Option<LineSegment> {
        LineSegment::from_polyline_shape(shape, no)
    }

    /// Creates the `no`-th line segment of `shape`, for `no` between 0 and
    /// `shape.border_line_count() - 1` (LineSegment.java:44-65).
    ///
    /// Java's parameter is the abstract `PolylineShape`, so both [`TileShape`] and
    /// [`crate::polygon_shape::PolygonShape`] can be passed. Java returns an object with three
    /// `null` lines when `no` is out of range; this port returns `None`. The `no < 0` half of
    /// Java's range check is unrepresentable with a `usize` index.
    pub fn from_polyline_shape(shape: &dyn PolylineShapeOps, no: usize) -> Option<LineSegment> {
        let line_count = shape.border_line_count();
        if no >= line_count {
            return None;
        }
        let start = if no == 0 {
            shape.border_line(line_count - 1)?
        } else {
            shape.border_line(no - 1)?
        };
        let middle = shape.border_line(no)?;
        let end = if no == line_count - 1 {
            shape.border_line(0)?
        } else {
            shape.border_line(no + 1)?
        };
        Some(LineSegment { start, middle, end })
    }

    /// Returns the intersection of the first 2 lines of this segment (LineSegment.java:67-73).
    pub fn start_point(&self) -> Point {
        self.middle.intersection(&self.start)
    }

    /// Returns the intersection of the last 2 lines of this segment (LineSegment.java:75-81).
    pub fn end_point(&self) -> Point {
        self.middle.intersection(&self.end)
    }

    /// Returns an approximation of the intersection of the first 2 lines of this segment
    /// (LineSegment.java:83-92).
    ///
    /// Java picks between `precalculatedStartPoint.toFloat()` and
    /// `start.intersectionApprox(middle)` depending on whether [`LineSegment::start_point`] has
    /// already been called on this object — the two can differ in the last bits, so Java's answer
    /// depends on the call history. Without the cache this port always takes Java's second
    /// branch, the floating-point shortcut, which is the one a freshly built segment uses.
    pub fn start_point_approx(&self) -> FloatPoint {
        self.start.intersection_approx(&self.middle)
    }

    /// Returns an approximation of the intersection of the last 2 lines of this segment
    /// (LineSegment.java:94-103). See [`LineSegment::start_point_approx`] on the dropped cache.
    pub fn end_point_approx(&self) -> FloatPoint {
        self.end.intersection_approx(&self.middle)
    }

    /// Returns the (infinite) line of this segment (LineSegment.java:105-108).
    pub fn get_line(&self) -> Line {
        self.middle
    }

    /// Returns the start closing line of this segment (LineSegment.java:110-113).
    pub fn get_start_closing_line(&self) -> Line {
        self.start
    }

    /// Returns the end closing line of this segment (LineSegment.java:115-118).
    pub fn get_end_closing_line(&self) -> Line {
        self.end
    }

    /// Returns the line segment with the opposite direction (LineSegment.java:120-123).
    pub fn opposite(&self) -> LineSegment {
        LineSegment::new(
            self.end.opposite(),
            self.middle.opposite(),
            self.start.opposite(),
        )
    }

    /// Creates a 1-dimensional simplex from this line segment, which has the same shape as the
    /// line segment (LineSegment.java:134-153).
    ///
    /// The two side tests are Java's `Point.sideOf(Line)` — where the *point* lies relative to
    /// the line — which is the negation of [`Line::side_of`].
    pub fn to_simplex(&self) -> Simplex {
        let mut lines: Vec<Line> = Vec::with_capacity(4);
        lines.push(
            if self.end_point().side_of_line(&self.start) == Side::OnTheRight {
                self.start.opposite()
            } else {
                self.start
            },
        );
        lines.push(self.middle);
        lines.push(self.middle.opposite());
        lines.push(
            if self.start_point().side_of_line(&self.end) == Side::OnTheRight {
                self.end.opposite()
            } else {
                self.end
            },
        );
        Simplex::from_lines(lines)
    }

    /// Checks if `point` is contained in this line segment (LineSegment.java:155-171).
    ///
    /// Java warns "currently only implemented for IntPoints" and answers `false` for any other
    /// `Point`; kept verbatim, minus the log.
    pub fn contains(&self, point: &Point) -> bool {
        let Point::Int(int_point) = point else {
            return false;
        };
        if self.middle.side_of(point) != Side::Collinear {
            return false;
        }
        // create a perpendicular line at point and check, that the two
        // endpoints of this segment are on different sides of that line.
        let perpendicular_direction = self.middle.direction().turn_45_degree(2);
        let perpendicular_line = Line::from_direction(*int_point, &perpendicular_direction);
        let start_point_side = perpendicular_line.side_of(&self.start_point());
        let end_point_side = perpendicular_line.side_of(&self.end_point());
        start_point_side != end_point_side || start_point_side == Side::Collinear
    }

    /// Calculates the smallest surrounding box of this line segment (LineSegment.java:173-184).
    pub fn bounding_box(&self) -> IntBox {
        let start_corner = self.middle.intersection_approx(&self.start);
        let end_corner = self.middle.intersection_approx(&self.end);
        let llx = java_min(start_corner.x, end_corner.x);
        let lly = java_min(start_corner.y, end_corner.y);
        let urx = java_max(start_corner.x, end_corner.x);
        let ury = java_max(start_corner.y, end_corner.y);
        let lower_left = IntPoint::new(llx.floor() as i32, lly.floor() as i32);
        let upper_right = IntPoint::new(urx.ceil() as i32, ury.ceil() as i32);
        IntBox::new(lower_left, upper_right)
    }

    /// Calculates the smallest surrounding octagon of this line segment
    /// (LineSegment.java:186-206).
    pub fn bounding_octagon(&self) -> IntOctagon {
        let start_corner = self.middle.intersection_approx(&self.start);
        let end_corner = self.middle.intersection_approx(&self.end);
        let lx = java_min(start_corner.x, end_corner.x).floor();
        let ly = java_min(start_corner.y, end_corner.y).floor();
        let rx = java_max(start_corner.x, end_corner.x).ceil();
        let uy = java_max(start_corner.y, end_corner.y).ceil();
        let start_x_minus_y = start_corner.x - start_corner.y;
        let end_x_minus_y = end_corner.x - end_corner.y;
        let ulx = java_min(start_x_minus_y, end_x_minus_y).floor();
        let lrx = java_max(start_x_minus_y, end_x_minus_y).ceil();
        let start_x_plus_y = start_corner.x + start_corner.y;
        let end_x_plus_y = end_corner.x + end_corner.y;
        let llx = java_min(start_x_plus_y, end_x_plus_y).floor();
        let urx = java_max(start_x_plus_y, end_x_plus_y).ceil();
        // Java's `(int)` narrowing of a double saturates at Integer.MIN_VALUE/MAX_VALUE and maps
        // NaN to 0; Rust's `as i32` on an `f64` does exactly the same.
        let result = IntOctagon::new(
            lx as i32, ly as i32, rx as i32, uy as i32, ulx as i32, lrx as i32, llx as i32,
            urx as i32,
        );
        result.normalize()
    }

    /// Creates a new line segment with the same start and middle line and an end line, so that
    /// the length of the new line segment is about `new_length` (LineSegment.java:208-217).
    pub fn change_length_approx(&self, new_length: f64) -> LineSegment {
        let new_end_point = self
            .start_point_approx()
            .change_length(&self.end_point_approx(), new_length);
        let perpendicular_direction = self.middle.direction().turn_45_degree(2);
        let new_end_line = Line::from_direction(new_end_point.round(), &perpendicular_direction);
        LineSegment::new(self.start, self.middle, new_end_line)
    }

    /// Looks up the intersections of this line segment with `other` (LineSegment.java:219-278).
    ///
    /// The result may have length 0, 1 or 2. If the segments do not intersect the result is
    /// empty. The result lines are so that the intersections of the result lines with this line
    /// segment will deliver the intersection points. If the segments overlap, the result has
    /// length 2 and the intersection points are the first and the last overlap point. Otherwise,
    /// the result has length 1 and the intersection point is the unique intersection or touching
    /// point. The result is not symmetric in `self` and `other`, because intersecting lines and
    /// not the intersection points are returned.
    pub fn intersection(&self, other: &LineSegment) -> Vec<Line> {
        if !self.bounding_box().intersects(&other.bounding_box()) {
            return Vec::new();
        }
        let start_point_side = self.start_point().side_of_line(&other.middle);
        let end_point_side = self.end_point().side_of_line(&other.middle);
        if start_point_side == Side::Collinear && end_point_side == Side::Collinear {
            // there may be an overlap
            let this_sorted = self.sort_endpoints_in_xy();
            let other_sorted = other.sort_endpoints_in_xy();
            let (left_line, right_line) = if this_sorted
                .start_point()
                .compare_xy(&other_sorted.start_point())
                != Ordering::Greater
            {
                (this_sorted, other_sorted)
            } else {
                (other_sorted, this_sorted)
            };
            let cmp = left_line.end_point().compare_xy(&right_line.start_point());
            if cmp == Ordering::Less {
                // end point of the left line is to the left of the start point of the right line
                return Vec::new();
            }
            if cmp == Ordering::Equal {
                // end point of the left line is equal to the start point of the right line
                return vec![left_line.end];
            }
            // now there is a real overlap
            let second =
                if right_line.end_point().compare_xy(&left_line.end_point()) != Ordering::Less {
                    left_line.end
                } else {
                    right_line.end
                };
            return vec![right_line.start, second];
        }
        if start_point_side == end_point_side
            || other.start_point().side_of_line(&self.middle)
                == other.end_point().side_of_line(&self.middle)
        {
            return Vec::new(); // no intersection possible
        }
        // now both start points and both end points are on different sides of the middle
        // line of the other segment.
        vec![other.middle]
    }

    /// Checks if this LineSegment and `other` contain a common point (LineSegment.java:280-284).
    pub fn intersects(&self, other: &LineSegment) -> bool {
        !self.intersection(other).is_empty()
    }

    /// Checks if this LineSegment and `other` contain a common LineSegment, which is not reduced
    /// to a point (LineSegment.java:286-293).
    pub fn overlaps(&self, other: &LineSegment) -> bool {
        self.intersection(other).len() > 1
    }

    /// Constructs an approximation of this line segment by orthogonal stairs with integer
    /// coordinates (LineSegment.java:295-377). The length of the stairs will be at most `width`.
    /// If `to_the_right`, the stairs will be to the right of this line segment, else to the left.
    ///
    /// # Panics
    /// When the computed stair width rounds to 0, exactly where Java throws
    /// `ArithmeticException: / by zero`.
    pub fn stair_approximation(&self, width: f64, to_the_right: bool) -> Vec<IntPoint> {
        let start_point = self.start_point().to_float().round();
        let end_point = self.end_point().to_float().round();
        if start_point == end_point {
            return Vec::new();
        }

        if start_point.x == end_point.x || start_point.y == end_point.y {
            return vec![start_point, end_point];
        }

        let dx = end_point.x - start_point.x;
        let dy = end_point.y - start_point.y;
        let abs_dx = dx.abs();
        let abs_dy = dy.abs();
        let function_of_x = abs_dx >= abs_dy;
        // use otherwise function of y for better numerical  stability

        let mut stair_width: i32;
        let stair_count: i32;

        if function_of_x {
            stair_width = java_round((width * abs_dx as f64) / abs_dy as f64) as i32;
            stair_count = (abs_dx - 1) / stair_width + 1;
            if end_point.x < start_point.x {
                stair_width = -stair_width;
            }
        } else {
            stair_width = java_round((width * abs_dy as f64) / abs_dx as f64) as i32;
            stair_count = (abs_dy - 1) / stair_width + 1;
            if end_point.y < start_point.y {
                stair_width = -stair_width;
            }
        }
        let mut result: Vec<IntPoint> = Vec::with_capacity((2 * stair_count + 1).max(0) as usize);

        result.push(start_point);
        let det = dx as f64 * dy as f64;
        let change_x_first = to_the_right && det > 0.0 || !to_the_right && det < 0.0;

        let mut prev_line_point_x = start_point.x;
        let mut prev_line_point_y = start_point.y;
        for i in 1..stair_count {
            let current_line_point_x;
            let current_line_point_y;
            if function_of_x {
                current_line_point_x = start_point.x + i * stair_width;
                current_line_point_y = java_round(
                    self.get_line()
                        .function_value_approx(current_line_point_x as f64),
                ) as i32;
            } else {
                current_line_point_y = start_point.y + i * stair_width;
                current_line_point_x = java_round(
                    self.get_line()
                        .function_in_y_value_approx(current_line_point_y as f64),
                ) as i32;
            }
            if change_x_first {
                result.push(IntPoint::new(current_line_point_x, prev_line_point_y));
            } else {
                result.push(IntPoint::new(prev_line_point_x, current_line_point_y));
            }
            result.push(IntPoint::new(current_line_point_x, current_line_point_y));
            prev_line_point_x = current_line_point_x;
            prev_line_point_y = current_line_point_y;
        }
        if change_x_first {
            result.push(IntPoint::new(end_point.x, prev_line_point_y));
        } else {
            result.push(IntPoint::new(prev_line_point_x, end_point.y));
        }
        result.push(end_point);
        result
    }

    /// Constructs an approximation of this line segment by 45 degree stairs with integer
    /// coordinates (LineSegment.java:379-474). The length of the stairs will be at most `width`.
    /// If `to_the_right`, the stairs will be to the right of this line segment, else to the left.
    ///
    /// # Panics
    /// When the computed stair width rounds to 0, exactly where Java throws
    /// `ArithmeticException: / by zero`.
    pub fn stair_approximation_45(&self, width: f64, to_the_right: bool) -> Vec<IntPoint> {
        let start_point = self.start_point().to_float().round();
        let end_point = self.end_point().to_float().round();
        if start_point == end_point {
            return Vec::new();
        }
        let delta = end_point.difference_by(&start_point);
        if delta.is_multiple_of_45_degree() {
            return vec![start_point, end_point];
        }
        let abs_delta = IntVector::new(delta.x.abs(), delta.y.abs());
        let function_of_x = abs_delta.x >= abs_delta.y;
        // use otherwise function of y for better numerical  stability
        let det = delta.x as f64 * delta.y as f64;
        let mut stair_width: i32;
        let stair_count: i32;
        if function_of_x {
            stair_width = java_round((width * abs_delta.x as f64) / abs_delta.y as f64) as i32;
            stair_count = (abs_delta.x - 1) / stair_width + 1;
            if end_point.x < start_point.x {
                stair_width = -stair_width;
            }
        } else {
            stair_width = java_round((width * abs_delta.y as f64) / abs_delta.x as f64) as i32;
            stair_count = (abs_delta.y - 1) / stair_width + 1;
            if end_point.y < start_point.y {
                stair_width = -stair_width;
            }
        }
        let mut result: Vec<IntPoint> = Vec::with_capacity((2 * stair_count + 1).max(0) as usize);
        result.push(start_point);
        let mut prev_line_point = start_point;
        for i in 1..=stair_count {
            let current_line_point;
            let mut current_x;
            let mut current_y;
            if i == stair_count {
                current_line_point = end_point;
            } else {
                if function_of_x {
                    current_x = start_point.x + i * stair_width;
                    current_y =
                        java_round(self.get_line().function_value_approx(current_x as f64)) as i32;
                } else {
                    current_y = start_point.y + i * stair_width;
                    // Java quirk kept verbatim (LineSegment.java:432): the function-of-y branch
                    // calls `functionValueApprox` — the x -> y function — on a y coordinate,
                    // where `functionInYValueApprox` is meant (as used in `stairApproximation`).
                    current_x =
                        java_round(self.get_line().function_value_approx(current_y as f64)) as i32;
                }
                current_line_point = IntPoint::new(current_x, current_y);
            }
            if function_of_x {
                let diagonal_first = to_the_right && det < 0.0 || !to_the_right && det > 0.0;

                if diagonal_first {
                    current_x = prev_line_point.x
                        + Signum::as_int_i64(stair_width as i64)
                            * (current_line_point.y - prev_line_point.y).abs();
                    current_y = current_line_point.y;
                } else {
                    // horizontal first
                    current_x = current_line_point.x
                        - Signum::as_int_i64(stair_width as i64)
                            * (current_line_point.y - prev_line_point.y).abs();
                    current_y = prev_line_point.y;
                }
            } else {
                // function of y
                let diagonal_first = to_the_right && det > 0.0 || !to_the_right && det < 0.0;

                if diagonal_first {
                    current_x = current_line_point.x;
                    current_y = prev_line_point.y
                        + Signum::as_int_i64(stair_width as i64)
                            * (current_line_point.x - prev_line_point.x).abs();
                } else {
                    current_x = prev_line_point.x;
                    current_y = current_line_point.y
                        - Signum::as_int_i64(stair_width as i64)
                            * (current_line_point.x - prev_line_point.x).abs();
                }
            }
            result.push(IntPoint::new(current_x, current_y));
            result.push(current_line_point);
            prev_line_point = current_line_point;
        }
        result
    }

    /// Returns the border line numbers of `shape` which are intersected by this line segment
    /// (LineSegment.java:476-645).
    ///
    /// Intersections at an endpoint of this line segment are only counted if the line segment
    /// intersects with the interior of `shape`. The result may have length 0, 1 or 2. With 2
    /// intersections the one nearest to the start point of the line segment comes first.
    ///
    /// Java's two `FRLogger.warn` calls are dropped per the porting conventions. The first
    /// guards a real truncation — a third intersection is *discarded* — which is kept. The
    /// second is unreachable: `result` is a `int[2]`, so a count that is neither 0 nor 2 is
    /// necessarily 1.
    pub fn border_intersections(&self, shape: &TileShape) -> Vec<usize> {
        if !self.bounding_box().intersects(&shape.bounding_box()) {
            return Vec::new();
        }

        let edge_count = shape.border_line_count();
        if edge_count == 0 {
            // The empty simplex. Java reads `borderLine(-1)`, gets `null` back and then falls
            // straight out of the `edgeCount == 0` loop with an empty result.
            return Vec::new();
        }
        // `border_line` is `None` only for the empty simplex, excluded just above.
        let border_line = |no: usize| {
            shape
                .border_line(no)
                .expect("a non-empty shape has a border line at every index below the count")
        };

        let mut prev_line = border_line(edge_count - 1);
        let mut current_line = border_line(0);
        let mut result: [usize; 2] = [0, 0];
        let mut intersection: [Option<Point>; 2] = [None, None];
        let mut intersection_count = 0usize;
        let line_start = self.start_point();
        let line_end = self.end_point();

        for edge_line_no in 0..edge_count {
            let next_line = if edge_line_no == edge_count - 1 {
                border_line(0)
            } else {
                border_line(edge_line_no + 1)
            };

            let start_point_side = current_line.side_of(&line_start);
            let end_point_side = current_line.side_of(&line_end);
            if start_point_side == Side::OnTheLeft && end_point_side == Side::OnTheLeft {
                // both endpoints are outside the borderLine,
                // no intersection possible
                return Vec::new();
            }

            if start_point_side == Side::Collinear {
                // the start is on currentLine, check that the end point is inside
                // the halfplane, because touches count only, if the interior
                // is entered
                if end_point_side != Side::OnTheRight {
                    return Vec::new();
                }
            }

            if end_point_side == Side::Collinear {
                // the end is on currentLine, check that the start point is inside
                // the halfplane, because touches count only, if the interior
                // is entered
                if start_point_side != Side::OnTheRight {
                    return Vec::new();
                }
            }

            if start_point_side != Side::OnTheRight || end_point_side != Side::OnTheRight {
                // not both points are inside the halplane defined by currentLine
                let is = self.middle.intersection(&current_line);
                let prev_line_side_of_is = prev_line.side_of(&is);
                let next_line_side_of_is = next_line.side_of(&is);
                if prev_line_side_of_is != Side::OnTheLeft
                    && next_line_side_of_is != Side::OnTheLeft
                {
                    // this line segment intersects currentLine between the
                    // previous and the next corner of simplex

                    if prev_line_side_of_is == Side::Collinear {
                        // this line segment goes through the previous
                        // corner of simplex. Check, that the intersection
                        // isn't merely a touch.
                        let prev_prev_corner = if edge_line_no == 0 {
                            shape.corner(edge_count - 1)
                        } else {
                            shape.corner(edge_line_no - 1)
                        };

                        let next_corner = if edge_line_no == edge_count - 1 {
                            shape.corner(0)
                        } else {
                            shape.corner(edge_line_no + 1)
                        };
                        // check, that prevPrevCorner and nextCorner
                        // are on different sides of this line segment.
                        let prev_prev_corner_side = self.middle.side_of(&prev_prev_corner);
                        let next_corner_side = self.middle.side_of(&next_corner);
                        if prev_prev_corner_side == Side::Collinear
                            || next_corner_side == Side::Collinear
                            || prev_prev_corner_side == next_corner_side
                        {
                            return Vec::new();
                        }
                    }
                    if next_line_side_of_is == Side::Collinear {
                        // this line segment goes through the next
                        // corner of simplex. Check, that the intersection
                        // isn't merely a touch.
                        let prev_corner = shape.corner(edge_line_no);
                        let next_next_corner = if edge_line_no == edge_count - 2 {
                            shape.corner(0)
                        } else if edge_line_no == edge_count - 1 {
                            shape.corner(1)
                        } else {
                            shape.corner(edge_line_no + 2)
                        };
                        // check, that prevCorner and nextNextCorner
                        // are on different sides of this line segment.
                        let prev_corner_side = self.middle.side_of(&prev_corner);
                        let next_next_corner_side = self.middle.side_of(&next_next_corner);
                        if prev_corner_side == Side::Collinear
                            || next_next_corner_side == Side::Collinear
                            || prev_corner_side == next_next_corner_side
                        {
                            return Vec::new();
                        }
                    }
                    let mut intersection_already_handled = false;
                    for slot in intersection.iter().take(intersection_count) {
                        if slot.as_ref() == Some(&is) {
                            intersection_already_handled = true;
                            break;
                        }
                    }
                    if !intersection_already_handled && intersection_count < result.len() {
                        // a new intersection is found
                        result[intersection_count] = edge_line_no;
                        intersection[intersection_count] = Some(is);
                        intersection_count += 1;
                    }
                    // else: Java warns "intersection_count is too big" and drops the
                    // intersection; the drop is kept, the log is not.
                }
            }

            prev_line = current_line;
            current_line = next_line;
        }

        if intersection_count == 0 {
            return Vec::new();
        }

        if intersection_count == 2 {
            // assure the correct order
            let is0 = intersection[0]
                .as_ref()
                .expect("filled together with intersection_count")
                .to_float();
            let is1 = intersection[1]
                .as_ref()
                .expect("filled together with intersection_count")
                .to_float();
            let current_start = line_start.to_float();
            if current_start.distance_square(&is1) < current_start.distance_square(&is0) {
                // swap the result points
                result.swap(0, 1);
            }

            return result.to_vec();
        }

        vec![result[0]]
    }

    /// Inverts the direction of `self.middle`, if `start_point()` has a bigger x coordinate than
    /// `end_point()`, or an equal x coordinate and a bigger y coordinate
    /// (LineSegment.java:647-665).
    ///
    /// Java swaps the two *closing* lines and leaves `middle` alone, despite the doc comment;
    /// ported verbatim. Java also hands the memoised end points over to the swapped result — a
    /// no-op here, where nothing is memoised.
    pub fn sort_endpoints_in_xy(&self) -> LineSegment {
        let swap_endlines = self.start_point().compare_xy(&self.end_point()) == Ordering::Greater;

        if swap_endlines {
            LineSegment::new(self.end, self.middle, self.start)
        } else {
            *self
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
    use crate::tile_shape::TileShape;

    fn seg(ax: i32, ay: i32, bx: i32, by: i32) -> LineSegment {
        let middle = Line::from_coords(ax, ay, bx, by);
        let d = middle.direction();
        let perp = d.turn_45_degree(2);
        let start = Line::from_direction(IntPoint::new(ax, ay), &perp);
        let end = Line::from_direction(IntPoint::new(bx, by), &perp);
        LineSegment::new(start, middle, end)
    }

    #[test]
    fn endpoints_and_box() {
        let s = seg(0, 0, 10, 0);
        assert_eq!(s.start_point(), Point::Int(IntPoint::new(0, 0)));
        assert_eq!(s.end_point(), Point::Int(IntPoint::new(10, 0)));
        assert_eq!(s.bounding_box(), IntBox::from_coords(0, 0, 10, 0));
        assert_eq!(s.opposite().start_point(), Point::Int(IntPoint::new(10, 0)));
        assert!(s.contains(&Point::Int(IntPoint::new(4, 0))));
        assert!(!s.contains(&Point::Int(IntPoint::new(11, 0))));
    }

    #[test]
    fn intersections() {
        let h = seg(0, 0, 10, 0);
        let v = seg(5, -5, 5, 5);
        assert!(h.intersects(&v));
        assert_eq!(h.intersection(&v).len(), 1);
        assert!(!h.intersects(&seg(20, -5, 20, 5)));
        assert!(h.overlaps(&seg(5, 0, 15, 0)));
        assert!(!h.overlaps(&seg(11, 0, 15, 0)));
    }

    #[test]
    fn stair_approximation_stays_near_segment() {
        let s = seg(0, 0, 20, 7);
        let pts = s.stair_approximation(2.0, true);
        assert!(pts.len() >= 3);
        assert_eq!(pts.first().copied(), Some(IntPoint::new(0, 0)));
        assert_eq!(pts.last().copied(), Some(IntPoint::new(20, 7)));
        for w in pts.windows(2) {
            let d = w[1].difference_by(&w[0]);
            assert!(d.is_orthogonal(), "stairs must be orthogonal steps");
        }
    }

    #[test]
    fn border_intersections_with_tile() {
        let s = seg(-5, 5, 15, 5);
        let b = TileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        let hits = s.border_intersections(&b);
        assert_eq!(hits.len(), 2);
        assert!(b.is_intersected_interior_by(&s));
        assert!(!b.is_intersected_interior_by(&seg(-5, 0, 15, 0))); // runs along the border
    }

    #[test]
    fn closing_lines_and_opposite() {
        let s = seg(0, 0, 10, 0);
        assert_eq!(s.get_line(), Line::from_coords(0, 0, 10, 0));
        assert_eq!(s.get_start_closing_line(), Line::from_coords(0, 0, 0, 1));
        assert_eq!(s.get_end_closing_line(), Line::from_coords(10, 0, 10, 1));
        // opposite() swaps and reverses all three lines (LineSegment.java:120-123).
        let o = s.opposite();
        assert_eq!(o.get_line(), Line::from_coords(10, 0, 0, 0));
        assert_eq!(o.get_start_closing_line(), Line::from_coords(10, 1, 10, 0));
        assert_eq!(o.get_end_closing_line(), Line::from_coords(0, 1, 0, 0));
        assert_eq!(o.end_point(), Point::Int(IntPoint::new(0, 0)));
    }

    #[test]
    fn contains_rejects_non_int_points() {
        // Java warns "only implemented for IntPoints" and answers false, even for a rational
        // point that is geometrically on the segment (LineSegment.java:155-160).
        let s = seg(0, 0, 10, 0);
        let on_the_segment = Point::Rational(crate::rational_point::RationalPoint::from_int(
            &IntPoint::new(4, 0),
        ));
        assert!(!s.contains(&on_the_segment));
    }

    #[test]
    fn from_tile_shape_wraps_around_the_border() {
        let b = TileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        // no == 0 takes the *last* border line as the start closing line.
        let first = LineSegment::from_tile_shape(&b, 0).expect("0 is in range");
        assert_eq!(first.get_start_closing_line(), b.border_line(3).unwrap());
        assert_eq!(first.get_line(), b.border_line(0).unwrap());
        assert_eq!(first.get_end_closing_line(), b.border_line(1).unwrap());
        assert_eq!(first.start_point(), Point::Int(IntPoint::new(0, 0)));
        assert_eq!(first.end_point(), Point::Int(IntPoint::new(10, 0)));
        // no == lineCount - 1 takes the *first* border line as the end closing line.
        let last = LineSegment::from_tile_shape(&b, 3).expect("3 is in range");
        assert_eq!(last.get_end_closing_line(), b.border_line(0).unwrap());
        assert_eq!(last.start_point(), Point::Int(IntPoint::new(0, 10)));
        assert_eq!(last.end_point(), Point::Int(IntPoint::new(0, 0)));
        // Out of range: Java builds an object with three null lines, this port returns None.
        assert_eq!(LineSegment::from_tile_shape(&b, 4), None);
    }

    #[test]
    fn bounding_octagon_covers_the_segment() {
        let s = seg(0, 0, 10, 0);
        let oct = s.bounding_octagon();
        assert_eq!(oct.bounding_box(), IntBox::from_coords(0, 0, 10, 0));
        assert!(oct.contains_float(&crate::float_point::FloatPoint::new(0.0, 0.0)));
        assert!(oct.contains_float(&crate::float_point::FloatPoint::new(10.0, 0.0)));
        assert!(oct.contains_float(&crate::float_point::FloatPoint::new(5.0, 0.0)));
    }

    #[test]
    fn change_length_approx_moves_the_end_line() {
        let s = seg(0, 0, 10, 0);
        let shorter = s.change_length_approx(5.0);
        assert_eq!(shorter.get_start_closing_line(), s.get_start_closing_line());
        assert_eq!(shorter.get_line(), s.get_line());
        assert_eq!(shorter.start_point(), Point::Int(IntPoint::new(0, 0)));
        assert_eq!(shorter.end_point(), Point::Int(IntPoint::new(5, 0)));
        let longer = s.change_length_approx(20.0);
        assert_eq!(longer.end_point(), Point::Int(IntPoint::new(20, 0)));
    }

    #[test]
    fn sort_endpoints_in_xy_swaps_only_the_closing_lines() {
        let forward = seg(0, 0, 10, 0);
        assert_eq!(forward.sort_endpoints_in_xy(), forward);
        let backward = seg(10, 0, 0, 0);
        let sorted = backward.sort_endpoints_in_xy();
        assert_eq!(sorted.start_point(), Point::Int(IntPoint::new(0, 0)));
        assert_eq!(sorted.end_point(), Point::Int(IntPoint::new(10, 0)));
        // Java swaps `start` and `end` and leaves `middle` alone, despite its doc comment
        // (LineSegment.java:647-665): the middle line still points to the left.
        assert_eq!(sorted.get_line(), backward.get_line());
    }

    #[test]
    fn collinear_segments_touching_in_one_point() {
        let h = seg(0, 0, 10, 0);
        let t = seg(10, 0, 20, 0);
        // A shared endpoint is an intersection but not an overlap.
        assert_eq!(h.intersection(&t), vec![h.get_end_closing_line()]);
        assert!(h.intersects(&t));
        assert!(!h.overlaps(&t));
    }

    #[test]
    fn stair_approximation_45_uses_diagonal_steps() {
        let s = seg(0, 0, 20, 7);
        let pts = s.stair_approximation_45(2.0, true);
        assert_eq!(
            pts,
            vec![
                IntPoint::new(0, 0),
                IntPoint::new(4, 0),
                IntPoint::new(6, 2),
                IntPoint::new(10, 2),
                IntPoint::new(12, 4),
                IntPoint::new(16, 4),
                IntPoint::new(18, 6),
                IntPoint::new(19, 6),
                IntPoint::new(20, 7),
            ]
        );
        for w in pts.windows(2) {
            let d = w[1].difference_by(&w[0]);
            assert!(d.is_multiple_of_45_degree(), "45 degree steps expected");
        }
        // A segment that already runs at a multiple of 45 degrees is returned as its endpoints.
        let diagonal = seg(0, 0, 10, 10);
        assert_eq!(
            diagonal.stair_approximation_45(2.0, true),
            vec![IntPoint::new(0, 0), IntPoint::new(10, 10)]
        );
        // A degenerate segment has no stairs at all.
        let point = seg(0, 0, 10, 0).change_length_approx(0.0);
        assert!(point.stair_approximation_45(2.0, true).is_empty());
        assert!(point.stair_approximation(2.0, true).is_empty());
    }

    #[test]
    fn border_intersections_order_and_counts() {
        let b = TileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        // Crossing the whole box: the border line nearest the start point comes first. The
        // segment starts left of the box, so the left border (3) precedes the right one (1).
        assert_eq!(seg(-5, 5, 15, 5).border_intersections(&b), vec![3, 1]);
        assert_eq!(seg(15, 5, -5, 5).border_intersections(&b), vec![1, 3]);
        // Starting inside: only the exit border is hit.
        assert_eq!(seg(5, 5, 15, 5).border_intersections(&b), vec![1]);
        // Wholly inside: no border line is crossed.
        assert!(seg(2, 5, 8, 5).border_intersections(&b).is_empty());
        // Merely touching the upper right corner is not an intersection
        // (LineSegment.java:568-591).
        let corner_touch = seg(0, 20, 20, 0);
        assert!(corner_touch.border_intersections(&b).is_empty());
        assert!(!b.is_intersected_interior_by(&corner_touch));
    }

    #[test]
    fn to_simplex_is_the_segment_strip() {
        let s = seg(0, 0, 10, 0);
        let sx = s.to_simplex();
        assert_eq!(sx.dimension(), 1);
        assert_eq!(sx.bounding_box(), IntBox::from_coords(0, 0, 10, 0));
    }

    #[test]
    fn from_polyline_and_to_polyline() {
        // LineSegment.java:30-42 and 125-132, values taken from the Java original.
        let polyline = Polyline::from_points(&[
            Point::Int(IntPoint::new(0, 0)),
            Point::Int(IntPoint::new(10, 0)),
            Point::Int(IntPoint::new(10, 10)),
        ]);
        let seg = LineSegment::from_polyline(&polyline, 1).unwrap();
        assert_eq!(seg.get_start_closing_line(), Line::from_coords(0, 0, 0, 1));
        assert_eq!(seg.get_line(), Line::from_coords(0, 0, 10, 0));
        assert_eq!(seg.get_end_closing_line(), Line::from_coords(10, 0, 10, 10));
        assert_eq!(seg.start_point(), Point::Int(IntPoint::new(0, 0)));
        assert_eq!(seg.end_point(), Point::Int(IntPoint::new(10, 0)));
        assert_eq!(seg.to_polyline().unwrap().lines(), &polyline.lines()[0..3]);
        assert_eq!(
            LineSegment::from_polyline(&polyline, 2)
                .unwrap()
                .to_polyline()
                .unwrap()
                .lines(),
            &polyline.lines()[1..4]
        );
        // Java's range is 1 ..= lineCount - 2; outside it the segment gets three null lines.
        assert_eq!(LineSegment::from_polyline(&polyline, 0), None);
        assert_eq!(LineSegment::from_polyline(&polyline, 3), None);
    }
}
