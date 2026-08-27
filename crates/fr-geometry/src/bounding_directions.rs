//! Port of `app.freerouting.geometry.planar.ShapeBoundingDirections` together with its two
//! singleton implementations `OrthogonalBoundingDirections` and
//! `FortyfiveDegreeBoundingDirections`, and of the enum `FortyfiveDegreeDirection`.
//!
//! Java models the fixed direction set of a [`RegularTileShape`] as an interface with two
//! stateless singletons; both collapse into one two-variant enum here. `bounds(ConvexShape)`
//! double-dispatches through `shape.boundingShape(this)`, which this port replaces with the
//! per-type `bounds_*` methods plus [`ShapeBoundingDirections::bounds_tile`].
//!
//! `IntOctagon.borderPoint(IntPoint, FortyfiveDegreeDirection)` and the
//! `IntOctagon.nearestBorderProjections` that iterates over it live here too: they are the only
//! users of `FortyfiveDegreeDirection` in `geometry/planar` and could not be ported before this
//! enum existed.

use crate::int_box::IntBox;
use crate::int_direction::IntDirection;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::point::Point;
use crate::regular_tile_shape::RegularTileShape;
use crate::simplex::Simplex;
use crate::tile_shape::TileShape;

/// The eight 45-degree directions, starting from right in counterclock sense to down45
/// (FortyfiveDegreeDirection.java:6-46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FortyfiveDegreeDirection {
    /// Java `RIGHT`.
    Right,
    /// Java `RIGHT45`.
    Right45,
    /// Java `UP`.
    Up,
    /// Java `UP45`.
    Up45,
    /// Java `LEFT`.
    Left,
    /// Java `LEFT45`.
    Left45,
    /// Java `DOWN`.
    Down,
    /// Java `DOWN45`.
    Down45,
}

// not ported: `FortyfiveDegreeDirection.getInstance(...)` factories — Java has none. The enum
// declares only the per-constant `getDirection()` below, so there is no Java-side conversion
// from an `IntDirection` or a `Line` to mirror.

impl FortyfiveDegreeDirection {
    /// The eight constants in declaration order, i.e. Java's implicit `values()`. Used by
    /// [`IntOctagon::nearest_border_projections`].
    pub const VALUES: [FortyfiveDegreeDirection; 8] = [
        FortyfiveDegreeDirection::Right,
        FortyfiveDegreeDirection::Right45,
        FortyfiveDegreeDirection::Up,
        FortyfiveDegreeDirection::Up45,
        FortyfiveDegreeDirection::Left,
        FortyfiveDegreeDirection::Left45,
        FortyfiveDegreeDirection::Down,
        FortyfiveDegreeDirection::Down45,
    ];

    /// Java `getDirection()` (FortyfiveDegreeDirection.java:8-45).
    pub fn to_int_direction(self) -> IntDirection {
        match self {
            FortyfiveDegreeDirection::Right => IntDirection::RIGHT,
            FortyfiveDegreeDirection::Right45 => IntDirection::RIGHT45,
            FortyfiveDegreeDirection::Up => IntDirection::UP,
            FortyfiveDegreeDirection::Up45 => IntDirection::UP45,
            FortyfiveDegreeDirection::Left => IntDirection::LEFT,
            FortyfiveDegreeDirection::Left45 => IntDirection::LEFT45,
            FortyfiveDegreeDirection::Down => IntDirection::DOWN,
            FortyfiveDegreeDirection::Down45 => IntDirection::DOWN45,
        }
    }
}

/// The fixed border-line directions of a [`RegularTileShape`]
/// (ShapeBoundingDirections.java:4-29). `Orthogonal` produces [`IntBox`] bounds,
/// `FortyfiveDegree` produces [`IntOctagon`] bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShapeBoundingDirections {
    /// Java `OrthogonalBoundingDirections.INSTANCE`: the 4 orthogonal directions.
    Orthogonal,
    /// Java `FortyfiveDegreeBoundingDirections.INSTANCE`: the 8 multiples of 45 degree.
    FortyfiveDegree,
}

