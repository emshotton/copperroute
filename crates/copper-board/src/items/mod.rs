pub mod area;
pub mod clearance_violation;
pub mod drill;
pub mod header;
pub mod trace;

use std::cmp::Ordering;

use copper_geometry::{FloatPoint, IntBox, IntPoint, PolylineError, TileShape, Vector};

use crate::datastructures::LeafId;
use crate::ids::{ItemId, TreeId};
use crate::rules::{BoardRules, Nets};
use crate::structure::FixedState;

pub use area::{
    ComponentObstacleArea, ComponentOutline, ConductionArea, ObstacleArea, ObstacleAreaData,
    ViaObstacleArea,
};
pub use clearance_violation::ClearanceViolation;
pub use drill::{
    DEFAULT_MAX_TREE_SHAPE_WIDTH, DrillItemData, ItemCtx, Pin, TraceExitRestriction, Via,
};
pub use header::{AutorouteInfo, ItemHeader, TreeEntries};
pub use trace::PolylineTrace;

pub use crate::structure::board_outline::BoardOutline;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Trace,
    Pin,
    Via,
    ObstacleArea,
    ViaObstacleArea,
    ConductionArea,
    ComponentObstacleArea,
    BoardOutline,
    ComponentOutline,
    Other,
}

// ---------------------------------------------------------------------------------------------
// The nine variant structs.
//
// Each holds the shared `hdr` plus its own geometry and the flags `isObstacle` reads
// (`ConductionArea.isObstacle`, `Via.attachAllowed`).
// ---------------------------------------------------------------------------------------------

pub(crate) const BOARD_OUTLINE_HALF_WIDTH: i32 = 100;

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Trace(PolylineTrace),
    Via(Via),
    Pin(Pin),
    ObstacleArea(ObstacleArea),
    ConductionArea(ConductionArea),
    ViaObstacleArea(ViaObstacleArea),
    ComponentObstacleArea(ComponentObstacleArea),
    ComponentOutline(ComponentOutline),
    BoardOutline(BoardOutline),
}

impl Item {
    pub fn header(&self) -> &ItemHeader {
        match self {
            Item::Trace(i) => &i.hdr,
            Item::Via(i) => &i.hdr,
            Item::Pin(i) => &i.hdr,
            Item::ObstacleArea(i) => &i.hdr,
            Item::ConductionArea(i) => &i.hdr,
            Item::ViaObstacleArea(i) => &i.hdr,
            Item::ComponentObstacleArea(i) => &i.hdr,
            Item::ComponentOutline(i) => &i.hdr,
            Item::BoardOutline(i) => &i.hdr,
        }
    }

    /// Mutable counterpart of [`Self::header`].
    pub fn header_mut(&mut self) -> &mut ItemHeader {
        match self {
            Item::Trace(i) => &mut i.hdr,
            Item::Via(i) => &mut i.hdr,
            Item::Pin(i) => &mut i.hdr,
            Item::ObstacleArea(i) => &mut i.hdr,
            Item::ConductionArea(i) => &mut i.hdr,
            Item::ViaObstacleArea(i) => &mut i.hdr,
            Item::ComponentObstacleArea(i) => &mut i.hdr,
            Item::ComponentOutline(i) => &mut i.hdr,
            Item::BoardOutline(i) => &mut i.hdr,
        }
    }

    pub fn id(&self) -> ItemId {
        self.header().id()
    }

    pub fn compare_to(&self, other: &Item) -> Ordering {
        other.id().cmp(&self.id())
    }

    pub fn kind(&self) -> ItemKind {
        match self {
            Item::Trace(_) => ItemKind::Trace,
            Item::Via(_) => ItemKind::Via,
            Item::Pin(_) => ItemKind::Pin,
            Item::ObstacleArea(_) => ItemKind::ObstacleArea,
            Item::ConductionArea(_) => ItemKind::ConductionArea,
            Item::ViaObstacleArea(_) => ItemKind::ViaObstacleArea,
            Item::ComponentObstacleArea(_) => ItemKind::ComponentObstacleArea,
            Item::ComponentOutline(_) => ItemKind::ComponentOutline,
            Item::BoardOutline(_) => ItemKind::BoardOutline,
        }
    }

    pub fn is_trace(&self) -> bool {
        matches!(self, Item::Trace(_))
    }

    pub fn is_drill_item(&self) -> bool {
        matches!(self, Item::Via(_) | Item::Pin(_))
    }

    pub fn is_obstacle_area(&self) -> bool {
        matches!(
            self,
            Item::ObstacleArea(_)
                | Item::ConductionArea(_)
                | Item::ViaObstacleArea(_)
                | Item::ComponentObstacleArea(_)
        )
    }

    pub fn net_nos(&self) -> &[i32] {
        &self.header().net_nos
    }

    pub fn net_count(&self) -> usize {
        self.header().net_count()
    }

    pub fn get_net_number(&self, no: usize) -> i32 {
        self.header().get_net_number(no)
    }

    pub fn contains_net(&self, net_number: i32) -> bool {
        self.header().contains_net(net_number)
    }

    pub fn shares_net(&self, other: &Item) -> bool {
        self.shares_net_no(other.net_nos())
    }

    pub fn shares_net_no(&self, net_nos: &[i32]) -> bool {
        self.header().shares_net_no(net_nos)
    }

    pub fn nets_equal(&self, other: &Item) -> bool {
        self.header().nets_equal(other.net_nos())
    }

    pub fn nets_equal_to(&self, net_nos: &[i32]) -> bool {
        self.header().nets_equal(net_nos)
    }

    pub fn nets_normal(&self) -> bool {
        self.header().nets_normal()
    }

    pub fn assign_net_no(&mut self, net_number: i32, nets: &Nets) {
        self.header_mut().assign_net_no(net_number, nets);
    }

    pub fn remove_from_net(&mut self, net_number: i32) -> bool {
        self.header_mut().remove_from_net(net_number)
    }

    pub fn is_connectable(&self) -> bool {
        self.as_connectable().is_some() && self.net_count() > 0
    }

