use std::sync::OnceLock;

use fr_geometry::{Area, FloatPoint, IntBox, IntPoint, Point, TileShape, Vector};

use crate::ids::{ItemId, TreeId};
use crate::items::header::ItemHeader;
use crate::items::{Connectable, ItemCtx, copied_header};

#[derive(Debug, Clone)]
pub struct ObstacleAreaData {
    name: Option<String>,
    relative_area: Area,
    layer: usize,
    translation: Vector,
    rotation_in_degree: f64,
    side_changed: bool,
    absolute_area: OnceLock<Area>,
    convex_pieces: OnceLock<Option<Vec<TileShape>>>,
}

/// Compares the six real fields; the absolute-area memo is derived state (see the module doc).
impl PartialEq for ObstacleAreaData {
    fn eq(&self, other: &ObstacleAreaData) -> bool {
        self.name == other.name
            && self.relative_area == other.relative_area
            && self.layer == other.layer
            && self.translation == other.translation
            && self.rotation_in_degree == other.rotation_in_degree
            && self.side_changed == other.side_changed
    }
}

impl ObstacleAreaData {
    pub fn new(
        relative_area: Area,
        layer: usize,
        translation: Vector,
        rotation_in_degree: f64,
        side_changed: bool,
        name: Option<String>,
    ) -> ObstacleAreaData {
        ObstacleAreaData {
            name,
            relative_area,
            layer,
            translation,
            rotation_in_degree,
            side_changed,
            absolute_area: OnceLock::new(),
            convex_pieces: OnceLock::new(),
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn get_relative_area(&self) -> &Area {
        &self.relative_area
    }

    pub fn get_layer(&self) -> usize {
        self.layer
    }

    pub fn get_translation(&self) -> &Vector {
        &self.translation
    }

    pub fn get_rotation_in_degree(&self) -> f64 {
        self.rotation_in_degree
    }

    pub fn get_side_changed(&self) -> bool {
        self.side_changed
    }

    pub fn get_area(&self, ctx: &ItemCtx<'_>) -> &Area {
        self.absolute_area.get_or_init(|| {
            absolute_area_of(
                &self.relative_area,
                self.side_changed,
                self.rotation_in_degree,
                &self.translation,
                ctx.components.get_flip_style_rotate_first(),
            )
        })
    }

    pub fn bounding_box(&self, ctx: &ItemCtx<'_>) -> IntBox {
        self.get_area(ctx).bounding_box()
    }

    pub fn split_to_convex(&self, ctx: &ItemCtx<'_>) -> Option<&[TileShape]> {
        self.convex_pieces
            .get_or_init(|| self.get_area(ctx).split_to_convex())
            .as_deref()
    }

    pub fn tile_shape_count(&self, ctx: &ItemCtx<'_>) -> usize {
        self.split_to_convex(ctx).map_or(0, <[TileShape]>::len)
    }

    pub fn get_tile_shape(&self, index: usize, ctx: &ItemCtx<'_>) -> Option<TileShape> {
        self.split_to_convex(ctx)?.get(index).cloned()
    }

    fn translate_by(&mut self, vector: &Vector) {
        self.translation = self.translation.add(vector);
        self.absolute_area.take();
        self.convex_pieces.take();
    }

    fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) {
        self.rotation_in_degree =
            wrap_into_a_full_turn(self.rotation_in_degree + f64::from(factor) * 90.0);
        let rel_location = Point::ZERO.translate_by(&self.translation);
        self.translation = rel_location
            .turn_90_degree(factor, &Point::Int(*pole))
            .difference_by(&Point::ZERO);
        self.absolute_area.take();
        self.convex_pieces.take();
    }

    fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint, ctx: &ItemCtx<'_>) {
        let mut turn_angle = angle_in_degree;
        if self.side_changed && ctx.components.get_flip_style_rotate_first() {
            turn_angle = 360.0 - angle_in_degree;
        }
        self.rotation_in_degree = wrap_into_a_full_turn(self.rotation_in_degree + turn_angle);
        let new_translation = self
            .translation
            .to_float()
            .rotate(turn_angle.to_radians(), pole);
        self.translation = Point::Int(new_translation.round()).difference_by(&Point::ZERO);
        self.absolute_area.take();
        self.convex_pieces.take();
    }

    fn change_placement_side(&mut self, pole: &IntPoint, ctx: &ItemCtx<'_>) {
        self.side_changed = !self.side_changed;
        let layer_count = ctx.rules.layer_structure().count();
        self.layer = layer_count.checked_sub(self.layer + 1).expect(
            "ObstacleArea.changePlacementSide: layer >= board.getLayerCount() — Java stores a \
             negative layer here (ObstacleArea.java:250)",
        );
        let rel_location = Point::ZERO.translate_by(&self.translation);
        self.translation = rel_location
            .mirror_vertical(&Point::Int(*pole))
            .difference_by(&Point::ZERO);
        self.absolute_area.take();
        self.convex_pieces.take();
    }