impl ShapeBoundingDirections {
    /// Returns the count of the fixed directions (OrthogonalBoundingDirections.java:16-18,
    /// FortyfiveDegreeBoundingDirections.java:17-19).
    pub fn count(self) -> usize {
        match self {
            ShapeBoundingDirections::Orthogonal => 4,
            ShapeBoundingDirections::FortyfiveDegree => 8,
        }
    }

    /// Java `bounds(IntBox box)`: the box itself for orthogonal directions,
    /// `box.toIntOctagon()` for 45-degree directions.
    pub fn bounds_box(self, box_: &IntBox) -> RegularTileShape {
        match self {
            ShapeBoundingDirections::Orthogonal => RegularTileShape::Box(*box_),
            ShapeBoundingDirections::FortyfiveDegree => {
                RegularTileShape::Octagon(box_.to_int_octagon())
            }
        }
    }

    /// Java `bounds(IntOctagon oct)`: `oct.boundingBox()` for orthogonal directions, the octagon
    /// itself for 45-degree directions.
    pub fn bounds_octagon(self, oct: &IntOctagon) -> RegularTileShape {
        match self {
            ShapeBoundingDirections::Orthogonal => RegularTileShape::Box(oct.bounding_box()),
            ShapeBoundingDirections::FortyfiveDegree => RegularTileShape::Octagon(*oct),
        }
    }

    /// Java `bounds(Simplex simplex)`: `simplex.boundingBox()` resp. `simplex.boundingOctagon()`.
    /// The latter returns `null` for an unbounded simplex, hence the `Option`.
    pub fn bounds_simplex(self, simplex: &Simplex) -> Option<RegularTileShape> {
        match self {
            ShapeBoundingDirections::Orthogonal => {
                Some(RegularTileShape::Box(simplex.bounding_box()))
            }
            ShapeBoundingDirections::FortyfiveDegree => {
                simplex.bounding_octagon().map(RegularTileShape::Octagon)
            }
        }
    }

    /// Java `bounds(ConvexShape shape)` narrowed to a `TileShape`: `shape.boundingShape(this)`,
    /// which every `TileShape` implements as `dirs.bounds(this)`.
    pub fn bounds_tile(self, shape: &TileShape) -> Option<RegularTileShape> {
        match shape {
            TileShape::Box(b) => Some(self.bounds_box(b)),
            TileShape::Octagon(o) => Some(self.bounds_octagon(o)),
            TileShape::Simplex(s) => self.bounds_simplex(s),
        }
    }

    // added in Task 17 (Circle / PolygonShape / Shape): bounds(Circle), bounds(PolygonShape) and
    // the `ConvexShape`-typed `bounds(Shape)` that dispatches over all of them.
}

impl IntBox {
    /// Java `IntBox.boundingShape(ShapeBoundingDirections dirs)`: `dirs.bounds(this)`
    /// (IntBox.java:382-384).
    pub fn bounding_shape(&self, dirs: ShapeBoundingDirections) -> RegularTileShape {
        dirs.bounds_box(self)
    }
}

impl IntOctagon {
    /// Java `IntOctagon.boundingShape(ShapeBoundingDirections dirs)`: `dirs.bounds(this)`
    /// (IntOctagon.java:573-575).
    pub fn bounding_shape(&self, dirs: ShapeBoundingDirections) -> RegularTileShape {
        dirs.bounds_octagon(self)
    }

