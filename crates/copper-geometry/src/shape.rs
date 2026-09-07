use crate::bounding_directions::ShapeBoundingDirections;
use crate::circle::Circle;
use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::point::Point;
use crate::polygon_shape::PolygonShape;
use crate::polyline::{Polyline, PolylineError};
use crate::polyline_area::PolylineArea;
use crate::polyline_shape::PolylineShapeOps;
use crate::regular_tile_shape::RegularTileShape;
use crate::simplex::Simplex;
use crate::tile_shape::TileShape;
use crate::vector::Vector;

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Tile(TileShape),
    Polygon(PolygonShape),
    Circle(Circle),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Area {
    Shape(Shape),
    Polyline(PolylineArea),
}

pub trait ShapeOps {
    fn is_empty(&self) -> bool;

    fn is_bounded(&self) -> bool;

    fn dimension(&self) -> i32;

    fn is_contained_in(&self, b: &IntBox) -> bool;

    fn get_border(&self) -> Shape;

    fn get_holes(&self) -> Vec<Shape>;

    fn bounding_box(&self) -> IntBox;

    fn bounding_octagon(&self) -> Option<IntOctagon>;

    fn contains_float(&self, point: &FloatPoint) -> bool;

    fn contains(&self, point: &Point) -> bool;

    fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint>;

    fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Shape;

    fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> Shape;

    fn translate_by(&self, vector: &Vector) -> Shape;

    fn mirror_horizontal(&self, pole: &IntPoint) -> Shape;

    fn mirror_vertical(&self, pole: &IntPoint) -> Shape;

    fn corner_approx_arr(&self) -> Vec<FloatPoint>;

    fn split_to_convex(&self) -> Option<Vec<TileShape>>;

    fn circumference(&self) -> f64;

    fn area(&self) -> f64;

    fn centre_of_gravity(&self) -> FloatPoint;

    fn is_outside(&self, point: &Point) -> bool;

    fn contains_inside(&self, point: &Point) -> bool;

    fn contains_on_border(&self, point: &Point) -> bool;

    fn distance(&self, point: &FloatPoint) -> f64;

    fn bounding_tile(&self) -> TileShape;

    fn bounding_shape(&self, dirs: ShapeBoundingDirections) -> Option<RegularTileShape>;

    fn border_distance(&self, point: &FloatPoint) -> f64;

    fn smallest_radius(&self) -> f64;

    fn enlarge(&self, offset: f64) -> Option<Shape>;

    fn intersects(&self, other: &Shape) -> bool;

    fn intersects_box(&self, other: &IntBox) -> bool;

    fn intersects_octagon(&self, other: &IntOctagon) -> bool;

    fn intersects_simplex(&self, other: &Simplex) -> bool;

    fn intersects_circle(&self, other: &Circle) -> bool;

    fn cutout(&self, polyline: &Polyline) -> Option<Result<Vec<Polyline>, PolylineError>>;
}

