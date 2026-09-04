use crate::float_line::FloatLine;
use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::line::Line;
use crate::point::Point;
use crate::polygon_shape::PolygonShape;
use crate::side::Side;
use crate::tile_shape::TileShape;
use crate::vector::Vector;

pub trait PolylineShapeOps {

        fn corner_is_bounded(&self, no: usize) -> bool;

        fn border_line_count(&self) -> usize;

                        fn corner(&self, no: usize) -> Point;

                    fn border_line(&self, no: usize) -> Option<Line>;

        fn is_bounded(&self) -> bool;

        fn is_empty(&self) -> bool;

            fn dimension(&self) -> i32;

        fn bounding_box(&self) -> IntBox;


            fn bounded_corners(&self) -> Vec<Point> {
        let corner_count = self.border_line_count();
        let mut result = Vec::new();
        for i in 0..corner_count {
            if self.corner_is_bounded(i) {
                result.push(self.corner(i));
            }
        }
        result
    }

                    fn corner_approx(&self, no: usize) -> Option<FloatPoint> {
        Some(self.corner(no).to_float())
    }

        fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        (0..self.border_line_count())
            .map(|i| self.corner_approx_at(i))
            .collect()
    }

            fn equals_corner(&self, point: &Point) -> Option<usize> {
        (0..self.border_line_count()).find(|&i| *point == self.corner(i))
    }

            fn circumference(&self) -> f64 {
        if !self.is_bounded() {
            return i32::MAX as f64;
        }
        let corner_count = self.border_line_count();
        if corner_count == 0 {
            return 0.0;
        }
        let mut result = 0.0;
        let mut prev_corner = self.corner_approx_at(corner_count - 1);
        for i in 0..corner_count {
            let current_corner = self.corner_approx_at(i);
            result += current_corner.distance(&prev_corner);
            prev_corner = current_corner;
        }
        result
    }

        fn centre_of_gravity(&self) -> FloatPoint {
        let corner_count = self.border_line_count();
        let mut x = 0.0;
        let mut y = 0.0;
        for i in 0..corner_count {
            let current_point = self.corner_approx_at(i);
            x += current_point.x;
            y += current_point.y;
        }
        x /= corner_count as f64;
        y /= corner_count as f64;
        FloatPoint::new(x, y)
    }

        fn is_contained_in(&self, b: &IntBox) -> bool {
        b.contains(&self.bounding_box())
    }

            fn index_of_left_most_corner(&self, from_point: &FloatPoint) -> usize {
        let corner_count = self.border_line_count();
        if corner_count == 0 {
            return 0;
        }
        let mut left_most_corner = self.corner_approx_at(0);
        let mut result = 0;
        for i in 1..corner_count {
            let current_corner = self.corner_approx_at(i);
            if current_corner.side_of(from_point, &left_most_corner) == Side::OnTheLeft {
                left_most_corner = current_corner;
                result = i;
            }
        }
        result
    }

            fn index_of_right_most_corner(&self, from_point: &FloatPoint) -> usize {
        let corner_count = self.border_line_count();
        if corner_count == 0 {
            return 0;
        }
        let mut right_most_corner = self.corner_approx_at(0);
        let mut result = 0;
        for i in 1..corner_count {
            let current_corner = self.corner_approx_at(i);
            if current_corner.side_of(from_point, &right_most_corner) == Side::OnTheRight {
                right_most_corner = current_corner;
                result = i;
            }
        }
        result
    }

                    fn polar_line_segment(&self, from_point: &FloatPoint) -> Option<FloatLine> {
        if self.is_empty() {
            // Java: FRLogger.warn("PolylineShape.polarLineSegment: shape is empty")
            return None;
        }
        let mut left_most_corner = self.corner_approx_at(0);
        let mut right_most_corner = left_most_corner;
        let corner_count = self.border_line_count();
        for i in 1..corner_count {
            let current_corner = self.corner_approx_at(i);
            if current_corner.side_of(from_point, &right_most_corner) == Side::OnTheRight {
                right_most_corner = current_corner;
            }
            if current_corner.side_of(from_point, &left_most_corner) == Side::OnTheLeft {
                left_most_corner = current_corner;
            }
        }
        Some(FloatLine::new(left_most_corner, right_most_corner))
    }

                        fn prev_no(&self, no: usize) -> usize {
        if no == 0 {
            self.border_line_count() - 1
        } else {
            no - 1
        }
    }

                    fn next_no(&self, no: usize) -> usize {
        (no + 1) % self.border_line_count()
    }

        fn intersects_line(&self, line: &Line) -> bool {
        let side_of_first_corner = line.side_of(&self.corner(0));
        if side_of_first_corner == Side::Collinear {
            return true;
        }
        for i in 1..self.border_line_count() {
            if line.side_of(&self.corner(i)) != side_of_first_corner {
                return true;
            }
        }
        false
    }

            fn left_most_corner(&self, from_point: &Point) -> Point {
        if self.is_empty() {
            return from_point.clone();
        }
        let mut result = self.corner(0);
        let corner_count = self.border_line_count();
        for i in 1..corner_count {
            let current_corner = self.corner(i);
            if current_corner.side_of(from_point, &result) == Side::OnTheLeft {
                result = current_corner;
            }
        }
        result
    }

            fn right_most_corner(&self, from_point: &Point) -> Point {
        if self.is_empty() {
            return from_point.clone();
        }
        let mut result = self.corner(0);
        let corner_count = self.border_line_count();
        for i in 1..corner_count {
            let current_corner = self.corner(i);
            if current_corner.side_of(from_point, &result) == Side::OnTheRight {
                result = current_corner;
            }
        }
        result
    }

            fn corner_approx_at(&self, no: usize) -> FloatPoint {
        self.corner_approx(no)
            .expect("corner index is below border_line_count()")
    }
}