    fn clear_derived_data(&mut self) {
        self.absolute_area.take();
        self.convex_pieces.take();
    }

    fn copied(&self) -> ObstacleAreaData {
        ObstacleAreaData::new(
            self.relative_area.clone(),
            self.layer,
            self.translation.clone(),
            self.rotation_in_degree,
            self.side_changed,
            self.name.clone(),
        )
    }
}

fn absolute_area_of(
    relative_area: &Area,
    mirrored: bool,
    rotation_in_degree: f64,
    translation: &Vector,
    flip_style_rotate_first: bool,
) -> Area {
    let mut turned_area = relative_area.clone();
    if mirrored && !flip_style_rotate_first {
        turned_area = turned_area.mirror_vertical(&IntPoint::ZERO);
    }
    if rotation_in_degree != 0.0 {
        if rotation_in_degree % 90.0 == 0.0 {
            turned_area =
                turned_area.turn_90_degree(rotation_in_degree as i32 / 90, &IntPoint::ZERO);
        } else {
            turned_area =
                turned_area.rotate_approx(rotation_in_degree.to_radians(), &FloatPoint::ZERO);
        }
    }
    if mirrored && flip_style_rotate_first {
        turned_area = turned_area.mirror_vertical(&IntPoint::ZERO);
    }
    turned_area.translate_by(translation)
}

fn wrap_into_a_full_turn(mut rotation_in_degree: f64) -> f64 {
    while rotation_in_degree >= 360.0 {
        rotation_in_degree -= 360.0;
    }
    while rotation_in_degree < 0.0 {
        rotation_in_degree += 360.0;
    }
    rotation_in_degree
}

macro_rules! obstacle_area_impl {
    ($ty:ident) => {
        impl $ty {
            pub fn name(&self) -> Option<&str> {
                self.area.name()
            }

            pub fn get_area(&self, ctx: &ItemCtx<'_>) -> &Area {
                self.area.get_area(ctx)
            }

            pub fn get_relative_area(&self) -> &Area {
                self.area.get_relative_area()
            }

            pub fn get_layer(&self) -> usize {
                self.area.get_layer()
            }

            pub fn get_translation(&self) -> &Vector {
                self.area.get_translation()
            }

            pub fn get_rotation_in_degree(&self) -> f64 {
                self.area.get_rotation_in_degree()
            }

            pub fn get_side_changed(&self) -> bool {
                self.area.get_side_changed()
            }

            pub fn bounding_box(&self, ctx: &ItemCtx<'_>) -> IntBox {
                self.area.bounding_box(ctx)
            }

            pub fn tile_shape_count(&self, ctx: &ItemCtx<'_>) -> usize {
                self.area.tile_shape_count(ctx)
            }

            pub fn get_tile_shape(&self, index: usize, ctx: &ItemCtx<'_>) -> Option<TileShape> {
                self.area.get_tile_shape(index, ctx)
            }

            pub fn split_to_convex(&self, ctx: &ItemCtx<'_>) -> Option<&[TileShape]> {
                self.area.split_to_convex(ctx)
            }

            pub fn translate_by(&mut self, vector: &Vector) {
                self.area.translate_by(vector);
                self.hdr.clear_derived_data();
            }

            pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) {
                self.area.turn_90_degree(factor, pole);
                self.hdr.clear_derived_data();
            }

            pub fn rotate_approx(
                &mut self,
                angle_in_degree: f64,
                pole: &FloatPoint,
                ctx: &ItemCtx<'_>,
            ) {
                self.area.rotate_approx(angle_in_degree, pole, ctx);
                self.hdr.clear_derived_data();
            }

            pub fn change_placement_side(&mut self, pole: &IntPoint, ctx: &ItemCtx<'_>) {
                self.area.change_placement_side(pole, ctx);
                self.hdr.clear_derived_data();
            }

            pub fn clear_derived_data(&mut self) {
                self.hdr.clear_derived_data();
                self.area.clear_derived_data();
            }
        }
    };
}

// ---------------------------------------------------------------------------------------------
// ObstacleArea
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ObstacleArea {
    pub hdr: ItemHeader,
    pub area: ObstacleAreaData,
}

impl ObstacleArea {
    pub fn new(hdr: ItemHeader, area: ObstacleAreaData) -> ObstacleArea {
        ObstacleArea { hdr, area }
    }

    pub fn copy(&self, new_id: ItemId) -> ObstacleArea {
        ObstacleArea {
            hdr: copied_header(&self.hdr, new_id),
            area: self.area.copied(),
        }
    }
}
obstacle_area_impl!(ObstacleArea);

