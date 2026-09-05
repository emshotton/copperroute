use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::point::Point;
use crate::polyline_shape::{PolylineShapeOps, PolylineShapeRef};
use crate::tile_shape::TileShape;
use crate::vector::Vector;

#[derive(Debug, Clone, PartialEq)]
pub struct PolylineArea {
    border_shape: PolylineShapeRef,
    hole_arr: Vec<PolylineShapeRef>,
}

impl PolylineArea {
    pub fn new(border_shape: PolylineShapeRef, hole_arr: Vec<PolylineShapeRef>) -> PolylineArea {
        PolylineArea {
            border_shape,
            hole_arr,
        }
    }

    fn cutout_hole_piece(
        divide_piece: &TileShape,
        hole_piece: &TileShape,
        pieces: &mut Vec<TileShape>,
    ) {
        let result_pieces = divide_piece
            .cutout(hole_piece)
            .expect("TileShape.cutout returned null: the hole piece is a Simplex of dimension < 2");
        for current_piece in result_pieces {
            if current_piece.dimension() == 2 {
                pieces.push(current_piece);
            }
        }
    }

    pub fn dimension(&self) -> i32 {
        self.border_shape.as_ops().dimension()
    }

    pub fn is_bounded(&self) -> bool {
        self.border_shape.as_ops().is_bounded()
    }

    pub fn is_empty(&self) -> bool {
        self.border_shape.as_ops().is_empty()
    }

    pub fn is_contained_in(&self, b: &IntBox) -> bool {
        match &self.border_shape {
            PolylineShapeRef::Tile(t) => crate::shape::ShapeOps::is_contained_in(t, b),
            PolylineShapeRef::Polygon(p) => PolylineShapeOps::is_contained_in(p, b),
        }
    }

    pub fn get_border(&self) -> &PolylineShapeRef {
        &self.border_shape
    }

    pub fn get_holes(&self) -> &[PolylineShapeRef] {
        &self.hole_arr
    }

    pub fn bounding_box(&self) -> IntBox {
        self.border_shape.as_ops().bounding_box()
    }

    pub fn bounding_octagon(&self) -> Option<IntOctagon> {
        self.border_shape.bounding_octagon()
    }

    pub fn contains_float(&self, point: &FloatPoint) -> bool {
        if !self.border_shape.contains_float(point) {
            return false;
        }
        !self.hole_arr.iter().any(|hole| hole.contains_float(point))
    }

    pub fn contains(&self, point: &Point) -> bool {
        if !self.border_shape.contains(point) {
            return false;
        }
        !self.hole_arr.iter().any(|hole| hole.contains_inside(point))
    }

