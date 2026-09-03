//! Plan 9 Task 12 — the two shapes the whole directed geometry suite rests on.
//! Paste into `crates/fr-geometry/tests/polygon_geometry.rs`.
//!
//! Both were constructed against the port at tag v1.0.0 and their corner order,
//! `dimension()` and `centre_of_gravity()` were MEASURED, not assumed:
//!
//!     SQ: dimension 2, centre_of_gravity (50, 50), bounding_box [0,0 .. 100,100]
//!     L : dimension 2, centre_of_gravity (60, 60), bounding_box [0,0 .. 100,100]
//!
//! `centre_of_gravity` is the arithmetic MEAN OF THE CORNERS (polyline_shape.rs:124-136),
//! not the area centroid. L's area centroid is (45.24, 45.24); its corner mean is (60, 60),
//! and the corner mean is what `smallest_radius` uses. The notch below is placed so that
//! (60, 60) lands strictly inside L — most natural L-shapes put the corner mean OUTSIDE,
//! which makes every expectation derived from it wrong. If you change the shape, re-check
//! that first.

use fr_geometry::{int_point::IntPoint, point::Point, polygon::Polygon, polygon_shape::PolygonShape};

fn poly(pts: &[(i32, i32)]) -> PolygonShape {
    PolygonShape::from_polygon(&Polygon::new(
        pts.iter()
            .map(|(x, y)| Point::Int(IntPoint::new(*x, *y)))
            .collect(),
    ))
}

/// A 100 x 100 square.
///
///   area (shoelace)     = 10000
///   perimeter           =   400
///   centre_of_gravity   = (50, 50)
///   smallest_radius     =    50      (all four edges 50 from the CoG)
///   enlarge(10).area()  = 14400      (mitre: A + P*d + 4d^2 = 10000 + 4000 + 400 = 120^2)
pub fn sq() -> PolygonShape {
    poly(&[(0, 0), (100, 0), (100, 100), (0, 100)])
}

/// A 100 x 100 square with a 20 x 20 notch removed from the top-right corner.
///
///   winding (shoelace S)= 0 + 10000 + 2000 - 1600 + 6400 + 0 = 16800 > 0  (counter-clockwise,
///                         so the constructor does NOT reverse the corner list)
///   area                = 16800 / 2 = 8400      (= 100*80 + 20*20)
///   centre_of_gravity   = (360/6, 360/6) = (60, 60), strictly inside
///   smallest_radius     = 20                    (perpendicular foot (60,80) on the edge
///                                                y = 80, x in [0,80]; every other edge is
///                                                further: 60, 40, sqrt(2000), sqrt(800), 60)
///   bounding_tile()     = the 5-line CONVEX HULL, not L -- which is why the (85,85)-(95,95)
///                         square is the trap case for #27.
pub fn l_shape() -> PolygonShape {
    poly(&[(0, 0), (100, 0), (100, 100), (80, 100), (80, 80), (0, 80)])
}

/// #27's four cases plus the trap. See expected-outcomes.md section 8.
pub fn disjoint() -> PolygonShape { poly(&[(200, 0), (300, 0), (300, 100), (200, 100)]) }
pub fn touching() -> PolygonShape { poly(&[(100, 0), (200, 0), (200, 100), (100, 100)]) }
pub fn overlapping() -> PolygonShape { poly(&[(50, 50), (150, 50), (150, 150), (50, 150)]) }
pub fn contained() -> PolygonShape { poly(&[(25, 25), (75, 25), (75, 75), (25, 75)]) }
/// Inside L's bounding box AND inside L's convex hull, but OUTSIDE L. Expected: `false`.
pub fn in_the_notch() -> PolygonShape { poly(&[(85, 85), (95, 85), (95, 95), (85, 95)]) }