    pub fn as_connectable(&self) -> Option<ConnectableRef<'_>> {
        match self {
            Item::Trace(i) => Some(ConnectableRef::Trace(i)),
            Item::Via(i) => Some(ConnectableRef::Via(i)),
            Item::Pin(i) => Some(ConnectableRef::Pin(i)),
            Item::ConductionArea(i) => Some(ConnectableRef::ConductionArea(i)),
            _ => None,
        }
    }

    pub fn get_fixed_state(&self) -> FixedState {
        self.header().get_fixed_state()
    }

    pub fn set_fixed_state(&mut self, fixed_state: FixedState) {
        self.header_mut().set_fixed_state(fixed_state);
    }

    pub fn unfix(&mut self) {
        self.header_mut().unfix();
    }

    pub fn is_user_fixed(&self) -> bool {
        self.header().is_user_fixed()
    }

    pub fn is_shove_fixed(&self, rules: &BoardRules) -> bool {
        if self.header().is_shove_fixed() {
            return true;
        }
        if !self.is_trace() {
            return false;
        }
        self.net_nos()
            .iter()
            .filter(|n| Nets::is_normal_net_number(**n))
            .any(|n| {
                let net = rules.nets.get(*n).expect(
                    "Trace.isShoveFixed: nets.get(currentNetNumber) is null — Java throws a \
                     NullPointerException here too (Trace.java:246)",
                );
                rules.net_classes.get(net.get_net_class()).is_shove_fixed()
            })
    }

    pub fn is_deletion_forbidden(&self, rules: &BoardRules) -> bool {
        if self.header().get_component_id() > 0 || self.is_user_fixed() {
            return true;
        }
        match self {
            Item::ConductionArea(area) => {
                !rules.layer_structure().layers[area.get_layer()].is_signal
            }
            _ => false,
        }
    }

    // -- clearance class, component, board membership -------------------------------------------

    pub fn clearance_class(&self) -> usize {
        self.header().clearance_class()
    }

    pub fn set_clearance_class(&mut self, index: usize, rules: &BoardRules) {
        self.header_mut().set_clearance_class(index, rules);
    }

    pub fn component_id(&self) -> i32 {
        self.header().get_component_id()
    }

    pub fn assign_component_id(&mut self, id: i32) {
        self.header_mut().assign_component_id(id);
    }

    pub fn is_on_the_board(&self) -> bool {
        self.header().is_on_the_board()
    }

    pub fn set_on_the_board(&mut self, value: bool) {
        self.header_mut().set_on_the_board(value);
    }

    pub fn is_obstacle(&self, other: &Item, ctx: &ItemCtx<'_>) -> bool {
        match self {
            Item::Trace(_) => {
                if other.id() == self.id()
                    || matches!(
                        other,
                        Item::ViaObstacleArea(_) | Item::ComponentObstacleArea(_)
                    )
                {
                    return false;
                }
                if let Item::ConductionArea(area) = other
                    && !area.get_is_obstacle()
                {
                    return false;
                }
                !other.shares_net(self)
            }
            Item::Via(via) => {
                if other.id() == self.id() || matches!(other, Item::ComponentObstacleArea(_)) {
                    return false;
                }
                if let Item::ConductionArea(area) = other
                    && !area.get_is_obstacle()
                {
                    return false;
                }
                if !other.shares_net(self) {
                    return true;
                }
                if other.is_trace() {
                    return false;
                }
                match other {
                    Item::Pin(pin) => !via.attach_allowed || !pin.drill_allowed(ctx),
                    _ => true,
                }
            }
            Item::Pin(pin) => {
                if other.id() == self.id() || other.is_obstacle_area() {
                    return false;
                }
                if !other.shares_net(self) {
                    return true;
                }
                if other.is_trace() {
                    return false;
                }
                !pin.drill_allowed(ctx) || !matches!(other, Item::Via(_))
            }
            Item::ObstacleArea(_) => obstacle_area_is_obstacle(self, other),
            Item::ConductionArea(area) => {
                if area.get_is_obstacle() {
                    obstacle_area_is_obstacle(self, other)
                } else {
                    false
                }
            }
            Item::ViaObstacleArea(_) => {
                if other.shares_net(self) {
                    return false;
                }
                matches!(other, Item::Via(_))
            }
            Item::ComponentObstacleArea(_) => {
                other.id() != self.id()
                    && matches!(other, Item::ComponentObstacleArea(_))
                    && other.component_id() != self.component_id()
            }
            Item::ComponentOutline(_) => false,
            Item::BoardOutline(_) => {
                !(matches!(other, Item::BoardOutline(_)) || other.is_obstacle_area())
            }
        }
    }

    pub fn is_obstacle_for_net(&self, net_number: i32) -> bool {
        !self.contains_net(net_number)
    }

    #[inline]
    pub fn is_trace_obstacle(&self, net_number: i32) -> bool {
        match self {
            Item::ConductionArea(area) => area.get_is_obstacle() && !self.contains_net(net_number),
            Item::ViaObstacleArea(_) | Item::ComponentObstacleArea(_) => false,
            _ => !self.contains_net(net_number),
        }
    }

    pub fn is_drillable(&self, net_number: i32) -> bool {
        match self {
            Item::Trace(_) => self.contains_net(net_number),
            Item::ConductionArea(area) => !area.get_is_obstacle() || self.contains_net(net_number),
            _ => false,
        }
    }

    pub fn is_routable(&self) -> bool {
        match self {
            Item::Trace(_) | Item::Via(_) => !self.is_user_fixed() && self.net_count() > 0,
            _ => false,
        }
    }

    // -- layers ----------------------------------------------------------------------------------

    #[inline]
    pub fn first_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        match self {
            Item::Trace(i) => i.first_layer(),
            Item::Via(i) => i.first_layer(ctx),
            Item::Pin(i) => i.first_layer(ctx),
            Item::ObstacleArea(i) => i.get_layer(),
            Item::ConductionArea(i) => i.get_layer(),
            Item::ViaObstacleArea(i) => i.get_layer(),
            Item::ComponentObstacleArea(i) => i.get_layer(),
            Item::ComponentOutline(i) => i.get_layer(ctx),
            Item::BoardOutline(_) => 0,
        }
    }

    pub fn last_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        match self {
            Item::Trace(i) => i.last_layer(),
            Item::Via(i) => i.last_layer(ctx),
            Item::Pin(i) => i.last_layer(ctx),
            Item::ObstacleArea(i) => i.get_layer(),
            Item::ConductionArea(i) => i.get_layer(),
            Item::ViaObstacleArea(i) => i.get_layer(),
            Item::ComponentObstacleArea(i) => i.get_layer(),
            Item::ComponentOutline(i) => i.get_layer(ctx),
            Item::BoardOutline(i) => i.last_layer(ctx),
        }
    }

    pub fn is_on_layer(&self, layer: usize, ctx: &ItemCtx<'_>) -> bool {
        match self {
            Item::Via(i) => i.is_on_layer(layer, ctx),
            Item::Pin(i) => i.is_on_layer(layer, ctx),
            Item::BoardOutline(_) => true,
            _ => self.first_layer(ctx) == layer,
        }
    }

    #[inline]
    pub fn shape_layer(&self, index: usize, ctx: &ItemCtx<'_>) -> usize {
        match self {
            Item::Via(i) => i.shape_layer(index, ctx),
            Item::Pin(i) => i.shape_layer(index, ctx),
            Item::BoardOutline(i) => i.shape_layer(index, ctx),
            // Every other override ignores `index` and answers the item's single layer.
            _ => self.first_layer(ctx),
        }
    }

    pub fn shares_layer(&self, other: &Item, ctx: &ItemCtx<'_>) -> bool {
        self.first_layer(ctx).max(other.first_layer(ctx))
            <= self.last_layer(ctx).min(other.last_layer(ctx))
    }

    pub fn first_common_layer(&self, other: &Item, ctx: &ItemCtx<'_>) -> Option<usize> {
        let max_first = self.first_layer(ctx).max(other.first_layer(ctx));
        let min_last = self.last_layer(ctx).min(other.last_layer(ctx));
        (max_first <= min_last).then_some(max_first)
    }

    pub fn last_common_layer(&self, other: &Item, ctx: &ItemCtx<'_>) -> Option<usize> {
        let max_first = self.first_layer(ctx).max(other.first_layer(ctx));
        let min_last = self.last_layer(ctx).min(other.last_layer(ctx));
        (max_first <= min_last).then_some(min_last)
    }

    pub fn bounding_box(&self, ctx: &ItemCtx<'_>) -> IntBox {
        match self {
            Item::Trace(i) => i.bounding_box(),
            Item::Via(i) => i.bounding_box(ctx),
            Item::Pin(i) => i.bounding_box(ctx),
            Item::ObstacleArea(i) => i.bounding_box(ctx),
            Item::ConductionArea(i) => i.bounding_box(ctx),
            Item::ViaObstacleArea(i) => i.bounding_box(ctx),
            Item::ComponentObstacleArea(i) => i.bounding_box(ctx),
            Item::ComponentOutline(i) => i.bounding_box(ctx),
            Item::BoardOutline(i) => i.bounding_box(),
        }
    }

    pub fn tile_shape_count(&self, ctx: &ItemCtx<'_>) -> usize {
        match self {
            Item::Trace(i) => i.tile_shape_count(),
            Item::Via(i) => i.tile_shape_count(ctx),
            Item::Pin(i) => i.tile_shape_count(ctx),
            Item::ObstacleArea(i) => i.tile_shape_count(ctx),
            Item::ConductionArea(i) => i.tile_shape_count(ctx),
            Item::ViaObstacleArea(i) => i.tile_shape_count(ctx),
            Item::ComponentObstacleArea(i) => i.tile_shape_count(ctx),
            Item::ComponentOutline(i) => i.tile_shape_count(),
            Item::BoardOutline(i) => i.tile_shape_count(ctx),
        }
    }

    pub fn get_tile_shape(
        &self,
        default_tree: TreeId,
        index: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<TileShape> {
        match self {
            Item::ObstacleArea(i) => i.get_tile_shape(index, ctx),
            Item::ConductionArea(i) => i.get_tile_shape(index, ctx),
            Item::ViaObstacleArea(i) => i.get_tile_shape(index, ctx),
            Item::ComponentObstacleArea(i) => i.get_tile_shape(index, ctx),
            _ => self.get_tree_shape(default_tree, index).cloned(),
        }
    }

    pub fn tree_shape_count(&self, tree: TreeId) -> usize {
        self.header()
            .get_precalculated_tree_shapes(tree)
            .map_or(0, <[Option<TileShape>]>::len)
    }

    pub fn get_tree_shape(&self, tree: TreeId, index: usize) -> Option<&TileShape> {
        self.header()
            .get_precalculated_tree_shapes(tree)
            .and_then(|shapes| shapes.get(index))
            .and_then(Option::as_ref)
    }

    pub fn set_tree_entries(&mut self, tree: TreeId, leaves: Vec<Option<LeafId>>) {
        self.header_mut().set_tree_entries(tree, leaves);
    }

    pub fn get_search_tree_entries(&self, tree: TreeId) -> Option<&[Option<LeafId>]> {
        self.header().get_tree_entries(tree)
    }

    pub fn set_precalculated_tree_shapes(&mut self, tree: TreeId, shapes: Vec<Option<TileShape>>) {
        self.header_mut()
            .set_precalculated_tree_shapes(tree, shapes);
    }

    pub fn clear_tree_entries(&mut self) {
        self.header_mut().clear_search_tree_entries();
    }

    pub fn get_autoroute_info(&mut self) -> &mut AutorouteInfo {
        self.header_mut().get_autoroute_info()
    }

    pub fn get_autoroute_info_pur(&self) -> Option<&AutorouteInfo> {
        self.header().get_autoroute_info_pur()
    }

    pub fn get_autoroute_info_pur_mut(&mut self) -> Option<&mut AutorouteInfo> {
        self.header_mut().get_autoroute_info_pur_mut()
    }

    pub fn clear_autoroute_info(&mut self) {
        match self {
            Item::Via(i) => i.clear_autoroute_info(),
            _ => self.header_mut().clear_autoroute_info(),
        }
    }

    pub fn clear_derived_data(&mut self) {
        match self {
            Item::Via(i) => i.clear_derived_data(),
            Item::Pin(i) => i.clear_derived_data(),
            Item::ObstacleArea(i) => i.clear_derived_data(),
            Item::ConductionArea(i) => i.clear_derived_data(),
            Item::ViaObstacleArea(i) => i.clear_derived_data(),
            Item::ComponentObstacleArea(i) => i.clear_derived_data(),
            Item::ComponentOutline(i) => i.clear_derived_data(),
            _ => self.header_mut().clear_derived_data(),
        }
    }

    pub fn translate_by(&mut self, vector: &Vector) -> Result<(), PolylineError> {
        match self {
            Item::Trace(i) => return i.translate_by(vector),
            Item::Via(i) => i.translate_by(vector),
            Item::Pin(i) => i.translate_by(vector),
            Item::ObstacleArea(i) => i.translate_by(vector),
            Item::ConductionArea(i) => i.translate_by(vector),
            Item::ViaObstacleArea(i) => i.translate_by(vector),
            Item::ComponentObstacleArea(i) => i.translate_by(vector),
            Item::ComponentOutline(i) => i.translate_by(vector),
            Item::BoardOutline(i) => i.translate_by(vector),
        }
        Ok(())
    }

    pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) -> Result<(), PolylineError> {
        match self {
            Item::Trace(i) => return i.turn_90_degree(factor, pole),
            Item::Via(i) => i.turn_90_degree(factor, pole),
            Item::Pin(i) => i.turn_90_degree(factor, pole),
            Item::ObstacleArea(i) => i.turn_90_degree(factor, pole),
            Item::ConductionArea(i) => i.turn_90_degree(factor, pole),
            Item::ViaObstacleArea(i) => i.turn_90_degree(factor, pole),
            Item::ComponentObstacleArea(i) => i.turn_90_degree(factor, pole),
            Item::ComponentOutline(i) => i.turn_90_degree(factor, pole),
            Item::BoardOutline(i) => i.turn_90_degree(factor, pole),
        }
        Ok(())
    }

    pub fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint, ctx: &ItemCtx<'_>) {
        match self {
            Item::Trace(i) => i.rotate_approx(angle_in_degree, pole),
            Item::Via(i) => i.rotate_approx(angle_in_degree, pole),
            Item::Pin(i) => i.rotate_approx(angle_in_degree, pole),
            Item::ObstacleArea(i) => i.rotate_approx(angle_in_degree, pole, ctx),
            Item::ConductionArea(i) => i.rotate_approx(angle_in_degree, pole, ctx),
            Item::ViaObstacleArea(i) => i.rotate_approx(angle_in_degree, pole, ctx),
            Item::ComponentObstacleArea(i) => i.rotate_approx(angle_in_degree, pole, ctx),
            Item::ComponentOutline(i) => i.rotate_approx(angle_in_degree, pole, ctx),
            Item::BoardOutline(i) => i.rotate_approx(angle_in_degree, pole),
        }
    }

    pub fn change_placement_side(
        &mut self,
        pole: &IntPoint,
        ctx: &ItemCtx<'_>,
    ) -> Result<(), PolylineError> {
        match self {
            Item::Trace(i) => return i.change_placement_side(pole, ctx),
            Item::Via(i) => i.change_placement_side(pole, ctx),
            Item::Pin(i) => i.change_placement_side(pole),
            Item::ObstacleArea(i) => i.change_placement_side(pole, ctx),
            Item::ConductionArea(i) => i.change_placement_side(pole, ctx),
            Item::ViaObstacleArea(i) => i.change_placement_side(pole, ctx),
            Item::ComponentObstacleArea(i) => i.change_placement_side(pole, ctx),
            Item::ComponentOutline(i) => i.change_placement_side(pole),
            Item::BoardOutline(i) => i.change_placement_side(pole),
        }
        Ok(())
    }

    pub fn copy(&self, new_id: ItemId) -> Option<Item> {
        match self {
            Item::Trace(i) => Some(Item::Trace(i.copy(new_id))),
            Item::Via(i) => Some(Item::Via(i.copy(new_id))),
            Item::Pin(i) => Some(Item::Pin(i.copy(new_id))),
            Item::ObstacleArea(i) => Some(Item::ObstacleArea(i.copy(new_id))),
            Item::ConductionArea(i) => i.copy(new_id).map(Item::ConductionArea),
            Item::ViaObstacleArea(i) => Some(Item::ViaObstacleArea(i.copy(new_id))),
            Item::ComponentObstacleArea(i) => Some(Item::ComponentObstacleArea(i.copy(new_id))),
            Item::ComponentOutline(i) => Some(Item::ComponentOutline(i.copy(new_id))),
            Item::BoardOutline(i) => Some(Item::BoardOutline(i.copy(new_id))),
        }
    }

    pub fn duplicate(&self) -> Option<Item> {
        let mut dup = self.copy(self.id())?;
        dup.set_on_the_board(self.is_on_the_board());
        Some(dup)
    }
}