impl PolylineShapeOps for TileShape {
    fn corner_is_bounded(&self, no: usize) -> bool {
        TileShape::corner_is_bounded(self, no)
    }
    fn border_line_count(&self) -> usize {
        TileShape::border_line_count(self)
    }
    fn corner(&self, no: usize) -> Point {
        TileShape::corner(self, no)
    }
    fn border_line(&self, no: usize) -> Option<Line> {
        TileShape::border_line(self, no)
    }
    fn is_bounded(&self) -> bool {
        TileShape::is_bounded(self)
    }
    fn is_empty(&self) -> bool {
        TileShape::is_empty(self)
    }
    fn dimension(&self) -> i32 {
        TileShape::dimension(self)
    }
    fn bounding_box(&self) -> IntBox {
        TileShape::bounding_box(self)
    }
    fn corner_approx(&self, no: usize) -> Option<FloatPoint> {
        TileShape::corner_approx(self, no)
    }
    fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        TileShape::corner_approx_arr(self)
    }
    fn circumference(&self) -> f64 {
        TileShape::circumference(self)
    }
    fn centre_of_gravity(&self) -> FloatPoint {
        TileShape::centre_of_gravity(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PolylineShapeRef {
        Tile(TileShape),
        Polygon(PolygonShape),
}

impl From<TileShape> for PolylineShapeRef {
    fn from(value: TileShape) -> Self {
        PolylineShapeRef::Tile(value)
    }
}

impl From<IntBox> for PolylineShapeRef {
    fn from(value: IntBox) -> Self {
        PolylineShapeRef::Tile(TileShape::Box(value))
    }
}

impl From<IntOctagon> for PolylineShapeRef {
    fn from(value: IntOctagon) -> Self {
        PolylineShapeRef::Tile(TileShape::Octagon(value))
    }
}

impl From<PolygonShape> for PolylineShapeRef {
    fn from(value: PolygonShape) -> Self {
        PolylineShapeRef::Polygon(value)
    }
}

impl PolylineShapeRef {
        pub fn contains_float(&self, point: &FloatPoint) -> bool {
        match self {
            PolylineShapeRef::Tile(t) => t.contains_float(point),
            PolylineShapeRef::Polygon(p) => p.contains_float(point),
        }
    }

        pub fn contains(&self, point: &Point) -> bool {
        match self {
            PolylineShapeRef::Tile(t) => t.contains(point),
            PolylineShapeRef::Polygon(p) => p.contains(point),
        }
    }

        pub fn contains_inside(&self, point: &Point) -> bool {
        match self {
            PolylineShapeRef::Tile(t) => t.contains_inside(point),
            PolylineShapeRef::Polygon(p) => p.contains_inside(point),
        }
    }

        pub fn bounding_octagon(&self) -> Option<IntOctagon> {
        match self {
            PolylineShapeRef::Tile(t) => t.bounding_octagon(),
            PolylineShapeRef::Polygon(p) => Some(p.bounding_octagon()),
        }
    }

            pub fn split_to_convex(&self) -> Option<Vec<TileShape>> {
        match self {
            PolylineShapeRef::Tile(t) => Some(t.split_to_convex()),
            PolylineShapeRef::Polygon(p) => p.split_to_convex(),
        }
    }

        pub fn translate_by(&self, vector: &Vector) -> PolylineShapeRef {
        match self {
            PolylineShapeRef::Tile(t) => PolylineShapeRef::Tile(t.translate_by(vector)),
            PolylineShapeRef::Polygon(p) => PolylineShapeRef::Polygon(p.translate_by(vector)),
        }
    }

        pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> PolylineShapeRef {
        match self {
            PolylineShapeRef::Tile(t) => PolylineShapeRef::Tile(t.turn_90_degree(factor, pole)),
            PolylineShapeRef::Polygon(p) => {
                PolylineShapeRef::Polygon(p.turn_90_degree(factor, pole))
            }
        }
    }

        pub fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> PolylineShapeRef {
        match self {
            PolylineShapeRef::Tile(t) => PolylineShapeRef::Tile(t.rotate_approx(angle, pole)),
            PolylineShapeRef::Polygon(p) => PolylineShapeRef::Polygon(p.rotate_approx(angle, pole)),
        }
    }

        pub fn mirror_vertical(&self, pole: &IntPoint) -> PolylineShapeRef {
        match self {
            PolylineShapeRef::Tile(t) => PolylineShapeRef::Tile(t.mirror_vertical(pole)),
            PolylineShapeRef::Polygon(p) => PolylineShapeRef::Polygon(p.mirror_vertical(pole)),
        }
    }

        pub fn mirror_horizontal(&self, pole: &IntPoint) -> PolylineShapeRef {
        match self {
            PolylineShapeRef::Tile(t) => PolylineShapeRef::Tile(t.mirror_horizontal(pole)),
            PolylineShapeRef::Polygon(p) => PolylineShapeRef::Polygon(p.mirror_horizontal(pole)),
        }
    }

            pub fn to_shape(&self) -> crate::shape::Shape {
        match self {
            PolylineShapeRef::Tile(t) => crate::shape::Shape::Tile(t.clone()),
            PolylineShapeRef::Polygon(p) => crate::shape::Shape::Polygon(p.clone()),
        }
    }

        pub fn as_ops(&self) -> &dyn PolylineShapeOps {
        match self {
            PolylineShapeRef::Tile(t) => t,
            PolylineShapeRef::Polygon(p) => p,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;

    fn unit_box() -> TileShape {
        TileShape::Box(IntBox::from_coords(0, 0, 10, 10))
    }

    #[test]
    fn tile_shape_implements_the_polyline_shape_members() {
        let b = unit_box();
        assert_eq!(PolylineShapeOps::border_line_count(&b), 4);
        assert_eq!(b.bounded_corners().len(), 4);
        assert_eq!(
            b.equals_corner(&Point::Int(IntPoint::new(10, 10))),
            Some(2) 
        );
        assert_eq!(b.equals_corner(&Point::Int(IntPoint::new(3, 3))), None);
        assert_eq!(PolylineShapeOps::circumference(&b), 40.0);
        assert!(b.is_contained_in(&IntBox::from_coords(-1, -1, 11, 11)));
        assert!(!b.is_contained_in(&IntBox::from_coords(1, 1, 11, 11)));
        assert_eq!(b.prev_no(0), 3);
        assert_eq!(b.next_no(3), 0);
        let seg = b
            .polar_line_segment(&FloatPoint::new(-10.0, 5.0))
            .expect("box is not empty");
        assert_eq!(seg.a, FloatPoint::new(0.0, 10.0));
        assert_eq!(seg.b, FloatPoint::new(0.0, 0.0));
        assert!(b.intersects_line(&Line::from_coords(5, -5, 5, 15)));
        assert!(!b.intersects_line(&Line::from_coords(20, -5, 20, 15)));
    }

    #[test]
    fn polyline_shape_ref_dispatches_to_both_variants() {
        let tile: PolylineShapeRef = IntBox::from_coords(0, 0, 10, 10).into();
        assert!(tile.contains(&Point::Int(IntPoint::new(5, 5))));
        assert_eq!(tile.split_to_convex().expect("box splits").len(), 1);
        let poly: PolylineShapeRef = PolygonShape::from_points(&[
            Point::Int(IntPoint::new(0, 0)),
            Point::Int(IntPoint::new(10, 0)),
            Point::Int(IntPoint::new(10, 10)),
        ])
        .into();
        assert!(poly.contains(&Point::Int(IntPoint::new(8, 4))));
        assert!(!poly.contains(&Point::Int(IntPoint::new(2, 8))));
    }
}
