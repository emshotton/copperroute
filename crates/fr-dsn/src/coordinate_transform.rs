//! conversion.
use fr_geometry::{Circle, FloatPoint, IntBox, Line, Shape, ShapeOps, TileShape, Vector};

use crate::error::DsnError;
use crate::parser::geometry::{DsnCircle, DsnLayer, DsnPolygon, DsnRectangle, DsnShape};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoordinateTransform {
    scale_factor: f64,
    base_x: f64,
    base_y: f64,
}

impl CoordinateTransform {
    pub fn new(
        scale_factor: f64,
        base_x: f64,
        base_y: f64,
    ) -> Result<CoordinateTransform, DsnError> {
        if !scale_factor.is_finite() || scale_factor == 0.0 {
            return Err(DsnError::InvalidScaleFactor { scale_factor });
        }
        Ok(CoordinateTransform {
            scale_factor,
            base_x,
            base_y,
        })
    }

    #[must_use]
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    #[must_use]
    pub fn base_x(&self) -> f64 {
        self.base_x
    }

    #[must_use]
    pub fn base_y(&self) -> f64 {
        self.base_y
    }

    #[must_use]
    pub fn board_to_dsn(&self, value: f64) -> f64 {
        value / self.scale_factor
    }

    #[must_use]
    pub fn board_to_dsn_point(&self, point: &FloatPoint) -> [f64; 2] {
        [
            self.board_to_dsn(point.x) + self.base_x,
            self.board_to_dsn(point.y) + self.base_y,
        ]
    }

    #[must_use]
    pub fn board_to_dsn_points(&self, points: &[FloatPoint]) -> Vec<f64> {
        let mut result = Vec::with_capacity(2 * points.len());
        for point in points {
            result.push(self.board_to_dsn(point.x) + self.base_x);
            result.push(self.board_to_dsn(point.y) + self.base_y);
        }
        result
    }

    #[must_use]
    pub fn board_to_dsn_lines(&self, lines: &[Line]) -> Vec<f64> {
        let mut result = Vec::with_capacity(4 * lines.len());
        for line in lines {
            let a = FloatPoint::from_int(&line.a);
            let b = FloatPoint::from_int(&line.b);
            result.push(self.board_to_dsn(a.x) + self.base_x);
            result.push(self.board_to_dsn(a.y) + self.base_y);
            result.push(self.board_to_dsn(b.x) + self.base_x);
            result.push(self.board_to_dsn(b.y) + self.base_y);
        }
        result
    }

    #[must_use]
    pub fn board_to_dsn_vector(&self, vector: &Vector) -> [f64; 2] {
        let value = vector.to_float();
        [self.board_to_dsn(value.x), self.board_to_dsn(value.y)]
    }

    #[must_use]
    pub fn board_to_dsn_box(&self, b: &IntBox) -> [f64; 4] {
        [
            f64::from(b.ll.x) / self.scale_factor + self.base_x,
            f64::from(b.ll.y) / self.scale_factor + self.base_y,
            f64::from(b.ur.x) / self.scale_factor + self.base_x,
            f64::from(b.ur.y) / self.scale_factor + self.base_y,
        ]
    }

    #[must_use]
    pub fn board_to_dsn_shape(&self, board_shape: &Shape, layer: DsnLayer) -> Option<DsnShape> {
        match board_shape {
            Shape::Tile(TileShape::Box(b)) => Some(DsnShape::Rect(DsnRectangle::new(
                layer,
                self.board_to_dsn_box(b),
            ))),
            Shape::Tile(_) | Shape::Polygon(_) => {
                let corners = board_shape.corner_approx_arr();
                let coordinates = self.board_to_dsn_points(&corners);
                Some(DsnShape::Polygon(DsnPolygon::new(layer, coordinates)))
            }
            Shape::Circle(board_circle) => {
                Some(DsnShape::Circle(self.circle(layer, board_circle, false)))
            }
        }
    }

    #[must_use]
    pub fn board_to_dsn_rel_point(&self, point: &FloatPoint) -> [f64; 2] {
        [self.board_to_dsn(point.x), self.board_to_dsn(point.y)]
    }

