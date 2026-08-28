//! Port of `app.freerouting.geometry.planar.Polygon`: "A Polygon is a list of points in the
//! plane, where no 2 consecutive points may be equal and no 3 consecutive points collinear"
//! (Polygon.java:10-13).
//!
//! Java stores the corners in a `LinkedList` and normalises them in place with two nested
//! iterators; this port keeps a `Vec` and reproduces the same removal order, which matters
//! because the collinearity pass restarts the whole normalisation after the *first* removal.
//!
//! **Equality.** Java does not override `equals`, so Java `Polygon`s compare by identity. This
//! port derives structural equality over the corner list, as [`crate::polyline::Polyline`] does.

use crate::limits::java_round;
use crate::point::Point;
use crate::side::Side;

/// A list of points in the plane, normalised so that no 2 consecutive points are equal and no 3
/// consecutive points are collinear.
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    corners: Vec<Point>,
}

impl Polygon {
    /// Creates a polygon from points. Multiple points, and points which are collinear with their
    /// previous and next point, will be removed (Polygon.java:18-72).
    pub fn new(points: Vec<Point>) -> Polygon {
        let mut corners = points;
        if corners.is_empty() {
            return Polygon { corners };
        }

        let mut corner_removed = true;
        while corner_removed {
            corner_removed = false;
            // remove multiple points

            if corners.is_empty() {
                return Polygon { corners };
            }
            let mut kept: Vec<Point> = Vec::with_capacity(corners.len());
            kept.push(corners[0].clone());
            for next_ob in corners.iter().skip(1) {
                // Java compares with the last *kept* element (`currentObject` only advances when
                // nothing was removed), so a run of equal points collapses to one.
                if *next_ob == *kept.last().expect("kept is never empty") {
                    corner_removed = true;
                } else {
                    kept.push(next_ob.clone());
                }
            }
            corners = kept;

            // remove points which are collinear with the previous
            // and next point.
            if corners.len() < 2 {
                // Java: `if (!i.hasNext()) continue;` after taking the first corner.
                continue;
            }
            // `prevI` trails `i` by one element, so `prevI.remove()` deletes the middle corner.
            let mut prev = 0usize;
            let mut current = 1usize;
            let mut next = 2usize;
            while next < corners.len() {
                if corners[current].side_of(&corners[prev], &corners[next]) == Side::Collinear {
                    corners.remove(current);
                    corner_removed = true;
                    break;
                }
                prev = current;
                current = next;
                next += 1;
            }
        }
        Polygon { corners }
    }

    /// Returns the array of corners of this polygon (Polygon.java:74-83).
    pub fn corner_array(&self) -> &[Point] {
        &self.corners
    }

    /// Reverts the order of the corners of this polygon (Polygon.java:85-93).
    pub fn revert_corners(&self) -> Polygon {
        let mut reverse_corner_arr = self.corners.clone();
        reverse_corner_arr.reverse();
        Polygon::new(reverse_corner_arr)
    }

    /// Returns the winding number of this polygon, treated as closed. It will be `> 0` if the
    /// corners are in counterclock sense, and `< 0` if the corners are in clockwise sense
    /// (Polygon.java:95-129).
    ///
    /// Java's `FRLogger.warn` for `|angleSum| < 0.5` is a pure diagnostic and is dropped here.
    pub fn winding_number_after_closing(&self) -> i32 {
        let corners = &self.corners;
        if corners.len() < 2 {
            return 0;
        }
        let first_side_vector = corners[1].difference_by(&corners[0]);
        let mut prev_side_vector = first_side_vector.clone();
        let mut corner_count = corners.len();
        // Skip the last corner, if it is equal to the first corner.
        if corners[0] == corners[corner_count - 1] {
            corner_count -= 1;
        }
        let mut angle_sum = 0.0;
        for i in 1..corner_count.saturating_sub(1) {
            let next_side_vector = corners[i + 1].difference_by(&corners[i]);
            angle_sum += prev_side_vector.angle_approx_to(&next_side_vector);
            prev_side_vector = next_side_vector;
        }
        if corner_count > 1 {
            let next_side_vector = corners[0].difference_by(&corners[corner_count - 1]);
            angle_sum += prev_side_vector.angle_approx_to(&next_side_vector);
            prev_side_vector = next_side_vector;
        }
        angle_sum += prev_side_vector.angle_approx_to(&first_side_vector);
        angle_sum /= 2.0 * std::f64::consts::PI;
        java_round(angle_sum) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    fn pts(v: &[(i32, i32)]) -> Vec<Point> {
        v.iter()
            .map(|&(x, y)| Point::Int(IntPoint::new(x, y)))
            .collect()
    }

    #[test]
    fn winding_number_sign_follows_orientation() {
        let ccw = Polygon::new(pts(&[(0, 0), (10, 0), (10, 10), (0, 10)]));
        let cw = ccw.revert_corners();
        assert_eq!(ccw.winding_number_after_closing(), 1);
        assert_eq!(cw.winding_number_after_closing(), -1);
        assert_eq!(
            ccw.winding_number_after_closing().signum(),
            -cw.winding_number_after_closing().signum()
        );
        assert_eq!(ccw.corner_array().len(), 4);
    }

    #[test]
    fn duplicates_and_collinear_removed() {
        let p = Polygon::new(pts(&[(0, 0), (0, 0), (5, 0), (10, 0), (10, 10)]));
        assert_eq!(p.corner_array().len(), 3);
        assert_eq!(p.corner_array(), pts(&[(0, 0), (10, 0), (10, 10)]));
    }

    #[test]
    fn empty_and_degenerate_inputs() {
        assert_eq!(Polygon::new(Vec::new()).corner_array().len(), 0);
        assert_eq!(Polygon::new(pts(&[(3, 4)])).corner_array().len(), 1);
        // a run of equal points collapses to a single corner
        assert_eq!(
            Polygon::new(pts(&[(1, 1), (1, 1), (1, 1)])).corner_array(),
            pts(&[(1, 1)])
        );
        // fewer than 2 corners: no winding number
        assert_eq!(
            Polygon::new(pts(&[(1, 1)])).winding_number_after_closing(),
            0
        );
    }

    #[test]
    fn a_closing_duplicate_of_the_first_corner_is_skipped() {
        // Polygon.java:108-111: the winding number treats a repeated first corner as the closing
        // point. (The constructor keeps it: it is only equal to the *first* corner, not to its
        // predecessor, and (0,10),(0,0),(10,0) are not collinear.)
        let p = Polygon::new(pts(&[(0, 0), (10, 0), (10, 10), (0, 10), (0, 0)]));
        assert_eq!(p.corner_array().len(), 5);
        assert_eq!(p.winding_number_after_closing(), 1);
    }
}
