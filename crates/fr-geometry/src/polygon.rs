use crate::limits::java_round;
use crate::point::Point;
use crate::side::Side;

#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    corners: Vec<Point>,
}

impl Polygon {
            pub fn new(points: Vec<Point>) -> Polygon {
        let mut corners = points;
        if corners.is_empty() {
            return Polygon { corners };
        }

        let mut corner_removed = true;
        while corner_removed {
            corner_removed = false;

            if corners.is_empty() {
                return Polygon { corners };
            }
            let mut kept: Vec<Point> = Vec::with_capacity(corners.len());
            kept.push(corners[0].clone());
            for next_ob in corners.iter().skip(1) {
                if *next_ob == *kept.last().expect("kept is never empty") {
                    corner_removed = true;
                } else {
                    kept.push(next_ob.clone());
                }
            }
            corners = kept;

            if corners.len() < 2 {
                continue;
            }
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

        pub fn corner_array(&self) -> &[Point] {
        &self.corners
    }

        pub fn revert_corners(&self) -> Polygon {
        let mut reverse_corner_arr = self.corners.clone();
        reverse_corner_arr.reverse();
        Polygon::new(reverse_corner_arr)
    }

                        pub fn winding_number_after_closing(&self) -> i32 {
        let corners = &self.corners;
        if corners.len() < 2 {
            return 0;
        }
        let first_side_vector = corners[1].difference_by(&corners[0]);
        let mut prev_side_vector = first_side_vector.clone();
        let mut corner_count = corners.len();
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
        assert_eq!(
            Polygon::new(pts(&[(1, 1), (1, 1), (1, 1)])).corner_array(),
            pts(&[(1, 1)])
        );
        assert_eq!(
            Polygon::new(pts(&[(1, 1)])).winding_number_after_closing(),
            0
        );
    }

    #[test]
    fn a_closing_duplicate_of_the_first_corner_is_skipped() {
        let p = Polygon::new(pts(&[(0, 0), (10, 0), (10, 10), (0, 10), (0, 0)]));
        assert_eq!(p.corner_array().len(), 5);
        assert_eq!(p.winding_number_after_closing(), 1);
    }
}
