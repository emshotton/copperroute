use std::sync::OnceLock;

use fr_geometry::{
    Area, FloatPoint, IntBox, IntPoint, PolylineArea, PolylineShapeRef, TileShape, Vector,
};

use crate::ids::ItemId;
use crate::items::header::ItemHeader;
use crate::items::{BOARD_OUTLINE_HALF_WIDTH, ItemCtx};
use crate::structure::FixedState;

#[derive(Debug, Clone)]
pub struct BoardOutline {
        pub hdr: ItemHeader,
            shapes: Vec<PolylineShapeRef>,
                                keepout_area: OnceLock<Area>,
                                keepout_lines: Option<Vec<TileShape>>,
                                        keepout_convex_pieces: OnceLock<Option<Vec<TileShape>>>,
            keepout_outside_outline: bool,
}

impl PartialEq for BoardOutline {
    fn eq(&self, other: &BoardOutline) -> bool {
        self.hdr == other.hdr
            && self.shapes == other.shapes
            && self.keepout_outside_outline == other.keepout_outside_outline
            && self.keepout_area.get() == other.keepout_area.get()
            && self.keepout_lines == other.keepout_lines
    }
}

impl BoardOutline {
                        pub fn new(hdr: ItemHeader, shapes: Vec<PolylineShapeRef>) -> BoardOutline {
        BoardOutline {
            hdr,
            shapes,
            keepout_area: OnceLock::new(),
            keepout_lines: None,
            keepout_convex_pieces: OnceLock::new(),
            keepout_outside_outline: false,
        }
    }

                        pub fn copy(&self, new_id: ItemId) -> BoardOutline {
        BoardOutline::new(
            ItemHeader::new(
                new_id,
                Vec::new(),
                self.hdr.clearance_class(),
                0,
                FixedState::SystemFixed,
            ),
            self.shapes.clone(),
        )
    }

        pub fn get_half_width(&self) -> i32 {
        BOARD_OUTLINE_HALF_WIDTH
    }

            pub fn last_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        ctx.rules.layer_structure().count().checked_sub(1).expect(
            "BoardOutline.lastLayer: the board has no layers — Java answers -1 here \
                 (BoardOutline.java:104)",
        )
    }

        pub fn tile_shape_count(&self, ctx: &ItemCtx<'_>) -> usize {
        let layer_count = ctx.rules.layer_structure().count();
        if self.keepout_outside_outline {
            self.keepout_convex_pieces(ctx)
                .map_or(0, |tiles| tiles.len() * layer_count)
        } else {
            self.line_count() * layer_count
        }
    }

                    pub fn shape_layer(&self, index: usize, ctx: &ItemCtx<'_>) -> usize {
        let layer_count = ctx.rules.layer_structure().count();
        let shape_count = self.tile_shape_count(ctx);
        if shape_count > 0 {
            index * layer_count / shape_count
        } else {
            0
        }
    }

            pub fn bounding_box(&self) -> IntBox {
        self.shapes.iter().fold(IntBox::EMPTY, |acc, shape| {
            acc.union(&shape.as_ops().bounding_box())
        })
    }

        pub fn translate_by(&mut self, vector: &Vector) {
        if let Some(keepout_area) = self.keepout_area.get_mut() {
            *keepout_area = keepout_area.translate_by(vector);
        }
        self.keepout_lines = None;
        self.keepout_convex_pieces.take();
    }

            pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) {
        if let Some(keepout_area) = self.keepout_area.get_mut() {
            *keepout_area = keepout_area.turn_90_degree(factor, pole);
        }
        self.keepout_lines = None;
        self.keepout_convex_pieces.take();
    }

            pub fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint) {
        let angle = angle_in_degree.to_radians();
        if let Some(keepout_area) = self.keepout_area.get_mut() {
            *keepout_area = keepout_area.rotate_approx(angle, pole);
        }
        self.keepout_lines = None;
        self.keepout_convex_pieces.take();
    }

            pub fn change_placement_side(&mut self, pole: &IntPoint) {
        if let Some(keepout_area) = self.keepout_area.get_mut() {
            *keepout_area = keepout_area.mirror_vertical(pole);
        }
        self.keepout_lines = None;
        self.keepout_convex_pieces.take();
    }

        pub fn shape_count(&self) -> usize {
        self.shapes.len()
    }

            pub fn get_shape(&self, index: usize) -> Option<&PolylineShapeRef> {
        self.shapes.get(index)
    }

                                            pub fn keepout_convex_pieces(&self, ctx: &ItemCtx<'_>) -> Option<&[TileShape]> {
        self.keepout_convex_pieces
            .get_or_init(|| self.get_keepout_area(ctx).split_to_convex())
            .as_deref()
    }

    pub fn get_keepout_area(&self, ctx: &ItemCtx<'_>) -> &Area {
        self.keepout_area.get_or_init(|| {
            Area::Polyline(PolylineArea::new(
                PolylineShapeRef::Tile(TileShape::Box(*ctx.bounding_box)),
                self.shapes.clone(),
            ))
        })
    }

                                pub fn get_keepout_lines(&mut self) -> &[TileShape] {
        self.keepout_lines.get_or_insert_with(Vec::new)
    }

        pub fn keepout_outside_outline_generated(&self) -> bool {
        self.keepout_outside_outline
    }

                pub fn generate_keepout_outside(&mut self, value: bool) {
        if value == self.keepout_outside_outline {
            return;
        }
        self.keepout_outside_outline = value;
    }

            pub fn line_count(&self) -> usize {
        self.shapes
            .iter()
            .map(|shape| shape.as_ops().border_line_count())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_outline_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BoardOutline>();
    }

    #[test]
    fn get_keepout_lines_is_always_empty() {
        let mut outline = BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
            Vec::new(),
        );
        assert!(outline.get_keepout_lines().is_empty());
        outline.translate_by(&Vector::new(1, 1));
        assert!(outline.get_keepout_lines().is_empty());
    }
}