impl ShapeOps for TileShape {
    fn is_empty(&self) -> bool {
        TileShape::is_empty(self)
    }
    fn is_bounded(&self) -> bool {
        TileShape::is_bounded(self)
    }
    fn dimension(&self) -> i32 {
        TileShape::dimension(self)
    }
    fn is_contained_in(&self, b: &IntBox) -> bool {
        match self {
            TileShape::Box(this) => this.is_contained_in(b),
            TileShape::Octagon(this) => RegularTileShape::Octagon(*this).is_contained_in_box(b),
            TileShape::Simplex(_) => b.contains(&TileShape::bounding_box(self)),
        }
    }
    fn get_border(&self) -> Shape {
        Shape::Tile(self.clone())
    }
    fn get_holes(&self) -> Vec<Shape> {
        Vec::new()
    }
    fn bounding_box(&self) -> IntBox {
        TileShape::bounding_box(self)
    }
    fn bounding_octagon(&self) -> Option<IntOctagon> {
        TileShape::bounding_octagon(self)
    }
    fn contains_float(&self, point: &FloatPoint) -> bool {
        TileShape::contains_float(self, point)
    }
    fn contains(&self, point: &Point) -> bool {
        TileShape::contains(self, point)
    }
    fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        TileShape::nearest_point_approx(self, from_point)
    }
    fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Shape {
        Shape::Tile(TileShape::turn_90_degree(self, factor, pole))
    }
    fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> Shape {
        Shape::Tile(TileShape::rotate_approx(self, angle, pole))
    }
    fn translate_by(&self, vector: &Vector) -> Shape {
        Shape::Tile(TileShape::translate_by(self, vector))
    }
    fn mirror_horizontal(&self, pole: &IntPoint) -> Shape {
        Shape::Tile(TileShape::mirror_horizontal(self, pole))
    }
    fn mirror_vertical(&self, pole: &IntPoint) -> Shape {
        Shape::Tile(TileShape::mirror_vertical(self, pole))
    }
    fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        TileShape::corner_approx_arr(self)
    }
    fn split_to_convex(&self) -> Option<Vec<TileShape>> {
        Some(TileShape::split_to_convex(self))
    }
    fn circumference(&self) -> f64 {
        TileShape::circumference(self)
    }
    fn area(&self) -> f64 {
        TileShape::area(self)
    }
    fn centre_of_gravity(&self) -> FloatPoint {
        TileShape::centre_of_gravity(self)
    }
    fn is_outside(&self, point: &Point) -> bool {
        TileShape::is_outside(self, point)
    }
    fn contains_inside(&self, point: &Point) -> bool {
        TileShape::contains_inside(self, point)
    }
    fn contains_on_border(&self, point: &Point) -> bool {
        TileShape::contains_on_border(self, point)
    }
    fn distance(&self, point: &FloatPoint) -> f64 {
        TileShape::distance(self, point)
    }
    fn bounding_tile(&self) -> TileShape {
        TileShape::bounding_tile(self)
    }
    fn bounding_shape(&self, dirs: ShapeBoundingDirections) -> Option<RegularTileShape> {
        dirs.bounds_tile(self)
    }
    fn border_distance(&self, point: &FloatPoint) -> f64 {
        TileShape::border_distance(self, point)
    }
    fn smallest_radius(&self) -> f64 {
        TileShape::smallest_radius(self)
    }
    fn enlarge(&self, offset: f64) -> Option<Shape> {
        Some(Shape::Tile(TileShape::enlarge(self, offset)))
    }
    fn intersects(&self, other: &Shape) -> bool {
        other.intersects_tile(self)
    }
    fn intersects_box(&self, other: &IntBox) -> bool {
        TileShape::intersects_box(self, other)
    }
    fn intersects_octagon(&self, other: &IntOctagon) -> bool {
        TileShape::intersects_octagon(self, other)
    }
    fn intersects_simplex(&self, other: &Simplex) -> bool {
        TileShape::intersects_simplex(self, other)
    }
    fn intersects_circle(&self, other: &Circle) -> bool {
        other.intersects_tile(self)
    }
    fn cutout(&self, polyline: &Polyline) -> Option<Result<Vec<Polyline>, PolylineError>> {
        Some(TileShape::cutout_polyline(self, polyline))
    }
}

impl ShapeOps for PolygonShape {
    fn is_empty(&self) -> bool {
        PolygonShape::is_empty(self)
    }
    fn is_bounded(&self) -> bool {
        PolygonShape::is_bounded(self)
    }
    fn dimension(&self) -> i32 {
        PolygonShape::dimension(self)
    }
    fn is_contained_in(&self, b: &IntBox) -> bool {
        PolylineShapeOps::is_contained_in(self, b)
    }
    fn get_border(&self) -> Shape {
        Shape::Polygon(self.clone())
    }
    fn get_holes(&self) -> Vec<Shape> {
        Vec::new()
    }
    fn bounding_box(&self) -> IntBox {
        PolygonShape::bounding_box(self)
    }
    fn bounding_octagon(&self) -> Option<IntOctagon> {
        Some(PolygonShape::bounding_octagon(self))
    }
    fn contains_float(&self, point: &FloatPoint) -> bool {
        PolygonShape::contains_float(self, point)
    }
    fn contains(&self, point: &Point) -> bool {
        PolygonShape::contains(self, point)
    }
    fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        PolygonShape::nearest_point_approx(self, from_point)
    }
    fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Shape {
        Shape::Polygon(PolygonShape::turn_90_degree(self, factor, pole))
    }
    fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> Shape {
        Shape::Polygon(PolygonShape::rotate_approx(self, angle, pole))
    }
    fn translate_by(&self, vector: &Vector) -> Shape {
        Shape::Polygon(PolygonShape::translate_by(self, vector))
    }
    fn mirror_horizontal(&self, pole: &IntPoint) -> Shape {
        Shape::Polygon(PolygonShape::mirror_horizontal(self, pole))
    }
    fn mirror_vertical(&self, pole: &IntPoint) -> Shape {
        Shape::Polygon(PolygonShape::mirror_vertical(self, pole))
    }
    fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        PolylineShapeOps::corner_approx_arr(self)
    }
    fn split_to_convex(&self) -> Option<Vec<TileShape>> {
        PolygonShape::split_to_convex(self)
    }
    fn circumference(&self) -> f64 {
        PolylineShapeOps::circumference(self)
    }
    fn area(&self) -> f64 {
        PolygonShape::area(self)
    }
    fn centre_of_gravity(&self) -> FloatPoint {
        PolylineShapeOps::centre_of_gravity(self)
    }
    fn is_outside(&self, point: &Point) -> bool {
        PolygonShape::is_outside(self, point)
    }
    fn contains_inside(&self, point: &Point) -> bool {
        PolygonShape::contains_inside(self, point)
    }
    fn contains_on_border(&self, point: &Point) -> bool {
        PolygonShape::contains_on_border(self, point)
    }
    fn distance(&self, point: &FloatPoint) -> f64 {
        PolygonShape::distance(self, point)
    }
    fn bounding_tile(&self) -> TileShape {
        PolygonShape::bounding_tile(self)
    }
    fn bounding_shape(&self, dirs: ShapeBoundingDirections) -> Option<RegularTileShape> {
        Some(PolygonShape::bounding_shape(self, dirs))
    }
    fn border_distance(&self, point: &FloatPoint) -> f64 {
        PolygonShape::border_distance(self, point)
    }
    fn smallest_radius(&self) -> f64 {
        PolygonShape::smallest_radius(self)
    }
    fn enlarge(&self, offset: f64) -> Option<Shape> {
        PolygonShape::enlarge(self, offset).map(Shape::Polygon)
    }
    fn intersects(&self, other: &Shape) -> bool {
        PolygonShape::intersects(self, other)
    }
    fn intersects_box(&self, other: &IntBox) -> bool {
        PolygonShape::intersects_box(self, other)
    }
    fn intersects_octagon(&self, other: &IntOctagon) -> bool {
        PolygonShape::intersects_octagon(self, other)
    }
    fn intersects_simplex(&self, other: &Simplex) -> bool {
        PolygonShape::intersects_simplex(self, other)
    }
    fn intersects_circle(&self, other: &Circle) -> bool {
        PolygonShape::intersects_circle(self, other)
    }
    fn cutout(&self, polyline: &Polyline) -> Option<Result<Vec<Polyline>, PolylineError>> {
        PolygonShape::cutout(self, polyline).map(Ok)
    }
}

