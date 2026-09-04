#![forbid(unsafe_code)]

pub mod bigint_aux;
pub mod bigint_direction;
pub mod bounding_directions;
pub mod circle;
pub mod direction;
pub mod ellipse;
pub mod float_line;
pub mod float_point;
pub mod int_box;
pub mod int_direction;
pub mod int_octagon;
pub mod int_point;
pub mod int_vector;
pub mod java_random;
pub mod limits;
pub mod line;
pub mod line_segment;
pub mod point;
pub mod polygon;
pub mod polygon_shape;
pub mod polyline;
pub mod polyline_area;
pub mod polyline_shape;
pub mod rational_point;
pub mod rational_vector;
pub mod regular_tile_shape;
pub mod shape;
pub mod side;
pub mod signum;
pub mod simplex;
pub mod tile_shape;
pub mod vector;

pub use bigint_direction::BigIntDirection;
pub use bounding_directions::{FortyfiveDegreeDirection, ShapeBoundingDirections};
pub use circle::Circle;
pub use direction::Direction;
pub use ellipse::Ellipse;
pub use float_line::FloatLine;
pub use float_point::FloatPoint;
pub use int_box::IntBox;
pub use int_direction::IntDirection;
pub use int_octagon::IntOctagon;
pub use int_point::IntPoint;
pub use int_vector::IntVector;
pub use java_random::JavaRandom;
pub use limits::{
    CRIT_INT, JAVA_DOUBLE_MIN_VALUE, java_max, java_max_f32, java_min, java_min_f32, java_round,
};
pub use line::Line;
pub use line_segment::LineSegment;
pub use point::Point;
pub use polygon::Polygon;
pub use polygon_shape::PolygonShape;
pub use polyline::{Polyline, PolylineError};
pub use polyline_area::PolylineArea;
pub use polyline_shape::{PolylineShapeOps, PolylineShapeRef};
pub use rational_point::RationalPoint;
pub use rational_vector::RationalVector;
pub use regular_tile_shape::RegularTileShape;
pub use shape::{Area, Shape, ShapeOps};
pub use side::Side;
pub use signum::Signum;
pub use simplex::Simplex;
pub use tile_shape::TileShape;
pub use vector::Vector;

pub mod prelude {
    pub use crate::{
        Area, BigIntDirection, CRIT_INT, Circle, Direction, Ellipse, FloatLine, FloatPoint,
        FortyfiveDegreeDirection, IntBox, IntDirection, IntOctagon, IntPoint, IntVector,
        JAVA_DOUBLE_MIN_VALUE, JavaRandom, Line, LineSegment, Point, Polygon, PolygonShape,
        Polyline, PolylineArea, PolylineError, PolylineShapeOps, PolylineShapeRef, RationalPoint,
        RationalVector, RegularTileShape, Shape, ShapeBoundingDirections, ShapeOps, Side, Signum,
        Simplex, TileShape, Vector, java_max, java_max_f32, java_min, java_min_f32, java_round,
    };
}