    pub fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        let mut min_dist = f64::MAX;
        let mut result = None;
        let convex_shapes = self
            .split_to_convex(None)
            .expect("PolylineArea.splitToConvex failed: a border or hole polygon may have selfintersections");
        for shape in &convex_shapes {
            let current_nearest_point = shape.nearest_point_approx(from_point).expect(
                "TileShape.nearestPointApprox returned null for a convex piece with no border lines",
            );
            let current_distance = current_nearest_point.distance_square(from_point);
            if current_distance < min_dist {
                min_dist = current_distance;
                result = Some(current_nearest_point);
            }
        }
        result
    }

    pub fn translate_by(&self, vector: &Vector) -> PolylineArea {
        if *vector == Vector::ZERO {
            return self.clone();
        }
        PolylineArea {
            border_shape: self.border_shape.translate_by(vector),
            hole_arr: self
                .hole_arr
                .iter()
                .map(|hole| hole.translate_by(vector))
                .collect(),
        }
    }

    pub fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        let mut result = self.border_shape.as_ops().corner_approx_arr();
        for hole in &self.hole_arr {
            result.extend(hole.as_ops().corner_approx_arr());
        }
        result
    }

    pub fn split_to_convex(&self, stop_check: Option<&dyn Fn() -> bool>) -> Option<Vec<TileShape>> {
        let convex_border_pieces = self.border_shape.split_to_convex()?;
        let mut current_piece_list: Vec<TileShape> = convex_border_pieces;
        for hole in &self.hole_arr {
            if hole.as_ops().dimension() < 2 {
                continue;
            }
            let convex_hole_pieces = hole.split_to_convex()?;
            for current_hole_piece in &convex_hole_pieces {
                let mut new_piece_list: Vec<TileShape> = Vec::new();
                for current_divide_piece in &current_piece_list {
                    if let Some(stop) = stop_check
                        && stop()
                    {
                        return None;
                    }
                    PolylineArea::cutout_hole_piece(
                        current_divide_piece,
                        current_hole_piece,
                        &mut new_piece_list,
                    );
                }
                current_piece_list = new_piece_list;
            }
        }
        Some(current_piece_list)
    }

    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> PolylineArea {
        PolylineArea {
            border_shape: self.border_shape.turn_90_degree(factor, pole),
            hole_arr: self
                .hole_arr
                .iter()
                .map(|hole| hole.turn_90_degree(factor, pole))
                .collect(),
        }
    }

    pub fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> PolylineArea {
        PolylineArea {
            border_shape: self.border_shape.rotate_approx(angle, pole),
            hole_arr: self
                .hole_arr
                .iter()
                .map(|hole| hole.rotate_approx(angle, pole))
                .collect(),
        }
    }

    pub fn mirror_vertical(&self, pole: &IntPoint) -> PolylineArea {
        PolylineArea {
            border_shape: self.border_shape.mirror_vertical(pole),
            hole_arr: self
                .hole_arr
                .iter()
                .map(|hole| hole.mirror_vertical(pole))
                .collect(),
        }
    }

    pub fn mirror_horizontal(&self, pole: &IntPoint) -> PolylineArea {
        PolylineArea {
            border_shape: self.border_shape.mirror_horizontal(pole),
            hole_arr: self
                .hole_arr
                .iter()
                .map(|hole| hole.mirror_horizontal(pole))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::polygon_shape::PolygonShape;

    fn pts(v: &[(i32, i32)]) -> Vec<Point> {
        v.iter()
            .map(|&(x, y)| Point::Int(IntPoint::new(x, y)))
            .collect()
    }

    fn square_with_a_hole() -> PolylineArea {
        let border = PolygonShape::from_points(&pts(&[(0, 0), (30, 0), (30, 30), (0, 30)]));
        let hole = PolygonShape::from_points(&pts(&[(10, 10), (20, 10), (20, 20), (10, 20)]));
        PolylineArea::new(border.into(), vec![hole.into()])
    }

    #[test]
    fn square_with_hole() {
        let a = square_with_a_hole();
        assert!(a.is_bounded());
        assert_eq!(a.bounding_box(), IntBox::from_coords(0, 0, 30, 30));
        assert!(a.contains(&Point::Int(IntPoint::new(5, 5))));
        assert!(!a.contains(&Point::Int(IntPoint::new(15, 15))));
        let parts = a.split_to_convex(None).expect("the split succeeds");
        assert_eq!(parts.len(), 4);
        assert!((parts.iter().map(|t| t.area()).sum::<f64>() - 800.0).abs() < 1e-9);
        for t in &parts {
            assert!(!t.contains_inside(&Point::Int(IntPoint::new(15, 15))));
        }
    }

    #[test]
    fn a_stop_request_abandons_the_split() {
        let a = square_with_a_hole();
        assert_eq!(a.split_to_convex(Some(&|| true)), None);
        assert!(a.split_to_convex(Some(&|| false)).is_some());
    }

    #[test]
    fn a_tile_shape_border_with_tile_shape_holes_is_representable() {
        let a = PolylineArea::new(
            IntBox::from_coords(0, 0, 30, 30).into(),
            vec![IntBox::from_coords(10, 10, 20, 20).into()],
        );
        assert_eq!(a.dimension(), 2);
        assert!(a.is_bounded());
        assert!(!a.is_empty());
        assert!(a.is_contained_in(&IntBox::from_coords(0, 0, 30, 30)));
        assert_eq!(a.bounding_box(), IntBox::from_coords(0, 0, 30, 30));
        assert_eq!(
            a.bounding_octagon(),
            Some(IntBox::from_coords(0, 0, 30, 30).to_int_octagon())
        );
        let parts = a.split_to_convex(None).expect("the split succeeds");
        assert!((parts.iter().map(|t| t.area()).sum::<f64>() - 800.0).abs() < 1e-9);
        assert!(a.contains_float(&FloatPoint::new(5.0, 5.0)));
        assert!(!a.contains_float(&FloatPoint::new(15.0, 15.0)));
        assert_eq!(
            a.nearest_point_approx(&FloatPoint::new(15.0, 15.0)),
            Some(FloatPoint::new(15.0, 10.0))
        );
    }

    #[test]
    fn transformations_map_border_and_holes_alike() {
        let a = square_with_a_hole();
        assert_eq!(a.translate_by(&Vector::ZERO), a);
        let moved = a.translate_by(&Vector::Int(crate::int_vector::IntVector::new(5, 5)));
        assert_eq!(moved.bounding_box(), IntBox::from_coords(5, 5, 35, 35));
        assert_eq!(moved.get_holes().len(), 1);
        assert_eq!(a.turn_90_degree(4, &IntPoint::new(0, 0)), a);
        assert_eq!(
            a.turn_90_degree(1, &IntPoint::new(0, 0)).bounding_box(),
            IntBox::from_coords(-30, 0, 0, 30)
        );
        assert_eq!(
            a.mirror_vertical(&IntPoint::new(0, 0))
                .mirror_vertical(&IntPoint::new(0, 0)),
            a
        );
        assert_eq!(
            a.mirror_horizontal(&IntPoint::new(0, 0))
                .mirror_horizontal(&IntPoint::new(0, 0)),
            a
        );
        assert_eq!(a.rotate_approx(0.0, &FloatPoint::new(0.0, 0.0)), a);
        assert_eq!(a.corner_approx_arr().len(), 8);
    }
}