impl ShapeOps for Circle {
    fn is_empty(&self) -> bool {
        Circle::is_empty(self)
    }
    fn is_bounded(&self) -> bool {
        Circle::is_bounded(self)
    }
    fn dimension(&self) -> i32 {
        Circle::dimension(self)
    }
    fn is_contained_in(&self, b: &IntBox) -> bool {
        Circle::is_contained_in(self, b)
    }
    fn get_border(&self) -> Shape {
        Shape::Circle(*self)
    }
    fn get_holes(&self) -> Vec<Shape> {
        Circle::get_holes(self)
    }
    fn bounding_box(&self) -> IntBox {
        Circle::bounding_box(self)
    }
    fn bounding_octagon(&self) -> Option<IntOctagon> {
        Some(Circle::bounding_octagon(self))
    }
    fn contains_float(&self, point: &FloatPoint) -> bool {
        Circle::contains_float(self, point)
    }
    fn contains(&self, point: &Point) -> bool {
        Circle::contains(self, point)
    }
    fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        Circle::nearest_point_approx(self, from_point)
    }
    fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Shape {
        Shape::Circle(Circle::turn_90_degree(self, factor, pole))
    }
    fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> Shape {
        Shape::Circle(Circle::rotate_approx(self, angle, pole))
    }
    fn translate_by(&self, vector: &Vector) -> Shape {
        Shape::Circle(Circle::translate_by(self, vector))
    }
    fn mirror_horizontal(&self, pole: &IntPoint) -> Shape {
        Shape::Circle(Circle::mirror_horizontal(self, pole))
    }
    fn mirror_vertical(&self, pole: &IntPoint) -> Shape {
        Shape::Circle(Circle::mirror_vertical(self, pole))
    }
    fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        Circle::corner_approx_arr(self)
    }
    fn split_to_convex(&self) -> Option<Vec<TileShape>> {
        Some(Circle::split_to_convex(self))
    }
    fn circumference(&self) -> f64 {
        Circle::circumference(self)
    }
    fn area(&self) -> f64 {
        Circle::area(self)
    }
    fn centre_of_gravity(&self) -> FloatPoint {
        Circle::centre_of_gravity(self)
    }
    fn is_outside(&self, point: &Point) -> bool {
        Circle::is_outside(self, point)
    }
    fn contains_inside(&self, point: &Point) -> bool {
        Circle::contains_inside(self, point)
    }
    fn contains_on_border(&self, point: &Point) -> bool {
        Circle::contains_on_border(self, point)
    }
    fn distance(&self, point: &FloatPoint) -> f64 {
        Circle::distance(self, point)
    }
    fn bounding_tile(&self) -> TileShape {
        Circle::bounding_tile(self)
    }
    fn bounding_shape(&self, dirs: ShapeBoundingDirections) -> Option<RegularTileShape> {
        Some(Circle::bounding_shape(self, dirs))
    }
    fn border_distance(&self, point: &FloatPoint) -> f64 {
        Circle::border_distance(self, point)
    }
    fn smallest_radius(&self) -> f64 {
        Circle::smallest_radius(self)
    }
    fn enlarge(&self, offset: f64) -> Option<Shape> {
        Some(Shape::Circle(Circle::enlarge(self, offset)))
    }
    fn intersects(&self, other: &Shape) -> bool {
        Circle::intersects(self, other)
    }
    fn intersects_box(&self, other: &IntBox) -> bool {
        Circle::intersects_box(self, other)
    }
    fn intersects_octagon(&self, other: &IntOctagon) -> bool {
        Circle::intersects_octagon(self, other)
    }
    fn intersects_simplex(&self, other: &Simplex) -> bool {
        Circle::intersects_simplex(self, other)
    }
    fn intersects_circle(&self, other: &Circle) -> bool {
        Circle::intersects_circle(self, other)
    }
    fn cutout(&self, polyline: &Polyline) -> Option<Result<Vec<Polyline>, PolylineError>> {
        Circle::cutout(self, polyline).map(Ok)
    }
}

