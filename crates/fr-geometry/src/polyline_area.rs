//! Port of `app.freerouting.geometry.planar.PolylineArea`: "A PolylineArea is an Area, where the
//! outside border curve and the hole borders consist of straight lines" (PolylineArea.java:11-14).
//!
//! Java stores the border and every hole as an abstract `PolylineShape`, and the tree really does
//! use both concrete subclasses — `DrillPage.java:102` and `BoardOutline.java:186` pass an
//! `IntBox` border (with `TileShape[]` holes in the first case), while
//! `io/specctra/parser/Shape.java:552,591` and `HoleConstructionState.java:127` pass
//! `PolygonShape`s. The border and hole type is therefore
//! [`crate::polyline_shape::PolylineShapeRef`], not `PolygonShape`.
//!
//! Java's `splitToConvex(Stoppable)` is the copper-pour division; the `Stoppable` becomes an
//! optional stop-check closure.

use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::point::Point;
use crate::polyline_shape::{PolylineShapeOps, PolylineShapeRef};
use crate::tile_shape::TileShape;
use crate::vector::Vector;

/// An area whose outside border curve and hole borders consist of straight lines.
#[derive(Debug, Clone, PartialEq)]
pub struct PolylineArea {
    border_shape: PolylineShapeRef,
    hole_arr: Vec<PolylineShapeRef>,
}

impl PolylineArea {
    /// Creates a new instance of `PolylineArea` (PolylineArea.java:21-25).
    pub fn new(border_shape: PolylineShapeRef, hole_arr: Vec<PolylineShapeRef>) -> PolylineArea {
        PolylineArea {
            border_shape,
            hole_arr,
        }
    }

    /// Cuts `hole_piece` out of `divide_piece` and appends the 2-dimensional remainders to
    /// `pieces` (PolylineArea.java:27-36).
    ///
    /// Java's `dividePiece.cutout(holePiece)` returns `null` when `holePiece` is a `Simplex` of
    /// dimension < 2 (Simplex.java:706-710), and the loop that follows raises a
    /// `NullPointerException`; this port keeps that crash observable instead of inventing a
    /// value, panicking when `cutout` answers `None`. See the panic below.
    fn cutout_hole_piece(
        divide_piece: &TileShape,
        hole_piece: &TileShape,
        pieces: &mut Vec<TileShape>,
    ) {
        // Java raises a NullPointerException on `resultPieces.length` when `TileShape.cutout`
        // answers null (a hole piece that is a Simplex of dimension < 2). `PolylineArea.
        // splitToConvex` filters hole *shapes* of dimension < 2 one level up
        // (PolylineArea.java:177-180), but not the individual convex pieces, so the panic below
        // keeps the crash observable rather than inventing a value. Not a `// totalized:` case
        // (that tag is for crash->value divergences; this is crash->panic, i.e. the crash stays
        // a crash).
        let result_pieces = divide_piece
            .cutout(hole_piece)
            .expect("TileShape.cutout returned null: the hole piece is a Simplex of dimension < 2");
        for current_piece in result_pieces {
            if current_piece.dimension() == 2 {
                pieces.push(current_piece);
            }
        }
    }

    /// The dimension of the border shape (PolylineArea.java:38-41).
    pub fn dimension(&self) -> i32 {
        self.border_shape.as_ops().dimension()
    }

    /// Returns true if the border shape is bounded (PolylineArea.java:43-46).
    pub fn is_bounded(&self) -> bool {
        self.border_shape.as_ops().is_bounded()
    }

    /// Returns true if the border shape is empty (PolylineArea.java:48-51).
    pub fn is_empty(&self) -> bool {
        self.border_shape.as_ops().is_empty()
    }

    /// Checks if this area is completely contained in `b` (PolylineArea.java:53-56).
    pub fn is_contained_in(&self, b: &IntBox) -> bool {
        match &self.border_shape {
            // `IntBox` and `IntOctagon` override `PolylineShape.isContainedIn`.
            PolylineShapeRef::Tile(t) => crate::shape::ShapeOps::is_contained_in(t, b),
            PolylineShapeRef::Polygon(p) => PolylineShapeOps::is_contained_in(p, b),
        }
    }

    /// The border shape of this area (PolylineArea.java:58-61).
    pub fn get_border(&self) -> &PolylineShapeRef {
        &self.border_shape
    }