fn obstacle_area_is_obstacle(this: &Item, other: &Item) -> bool {
    if other.shares_net(this) {
        return false;
    }
    other.is_trace() || matches!(other, Item::Via(_))
}

impl From<&Item> for crate::ids::TreeObject {
    fn from(item: &Item) -> crate::ids::TreeObject {
        crate::ids::TreeObject::Item(item.id())
    }
}

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let simple_name = match self {
            Item::Trace(_) => "polylinetrace",
            Item::Via(_) => "via",
            Item::Pin(_) => "pin",
            Item::ObstacleArea(_) => "obstaclearea",
            Item::ConductionArea(_) => "conductionarea",
            Item::ViaObstacleArea(_) => "viaobstaclearea",
            Item::ComponentObstacleArea(_) => "componentobstaclearea",
            Item::ComponentOutline(_) => "componentoutline",
            Item::BoardOutline(_) => "boardoutline",
        };
        f.write_str(simple_name)?;
        if let Item::Pin(pin) = self
            && pin.get_pin_index() > 0
        {
            write!(f, " #{}", pin.get_pin_index())?;
        }
        if self.component_id() > 0 {
            write!(f, " of component #{}", self.component_id())?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Connectable
// ---------------------------------------------------------------------------------------------

pub trait Connectable {
    /// The item's shared state, so the two net methods below need no per-implementor body.
    fn header(&self) -> &ItemHeader;

    fn contains_net(&self, net_number: i32) -> bool {
        self.header().contains_net(net_number)
    }

    fn shares_net_no(&self, net_nos: &[i32]) -> bool {
        self.header().shares_net_no(net_nos)
    }

    fn get_trace_connection_shape(
        &self,
        tree: TreeId,
        index: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<TileShape>;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectableRef<'a> {
    Trace(&'a PolylineTrace),
    Via(&'a Via),
    Pin(&'a Pin),
    ConductionArea(&'a ConductionArea),
}

impl ConnectableRef<'_> {
    /// The `Connectable` behind this reference, for callers that only need the interface.
    pub fn as_dyn(&self) -> &dyn Connectable {
        match self {
            ConnectableRef::Trace(i) => *i,
            ConnectableRef::Via(i) => *i,
            ConnectableRef::Pin(i) => *i,
            ConnectableRef::ConductionArea(i) => *i,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Per-variant methods.
// ---------------------------------------------------------------------------------------------

pub(crate) fn copied_header(hdr: &ItemHeader, new_id: ItemId) -> ItemHeader {
    ItemHeader::new(
        new_id,
        hdr.net_nos.clone(),
        hdr.clearance_class(),
        hdr.get_component_id(),
        hdr.get_fixed_state(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use copper_geometry::{Point, Shape, TileShape};

    use crate::ids::{PadstackId, TreeObject};
    use crate::library::{BoardLibrary, PackagePin, Packages, Padstacks};
    use crate::rules::ClearanceMatrix;
    use crate::structure::{Components, Layer, LayerStructure};

    struct Fixture {
        library: BoardLibrary,
        components: Components,
        rules: BoardRules,
        bounding_box: IntBox,
    }

    /// The SMD component's id, whose single pin is on a one-layer padstack (`drillAllowed`).
    const SMD_COMPONENT: i32 = 1;
    /// The through-hole component's id, whose single pin spans both layers.
    const THT_COMPONENT: i32 = 2;
    const VIA_PADSTACK: PadstackId = PadstackId(1);

    impl Fixture {
        fn new() -> Fixture {
            let mut padstacks = Padstacks::new(layer_structure());
            let smd = padstacks.add(
                "SMD",
                vec![
                    Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                        -10, -10, 10, 10,
                    )))),
                    None,
                ],
                true,
                false,
            );
            let tht = padstacks.add(
                "THT",
                vec![
                    Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                        -10, -10, 10, 10,
                    )))),
                    Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                        -10, -10, 10, 10,
                    )))),
                ],
                true,
                false,
            );
            let mut packages = Packages::new();
            let smd_package =
                packages.add_pins(vec![PackagePin::new("1", smd, Vector::new(0, 0), 0.0)]);
            let tht_package =
                packages.add_pins(vec![PackagePin::new("1", tht, Vector::new(0, 0), 0.0)]);
            let mut components = Components::new();
            components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, smd_package);
            components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, tht_package);
            Fixture {
                library: BoardLibrary::new(padstacks, packages),
                components,
                rules: rules(),
                bounding_box: IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
            }
        }

        fn ctx(&self) -> ItemCtx<'_> {
            ItemCtx {
                library: &self.library,
                components: &self.components,
                rules: &self.rules,
                bounding_box: &self.bounding_box,
                max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
            }
        }
    }

    fn rules() -> BoardRules {
        let ls = layer_structure();
        let cm = ClearanceMatrix::get_default_instance(&ls, 100);
        BoardRules::new(ls, cm)
    }

    fn hdr(id: u32, net_nos: Vec<i32>) -> ItemHeader {
        ItemHeader::new(ItemId(id), net_nos, 1, 0, FixedState::Unfixed)
    }

    fn hdr_of_component(id: u32, component_id: i32) -> ItemHeader {
        ItemHeader::new(ItemId(id), Vec::new(), 1, component_id, FixedState::Unfixed)
    }

    fn trace(id: u32, net_nos: Vec<i32>) -> Item {
        // A degenerate one-segment polyline: these tests only exercise the `Item` dispatch,
        // never the geometry; `tests/polyline_trace.rs` is where the geometry is pinned.
        Item::Trace(PolylineTrace::new(
            hdr(id, net_nos),
            copper_geometry::Polyline::from_two_points(&Point::new(0, 0), &Point::new(100, 0)),
            0,
            50,
            None,
        ))
    }

    fn via(id: u32, net_nos: Vec<i32>, attach_allowed: bool) -> Item {
        Item::Via(Via::new(
            hdr(id, net_nos),
            VIA_PADSTACK,
            Point::new(0, 0),
            attach_allowed,
        ))
    }

    fn pin(id: u32, net_nos: Vec<i32>) -> Item {
        Item::Pin(Pin::new(
            ItemHeader::new(ItemId(id), net_nos, 1, SMD_COMPONENT, FixedState::Unfixed),
            0,
        ))
    }

    /// A through-hole pin — two layers, so `Pin.drillAllowed` is false.
    fn tht_pin(id: u32, net_nos: Vec<i32>) -> Item {
        Item::Pin(Pin::new(
            ItemHeader::new(ItemId(id), net_nos, 1, THT_COMPONENT, FixedState::Unfixed),
            0,
        ))
    }

    /// A unit square on layer 0 at the origin — enough geometry for the dispatch tests here;
    /// `crates/copper-board/tests/areas_and_outlines.rs` is where the geometry itself is pinned.
    fn area_data() -> ObstacleAreaData {
        ObstacleAreaData::new(
            copper_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                0, 0, 10, 10,
            )))),
            0,
            Vector::new(0, 0),
            0.0,
            false,
            None,
        )
    }

    fn obstacle_area(id: u32, net_nos: Vec<i32>) -> Item {
        Item::ObstacleArea(ObstacleArea::new(hdr(id, net_nos), area_data()))
    }

    fn conduction_area(id: u32, net_nos: Vec<i32>, is_obstacle: bool) -> Item {
        Item::ConductionArea(ConductionArea::new(
            hdr(id, net_nos),
            area_data(),
            is_obstacle,
        ))
    }

    fn via_keepout(id: u32, net_nos: Vec<i32>) -> Item {
        Item::ViaObstacleArea(ViaObstacleArea::new(hdr(id, net_nos), area_data()))
    }

    fn component_keepout(id: u32, component_id: i32) -> Item {
        Item::ComponentObstacleArea(ComponentObstacleArea::new(
            hdr_of_component(id, component_id),
            area_data(),
        ))
    }

    fn component_outline(id: u32) -> Item {
        Item::ComponentOutline(ComponentOutline::new(
            hdr(id, Vec::new()),
            copper_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                0, 0, 10, 10,
            )))),
            true,
            Vector::new(0, 0),
            0.0,
            true,
            false,
            true,
        ))
    }

    fn board_outline(id: u32) -> Item {
        Item::BoardOutline(BoardOutline::new(
            ItemHeader::new(ItemId(id), Vec::new(), 1, 0, FixedState::SystemFixed),
            Vec::new(),
        ))
    }

    /// Every variant, for tests that must cover the whole enum.
    fn one_of_each() -> Vec<Item> {
        vec![
            trace(1, vec![1]),
            via(2, vec![1], true),
            pin(3, vec![1]),
            obstacle_area(4, vec![]),
            conduction_area(5, vec![1], true),
            via_keepout(6, vec![]),
            component_keepout(7, 1),
            component_outline(8),
            board_outline(9),
        ]
    }

    // ---- kind / family predicates ------------------------------------------------------------

    #[test]
    fn kind_matches_get_board_item_type() {
        let expected = [
            ItemKind::Trace,
            ItemKind::Via,
            ItemKind::Pin,
            ItemKind::ObstacleArea,
            ItemKind::ConductionArea,
            ItemKind::ViaObstacleArea,
            ItemKind::ComponentObstacleArea,
            ItemKind::ComponentOutline,
            ItemKind::BoardOutline,
        ];
        for (item, kind) in one_of_each().iter().zip(expected) {
            assert_eq!(item.kind(), kind, "{item}");
        }
    }

    #[test]
    fn is_obstacle_area_covers_the_whole_java_subclass_family() {
        let expected = [false, false, false, true, true, true, true, false, false];
        for (item, is_area) in one_of_each().iter().zip(expected) {
            assert_eq!(item.is_obstacle_area(), is_area, "{item}");
        }
    }

    #[test]
    fn is_trace_and_is_drill_item_cover_their_families() {
        let traces = [true, false, false, false, false, false, false, false, false];
        let drills = [false, true, true, false, false, false, false, false, false];
        for ((item, is_trace), is_drill) in one_of_each().iter().zip(traces).zip(drills) {
            assert_eq!(item.is_trace(), is_trace, "{item}");
            assert_eq!(item.is_drill_item(), is_drill, "{item}");
        }
    }

    // ---- Connectable dispatch -----------------------------------------------------------------

    #[test]
    fn as_connectable_matches_javas_implements_clauses() {
        let expected = [true, true, true, false, true, false, false, false, false];
        for (item, connectable) in one_of_each().iter().zip(expected) {
            assert_eq!(item.as_connectable().is_some(), connectable, "{item}");
        }
    }

    #[test]
    fn is_connectable_also_needs_a_net() {
        assert!(trace(1, vec![3]).is_connectable());
        assert!(!trace(1, vec![]).is_connectable());
        assert!(!obstacle_area(1, vec![3]).is_connectable());
    }

    #[test]
    fn connectable_ref_forwards_the_two_net_methods() {
        let item = via(1, vec![4], true);
        let connectable = item.as_connectable().expect("a via is connectable");
        assert!(connectable.as_dyn().contains_net(4));
        assert!(!connectable.as_dyn().contains_net(5));
        assert!(connectable.as_dyn().shares_net_no(&[9, 4]));
    }

    #[test]
    fn trace_is_obstacle_matches_trace_java() {
        let f = Fixture::new();
        let t = trace(1, vec![5]);
        assert!(!t.is_obstacle(&trace(1, vec![9]), &f.ctx()));
        assert!(!t.is_obstacle(&via_keepout(2, vec![9]), &f.ctx()));
        assert!(!t.is_obstacle(&component_keepout(3, 1), &f.ctx()));
        assert!(!t.is_obstacle(&conduction_area(4, vec![9], false), &f.ctx()));
        assert!(t.is_obstacle(&conduction_area(5, vec![9], true), &f.ctx()));
        assert!(t.is_obstacle(&trace(6, vec![9]), &f.ctx()));
        assert!(!t.is_obstacle(&trace(7, vec![5]), &f.ctx()));
        assert!(t.is_obstacle(&obstacle_area(8, vec![9]), &f.ctx()));
    }

    #[test]
    fn via_is_obstacle_matches_via_java() {
        let f = Fixture::new();
        let v = via(1, vec![5], true);
        assert!(!v.is_obstacle(&via(1, vec![9], true), &f.ctx()));
        assert!(!v.is_obstacle(&component_keepout(2, 1), &f.ctx()));
        assert!(v.is_obstacle(&via_keepout(3, vec![9]), &f.ctx()));
        assert!(!v.is_obstacle(&conduction_area(4, vec![9], false), &f.ctx()));
        assert!(v.is_obstacle(&trace(5, vec![9]), &f.ctx()));
        assert!(!v.is_obstacle(&trace(6, vec![5]), &f.ctx()));
        assert!(v.is_obstacle(&via(7, vec![5], true), &f.ctx()));
    }

    #[test]
    fn via_is_obstacle_to_a_same_net_pin_only_when_attach_is_not_allowed() {
        let f = Fixture::new();
        assert!(via(1, vec![5], false).is_obstacle(&pin(2, vec![5]), &f.ctx()));
        assert!(!via(1, vec![5], true).is_obstacle(&pin(2, vec![5]), &f.ctx()));
        // A through-hole pin is not drillable, so an attachable via is an obstacle to it.
        assert!(via(1, vec![5], true).is_obstacle(&tht_pin(2, vec![5]), &f.ctx()));
    }

    #[test]
    fn pin_is_obstacle_matches_pin_java() {
        let f = Fixture::new();
        let p = pin(1, vec![5]);
        assert!(!p.is_obstacle(&pin(1, vec![9]), &f.ctx()));
        for area in [
            obstacle_area(2, vec![9]),
            conduction_area(3, vec![9], true),
            via_keepout(4, vec![9]),
            component_keepout(5, 1),
        ] {
            assert!(!p.is_obstacle(&area, &f.ctx()), "{area}");
        }
        assert!(p.is_obstacle(&via(6, vec![9], true), &f.ctx()));
        assert!(!p.is_obstacle(&trace(7, vec![5]), &f.ctx()));
    }

    #[test]
    fn pin_is_obstacle_to_a_same_net_via_only_when_it_is_not_an_smd_pad() {
        let f = Fixture::new();
        assert!(!pin(1, vec![5]).is_obstacle(&via(2, vec![5], true), &f.ctx()));
        assert!(tht_pin(1, vec![5]).is_obstacle(&via(2, vec![5], true), &f.ctx()));
    }

    #[test]
    fn obstacle_area_is_obstacle_only_to_foreign_net_traces_and_vias() {
        let f = Fixture::new();
        let a = obstacle_area(1, vec![5]);
        assert!(a.is_obstacle(&trace(2, vec![9]), &f.ctx()));
        assert!(a.is_obstacle(&via(3, vec![9], true), &f.ctx()));
        assert!(!a.is_obstacle(&trace(4, vec![5]), &f.ctx()));
        assert!(!a.is_obstacle(&pin(5, vec![9]), &f.ctx()));
        assert!(!a.is_obstacle(&obstacle_area(6, vec![9]), &f.ctx()));
    }

    #[test]
    fn conduction_area_is_obstacle_delegates_to_super_only_when_the_flag_is_set() {
        let f = Fixture::new();
        let on = conduction_area(1, vec![5], true);
        assert!(on.is_obstacle(&trace(2, vec![9]), &f.ctx()));
        assert!(!on.is_obstacle(&trace(3, vec![5]), &f.ctx()));
        let off = conduction_area(4, vec![5], false);
        assert!(!off.is_obstacle(&trace(5, vec![9]), &f.ctx()));
        assert!(!off.is_obstacle(&via(6, vec![9], true), &f.ctx()));
    }

    #[test]
    fn via_keepout_is_obstacle_only_to_foreign_net_vias() {
        let f = Fixture::new();
        let k = via_keepout(1, vec![5]);
        assert!(k.is_obstacle(&via(2, vec![9], true), &f.ctx()));
        assert!(!k.is_obstacle(&via(3, vec![5], true), &f.ctx()));
        assert!(!k.is_obstacle(&trace(4, vec![9]), &f.ctx()));
    }

    #[test]
    fn component_keepout_is_obstacle_only_to_other_components_keepouts() {
        let f = Fixture::new();
        let k = component_keepout(1, 7);
        assert!(k.is_obstacle(&component_keepout(2, 8), &f.ctx()));
        assert!(!k.is_obstacle(&component_keepout(3, 7), &f.ctx()));
        assert!(!k.is_obstacle(&component_keepout(1, 8), &f.ctx())); // `other == this`
        assert!(!k.is_obstacle(&trace(4, vec![9]), &f.ctx()));
    }

    #[test]
    fn component_outline_is_never_an_obstacle() {
        let f = Fixture::new();
        let o = component_outline(1);
        for other in one_of_each() {
            assert!(!o.is_obstacle(&other, &f.ctx()), "{other}");
        }
    }

    #[test]
    fn board_outline_is_an_obstacle_to_everything_but_outlines_and_areas() {
        let f = Fixture::new();
        let b = board_outline(1);
        let expected = [true, true, true, false, false, false, false, true, false];
        for (other, is_obstacle) in one_of_each().iter().zip(expected) {
            assert_eq!(b.is_obstacle(other, &f.ctx()), is_obstacle, "{other}");
        }
    }

    // ---- the net-number obstacle predicates ---------------------------------------------------

    #[test]
    fn is_obstacle_for_net_is_plain_non_membership() {
        for item in one_of_each() {
            assert_eq!(item.is_obstacle_for_net(1), !item.contains_net(1), "{item}");
        }
    }

    #[test]
    fn is_trace_obstacle_matches_the_base_and_its_three_overrides() {
        assert!(trace(1, vec![5]).is_trace_obstacle(9));
        assert!(!trace(1, vec![5]).is_trace_obstacle(5));
        assert!(conduction_area(2, vec![5], true).is_trace_obstacle(9));
        assert!(!conduction_area(2, vec![5], true).is_trace_obstacle(5));
        assert!(!conduction_area(2, vec![5], false).is_trace_obstacle(9));
        assert!(!via_keepout(3, vec![]).is_trace_obstacle(9));
        assert!(!component_keepout(4, 1).is_trace_obstacle(9));
        assert!(obstacle_area(5, vec![]).is_trace_obstacle(9));
    }

    #[test]
    fn is_drillable_matches_the_base_and_its_two_overrides() {
        assert!(trace(1, vec![5]).is_drillable(5));
        assert!(!trace(1, vec![5]).is_drillable(9));
        assert!(!conduction_area(2, vec![5], true).is_drillable(9));
        assert!(conduction_area(2, vec![5], true).is_drillable(5));
        assert!(conduction_area(2, vec![5], false).is_drillable(9));
        assert!(!via(3, vec![5], true).is_drillable(5));
        assert!(!obstacle_area(4, vec![]).is_drillable(5));
    }

    #[test]
    fn is_routable_is_true_only_for_unfixed_traces_and_vias_with_a_net() {
        assert!(trace(1, vec![5]).is_routable());
        assert!(via(2, vec![5], true).is_routable());
        assert!(!trace(3, vec![]).is_routable());
        assert!(!pin(4, vec![5]).is_routable());
        let mut fixed = trace(5, vec![5]);
        fixed.set_fixed_state(FixedState::UserFixed);
        assert!(!fixed.is_routable());
        let mut shoved = trace(6, vec![5]);
        shoved.set_fixed_state(FixedState::ShoveFixed);
        assert!(shoved.is_routable());
    }

    // ---- isShoveFixed / isDeletionForbidden (the two that take BoardRules) ---------------------

    fn layer_structure() -> LayerStructure {
        LayerStructure::new(vec![Layer::new("F.Cu", true), Layer::new("B.Cu", true)])
    }

    fn rules_with_a_shove_fixed_net() -> BoardRules {
        let layers = layer_structure();
        let matrix = ClearanceMatrix::new(2, &layers, &["default", "c1"]);
        let mut rules = BoardRules::new(layers.clone(), matrix);
        let plain = rules.net_classes.append("plain", &layers, false);
        let shove_fixed = rules.net_classes.append("shove_fixed", &layers, false);
        rules.net_classes.get_mut(shove_fixed).set_shove_fixed(true);
        rules.nets.add("n1", 1, false, plain);
        rules.nets.add("n2", 1, false, shove_fixed);
        rules
    }

    #[test]
    fn is_shove_fixed_applies_traces_net_class_override() {
        let rules = rules_with_a_shove_fixed_net();
        assert!(!trace(1, vec![1]).is_shove_fixed(&rules));
        assert!(trace(2, vec![2]).is_shove_fixed(&rules));
        assert!(trace(3, vec![1, 2]).is_shove_fixed(&rules));
    }

    #[test]
    fn is_shove_fixed_override_is_traces_only() {
        let rules = rules_with_a_shove_fixed_net();
        assert!(!via(1, vec![2], true).is_shove_fixed(&rules));
        assert!(!conduction_area(2, vec![2], true).is_shove_fixed(&rules));
    }

    #[test]
    fn is_shove_fixed_base_body_still_wins_for_a_fixed_trace() {
        let rules = rules_with_a_shove_fixed_net();
        let mut t = trace(1, vec![1]);
        t.set_fixed_state(FixedState::ShoveFixed);
        assert!(t.is_shove_fixed(&rules));
    }

    #[test]
    fn is_shove_fixed_skips_non_normal_net_numbers() {
        let rules = rules_with_a_shove_fixed_net();
        assert!(!trace(1, vec![0]).is_shove_fixed(&rules));
        assert!(!trace(2, vec![]).is_shove_fixed(&rules));
    }

    #[test]
    #[should_panic(expected = "NullPointerException")]
    fn is_shove_fixed_panics_on_a_net_number_past_the_net_list_like_java() {
        trace(1, vec![9]).is_shove_fixed(&rules_with_a_shove_fixed_net());
    }

    #[test]
    fn is_deletion_forbidden_for_component_items_and_user_fixed_items() {
        let rules = rules_with_a_shove_fixed_net();
        assert!(!trace(1, vec![1]).is_deletion_forbidden(&rules));

        let mut of_component = trace(2, vec![1]);
        of_component.assign_component_id(4);
        assert!(of_component.is_deletion_forbidden(&rules));

        let mut user_fixed = trace(3, vec![1]);
        user_fixed.set_fixed_state(FixedState::UserFixed);
        assert!(user_fixed.is_deletion_forbidden(&rules));

        let mut system_fixed = trace(4, vec![1]);
        system_fixed.set_fixed_state(FixedState::SystemFixed);
        assert!(system_fixed.is_deletion_forbidden(&rules));

        // SHOVE_FIXED is below it, so it does not forbid deletion on its own.
        let mut shove_fixed = trace(5, vec![1]);
        shove_fixed.set_fixed_state(FixedState::ShoveFixed);
        assert!(!shove_fixed.is_deletion_forbidden(&rules));
    }

    #[test]
    fn is_deletion_forbidden_takes_the_short_circuit_before_the_conduction_area_branch() {
        let rules = rules_with_a_shove_fixed_net();
        let mut area = conduction_area(1, vec![1], true);
        area.assign_component_id(2);
        assert!(area.is_deletion_forbidden(&rules));
    }

    #[test]
    fn is_deletion_forbidden_on_a_free_conduction_area_reads_its_layer() {
        let rules = rules_with_a_shove_fixed_net();
        assert!(!conduction_area(1, vec![1], true).is_deletion_forbidden(&rules));

        let power_plane = BoardRules::new(
            LayerStructure::new(vec![Layer::new("plane", false), Layer::new("back", true)]),
            ClearanceMatrix::get_default_instance(
                &LayerStructure::new(vec![Layer::new("plane", false), Layer::new("back", true)]),
                100,
            ),
        );
        assert!(conduction_area(1, vec![1], true).is_deletion_forbidden(&power_plane));
    }

    // ---- ordering, identity, display ------------------------------------------------------------

    #[test]
    fn compare_to_orders_by_descending_id_like_java() {
        let low = trace(1, vec![]);
        let high = trace(2, vec![]);
        assert_eq!(low.compare_to(&high), Ordering::Greater);
        assert_eq!(high.compare_to(&low), Ordering::Less);
        assert_eq!(low.compare_to(&trace(1, vec![7])), Ordering::Equal);
    }

    #[test]
    fn tree_object_from_an_item_is_its_id() {
        // plan-rulings.md #2: the shared search tree stores items by id.
        let item = via(42, vec![], true);
        assert_eq!(TreeObject::from(&item), TreeObject::Item(ItemId(42)));
    }

    #[test]
    fn tree_objects_of_two_items_order_like_item_compare_to() {
        let low = trace(1, vec![]);
        let high = trace(2, vec![]);
        assert_eq!(
            TreeObject::from(&low).cmp(&TreeObject::from(&high)),
            low.compare_to(&high)
        );
    }

    #[test]
    fn display_matches_java_to_string() {
        assert_eq!(trace(1, vec![]).to_string(), "polylinetrace");
        assert_eq!(board_outline(2).to_string(), "boardoutline");
        assert_eq!(
            component_keepout(3, 5).to_string(),
            "componentobstaclearea of component #5"
        );
    }

    // ---- copy ------------------------------------------------------------------------------------

    #[test]
    fn copy_carries_the_header_and_takes_the_new_id() {
        let mut original = trace(1, vec![4, 6]);
        original.set_fixed_state(FixedState::UserFixed);
        original.assign_component_id(3);
        original.set_on_the_board(true);
        let copy = original.copy(ItemId(99)).expect("a trace copy never fails");
        assert_eq!(copy.id(), ItemId(99));
        assert_eq!(copy.net_nos(), &[4, 6]);
        assert_eq!(copy.get_fixed_state(), FixedState::UserFixed);
        assert_eq!(copy.component_id(), 3);
        assert_eq!(copy.clearance_class(), 1);
        assert!(!copy.is_on_the_board());
    }

    #[test]
    fn duplicate_restores_on_the_board() {
        let mut original = trace(1, vec![4]);
        original.set_on_the_board(true);
        let dup = original.duplicate().expect("a trace clone never fails");
        assert_eq!(dup.id(), ItemId(1));
        assert!(dup.is_on_the_board());
    }

    #[test]
    fn conduction_area_copy_resets_is_filled_to_true() {
        let mut area = ConductionArea::new(hdr(1, vec![5]), area_data(), true);
        area.set_is_filled(false);
        assert!(!area.get_is_filled());
        let copy = area.copy(ItemId(2)).expect("one net, so the copy succeeds");
        assert!(copy.get_is_filled());
        assert!(copy.get_is_obstacle());
    }

    #[test]
    fn component_outline_has_no_tile_shapes() {
        let f = Fixture::new();
        assert_eq!(component_outline(1).tile_shape_count(&f.ctx()), 0);
    }

    #[test]
    fn conduction_area_copy_fails_only_above_one_net() {
        assert!(conduction_area(1, vec![5], true).copy(ItemId(2)).is_some());
        assert!(conduction_area(1, vec![], true).copy(ItemId(2)).is_some());
        assert!(
            conduction_area(1, vec![5, 6], true)
                .copy(ItemId(2))
                .is_none()
        );
    }

    #[test]
    fn component_outline_copy_drops_the_nets_and_the_clearance_class() {
        let mut original = ComponentOutline::new(
            ItemHeader::new(ItemId(1), vec![5], 7, 3, FixedState::UserFixed),
            copper_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                0, 0, 10, 10,
            )))),
            true,
            Vector::new(0, 0),
            0.0,
            true,
            false,
            true,
        );
        original.hdr.set_on_the_board(true);
        let copy = original.copy(ItemId(2));
        assert_eq!(copy.hdr.id(), ItemId(2));
        assert!(copy.hdr.net_nos.is_empty());
        assert_eq!(copy.hdr.clearance_class(), 0);
        assert_eq!(copy.hdr.get_component_id(), 3);
        assert_eq!(copy.hdr.get_fixed_state(), FixedState::UserFixed);
    }

    #[test]
    fn board_outline_copy_resets_to_the_constructors_hard_coded_values() {
        let original = BoardOutline::new(
            ItemHeader::new(ItemId(1), vec![5], 7, 3, FixedState::Unfixed),
            Vec::new(),
        );
        let copy = original.copy(ItemId(2));
        assert!(copy.hdr.net_nos.is_empty());
        assert_eq!(copy.hdr.clearance_class(), 7);
        assert_eq!(copy.hdr.get_component_id(), 0);
        assert_eq!(copy.hdr.get_fixed_state(), FixedState::SystemFixed);
    }

    #[test]
    fn component_keepout_copy_drops_the_nets() {
        let original = ComponentObstacleArea::new(
            ItemHeader::new(ItemId(1), vec![5], 7, 3, FixedState::Unfixed),
            area_data(),
        );
        let copy = original.copy(ItemId(2));
        assert!(copy.hdr.net_nos.is_empty());
        assert_eq!(copy.hdr.clearance_class(), 7);
    }

    #[test]
    fn via_copy_carries_attach_allowed_the_padstack_and_the_escape_via_fields() {
        let original = Via::new(hdr(1, vec![5]), VIA_PADSTACK, Point::new(3, 4), false);
        assert!(!original.copy(ItemId(2)).attach_allowed);
        let mut original = Via::new(hdr(1, vec![5]), VIA_PADSTACK, Point::new(3, 4), true);
        original.is_escape_via = true;
        original.escape_via_smd_layer = 1;
        let copy = original.copy(ItemId(2));
        assert!(copy.attach_allowed);
        assert_eq!(copy.get_padstack_id(), VIA_PADSTACK);
        assert_eq!(copy.get_center(), Point::new(3, 4));
        assert!(copy.is_escape_via);
        assert_eq!(copy.escape_via_smd_layer, 1);
        assert_eq!(copy.hdr.id(), ItemId(2));
    }

    // ---- header delegation and the tree-entry surface --------------------------------------------

    #[test]
    fn header_dispatch_reaches_every_variant() {
        for (index, item) in one_of_each().iter().enumerate() {
            assert_eq!(item.header().id(), ItemId(index as u32 + 1), "{item}");
            assert_eq!(item.id(), item.header().id());
        }
    }

    #[test]
    fn tree_shape_bookkeeping_goes_through_the_header() {
        let mut item = trace(1, vec![]);
        assert_eq!(item.tree_shape_count(TreeId(0)), 0);
        assert_eq!(item.get_tree_shape(TreeId(0), 0), None);
        item.set_precalculated_tree_shapes(TreeId(0), vec![]);
        assert_eq!(item.tree_shape_count(TreeId(0)), 0);
        item.set_tree_entries(TreeId(0), vec![None, None]);
        assert_eq!(
            item.get_search_tree_entries(TreeId(0)).map(<[_]>::len),
            Some(2)
        );
        item.clear_tree_entries();
        assert_eq!(item.get_search_tree_entries(TreeId(0)), None);
    }

    #[test]
    fn autoroute_scratch_goes_through_the_header() {
        let mut item = via(1, vec![], true);
        assert_eq!(item.get_autoroute_info_pur(), None);
        item.get_autoroute_info();
        assert!(item.get_autoroute_info_pur().is_some());
        item.clear_autoroute_info();
        assert_eq!(item.get_autoroute_info_pur(), None);
    }

    #[test]
    fn conduction_area_flags_round_trip() {
        let mut area = ConductionArea::new(hdr(1, vec![]), area_data(), false);
        assert!(!area.get_is_obstacle());
        assert!(area.get_is_filled());
        area.set_is_obstacle(true);
        assert!(area.get_is_obstacle());
        area.set_is_filled(false);
        assert!(!area.get_is_filled());
    }

    #[test]
    fn board_outline_half_width_is_javas_constant() {
        assert_eq!(board_outline_of().get_half_width(), 100);
    }

    fn board_outline_of() -> BoardOutline {
        BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
            Vec::new(),
        )
    }
}