impl From<TileShape> for Shape {
    fn from(value: TileShape) -> Self {
        Shape::Tile(value)
    }
}

impl From<IntBox> for Shape {
    fn from(value: IntBox) -> Self {
        Shape::Tile(TileShape::Box(value))
    }
}

impl From<IntOctagon> for Shape {
    fn from(value: IntOctagon) -> Self {
        Shape::Tile(TileShape::Octagon(value))
    }
}

impl From<PolygonShape> for Shape {
    fn from(value: PolygonShape) -> Self {
        Shape::Polygon(value)
    }
}

impl From<Circle> for Shape {
    fn from(value: Circle) -> Self {
        Shape::Circle(value)
    }
}

impl Shape {
    pub fn intersects_tile(&self, tile: &TileShape) -> bool {
        match tile {
            TileShape::Box(b) => self.intersects_box(b),
            TileShape::Octagon(o) => self.intersects_octagon(o),
            TileShape::Simplex(s) => self.intersects_simplex(s),
        }
    }

    pub fn intersects_polygon(&self, polygon: &PolygonShape) -> bool {
        match self {
            Shape::Tile(t) => polygon.intersects_tile_shape(t),
            Shape::Circle(c) => polygon.intersects_circle(c),
            Shape::Polygon(other) => polygon.intersects_polygon(other),
        }
    }
}