    /// The holes of this area (PolylineArea.java:63-66).
    pub fn get_holes(&self) -> &[PolylineShapeRef] {
        &self.hole_arr
    }

    /// The smallest surrounding box of the area (PolylineArea.java:68-71).
    pub fn bounding_box(&self) -> IntBox {
        self.border_shape.as_ops().bounding_box()
    }

    /// The smallest surrounding octagon of the area (PolylineArea.java:73-76).
    pub fn bounding_octagon(&self) -> Option<IntOctagon> {
        self.border_shape.bounding_octagon()
    }

    /// Returns true if `point` is contained in this area, but not inside a hole
    /// (PolylineArea.java:78-89).
    pub fn contains_float(&self, point: &FloatPoint) -> bool {
        if !self.border_shape.contains_float(point) {
            return false;
        }
        !self.hole_arr.iter().any(|hole| hole.contains_float(point))
    }

    /// Returns true if `point` is inside or on the border of this area, but not inside a hole
    /// (PolylineArea.java:91-102).
    ///
    /// Note the asymmetry that Java writes: the border is tested with `contains`, the holes with
    /// `containsInside`.
    pub fn contains(&self, point: &Point) -> bool {
        if !self.border_shape.contains(point) {
            return false;
        }
        !self.hole_arr.iter().any(|hole| hole.contains_inside(point))
    }

    /// An approximation of the nearest point of the area to `from_point`
    /// (PolylineArea.java:104-118). `None` where Java returns the `null` it starts from.
    /// # Panics
    /// When the convex split fails; see the note in the body.
    pub fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        let mut min_dist = f64::MAX;
        let mut result = None;
        // Java dereferences the `null` that a failed split returns (`convexShapes.length`,
        // PolylineArea.java:109) — a NullPointerException, surfaced here as a panic.
        let convex_shapes = self
            .split_to_convex(None)
            .expect("PolylineArea.splitToConvex failed: a border or hole polygon may have selfintersections");
        for shape in &convex_shapes {
            // Java reads `currentNearestPoint.distanceSquare(...)` without a null check
            // (PolylineArea.java:110-111).
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

    /// The affine translation of the area by `vector` (PolylineArea.java:120-131).
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

    /// An approximation of the corners of the border and of every hole
    /// (PolylineArea.java:133-149).
    pub fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        let mut result = self.border_shape.as_ops().corner_approx_arr();
        for hole in &self.hole_arr {
            result.extend(hole.as_ops().corner_approx_arr());
        }
        result
    }

    /// Splits this polygon shape with holes into convex pieces (PolylineArea.java:151-204).
    ///
    /// "The result is not exact, because rounded intersections of lines are used in the result
    /// pieces. It can be made exact, if Polylines are returned instead of Polygons, so that no
    /// intersection points are needed in the result."
    ///
    /// `stop_check` replaces Java's `Stoppable`: when it answers `true` the split is abandoned
    /// and `None` is returned, exactly as Java's `stoppableThread.isStopRequested()` branch does
    /// (PolylineArea.java:189-191). Passing `None` is Java's `splitToConvex()`
    /// (PolylineArea.java:157-160).
    pub fn split_to_convex(&self, stop_check: Option<&dyn Fn() -> bool>) -> Option<Vec<TileShape>> {
        // split failed
        let convex_border_pieces = self.border_shape.split_to_convex()?;
        let mut current_piece_list: Vec<TileShape> = convex_border_pieces;
        for hole in &self.hole_arr {
            if hole.as_ops().dimension() < 2 {
                // Java: FRLogger.warn("PolylineArea. split_to_convex: dimension 2 for hole
                // expected")
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

    /// Turns this area by `factor` times 90 degree around `pole` (PolylineArea.java:206-214).
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

    /// Rotates the area around `pole` by `angle` (PolylineArea.java:216-224).
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

    /// Mirrors this area at the vertical line through `pole` (PolylineArea.java:226-234).
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

    /// Mirrors this area at the horizontal line through `pole` (PolylineArea.java:236-244).
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
        // Pinned against Java: four pieces of 200 each.
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
        // The `DrillPage.java:102` shape: an IntBox border with TileShape holes.
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
        // border (4) + hole (4)
        assert_eq!(a.corner_approx_arr().len(), 8);
    }
}