    /// Calculates the border point of this octagon from `point` into the 45-degree direction
    /// `dir`. If this border point is not an `IntPoint`, the nearest outside `IntPoint` of the
    /// octagon is returned (IntOctagon.java:848-911).
    ///
    /// Java's `default` arm (an unexpected direction) cannot be reached from a Rust `enum`, so
    /// the `FRLogger.warn` + `(0, 0)` fallback is dropped.
    pub fn border_point(&self, point: &IntPoint, dir: FortyfiveDegreeDirection) -> IntPoint {
        let (result_x, result_y) = match dir {
            FortyfiveDegreeDirection::Right => {
                let mut x = self.right_x.min(self.upper_right_diagonal_x - point.y);
                x = x.min(self.lower_right_diagonal_x + point.y);
                (x, point.y)
            }
            FortyfiveDegreeDirection::Left => {
                let mut x = self.left_x.max(self.upper_left_diagonal_x + point.y);
                x = x.max(self.lower_left_diagonal_x - point.y);
                (x, point.y)
            }
            FortyfiveDegreeDirection::Up => {
                let mut y = self.top_y.min(point.x - self.upper_left_diagonal_x);
                y = y.min(self.upper_right_diagonal_x - point.x);
                (point.x, y)
            }
            FortyfiveDegreeDirection::Down => {
                let mut y = self.bottom_y.max(self.lower_left_diagonal_x - point.x);
                y = y.max(point.x - self.lower_right_diagonal_x);
                (point.x, y)
            }
            FortyfiveDegreeDirection::Right45 => {
                let mut x =
                    (0.5 * (point.x - point.y + self.upper_right_diagonal_x) as f64).ceil() as i32;
                x = x.min(self.right_x);
                x = x.min(point.x - point.y + self.top_y);
                (x, point.y - point.x + x)
            }
            FortyfiveDegreeDirection::Up45 => {
                let mut x =
                    (0.5 * (point.x + point.y + self.upper_left_diagonal_x) as f64).floor() as i32;
                x = x.max(self.left_x);
                x = x.max(point.x + point.y - self.top_y);
                (x, point.y + point.x - x)
            }
            FortyfiveDegreeDirection::Left45 => {
                let mut x =
                    (0.5 * (point.x - point.y + self.lower_left_diagonal_x) as f64).floor() as i32;
                x = x.max(self.left_x);
                x = x.max(point.x - point.y + self.bottom_y);
                (x, point.y - point.x + x)
            }
            FortyfiveDegreeDirection::Down45 => {
                let mut x =
                    (0.5 * (point.x + point.y + self.lower_right_diagonal_x) as f64).ceil() as i32;
                x = x.min(self.right_x);
                x = x.min(point.x + point.y - self.bottom_y);
                (x, point.y + point.x - x)
            }
        };
        IntPoint::new(result_x, result_y)
    }

    /// Calculates the sorted `max_result_points` nearest points on the border of this octagon in
    /// the 45-degree directions. `point` is assumed to be located in the interior of this octagon
    /// (IntOctagon.java:909-940).
    ///
    /// Java allocates an array of `max_result_points` and leaves the tail `null` when fewer
    /// candidates were inserted; the inserted entries always form a prefix, so this port returns
    /// just that prefix. Java's guard is `this.contains(point)` with an `IntPoint` argument,
    /// which resolves to the inherited `TileShape.contains(Point)` (the border-line test), not to
    /// the `IntOctagon.contains(FloatPoint)` overload.
    pub fn nearest_border_projections(
        &self,
        point: &IntPoint,
        max_result_points: usize,
    ) -> Vec<IntPoint> {
        if !TileShape::Octagon(*self).contains(&Point::Int(*point)) || max_result_points == 0 {
            return Vec::new();
        }
        let max_result_points = max_result_points.min(8);
        let mut result: Vec<Option<IntPoint>> = vec![None; max_result_points];
        let mut min_dist = vec![f64::MAX; max_result_points];
        let inside_point = point.to_float();
        for current_direction in FortyfiveDegreeDirection::VALUES {
            let current_border_point = self.border_point(point, current_direction);
            let current_distance = inside_point.distance_square(&current_border_point.to_float());
            for i in 0..max_result_points {
                if current_distance < min_dist[i] {
                    for k in ((i + 1)..max_result_points).rev() {
                        min_dist[k] = min_dist[k - 1];
                        result[k] = result[k - 1];
                    }
                    min_dist[i] = current_distance;
                    result[i] = Some(current_border_point);
                    break;
                }
            }
        }
        result.into_iter().flatten().collect()
    }
}