impl ShapeOps for Shape {
    fn is_empty(&self) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::is_empty(s),
            Shape::Polygon(s) => ShapeOps::is_empty(s),
            Shape::Circle(s) => ShapeOps::is_empty(s),
        }
    }
    fn is_bounded(&self) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::is_bounded(s),
            Shape::Polygon(s) => ShapeOps::is_bounded(s),
            Shape::Circle(s) => ShapeOps::is_bounded(s),
        }
    }
    fn dimension(&self) -> i32 {
        match self {
            Shape::Tile(s) => ShapeOps::dimension(s),
            Shape::Polygon(s) => ShapeOps::dimension(s),
            Shape::Circle(s) => ShapeOps::dimension(s),
        }
    }
    fn is_contained_in(&self, b: &IntBox) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::is_contained_in(s, b),
            Shape::Polygon(s) => ShapeOps::is_contained_in(s, b),
            Shape::Circle(s) => ShapeOps::is_contained_in(s, b),
        }
    }
    fn get_border(&self) -> Shape {
        match self {
            Shape::Tile(s) => ShapeOps::get_border(s),
            Shape::Polygon(s) => ShapeOps::get_border(s),
            Shape::Circle(s) => ShapeOps::get_border(s),
        }
    }
    fn get_holes(&self) -> Vec<Shape> {
        match self {
            Shape::Tile(s) => ShapeOps::get_holes(s),
            Shape::Polygon(s) => ShapeOps::get_holes(s),
            Shape::Circle(s) => ShapeOps::get_holes(s),
        }
    }
    fn bounding_box(&self) -> IntBox {
        match self {
            Shape::Tile(s) => ShapeOps::bounding_box(s),
            Shape::Polygon(s) => ShapeOps::bounding_box(s),
            Shape::Circle(s) => ShapeOps::bounding_box(s),
        }
    }
    fn bounding_octagon(&self) -> Option<IntOctagon> {
        match self {
            Shape::Tile(s) => ShapeOps::bounding_octagon(s),
            Shape::Polygon(s) => ShapeOps::bounding_octagon(s),
            Shape::Circle(s) => ShapeOps::bounding_octagon(s),
        }
    }
    fn contains_float(&self, point: &FloatPoint) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::contains_float(s, point),
            Shape::Polygon(s) => ShapeOps::contains_float(s, point),
            Shape::Circle(s) => ShapeOps::contains_float(s, point),
        }
    }
    fn contains(&self, point: &Point) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::contains(s, point),
            Shape::Polygon(s) => ShapeOps::contains(s, point),
            Shape::Circle(s) => ShapeOps::contains(s, point),
        }
    }
    fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        match self {
            Shape::Tile(s) => ShapeOps::nearest_point_approx(s, from_point),
            Shape::Polygon(s) => ShapeOps::nearest_point_approx(s, from_point),
            Shape::Circle(s) => ShapeOps::nearest_point_approx(s, from_point),
        }
    }
    fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Shape {
        match self {
            Shape::Tile(s) => ShapeOps::turn_90_degree(s, factor, pole),
            Shape::Polygon(s) => ShapeOps::turn_90_degree(s, factor, pole),
            Shape::Circle(s) => ShapeOps::turn_90_degree(s, factor, pole),
        }
    }
    fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> Shape {
        match self {
            Shape::Tile(s) => ShapeOps::rotate_approx(s, angle, pole),
            Shape::Polygon(s) => ShapeOps::rotate_approx(s, angle, pole),
            Shape::Circle(s) => ShapeOps::rotate_approx(s, angle, pole),
        }
    }
    fn translate_by(&self, vector: &Vector) -> Shape {
        match self {
            Shape::Tile(s) => ShapeOps::translate_by(s, vector),
            Shape::Polygon(s) => ShapeOps::translate_by(s, vector),
            Shape::Circle(s) => ShapeOps::translate_by(s, vector),
        }
    }
    fn mirror_horizontal(&self, pole: &IntPoint) -> Shape {
        match self {
            Shape::Tile(s) => ShapeOps::mirror_horizontal(s, pole),
            Shape::Polygon(s) => ShapeOps::mirror_horizontal(s, pole),
            Shape::Circle(s) => ShapeOps::mirror_horizontal(s, pole),
        }
    }
    fn mirror_vertical(&self, pole: &IntPoint) -> Shape {
        match self {
            Shape::Tile(s) => ShapeOps::mirror_vertical(s, pole),
            Shape::Polygon(s) => ShapeOps::mirror_vertical(s, pole),
            Shape::Circle(s) => ShapeOps::mirror_vertical(s, pole),
        }
    }
    fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        match self {
            Shape::Tile(s) => ShapeOps::corner_approx_arr(s),
            Shape::Polygon(s) => ShapeOps::corner_approx_arr(s),
            Shape::Circle(s) => ShapeOps::corner_approx_arr(s),
        }
    }
    fn split_to_convex(&self) -> Option<Vec<TileShape>> {
        match self {
            Shape::Tile(s) => ShapeOps::split_to_convex(s),
            Shape::Polygon(s) => ShapeOps::split_to_convex(s),
            Shape::Circle(s) => ShapeOps::split_to_convex(s),
        }
    }
    fn circumference(&self) -> f64 {
        match self {
            Shape::Tile(s) => ShapeOps::circumference(s),
            Shape::Polygon(s) => ShapeOps::circumference(s),
            Shape::Circle(s) => ShapeOps::circumference(s),
        }
    }
    fn area(&self) -> f64 {
        match self {
            Shape::Tile(s) => ShapeOps::area(s),
            Shape::Polygon(s) => ShapeOps::area(s),
            Shape::Circle(s) => ShapeOps::area(s),
        }
    }
    fn centre_of_gravity(&self) -> FloatPoint {
        match self {
            Shape::Tile(s) => ShapeOps::centre_of_gravity(s),
            Shape::Polygon(s) => ShapeOps::centre_of_gravity(s),
            Shape::Circle(s) => ShapeOps::centre_of_gravity(s),
        }
    }
    fn is_outside(&self, point: &Point) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::is_outside(s, point),
            Shape::Polygon(s) => ShapeOps::is_outside(s, point),
            Shape::Circle(s) => ShapeOps::is_outside(s, point),
        }
    }
    fn contains_inside(&self, point: &Point) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::contains_inside(s, point),
            Shape::Polygon(s) => ShapeOps::contains_inside(s, point),
            Shape::Circle(s) => ShapeOps::contains_inside(s, point),
        }
    }
    fn contains_on_border(&self, point: &Point) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::contains_on_border(s, point),
            Shape::Polygon(s) => ShapeOps::contains_on_border(s, point),
            Shape::Circle(s) => ShapeOps::contains_on_border(s, point),
        }
    }
    fn distance(&self, point: &FloatPoint) -> f64 {
        match self {
            Shape::Tile(s) => ShapeOps::distance(s, point),
            Shape::Polygon(s) => ShapeOps::distance(s, point),
            Shape::Circle(s) => ShapeOps::distance(s, point),
        }
    }
    fn bounding_tile(&self) -> TileShape {
        match self {
            Shape::Tile(s) => ShapeOps::bounding_tile(s),
            Shape::Polygon(s) => ShapeOps::bounding_tile(s),
            Shape::Circle(s) => ShapeOps::bounding_tile(s),
        }
    }
    fn bounding_shape(&self, dirs: ShapeBoundingDirections) -> Option<RegularTileShape> {
        match self {
            Shape::Tile(s) => ShapeOps::bounding_shape(s, dirs),
            Shape::Polygon(s) => ShapeOps::bounding_shape(s, dirs),
            Shape::Circle(s) => ShapeOps::bounding_shape(s, dirs),
        }
    }
    fn border_distance(&self, point: &FloatPoint) -> f64 {
        match self {
            Shape::Tile(s) => ShapeOps::border_distance(s, point),
            Shape::Polygon(s) => ShapeOps::border_distance(s, point),
            Shape::Circle(s) => ShapeOps::border_distance(s, point),
        }
    }
    fn smallest_radius(&self) -> f64 {
        match self {
            Shape::Tile(s) => ShapeOps::smallest_radius(s),
            Shape::Polygon(s) => ShapeOps::smallest_radius(s),
            Shape::Circle(s) => ShapeOps::smallest_radius(s),
        }
    }
    fn enlarge(&self, offset: f64) -> Option<Shape> {
        match self {
            Shape::Tile(s) => ShapeOps::enlarge(s, offset),
            Shape::Polygon(s) => ShapeOps::enlarge(s, offset),
            Shape::Circle(s) => ShapeOps::enlarge(s, offset),
        }
    }
    fn intersects(&self, other: &Shape) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::intersects(s, other),
            Shape::Polygon(s) => ShapeOps::intersects(s, other),
            Shape::Circle(s) => ShapeOps::intersects(s, other),
        }
    }
    fn intersects_box(&self, other: &IntBox) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::intersects_box(s, other),
            Shape::Polygon(s) => ShapeOps::intersects_box(s, other),
            Shape::Circle(s) => ShapeOps::intersects_box(s, other),
        }
    }
    fn intersects_octagon(&self, other: &IntOctagon) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::intersects_octagon(s, other),
            Shape::Polygon(s) => ShapeOps::intersects_octagon(s, other),
            Shape::Circle(s) => ShapeOps::intersects_octagon(s, other),
        }
    }
    fn intersects_simplex(&self, other: &Simplex) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::intersects_simplex(s, other),
            Shape::Polygon(s) => ShapeOps::intersects_simplex(s, other),
            Shape::Circle(s) => ShapeOps::intersects_simplex(s, other),
        }
    }
    fn intersects_circle(&self, other: &Circle) -> bool {
        match self {
            Shape::Tile(s) => ShapeOps::intersects_circle(s, other),
            Shape::Polygon(s) => ShapeOps::intersects_circle(s, other),
            Shape::Circle(s) => ShapeOps::intersects_circle(s, other),
        }
    }
    fn cutout(&self, polyline: &Polyline) -> Option<Result<Vec<Polyline>, PolylineError>> {
        match self {
            Shape::Tile(s) => ShapeOps::cutout(s, polyline),
            Shape::Polygon(s) => ShapeOps::cutout(s, polyline),
            Shape::Circle(s) => ShapeOps::cutout(s, polyline),
        }
    }
}

