use crate::circle::Circle;
use crate::int_box::IntBox;
use crate::int_direction::IntDirection;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::point::Point;
use crate::polygon_shape::PolygonShape;
use crate::regular_tile_shape::RegularTileShape;
use crate::shape::Shape;
use crate::simplex::Simplex;
use crate::tile_shape::TileShape;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FortyfiveDegreeDirection {
    Right,
    Right45,
    Up,
    Up45,
    Left,
    Left45,
    Down,
    Down45,
}

impl FortyfiveDegreeDirection {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShapeBoundingDirections {
    Orthogonal,
    FortyfiveDegree,
}

impl ShapeBoundingDirections {
    pub fn count(self) -> usize {
        match self {
            ShapeBoundingDirections::Orthogonal => 4,
            ShapeBoundingDirections::FortyfiveDegree => 8,
        }
    }

    pub fn bounds_box(self, box_: &IntBox) -> RegularTileShape {
        match self {
            ShapeBoundingDirections::Orthogonal => RegularTileShape::Box(*box_),
            ShapeBoundingDirections::FortyfiveDegree => {
                RegularTileShape::Octagon(box_.to_int_octagon())
            }
        }
    }

    pub fn bounds_octagon(self, oct: &IntOctagon) -> RegularTileShape {
        match self {
            ShapeBoundingDirections::Orthogonal => RegularTileShape::Box(oct.bounding_box()),
            ShapeBoundingDirections::FortyfiveDegree => RegularTileShape::Octagon(*oct),
        }
    }

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

    pub fn bounds_tile(self, shape: &TileShape) -> Option<RegularTileShape> {
        match shape {
            TileShape::Box(b) => Some(self.bounds_box(b)),
            TileShape::Octagon(o) => Some(self.bounds_octagon(o)),
            TileShape::Simplex(s) => self.bounds_simplex(s),
        }
    }

    pub fn bounds_circle(self, circle: &Circle) -> RegularTileShape {
        match self {
            ShapeBoundingDirections::Orthogonal => RegularTileShape::Box(circle.bounding_box()),
            ShapeBoundingDirections::FortyfiveDegree => {
                RegularTileShape::Octagon(circle.bounding_octagon())
            }
        }
    }

    pub fn bounds_polygon(self, polygon: &PolygonShape) -> RegularTileShape {
        match self {
            ShapeBoundingDirections::Orthogonal => RegularTileShape::Box(polygon.bounding_box()),
            ShapeBoundingDirections::FortyfiveDegree => {
                RegularTileShape::Octagon(polygon.bounding_octagon())
            }
        }
    }

    pub fn bounds_shape(self, shape: &Shape) -> Option<RegularTileShape> {
        match shape {
            Shape::Tile(t) => self.bounds_tile(t),
            Shape::Polygon(p) => Some(self.bounds_polygon(p)),
            Shape::Circle(c) => Some(self.bounds_circle(c)),
        }
    }
}

impl IntBox {
    pub fn bounding_shape(&self, dirs: ShapeBoundingDirections) -> RegularTileShape {
        dirs.bounds_box(self)
    }
}

impl IntOctagon {
    pub fn bounding_shape(&self, dirs: ShapeBoundingDirections) -> RegularTileShape {
        dirs.bounds_octagon(self)
    }

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
        assert_eq!(projections[0], IntPoint::new(0, 5));
        let from = IntPoint::new(2, 5).to_float();
        let d0 = from.distance(&projections[0].to_float());
        let d1 = from.distance(&projections[1].to_float());
        assert!(d0 <= d1);
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