    #[must_use]
    pub fn board_to_dsn_rel_points(&self, points: &[FloatPoint]) -> Vec<f64> {
        let mut result = Vec::with_capacity(2 * points.len());
        for point in points {
            result.push(self.board_to_dsn(point.x));
            result.push(self.board_to_dsn(point.y));
        }
        result
    }

    #[must_use]
    pub fn board_to_dsn_rel_box(&self, b: &IntBox) -> [f64; 4] {
        [
            f64::from(b.ll.x) / self.scale_factor,
            f64::from(b.ll.y) / self.scale_factor,
            f64::from(b.ur.x) / self.scale_factor,
            f64::from(b.ur.y) / self.scale_factor,
        ]
    }

    #[must_use]
    pub fn board_to_dsn_rel_shape(&self, board_shape: &Shape, layer: DsnLayer) -> Option<DsnShape> {
        match board_shape {
            Shape::Tile(TileShape::Box(b)) => Some(DsnShape::Rect(DsnRectangle::new(
                layer,
                self.board_to_dsn_rel_box(b),
            ))),
            Shape::Tile(_) | Shape::Polygon(_) => {
                let corners = board_shape.corner_approx_arr();
                let coordinates = self.board_to_dsn_rel_points(&corners);
                Some(DsnShape::Polygon(DsnPolygon::new(layer, coordinates)))
            }
            Shape::Circle(board_circle) => {
                Some(DsnShape::Circle(self.circle(layer, board_circle, true)))
            }
        }
    }

    fn circle(&self, layer: DsnLayer, board_circle: &Circle, relative: bool) -> DsnCircle {
        let diameter = 2.0 * self.board_to_dsn(f64::from(board_circle.radius));
        let center = FloatPoint::from_int(&board_circle.center);
        let center_coordinates = if relative {
            self.board_to_dsn_rel_point(&center)
        } else {
            self.board_to_dsn_point(&center)
        };
        DsnCircle::new(
            layer,
            [diameter, center_coordinates[0], center_coordinates[1]],
        )
    }

    #[must_use]
    pub fn dsn_to_board(&self, value: f64) -> f64 {
        value * self.scale_factor
    }

    #[must_use]
    pub fn dsn_to_board_point(&self, tuple: &[f64; 2]) -> FloatPoint {
        FloatPoint::new(
            self.dsn_to_board(tuple[0] - self.base_x),
            self.dsn_to_board(tuple[1] - self.base_y),
        )
    }

    #[must_use]
    pub fn dsn_to_board_rel(&self, tuple: &[f64; 2]) -> FloatPoint {
        FloatPoint::new(self.dsn_to_board(tuple[0]), self.dsn_to_board(tuple[1]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::IntPoint;

    #[test]
    fn board_to_dsn_vector_ignores_the_base_offsets() {
        let transform = CoordinateTransform::new(10.0, 100.0, 200.0).expect("a valid scale");
        let vector = Vector::Int(fr_geometry::IntVector::new(100, 200));
        assert_eq!(transform.board_to_dsn_vector(&vector), [10.0, 20.0]);
    }

    #[test]
    fn board_to_dsn_lines_writes_four_numbers_per_line() {
        let transform = CoordinateTransform::new(10.0, 0.0, 0.0).expect("a valid scale");
        let line = Line::new(IntPoint::new(0, 10), IntPoint::new(20, 30));
        assert_eq!(
            transform.board_to_dsn_lines(std::slice::from_ref(&line)),
            vec![0.0, 1.0, 2.0, 3.0]
        );
    }

    #[test]
    fn board_to_dsn_shape_of_a_circle_doubles_the_radius_into_a_diameter() {
        let transform = CoordinateTransform::new(10.0, 1.0, 2.0).expect("a valid scale");
        let shape = Shape::Circle(Circle::new(IntPoint::new(100, 200), 50));
        let Some(DsnShape::Circle(circle)) =
            transform.board_to_dsn_shape(&shape, DsnLayer::signal())
        else {
            panic!("a Circle becomes a Circle");
        };
        assert_eq!(circle.coor, [10.0, 11.0, 22.0]);

        let Some(DsnShape::Circle(rel)) =
            transform.board_to_dsn_rel_shape(&shape, DsnLayer::signal())
        else {
            panic!("a Circle becomes a Circle");
        };
        assert_eq!(rel.coor, [10.0, 10.0, 20.0]);
    }
}