impl From<Shape> for Area {
    fn from(value: Shape) -> Self {
        Area::Shape(value)
    }
}

impl From<PolylineArea> for Area {
    fn from(value: PolylineArea) -> Self {
        Area::Polyline(value)
    }
}

impl Area {
    pub fn is_empty(&self) -> bool {
        match self {
            Area::Shape(s) => ShapeOps::is_empty(s),
            Area::Polyline(a) => a.is_empty(),
        }
    }

    pub fn is_bounded(&self) -> bool {
        match self {
            Area::Shape(s) => ShapeOps::is_bounded(s),
            Area::Polyline(a) => a.is_bounded(),
        }
    }

    pub fn dimension(&self) -> i32 {
        match self {
            Area::Shape(s) => ShapeOps::dimension(s),
            Area::Polyline(a) => a.dimension(),
        }
    }

    pub fn is_contained_in(&self, b: &IntBox) -> bool {
        match self {
            Area::Shape(s) => ShapeOps::is_contained_in(s, b),
            Area::Polyline(a) => a.is_contained_in(b),
        }
    }

    pub fn get_border(&self) -> Shape {
        match self {
            Area::Shape(s) => ShapeOps::get_border(s),
            Area::Polyline(a) => a.get_border().to_shape(),
        }
    }

    pub fn get_holes(&self) -> Vec<Shape> {
        match self {
            Area::Shape(s) => ShapeOps::get_holes(s),
            Area::Polyline(a) => a.get_holes().iter().map(|h| h.to_shape()).collect(),
        }
    }

