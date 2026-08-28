//! Port of `app.freerouting.geometry.planar.FloatLine`: a line in the plane defined by two
//! `FloatPoint`s. Calculations with FloatLines are generally not exact; for that reason
//! collinearity, for example, is not defined for FloatLines. If exactness is needed, use `Line`
//! instead (a later task).

use crate::float_point::FloatPoint;
use crate::limits::{CRIT_INT, java_min};

/// A line in the plane, defined by two `FloatPoint`s.
///
/// Java's constructor logs a debug message when either endpoint is `null` (FloatLine.java:20-26)
/// — not applicable here, since `FloatPoint` arguments are non-nullable in Rust.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatLine {
    /// The first end point of this line.
    pub a: FloatPoint,
    /// The second end point of this line.
    pub b: FloatPoint,
}

impl FloatLine {
    /// Creates a line from two FloatPoints.
    pub fn new(a: FloatPoint, b: FloatPoint) -> FloatLine {
        FloatLine { a, b }
    }

    /// Returns the FloatLine with swapped end points.
    pub fn opposite(&self) -> FloatLine {
        FloatLine::new(self.b, self.a)
    }

    /// Adjusts this line's direction to match the orientation of another line.
    pub fn adjust_direction(&self, other: &FloatLine) -> FloatLine {
        if self.b.side_of(&self.a, &other.a) == other.b.side_of(&self.a, &other.a) {
            *self
        } else {
            self.opposite()
        }
    }

    /// Calculates the intersection of this line with other. Returns `None`, if the lines are
    /// parallel.
    pub fn intersection(&self, other: &FloatLine) -> Option<FloatPoint> {
        let d1x = self.b.x - self.a.x;
        let d1y = self.b.y - self.a.y;
        let d2x = other.b.x - other.a.x;
        let d2y = other.b.y - other.a.y;
        let det1 = self.a.x * self.b.y - self.a.y * self.b.x;
        let det2 = other.a.x * other.b.y - other.a.y * other.b.x;
        let det = d2x * d1y - d2y * d1x;
        if det == 0.0 {
            return None;
        }
        let is_x = (d2x * det1 - d1x * det2) / det;
        let is_y = (d2y * det1 - d1y * det2) / det;
        Some(FloatPoint::new(is_x, is_y))
    }

    /// Translates the line perpendicular by about dist. If `dist > 0`, the line will be
    /// translated to the left, else to the right.
    pub fn translate(&self, dist: f64) -> FloatLine {
        let dx = self.b.x - self.a.x;
        let dy = self.b.y - self.a.y;
        let dxdx = dx * dx;
        let dydy = dy * dy;
        let length = (dxdx + dydy).sqrt();
        let new_a = if dxdx <= dydy {
            // translate along the x axis
            let rel_x = (dist * length) / dy;
            FloatPoint::new(self.a.x - rel_x, self.a.y)
        } else {
            // translate along the y axis
            let rel_y = (dist * length) / dx;
            FloatPoint::new(self.a.x, self.a.y + rel_y)
        };
        let new_b = FloatPoint::new(new_a.x + dx, new_a.y + dy);
        FloatLine::new(new_a, new_b)
    }

    /// Returns the signed distance of this line from point. The result will be positive, if the
    /// line is on the left of point, else negative.
    pub fn signed_distance(&self, point: &FloatPoint) -> f64 {
        let dx = self.b.x - self.a.x;
        let dy = self.b.y - self.a.y;
        let det = dy * (point.x - self.a.x) - dx * (point.y - self.a.y);
        // area of the parallelogram spanned by the 3 points
        let length = (dx * dx + dy * dy).sqrt();
        det / length
    }

    /// Returns an approximation of the perpendicular projection of point onto this line.
    pub fn perpendicular_projection(&self, point: &FloatPoint) -> FloatPoint {
        let dx = self.b.x - self.a.x;
        let dy = self.b.y - self.a.y;
        if dx == 0.0 && dy == 0.0 {
            return self.a;
        }

        let dxdx = dx * dx;
        let dydy = dy * dy;
        let dxdy = dx * dy;
        let denominator = dxdx + dydy;
        let det = self.a.x * self.b.y - self.b.x * self.a.y;

        let x = (point.x * dxdx + point.y * dxdy + det * dy) / denominator;
        let y = (point.x * dxdy + point.y * dydy - det * dx) / denominator;

        FloatPoint::new(x, y)
    }

