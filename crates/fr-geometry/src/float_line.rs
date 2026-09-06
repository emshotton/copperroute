use crate::float_point::FloatPoint;
use crate::limits::CRIT_INT;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatLine {
    pub a: FloatPoint,
    pub b: FloatPoint,
}

impl FloatLine {
    pub fn new(a: FloatPoint, b: FloatPoint) -> FloatLine {
        FloatLine { a, b }
    }

    pub fn opposite(&self) -> FloatLine {
        FloatLine::new(self.b, self.a)
    }

    pub fn adjust_direction(&self, other: &FloatLine) -> FloatLine {
        if self.b.side_of(&self.a, &other.a) == other.b.side_of(&self.a, &other.a) {
            *self
        } else {
            self.opposite()
        }
    }

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

    pub fn translate(&self, dist: f64) -> FloatLine {
        let dx = self.b.x - self.a.x;
        let dy = self.b.y - self.a.y;
        let dxdx = dx * dx;
        let dydy = dy * dy;
        let length = (dxdx + dydy).sqrt();
        let new_a = if dxdx <= dydy {
            let rel_x = (dist * length) / dy;
            FloatPoint::new(self.a.x - rel_x, self.a.y)
        } else {
            let rel_y = (dist * length) / dx;
            FloatPoint::new(self.a.x, self.a.y + rel_y)
        };
        let new_b = FloatPoint::new(new_a.x + dx, new_a.y + dy);
        FloatLine::new(new_a, new_b)
    }

    pub fn signed_distance(&self, point: &FloatPoint) -> f64 {
        let dx = self.b.x - self.a.x;
        let dy = self.b.y - self.a.y;
        let det = dy * (point.x - self.a.x) - dx * (point.y - self.a.y);
        let length = (dx * dx + dy * dy).sqrt();
        det / length
    }

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

    pub fn segment_distance(&self, point: &FloatPoint) -> f64 {
        let projection = self.perpendicular_projection(point);
        if projection.is_contained_in_box(&self.a, &self.b, 0.01) {
            point.distance(&projection)
        } else {
            (point.distance(&self.a)).min(point.distance(&self.b))
        }
    }

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

    pub fn shrink_segment(&self, offset: f64) -> FloatLine {
        let dx = self.b.x - self.a.x;
        let dy = self.b.y - self.a.y;
        if dx == 0.0 && dy == 0.0 {
            return *self;
        }
        let length = (dx * dx + dy * dy).sqrt();
        let effective_offset = (offset).min(length / 2.0);
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

    pub fn nearest_segment_point(&self, from_point: &FloatPoint) -> FloatPoint {
        let projection = self.perpendicular_projection(from_point);
        if projection.is_contained_in_box(&self.a, &self.b, 0.01) {
            return projection;
        }
        if from_point.distance_square(&self.a) <= from_point.distance_square(&self.b) {
            self.a
        } else {
            self.b
        }
    }

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
        let i = h.intersection(&v).unwrap();
        assert_eq!(i.x, 3.0);
        assert_eq!(i.y, 1.0);
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
        let clamped = s.shrink_segment(100.0);
        assert_eq!(clamped.a, clamped.b);
        assert_eq!(clamped.a, FloatPoint::new(5.0, 0.0));
    }

    #[test]
    fn segment_projection_drops_a_perpendicular_segment_onto_self() {
        let s = FloatLine::new(FloatPoint::new(0.0, 0.0), FloatPoint::new(10.0, 0.0));
        let above = FloatLine::new(FloatPoint::new(2.0, 5.0), FloatPoint::new(8.0, 5.0));
        let projected = s.segment_projection(&above).unwrap();
        assert_eq!(projected.a, FloatPoint::new(2.0, 0.0));
        assert_eq!(projected.b, FloatPoint::new(8.0, 0.0));
    }
}