impl Simplex {
    /// Java `Simplex.boundingShape(ShapeBoundingDirections dirs)`: `dirs.bounds(this)`
    /// (Simplex.java:546-549). `None` when the 45-degree bound of an unbounded simplex is asked
    /// for (Java returns `null` from `boundingOctagon()` there).
    pub fn bounding_shape(&self, dirs: ShapeBoundingDirections) -> Option<RegularTileShape> {
        dirs.bounds_simplex(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_constants_match_java() {
        assert_eq!(
            FortyfiveDegreeDirection::Right45.to_int_direction(),
            IntDirection::RIGHT45
        );
        assert_eq!(
            FortyfiveDegreeDirection::Down45.to_int_direction(),
            IntDirection::DOWN45
        );
        assert_eq!(FortyfiveDegreeDirection::VALUES.len(), 8);
        assert_eq!(ShapeBoundingDirections::Orthogonal.count(), 4);
        assert_eq!(ShapeBoundingDirections::FortyfiveDegree.count(), 8);
    }

    #[test]
    fn border_point_of_a_box_octagon() {
        let oct = IntBox::from_coords(0, 0, 10, 10).to_int_octagon();
        let p = IntPoint::new(5, 5);
        assert_eq!(
            oct.border_point(&p, FortyfiveDegreeDirection::Right),
            IntPoint::new(10, 5)
        );
        assert_eq!(
            oct.border_point(&p, FortyfiveDegreeDirection::Left),
            IntPoint::new(0, 5)
        );
        assert_eq!(
            oct.border_point(&p, FortyfiveDegreeDirection::Up),
            IntPoint::new(5, 10)
        );
        assert_eq!(
            oct.border_point(&p, FortyfiveDegreeDirection::Down),
            IntPoint::new(5, 0)
        );
        // the 45-degree rays leave through the corners of the box
        assert_eq!(
            oct.border_point(&p, FortyfiveDegreeDirection::Right45),
            IntPoint::new(10, 10)
        );
        assert_eq!(
            oct.border_point(&p, FortyfiveDegreeDirection::Up45),
            IntPoint::new(0, 10)
        );
        assert_eq!(
            oct.border_point(&p, FortyfiveDegreeDirection::Left45),
            IntPoint::new(0, 0)
        );
        assert_eq!(
            oct.border_point(&p, FortyfiveDegreeDirection::Down45),
            IntPoint::new(10, 0)
        );
    }

    #[test]
    fn nearest_border_projections_are_sorted_and_bounded() {
        let oct = IntBox::from_coords(0, 0, 10, 10).to_int_octagon();
        let projections = oct.nearest_border_projections(&IntPoint::new(2, 5), 3);
        assert_eq!(projections.len(), 3);
        // the left border is nearest (distance 2), the up/down borders follow at distance 5
        assert_eq!(projections[0], IntPoint::new(0, 5));
        let from = IntPoint::new(2, 5).to_float();
        let d0 = from.distance(&projections[0].to_float());
        let d1 = from.distance(&projections[1].to_float());
        assert!(d0 <= d1);
        // outside points and a zero count give nothing
        assert!(
            oct.nearest_border_projections(&IntPoint::new(50, 50), 3)
                .is_empty()
        );
        assert!(
            oct.nearest_border_projections(&IntPoint::new(2, 5), 0)
                .is_empty()
        );
    }

    #[test]
    fn bounding_shape_dispatch() {
        let b = IntBox::from_coords(0, 0, 10, 10);
        assert_eq!(
            b.bounding_shape(ShapeBoundingDirections::Orthogonal),
            RegularTileShape::Box(b)
        );
        assert_eq!(
            b.bounding_shape(ShapeBoundingDirections::FortyfiveDegree),
            RegularTileShape::Octagon(b.to_int_octagon())
        );
        let oct = b.to_int_octagon();
        assert_eq!(
            oct.bounding_shape(ShapeBoundingDirections::Orthogonal),
            RegularTileShape::Box(b)
        );
        assert_eq!(
            ShapeBoundingDirections::Orthogonal.bounds_tile(&TileShape::Octagon(oct)),
            Some(RegularTileShape::Box(b))
        );
    }
}