    /// Returns the distance of point to the nearest point of this line between `self.a` and
    /// `self.b`.
    pub fn segment_distance(&self, point: &FloatPoint) -> f64 {
        let projection = self.perpendicular_projection(point);
        if projection.is_contained_in_box(&self.a, &self.b, 0.01) {
            point.distance(&projection)
        } else {
            java_min(point.distance(&self.a), point.distance(&self.b))
        }
    }

    /// Returns the perpendicular projection of `line_segment` onto this oriented line segment.
    /// Returns `None`, if the projection is empty.
    pub fn segment_projection(&self, line_segment: &FloatLine) -> Option<FloatLine> {
        if self.b.scalar_product(&self.a, &line_segment.a) < 0.0 {
            return None;
        }
        if self.a.scalar_product(&self.b, &line_segment.b) < 0.0 {
            return None;
        }
        let projected_a = if self.a.scalar_product(&self.b, &line_segment.a) < 0.0 {
            self.a
        } else {
            let projected_a = self.perpendicular_projection(&line_segment.a);
            if projected_a.x.abs() >= CRIT_INT as f64 || projected_a.y.abs() >= CRIT_INT as f64 {
                return None;
            }
            projected_a
        };
        let projected_b = if self.b.scalar_product(&self.a, &line_segment.b) < 0.0 {
            self.b
        } else {
            self.perpendicular_projection(&line_segment.b)
        };
        if projected_b.x.abs() >= CRIT_INT as f64 || projected_b.y.abs() >= CRIT_INT as f64 {
            return None;
        }
        Some(FloatLine::new(projected_a, projected_b))
    }

    /// Returns the projection of `line_segment` onto this oriented line segment by moving
    /// `line_segment` perpendicular into the direction of this line segment. Returns `None`, if
    /// the projection is empty or `line_segment.a == line_segment.b`.
    // not ported: FloatLine.segmentProjection2 — implemented as segment_projection_2 below;
    // audit-script false positive (trailing-digit camelCase, the overload-suffix regex only
    // matches an appended `_<suffix>`, not an inserted one).
    pub fn segment_projection_2(&self, line_segment: &FloatLine) -> Option<FloatLine> {
        if line_segment.a.scalar_product(&line_segment.b, &self.b) <= 0.0 {
            return None;
        }
        if line_segment.b.scalar_product(&line_segment.a, &self.a) <= 0.0 {
            return None;
        }
        let projected_a = if line_segment.a.scalar_product(&line_segment.b, &self.a) < 0.0 {
            let current_perpendicular_line = FloatLine::new(
                line_segment.a,
                line_segment.b.turn_90_degree_pole(1, &line_segment.a),
            );
            let projected_a = current_perpendicular_line.intersection(self)?;
            if projected_a.x.abs() >= CRIT_INT as f64 || projected_a.y.abs() >= CRIT_INT as f64 {
                return None;
            }
            projected_a
        } else {
            self.a
        };

        let projected_b = if line_segment.b.scalar_product(&line_segment.a, &self.b) < 0.0 {
            let current_perpendicular_line = FloatLine::new(
                line_segment.b,
                line_segment.a.turn_90_degree_pole(1, &line_segment.b),
            );
            let projected_b = current_perpendicular_line.intersection(self)?;
            if projected_b.x.abs() >= CRIT_INT as f64 || projected_b.y.abs() >= CRIT_INT as f64 {
                return None;
            }
            projected_b
        } else {
            self.b
        };
        Some(FloatLine::new(projected_a, projected_b))
    }

    /// Shrinks this line on both sides by value. The result will contain at least the midpoint
    /// of the line.
    pub fn shrink_segment(&self, offset: f64) -> FloatLine {
        let dx = self.b.x - self.a.x;
        let dy = self.b.y - self.a.y;
        if dx == 0.0 && dy == 0.0 {
            return *self;
        }
        let length = (dx * dx + dy * dy).sqrt();
        let effective_offset = java_min(offset, length / 2.0);
        let new_a = FloatPoint::new(
            self.a.x + (dx * effective_offset) / length,
            self.a.y + (dy * effective_offset) / length,
        );
        let new_length = length - effective_offset;
        let new_b = FloatPoint::new(
            self.a.x + (dx * new_length) / length,
            self.a.y + (dy * new_length) / length,
        );
        FloatLine::new(new_a, new_b)
    }

