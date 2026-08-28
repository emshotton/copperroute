//! Port of `app.freerouting.geometry.planar.Ellipse`: "Describes functionality of an ellipse in
//! the plane. Does not implement the ConvexShape interface, because coordinates are float"
//! (Ellipse.java:5-8).
//!
//! The Java class is data plus a normalising constructor; it declares no other member.

use crate::float_point::FloatPoint;

/// An ellipse in the plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipse {
    /// The centre of the ellipse.
    pub center: FloatPoint,
    /// Rotation of the ellipse in radian, normed to `0 <= rotation < pi`.
    pub rotation: f64,
    /// The bigger of the two radii.
    pub bigger_radius: f64,
    /// The smaller of the two radii.
    pub smaller_radius: f64,
}

impl Ellipse {
    /// Creates a new instance of `Ellipse` (Ellipse.java:19-39).
    ///
    /// The two radii are sorted; when they are swapped, the rotation is turned by a quarter turn.
    /// The rotation is then normed into `[0, pi)`.
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

        // Swapping the radii turns the rotation by a quarter turn.
        let e = Ellipse::new(c, 0.25, 4.0, 10.0);
        assert_eq!(e.bigger_radius, 10.0);
        assert_eq!(e.smaller_radius, 4.0);
        assert_eq!(e.rotation, 0.25 + 0.5 * std::f64::consts::PI);

        // ... and the result is normed back into [0, pi).
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