// ---------------------------------------------------------------------------------------------
// ConductionArea
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ConductionArea {
    pub hdr: ItemHeader,
    pub area: ObstacleAreaData,
    is_obstacle: bool,
    is_filled: bool,
}

impl ConductionArea {
    pub fn new(hdr: ItemHeader, area: ObstacleAreaData, is_obstacle: bool) -> ConductionArea {
        ConductionArea {
            hdr,
            area,
            is_obstacle,
            is_filled: true,
        }
    }

    pub fn get_is_obstacle(&self) -> bool {
        self.is_obstacle
    }

    pub fn set_is_obstacle(&mut self, value: bool) {
        self.is_obstacle = value;
    }

    pub fn get_is_filled(&self) -> bool {
        self.is_filled
    }

    pub fn set_is_filled(&mut self, value: bool) {
        self.is_filled = value;
        self.clear_derived_data();
    }

    pub fn copy(&self, new_id: ItemId) -> Option<ConductionArea> {
        if self.hdr.net_count() > 1 {
            return None;
        }
        Some(ConductionArea {
            hdr: copied_header(&self.hdr, new_id),
            area: self.area.copied(),
            is_obstacle: self.is_obstacle,
            is_filled: true,
        })
    }

    pub fn warm_detailed_fill_cache(&self) {}
}
obstacle_area_impl!(ConductionArea);

impl Connectable for ConductionArea {
    fn header(&self) -> &ItemHeader {
        &self.hdr
    }

    fn get_trace_connection_shape(
        &self,
        tree: TreeId,
        index: usize,
        _ctx: &ItemCtx<'_>,
    ) -> Option<TileShape> {
        self.hdr
            .get_precalculated_tree_shapes(tree)
            .and_then(|shapes| shapes.get(index))
            .cloned()
            .flatten()
    }
}

// ---------------------------------------------------------------------------------------------
// ViaObstacleArea
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ViaObstacleArea {
    pub hdr: ItemHeader,
    pub area: ObstacleAreaData,
}

impl ViaObstacleArea {
    pub fn new(hdr: ItemHeader, area: ObstacleAreaData) -> ViaObstacleArea {
        ViaObstacleArea { hdr, area }
    }

    pub fn copy(&self, new_id: ItemId) -> ViaObstacleArea {
        ViaObstacleArea {
            hdr: copied_header(&self.hdr, new_id),
            area: self.area.copied(),
        }
    }
}
obstacle_area_impl!(ViaObstacleArea);

// ---------------------------------------------------------------------------------------------
// ComponentObstacleArea
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentObstacleArea {
    pub hdr: ItemHeader,
    pub area: ObstacleAreaData,
}

impl ComponentObstacleArea {
    pub fn new(hdr: ItemHeader, area: ObstacleAreaData) -> ComponentObstacleArea {
        ComponentObstacleArea { hdr, area }
    }

    pub fn copy(&self, new_id: ItemId) -> ComponentObstacleArea {
        ComponentObstacleArea {
            hdr: ItemHeader::new(
                new_id,
                Vec::new(),
                self.hdr.clearance_class(),
                self.hdr.get_component_id(),
                self.hdr.get_fixed_state(),
            ),
            area: self.area.copied(),
        }
    }
}
obstacle_area_impl!(ComponentObstacleArea);

// ---------------------------------------------------------------------------------------------
// ComponentOutline
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ComponentOutline {
    pub hdr: ItemHeader,
    relative_area: Area,
    translation: Vector,
    rotation_in_degree: f64,
    is_front: bool,
    is_courtyard: bool,
    is_fabrication: bool,
    is_closed: bool,
    absolute_area: OnceLock<Area>,
}

/// Compares the seven real fields; the absolute-area memo is derived state.
impl PartialEq for ComponentOutline {
    fn eq(&self, other: &ComponentOutline) -> bool {
        self.hdr == other.hdr
            && self.relative_area == other.relative_area
            && self.translation == other.translation
            && self.rotation_in_degree == other.rotation_in_degree
            && self.is_front == other.is_front
            && self.is_courtyard == other.is_courtyard
            && self.is_fabrication == other.is_fabrication
            && self.is_closed == other.is_closed
    }
}