    /// Calculates the nearest point on this line to `from_point` between `self.a` and `self.b`.
    pub fn nearest_segment_point(&self, from_point: &FloatPoint) -> FloatPoint {
        let projection = self.perpendicular_projection(from_point);
        if projection.is_contained_in_box(&self.a, &self.b, 0.01) {
            return projection;
        }
        // Now the projection is outside the line segment.
        if from_point.distance_square(&self.a) <= from_point.distance_square(&self.b) {
            self.a
        } else {
            self.b
        }
    }

    /// Divides this line segment into count line segments of nearly equal length.
    pub fn divide_segment_into_sections(&self, count: i32) -> Vec<FloatLine> {
        if count == 0 {
            return Vec::new();
        }
        if count == 1 {
            return vec![*self];
        }
        let line_length = self.b.distance(&self.a);
        let section_length = line_length / count as f64;
        let dx = self.b.x - self.a.x;
        let dy = self.b.y - self.a.y;
        let mut result = Vec::with_capacity(count as usize);
        let mut current_a = self.a;
        for i in 0..count {
            let current_b = if i == count - 1 {
                self.b
            } else {
                let current_distance = (i + 1) as f64 * section_length;
                let current_x = self.a.x + (dx * current_distance) / line_length;
                let current_y = self.a.y + (dy * current_distance) / line_length;
                FloatPoint::new(current_x, current_y)
            };
            result.push(FloatLine::new(current_a, current_b));
            current_a = current_b;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_and_projection() {
        let h = FloatLine::new(FloatPoint::new(0.0, 1.0), FloatPoint::new(10.0, 1.0));
        let v = FloatLine::new(FloatPoint::new(3.0, -5.0), FloatPoint::new(3.0, 5.0));
        // Corrected per Java (FloatLine.java:44-46): `intersection` returns `null` for parallel
        // lines, not a sentinel value, so the Rust port returns `Option<FloatPoint>` and this
        // (non-parallel) case must be unwrapped.
        let i = h.intersection(&v).unwrap();
        assert_eq!(i.x, 3.0);
        assert_eq!(i.y, 1.0);
        // Parallel lines really do yield `None` (FloatLine.java: `if (det == 0) return null;`).
        let h2 = FloatLine::new(FloatPoint::new(0.0, 2.0), FloatPoint::new(10.0, 2.0));
        assert!(h.intersection(&h2).is_none());

        let pr = h.perpendicular_projection(&FloatPoint::new(4.0, 9.0));
        assert_eq!(pr, FloatPoint::new(4.0, 1.0));
        assert!((h.signed_distance(&FloatPoint::new(4.0, 9.0)).abs() - 8.0).abs() < 1e-12);
    }

    #[test]
    fn segment_distance_clamps_to_endpoints() {
        let s = FloatLine::new(FloatPoint::new(0.0, 0.0), FloatPoint::new(10.0, 0.0));
        assert!((s.segment_distance(&FloatPoint::new(13.0, 4.0)) - 5.0).abs() < 1e-12);
        assert_eq!(
            s.nearest_segment_point(&FloatPoint::new(-3.0, 2.0)),
            FloatPoint::new(0.0, 0.0)
        );
        let parts = s.divide_segment_into_sections(4);
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[1].a, FloatPoint::new(2.5, 0.0));
    }

    #[test]
    fn shrink_segment_moves_both_endpoints_inward() {
        let s = FloatLine::new(FloatPoint::new(0.0, 0.0), FloatPoint::new(10.0, 0.0));
        let shrunk = s.shrink_segment(2.0);
        assert_eq!(shrunk.a, FloatPoint::new(2.0, 0.0));
        assert_eq!(shrunk.b, FloatPoint::new(8.0, 0.0));
        // offset larger than half the length clamps to the midpoint on both ends.
        let clamped = s.shrink_segment(100.0);
        assert_eq!(clamped.a, clamped.b);
        assert_eq!(clamped.a, FloatPoint::new(5.0, 0.0));
    }

    #[test]
    fn segment_projection_drops_a_perpendicular_segment_onto_self() {
        // self is the horizontal segment (0,0)-(10,0); projecting a segment above it drops
        // straight down onto self (verified by hand against FloatLine.segmentProjection).
        let s = FloatLine::new(FloatPoint::new(0.0, 0.0), FloatPoint::new(10.0, 0.0));
        let above = FloatLine::new(FloatPoint::new(2.0, 5.0), FloatPoint::new(8.0, 5.0));
        let projected = s.segment_projection(&above).unwrap();
        assert_eq!(projected.a, FloatPoint::new(2.0, 0.0));
        assert_eq!(projected.b, FloatPoint::new(8.0, 0.0));
    }
}