    pub fn bounding_box(&self) -> IntBox {
        match self {
            Area::Shape(s) => ShapeOps::bounding_box(s),
            Area::Polyline(a) => a.bounding_box(),
        }
    }

    pub fn bounding_octagon(&self) -> Option<IntOctagon> {
        match self {
            Area::Shape(s) => ShapeOps::bounding_octagon(s),
            Area::Polyline(a) => a.bounding_octagon(),
        }
    }

    pub fn contains_float(&self, point: &FloatPoint) -> bool {
        match self {
            Area::Shape(s) => ShapeOps::contains_float(s, point),
            Area::Polyline(a) => a.contains_float(point),
        }
    }

    pub fn contains(&self, point: &Point) -> bool {
        match self {
            Area::Shape(s) => ShapeOps::contains(s, point),
            Area::Polyline(a) => a.contains(point),
        }
    }

    pub fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        match self {
            Area::Shape(s) => ShapeOps::nearest_point_approx(s, from_point),
            Area::Polyline(a) => a.nearest_point_approx(from_point),
        }
    }

    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Area {
        match self {
            Area::Shape(s) => Area::Shape(ShapeOps::turn_90_degree(s, factor, pole)),
            Area::Polyline(a) => Area::Polyline(a.turn_90_degree(factor, pole)),
        }
    }

    pub fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> Area {
        match self {
            Area::Shape(s) => Area::Shape(ShapeOps::rotate_approx(s, angle, pole)),
            Area::Polyline(a) => Area::Polyline(a.rotate_approx(angle, pole)),
        }
    }

    pub fn translate_by(&self, vector: &Vector) -> Area {
        match self {
            Area::Shape(s) => Area::Shape(ShapeOps::translate_by(s, vector)),
            Area::Polyline(a) => Area::Polyline(a.translate_by(vector)),
        }
    }

    pub fn mirror_horizontal(&self, pole: &IntPoint) -> Area {
        match self {
            Area::Shape(s) => Area::Shape(ShapeOps::mirror_horizontal(s, pole)),
            Area::Polyline(a) => Area::Polyline(a.mirror_horizontal(pole)),
        }
    }

    pub fn mirror_vertical(&self, pole: &IntPoint) -> Area {
        match self {
            Area::Shape(s) => Area::Shape(ShapeOps::mirror_vertical(s, pole)),
            Area::Polyline(a) => Area::Polyline(a.mirror_vertical(pole)),
        }
    }

    pub fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        match self {
            Area::Shape(s) => ShapeOps::corner_approx_arr(s),
            Area::Polyline(a) => a.corner_approx_arr(),
        }
    }

    pub fn split_to_convex(&self) -> Option<Vec<TileShape>> {
        match self {
            Area::Shape(s) => ShapeOps::split_to_convex(s),
            Area::Polyline(a) => a.split_to_convex(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounding_directions::ShapeBoundingDirections;
    use crate::circle::Circle;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::tile_shape::TileShape;

    fn pts(v: &[(i32, i32)]) -> Vec<Point> {
        v.iter()
            .map(|&(x, y)| Point::Int(IntPoint::new(x, y)))
            .collect()
    }

    #[test]
    fn shape_dispatch() {
        let c = Shape::Circle(Circle::new(IntPoint::new(0, 0), 10));
        let b = Shape::Tile(TileShape::Box(IntBox::from_coords(5, 5, 20, 20)));
        assert!(c.intersects(&b));
        assert!(b.intersects(&c));
        assert_eq!(b.bounding_box(), IntBox::from_coords(5, 5, 20, 20));
        assert!(matches!(
            ShapeBoundingDirections::Orthogonal.bounds_shape(&c),
            Some(crate::regular_tile_shape::RegularTileShape::Box(_))
        ));
        assert_eq!(Area::Shape(b.clone()).bounding_box(), b.bounding_box());
    }

    #[test]
    fn polygon_intersects_every_other_variant() {
        let p = Shape::Polygon(PolygonShape::from_points(&pts(&[
            (0, 0),
            (10, 0),
            (10, 10),
            (0, 10),
        ])));
        let b = Shape::Tile(TileShape::Box(IntBox::from_coords(5, 5, 20, 20)));
        let oct = Shape::Tile(TileShape::Octagon(
            IntBox::from_coords(5, 5, 20, 20).to_int_octagon(),
        ));
        let sx = Shape::Tile(TileShape::Simplex(
            IntBox::from_coords(5, 5, 20, 20).to_simplex(),
        ));
        let c = Shape::Circle(Circle::new(IntPoint::new(0, 0), 10));
        assert!(p.intersects(&b));
        assert!(b.intersects(&p));
        assert!(p.intersects(&oct));
        assert!(p.intersects(&sx));
        assert!(p.intersects(&c));
        assert!(c.intersects(&p));
    }

    #[test]
    fn all_nine_pairings_of_the_shape_enum() {
        let t = Shape::Tile(TileShape::Box(IntBox::from_coords(0, 0, 10, 10)));
        let t2 = Shape::Tile(TileShape::Box(IntBox::from_coords(5, 5, 20, 20)));
        let far = Shape::Tile(TileShape::Box(IntBox::from_coords(50, 50, 60, 60)));
        let p = Shape::Polygon(PolygonShape::from_points(&pts(&[
            (0, 0),
            (10, 0),
            (10, 10),
            (0, 10),
        ])));
        let c = Shape::Circle(Circle::new(IntPoint::new(0, 0), 10));
        let c2 = Shape::Circle(Circle::new(IntPoint::new(100, 0), 10));
        // covered by its own `#[should_panic]` test.
        assert!(t.intersects(&t2));
        assert!(!t.intersects(&far));
        assert!(t.intersects(&p));
        assert!(p.intersects(&t));
        assert!(t.intersects(&c));
        assert!(c.intersects(&t));
        assert!(p.intersects(&c));
        assert!(c.intersects(&p));
        assert!(c.intersects(&c));
        assert!(!c.intersects(&c2));
    }

    #[test]
    fn polygon_against_polygon_intersects() {
        let p1 = Shape::Polygon(PolygonShape::from_points(&pts(&[
            (0, 0),
            (10, 0),
            (10, 10),
            (0, 10),
        ])));
        let p2 = Shape::Polygon(PolygonShape::from_points(&pts(&[
            (5, 5),
            (20, 5),
            (20, 20),
            (5, 20),
        ])));
        assert!(p1.intersects(&p2));
    }

    #[test]
    fn shape_carries_the_area_interface() {
        let b = Shape::Tile(TileShape::Box(IntBox::from_coords(0, 0, 10, 10)));
        assert_eq!(b.get_border(), b);
        assert!(b.get_holes().is_empty());
        assert!(b.is_contained_in(&IntBox::from_coords(-1, -1, 11, 11)));
        assert_eq!(b.dimension(), 2);
        assert!(!b.is_empty());
        assert!(b.is_bounded());
        assert_eq!(b.area(), 100.0);
        assert_eq!(b.circumference(), 40.0);
        assert_eq!(b.centre_of_gravity(), FloatPoint::new(5.0, 5.0));
        assert_eq!(b.split_to_convex().map(|v| v.len()), Some(1));
        assert_eq!(b.corner_approx_arr().len(), 4);
        assert_eq!(
            b.turn_90_degree(4, &IntPoint::new(0, 0)),
            Shape::Tile(TileShape::Box(IntBox::from_coords(0, 0, 10, 10)))
        );
    }

    #[test]
    fn area_dispatches_to_the_polyline_variant() {
        let border = PolygonShape::from_points(&pts(&[(0, 0), (30, 0), (30, 30), (0, 30)]));
        let hole = PolygonShape::from_points(&pts(&[(10, 10), (20, 10), (20, 20), (10, 20)]));
        let area = Area::Polyline(PolylineArea::new(
            border.clone().into(),
            vec![hole.clone().into()],
        ));
        assert_eq!(area.bounding_box(), IntBox::from_coords(0, 0, 30, 30));
        assert_eq!(area.get_border(), Shape::Polygon(border));
        assert_eq!(area.get_holes(), vec![Shape::Polygon(hole)]);
        assert_eq!(area.dimension(), 2);
        assert!(area.is_bounded());
        assert!(!area.is_empty());
        assert_eq!(area.split_to_convex().map(|v| v.len()), Some(4));
        assert_eq!(area.corner_approx_arr().len(), 8);
        assert!(area.contains(&Point::Int(IntPoint::new(1, 1))));
        assert!(!area.contains(&Point::Int(IntPoint::new(15, 15))));
    }
}
