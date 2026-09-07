use crate::float_point::FloatPoint;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipse {
    pub center: FloatPoint,
    pub rotation: f64,
    pub bigger_radius: f64,
    pub smaller_radius: f64,
}

impl Ellipse {
    pub fn new(center: FloatPoint, rotation: f64, radius_1: f64, radius_2: f64) -> Ellipse {
        let (bigger_radius, smaller_radius, mut current_rotation) = if radius_1 >= radius_2 {
            (radius_1, radius_2, rotation)
        } else {
            (radius_2, radius_1, rotation + 0.5 * std::f64::consts::PI)
        };
        while current_rotation >= std::f64::consts::PI {
            current_rotation -= std::f64::consts::PI;
        }
        while current_rotation < 0.0 {
            current_rotation += std::f64::consts::PI;
        }
        Ellipse {
            center,
            rotation: current_rotation,
            bigger_radius,
            smaller_radius,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radii_are_sorted_and_the_rotation_is_normed() {
        let c = FloatPoint::new(1.0, 2.0);
        let e = Ellipse::new(c, 0.25, 10.0, 4.0);
        assert_eq!(e.bigger_radius, 10.0);
        assert_eq!(e.smaller_radius, 4.0);
        assert_eq!(e.rotation, 0.25);

        let e = Ellipse::new(c, 0.25, 4.0, 10.0);
        assert_eq!(e.bigger_radius, 10.0);
        assert_eq!(e.smaller_radius, 4.0);
        assert_eq!(e.rotation, 0.25 + 0.5 * std::f64::consts::PI);

        let e = Ellipse::new(c, 3.0, 4.0, 10.0);
        assert!(e.rotation >= 0.0 && e.rotation < std::f64::consts::PI);
        assert_eq!(
            e.rotation,
            3.0 + 0.5 * std::f64::consts::PI - std::f64::consts::PI
        );

        let e = Ellipse::new(c, -0.5, 10.0, 4.0);
        assert_eq!(e.rotation, -0.5 + std::f64::consts::PI);
        assert_eq!(e.center, c);
    }
}