impl ComponentOutline {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hdr: ItemHeader,
        relative_area: Area,
        is_front: bool,
        translation: Vector,
        rotation_in_degree: f64,
        is_courtyard: bool,
        is_fabrication: bool,
        is_closed: bool,
    ) -> ComponentOutline {
        ComponentOutline {
            hdr,
            relative_area,
            translation,
            rotation_in_degree,
            is_front,
            is_courtyard,
            is_fabrication,
            is_closed,
            absolute_area: OnceLock::new(),
        }
    }

    pub fn copy(&self, new_id: ItemId) -> ComponentOutline {
        ComponentOutline::new(
            ItemHeader::new(
                new_id,
                Vec::new(),
                0,
                self.hdr.get_component_id(),
                self.hdr.get_fixed_state(),
            ),
            self.relative_area.clone(),
            self.is_front,
            self.translation.clone(),
            self.rotation_in_degree,
            self.is_courtyard,
            self.is_fabrication,
            self.is_closed,
        )
    }

    pub fn is_front(&self) -> bool {
        self.is_front
    }

    pub fn is_courtyard(&self) -> bool {
        self.is_courtyard
    }

    pub fn is_fabrication(&self) -> bool {
        self.is_fabrication
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    pub fn get_translation(&self) -> &Vector {
        &self.translation
    }

    pub fn get_rotation_in_degree(&self) -> f64 {
        self.rotation_in_degree
    }

    pub fn get_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        if self.is_front {
            0
        } else {
            ctx.rules.layer_structure().count().checked_sub(1).expect(
                "ComponentOutline.getLayer: the board has no layers — Java answers -1 here \
                     (ComponentOutline.java:99)",
            )
        }
    }

    pub fn get_area(&self, ctx: &ItemCtx<'_>) -> &Area {
        self.absolute_area.get_or_init(|| {
            absolute_area_of(
                &self.relative_area,
                !self.is_front,
                self.rotation_in_degree,
                &self.translation,
                ctx.components.get_flip_style_rotate_first(),
            )
        })
    }

    pub fn tile_shape_count(&self) -> usize {
        0
    }

    pub fn bounding_box(&self, ctx: &ItemCtx<'_>) -> IntBox {
        self.get_area(ctx).bounding_box()
    }

    pub fn translate_by(&mut self, vector: &Vector) {
        self.translation = self.translation.add(vector);
        self.clear_derived_data();
    }

    pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) {
        self.rotation_in_degree =
            wrap_into_a_full_turn(self.rotation_in_degree + f64::from(factor) * 90.0);
        let rel_location = Point::ZERO.translate_by(&self.translation);
        self.translation = rel_location
            .turn_90_degree(factor, &Point::Int(*pole))
            .difference_by(&Point::ZERO);
        self.clear_derived_data();
    }

    pub fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint, ctx: &ItemCtx<'_>) {
        let mut turn_angle = angle_in_degree;
        if !self.is_front && ctx.components.get_flip_style_rotate_first() {
            turn_angle = 360.0 - angle_in_degree;
        }
        self.rotation_in_degree = wrap_into_a_full_turn(self.rotation_in_degree + turn_angle);
        let new_translation = self
            .translation
            .to_float()
            .rotate(turn_angle.to_radians(), pole);
        self.translation = Point::Int(new_translation.round()).difference_by(&Point::ZERO);
        self.clear_derived_data();
    }

    pub fn change_placement_side(&mut self, pole: &IntPoint) {
        self.is_front = !self.is_front;
        let rel_location = Point::ZERO.translate_by(&self.translation);
        self.translation = rel_location
            .mirror_vertical(&Point::Int(*pole))
            .difference_by(&Point::ZERO);
        self.clear_derived_data();
    }

    pub fn clear_derived_data(&mut self) {
        self.hdr.clear_derived_data();
        self.absolute_area.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_types_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ObstacleAreaData>();
        assert_send_sync::<ObstacleArea>();
        assert_send_sync::<ConductionArea>();
        assert_send_sync::<ViaObstacleArea>();
        assert_send_sync::<ComponentObstacleArea>();
        assert_send_sync::<ComponentOutline>();
    }

    #[test]
    fn wrap_into_a_full_turn_matches_javas_two_while_loops() {
        assert_eq!(wrap_into_a_full_turn(300.0 + 180.0), 120.0);
        assert_eq!(wrap_into_a_full_turn(30.0 - 180.0), 210.0);
        assert_eq!(wrap_into_a_full_turn(0.0), 0.0);
        assert_eq!(wrap_into_a_full_turn(360.0), 0.0);
        assert_eq!(wrap_into_a_full_turn(-360.0), 0.0);
        assert_eq!(wrap_into_a_full_turn(359.5), 359.5);
    }

    #[test]
    fn partial_eq_ignores_the_absolute_area_memo() {
        // The memo is derived state, so two areas that differ only in what has been computed so
        // far are equal.
        let data = || {
            ObstacleAreaData::new(
                Area::Shape(fr_geometry::Shape::Tile(TileShape::Box(
                    IntBox::from_coords(0, 0, 10, 10),
                ))),
                0,
                Vector::new(1, 2),
                0.0,
                false,
                None,
            )
        };
        let a = data();
        let b = data();
        a.absolute_area
            .set(Area::Shape(fr_geometry::Shape::Tile(TileShape::Box(
                IntBox::from_coords(1, 2, 11, 12),
            ))))
            .expect("a fresh OnceLock is empty");
        assert_eq!(a, b);
    }
}
