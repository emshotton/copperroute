
use fr_geometry::{
    FloatPoint, IntBox, IntOctagon, IntPoint, Line, LineSegment, Point, Polyline, PolylineError,
    TileShape, Vector,
};

use crate::ids::{ItemId, TreeId};
use crate::items::header::ItemHeader;
use crate::items::{Connectable, ItemCtx, copied_header};

#[derive(Debug, Clone, PartialEq)]
pub struct PolylineTrace {
        pub hdr: ItemHeader,
        lines: Polyline,
        layer: usize,
        half_width: i32,
}

impl PolylineTrace {
                                                                                    pub fn new(
        hdr: ItemHeader,
        lines: Polyline,
        layer: usize,
        half_width: i32,
        layer_count: Option<usize>,
    ) -> PolylineTrace {
        let layer = match layer_count {
            Some(count) => layer.min(count.checked_sub(1).expect(
                "Trace(...): board.getLayerCount() is 0, so Java stores a negative layer \
                 (Trace.java:46)",
            )),
            None => layer,
        };
        PolylineTrace {
            hdr,
            lines,
            layer,
            half_width,
        }
    }

                                        pub fn copy(&self, new_id: ItemId) -> PolylineTrace {
        PolylineTrace {
            hdr: copied_header(&self.hdr, new_id),
            lines: self.lines.clone(),
            layer: self.layer,
            half_width: self.half_width,
        }
    }


        pub fn polyline(&self) -> &Polyline {
        &self.lines
    }

                            pub fn set_polyline(&mut self, new_polyline: Polyline) {
        self.lines = new_polyline;
    }

        pub fn get_layer(&self) -> usize {
        self.layer
    }

            pub fn set_layer(&mut self, layer: usize) {
        self.layer = layer;
    }

        pub fn first_layer(&self) -> usize {
        self.get_layer()
    }

        pub fn last_layer(&self) -> usize {
        self.get_layer()
    }

        pub fn get_half_width(&self) -> i32 {
        self.half_width
    }


                        pub fn first_corner(&self) -> Option<Point> {
        self.lines.corner(0)
    }

                pub fn last_corner(&self) -> Option<Point> {
        self.lines.last_corner()
    }

                pub fn corner_count(&self) -> usize {
        self.lines.corner_count()
    }

            pub fn get_length(&self) -> f64 {
        self.lines.length_approx()
    }

                        pub fn bounding_box(&self) -> IntBox {
        self.lines.bounding_box().offset(f64::from(self.half_width))
    }

                        pub fn tile_shape_count(&self) -> usize {
        self.lines.lines().len().saturating_sub(2)
    }

                                pub fn offset_shapes(&self, half_width: i32) -> Vec<TileShape> {
        self.lines.offset_shapes(half_width)
    }

                            pub fn nearest_end_point(&self, from_point: &Point) -> Option<Point> {
        let p1 = self.first_corner()?;
        let p2 = self.last_corner()?;
        let from_point_float = from_point.to_float();
        let d1 = from_point_float.distance(&p1.to_float());
        let d2 = from_point_float.distance(&p2.to_float());
        Some(if d1 < d2 { p1 } else { p2 })
    }


                        pub fn translate_by(&mut self, vector: &Vector) -> Result<(), PolylineError> {
        self.lines = self.lines.translate_by(vector)?;
        self.hdr.clear_derived_data();
        Ok(())
    }

            pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) -> Result<(), PolylineError> {
        self.lines = self.lines.turn_90_degree(factor, pole)?;
        self.hdr.clear_derived_data();
        Ok(())
    }

            pub fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint) {
        self.lines = self.lines.rotate_approx(angle_in_degree.to_radians(), pole);
    }

                                            pub fn change_placement_side(
        &mut self,
        pole: &IntPoint,
        ctx: &ItemCtx<'_>,
    ) -> Result<(), PolylineError> {
        self.lines = self.lines.mirror_vertical(pole)?;
        let layer_count = ctx.rules.layer_structure().count();
        self.set_layer(layer_count.checked_sub(self.get_layer() + 1).expect(
            "PolylineTrace.changePlacementSide: layer >= board.getLayerCount() — Java stores a \
             negative layer here (PolylineTrace.java:165)",
        ));
        self.hdr.clear_derived_data();
        Ok(())
    }


                                pub fn split_polyline_at_line(
        &self,
        line_index: usize,
        new_end_line: &Line,
    ) -> Result<Option<[Polyline; 2]>, PolylineError> {
        self.lines.split(line_index, new_end_line)
    }

                                                        pub fn perpendicular_split_line(&self, segment_index: usize, point: &Point) -> Option<Line> {
        let segment = LineSegment::from_polyline(&self.lines, segment_index + 1)?;
        if !segment.contains(point) {
            return None;
        }
        let Point::Int(int_point) = point else {
            return None;
        };
        let split_line_direction = segment.get_line().direction().turn_45_degree(2);
        Some(Line::from_direction(*int_point, &split_line_direction))
    }

                                                                    pub fn split_polyline_at_point(
        &self,
        point: &Point,
    ) -> Result<Option<[Polyline; 2]>, PolylineError> {
        for i in 0..self.tile_shape_count() {
            let Some(split_line) = self.perpendicular_split_line(i, point) else {
                continue;
            };
            if let Some(pieces) = self.split_polyline_at_line(i + 1, &split_line)? {
                return Ok(Some(pieces));
            }
        }
        Ok(None)
    }

                        pub fn clip_intersects_segment(&self, index: usize, clip: &IntOctagon) -> bool {
        match LineSegment::from_polyline(&self.lines, index + 1) {
            Some(segment) => clip.intersects_box(&segment.bounding_box()),
            None => false,
        }
    }
}

impl Connectable for PolylineTrace {
    fn header(&self) -> &ItemHeader {
        &self.hdr
    }

                            fn get_trace_connection_shape(
        &self,
        _tree: TreeId,
        index: usize,
        _ctx: &ItemCtx<'_>,
    ) -> Option<TileShape> {
        if index >= self.tile_shape_count() {
            return None;
        }
        let segment = LineSegment::from_polyline(&self.lines, index + 1)?;
        Some(segment.to_simplex().simplify())
    }
}
