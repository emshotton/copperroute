//! Port of `board/searchtree/ShapeSearchTree.java` together with its two subclasses
//! `ShapeSearchTree45Degree.java` and `ShapeSearchTree90Degree.java`, collapsed into one
//! angle-parameterised type (see the module docs for the mapping table).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use fr_geometry::bounding_directions::ShapeBoundingDirections;
use fr_geometry::regular_tile_shape::RegularTileShape;
use fr_geometry::{Circle, Point, Polyline, Shape, ShapeOps, TileShape};

use crate::datastructures::{LeafId, ShapeTree, TreeEntry};
use crate::ids::{ItemId, RoomId, TreeId, TreeObject};
use crate::items::{Item, ItemCtx, PolylineTrace};
use crate::library::Padstack;
use crate::rules::{BoardRules, ClearanceMatrix};
use crate::structure::AngleRestriction;

/// `ShapeSearchTree.DRILL_HOLE_CLEARANCE_MARGIN` (ShapeSearchTree.java:52).
const DRILL_HOLE_CLEARANCE_MARGIN: i32 = 10;

/// Read access to the board's item list, for the query methods that have to look an item up by
/// the [`ItemId`] its tree leaves carry.
///
/// Java writes `(Item) currentLeaf.object` — the leaf holds the item itself. The port's leaves
/// hold a [`TreeObject`] key (plan-rulings.md #2), so the queries need the map back. `Board`
/// (Task 11) owns a `BTreeMap<ItemId, Item>` and satisfies this through the blanket impl below;
/// tests build the map directly.
pub trait ItemLookup {
    /// The item with this id, or `None` if the board has no such item.
    fn item(&self, id: ItemId) -> Option<&Item>;
}

impl ItemLookup for BTreeMap<ItemId, Item> {
    fn item(&self, id: ItemId) -> Option<&Item> {
        self.get(&id)
    }
}

/// Read access to the **expansion rooms** a router has inserted into this tree, for the query
/// methods that have to resolve a [`TreeObject::Room`] leaf.
///
/// The room counterpart of [`ItemLookup`], and it exists for the same reason: a leaf holds a
/// [`TreeObject`] key, not the object, and `fr-board` cannot name
/// `autoroute.expansion.CompleteFreeSpaceExpansionRoom` — the room and its arena live in
/// `fr-router` (plan-6 ruling 16). Java has no such interface because
/// `CompleteFreeSpaceExpansionRoom implements SearchTreeObject`
/// (`autoroute/expansion/CompleteFreeSpaceExpansionRoom.java:19-20`) and the tree simply asks
/// the object.
///
/// Both methods drop the `index` argument their Java originals take, because both Java bodies
/// ignore it: `getTreeShape(ShapeTree, int)` answers `getShape()` (`:66-69`) and
/// `shapeLayer(int)` answers `getLayer()` (`:71-74`) — a room has exactly one tree shape
/// (`treeShapeCount` is the constant 1, `:62-64`). The third `SearchTreeObject` method the
/// queries call, `isObstacle(int)`, is the constant `true` for every net (`:76-79`) and so needs
/// no lookup at all.
///
/// `None` means "this id is not a live room", which is Java's dead reference: every caller below
/// panics on it exactly where Java would have thrown a `NullPointerException`.
pub trait RoomLookup {
    /// `CompleteFreeSpaceExpansionRoom.getTreeShape(ShapeTree, int)`
    /// (CompleteFreeSpaceExpansionRoom.java:66-69).
    fn room_tree_shape(&self, id: RoomId) -> Option<&TileShape>;

    /// `CompleteFreeSpaceExpansionRoom.shapeLayer(int)`
    /// (CompleteFreeSpaceExpansionRoom.java:71-74).
    fn room_shape_layer(&self, id: RoomId) -> Option<usize>;
}

/// The [`RoomLookup`] of a tree that holds no expansion rooms — every board-level caller.
///
/// It is what the room-free overloads below pass, so a `TreeObject::Room` leaf reached through
/// one of them panics with the message it panicked with before this trait existed rather than
/// being silently skipped. Only `fr-router` ever supplies a real room lookup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoRooms;

impl RoomLookup for NoRooms {
    fn room_tree_shape(&self, _id: RoomId) -> Option<&TileShape> {
        None
    }

    fn room_shape_layer(&self, _id: RoomId) -> Option<usize> {
        None
    }
}

/// One `TreeSet<EntrySortedByClearance>` element (ShapeSearchTree.java:1135-1158).
///
/// Java's `compareTo` is `Signum.asInt(clearance - other.clearance)` and then
/// `entryId - other.entryId`, which is exactly the derived lexicographic `Ord` on
/// `(clearance, entry_id)` — the clearance values come from the clearance matrix and are
/// non-negative, so the `int` subtraction cannot overflow into the wrong sign.
///
/// `leaf` is not part of the ordering, and does not need to be: `entry_id` is unique, so no two
/// elements ever compare equal and Java's `TreeSet` never de-duplicates one away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EntrySortedByClearance {
    /// Java `EntrySortedByClearance.clearance` (ShapeSearchTree.java:1139).
    clearance: i32,
    /// Java `EntrySortedByClearance.entryId` (ShapeSearchTree.java:1137).
    entry_id: u64,
    /// Java `EntrySortedByClearance.leaf` (ShapeSearchTree.java:1138), as the `(object, index)`
    /// pair the port reads off the leaf.
    entry: TreeEntry<TreeObject>,
}

/// Port of `ShapeSearchTree` and its two angle subclasses: the spatial index the router queries.
///
/// not ported: `ShapeSearchTree.board` (ShapeSearchTree.java:64) — see the module docs.
///
/// not ported: `ShapeSearchTree.toString` (ShapeSearchTree.java:89-92), whose body is
/// `return key` — [`ShapeSearchTree::get_key`] is that value, and a `Display` impl is provided
/// below so `format!("{tree}")` still prints it.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeSearchTree {
    /// The tree's identity within its [`SearchTreeManager`](super::SearchTreeManager), used as
    /// the key of the per-item entry/shape caches. Java identifies a tree by object reference
    /// (`ItemSearchTreesInfo` keys its list on `ShapeTree` identity).
    id: TreeId,
    /// Which of Java's three tree classes this is — see the module docs' mapping table.
    angle: AngleRestriction,
    /// The `MinAreaTree` this class extends (ShapeSearchTree.java:50).
    tree: ShapeTree<TreeObject>,
    /// Java `ShapeSearchTree.compensatedClearanceClassNo` (ShapeSearchTree.java:61): the
    /// clearance class the stored shapes are compensated for, or 0 for no compensation.
    compensated_clearance_class: usize,
}

impl std::fmt::Display for ShapeSearchTree {
    /// `ShapeSearchTree.toString` (ShapeSearchTree.java:89-92) = `key`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.get_key())
    }
}

impl ShapeSearchTree {
    /// Port of the package-private `ShapeSearchTree(ShapeBoundingDirections, BasicBoard, int)`
    /// (ShapeSearchTree.java:71-77) and both subclass constructors
    /// (ShapeSearchTree45Degree.java:30-32, ShapeSearchTree90Degree.java:27-29).
    ///
    /// Java's three constructors differ only in the directions they pass up, and those
    /// directions are a function of the subclass: `FortyfiveDegreeBoundingDirections` for the
    /// base class and the 45-degree one, `OrthogonalBoundingDirections` for the 90-degree one.
    /// `angle` names the subclass, so the directions follow from it — see
    /// [`Self::bounding_directions_for`].
    pub fn new(
        id: TreeId,
        angle: AngleRestriction,
        compensated_clearance_class: usize,
    ) -> ShapeSearchTree {
        ShapeSearchTree {
            id,
            angle,
            tree: ShapeTree::new(Self::bounding_directions_for(angle)),
            compensated_clearance_class,
        }
    }

    /// The bounding directions Java's constructors pass to `super`:
    /// `OrthogonalBoundingDirections.INSTANCE` for `ShapeSearchTree90Degree`
    /// (ShapeSearchTree90Degree.java:28), `FortyfiveDegreeBoundingDirections.INSTANCE` for the
    /// other two (ShapeSearchTree.java:33 via `SearchTreeManager`,
    /// ShapeSearchTree45Degree.java:31).
    pub fn bounding_directions_for(angle: AngleRestriction) -> ShapeBoundingDirections {
        match angle {
            AngleRestriction::NinetyDegree => ShapeBoundingDirections::Orthogonal,
            AngleRestriction::None | AngleRestriction::FortyFiveDegree => {
                ShapeBoundingDirections::FortyfiveDegree
            }
        }
    }

    /// This tree's id, which items use as the key of their per-tree entry and shape caches.
    /// No Java counterpart (Java keys on object identity).
    pub fn id(&self) -> TreeId {
        self.id
    }

    /// Which of Java's three tree classes this is.
    pub fn angle(&self) -> AngleRestriction {
        self.angle
    }

    /// Java `ShapeSearchTree.compensatedClearanceClassNo` (ShapeSearchTree.java:61), a public
    /// final field.
    pub fn compensated_clearance_class(&self) -> usize {
        self.compensated_clearance_class
    }

    /// The underlying `MinAreaTree` (Java: the superclass this one extends).
    pub fn tree(&self) -> &ShapeTree<TreeObject> {
        &self.tree
    }

    /// Java `ShapeTree.size()` (ShapeTree.java:106-108).
    pub fn size(&self) -> usize {
        self.tree.leaf_count()
    }

    /// Port of the static `ShapeSearchTree.getKey` (ShapeSearchTree.java:80-87), reproducing
    /// Java's string exactly: the tree class's simple name, the directions class's simple name
    /// with `BoundingDirections` stripped, and `_cc` plus the compensated class number.
    ///
    /// Verified against the real JVM: a default tree prints `ShapeSearchTree_FortyfiveDegree_cc0`
    /// and a 90-degree autoroute tree `ShapeSearchTree90Degree_Orthogonal_cc1`.
    // renamed: the static `getKey(searchTree, directions, clearanceClass)` -> a method, because
    // every argument it takes is a property of the tree it is handed. Java's `key` field
    // (ShapeSearchTree.java:63) is this value, computed once in the constructor.
    pub fn get_key(&self) -> String {
        let class = match self.angle {
            AngleRestriction::None => "ShapeSearchTree",
            AngleRestriction::FortyFiveDegree => "ShapeSearchTree45Degree",
            AngleRestriction::NinetyDegree => "ShapeSearchTree90Degree",
        };
        let directions = match Self::bounding_directions_for(self.angle) {
            ShapeBoundingDirections::Orthogonal => "Orthogonal",
            ShapeBoundingDirections::FortyfiveDegree => "FortyfiveDegree",
        };
        format!(
            "{class}_{directions}_cc{}",
            self.compensated_clearance_class
        )
    }

    /// Port of `ShapeSearchTree.isClearanceCompensationUsed` (ShapeSearchTree.java:95-97).
    pub fn is_clearance_compensation_used(&self) -> bool {
        self.compensated_clearance_class > 0
    }

    /// Port of `ShapeSearchTree.clearanceCompensationValue` (ShapeSearchTree.java:104-114): how
    /// far this tree's shapes are inflated for an item of clearance class
    /// `clearance_class_index` on `layer`.
    ///
    /// Java's `clearanceClassIndex <= 0` short circuit (ShapeSearchTree.java:105-107) fires for
    /// the "none" class; with a `usize` only `== 0` is reachable, and an uncompensated tree
    /// answers 0 through the arithmetic anyway (class 0's clearances are all 0).
    pub fn clearance_compensation_value(
        &self,
        clearance_class_index: usize,
        layer: usize,
        rules: &BoardRules,
    ) -> i32 {
        if clearance_class_index == 0 {
            return 0;
        }
        let matrix = &rules.clearance_matrix;
        let result = matrix.get_value(
            clearance_class_index,
            self.compensated_clearance_class,
            layer,
            false,
        ) - matrix
            .clearance_compensation_value(self.compensated_clearance_class, layer);
        result.max(0)
    }

    /// Port of `Trace.getCompensatedHalfWidth(ShapeSearchTree)` (Trace.java:86-89):
    /// `halfWidth + searchTree.clearanceCompensationValue(clearanceClassIndex(), layer)`.
    // renamed: Trace.getCompensatedHalfWidth -> ShapeSearchTree::compensated_half_width; the
    // whole body is a call on the tree, and putting it here keeps `PolylineTrace` free of the
    // search tree (see the `renamed:` note on `Trace.getCompensatedHalfWidth` in
    // `items/trace.rs`).
    pub fn compensated_half_width(&self, trace: &PolylineTrace, rules: &BoardRules) -> i32 {
        trace.get_half_width()
            + self.clearance_compensation_value(
                trace.hdr.clearance_class(),
                trace.get_layer(),
                rules,
            )
    }
}

// -------------------------------------------------------------------------------------------
// Tree shapes: `calculateTreeShapes` and the two subclass overrides of each variant
// -------------------------------------------------------------------------------------------

impl ShapeSearchTree {
    /// Port of the four `ShapeSearchTree.calculateTreeShapes` overloads, dispatched the way
    /// Java's `Item.calculateTreeShapes(ShapeSearchTree)` overrides dispatch them:
    ///
    /// | variant | Java override | body |
    /// |---|---|---|
    /// | [`Item::Via`], [`Item::Pin`] | `DrillItem.calculateTreeShapes` (DrillItem.java:211-214) | ShapeSearchTree.java:871-906 + both subclass overrides |
    /// | the four area variants | `ObstacleArea.calculateTreeShapes` (ObstacleArea.java:183-186) | ShapeSearchTree.java:908-938 + both subclass overrides |
    /// | [`Item::BoardOutline`] | `BoardOutline.calculateTreeShapes` (BoardOutline.java:260-263) | ShapeSearchTree.java:940-990 + both subclass overrides |
    /// | [`Item::Trace`] | `PolylineTrace.calculateTreeShapes` (PolylineTrace.java:133-136) | ShapeSearchTree.java:992-1004 |
    /// | [`Item::ComponentOutline`] | `ComponentOutline.calculateTreeShapes` (ComponentOutline.java:135-137) | `return new TileShape[0]` |
    ///
    /// The result is `Vec<Option<TileShape>>` because Java's `TileShape[]` really does carry
    /// `null`s — see [`crate::items::TreeEntries::shapes`].
    pub fn calculate_tree_shapes(&self, item: &Item, ctx: &ItemCtx<'_>) -> Vec<Option<TileShape>> {
        match item {
            Item::Via(_) | Item::Pin(_) => self.calculate_drill_item_tree_shapes(item, ctx),
            Item::ObstacleArea(_)
            | Item::ConductionArea(_)
            | Item::ViaObstacleArea(_)
            | Item::ComponentObstacleArea(_) => self.calculate_obstacle_area_tree_shapes(item, ctx),
            Item::BoardOutline(_) => self.calculate_board_outline_tree_shapes(item, ctx),
            Item::Trace(trace) => self.calculate_trace_tree_shapes(trace, ctx),
            // ComponentOutline.java:135-137: `return new TileShape[0]`.
            Item::ComponentOutline(_) => Vec::new(),
        }
    }

    /// Port of `ShapeSearchTree.calculateTreeShapes(DrillItem)` (ShapeSearchTree.java:871-906)
    /// and its two overrides, ShapeSearchTree45Degree.java:488-519 and
    /// ShapeSearchTree90Degree.java:434-462.
    ///
    /// All three share the loop, the `getShape(i) == null -> drillHoleObstacle` fallback and the
    /// `offsetWidth`; they differ only in how the shape is regularised and inflated:
    ///
    /// * base (ShapeSearchTree.java:885-892, 900): `boundingBox` / `boundingOctagon` /
    ///   `boundingTile` per `rules.traceAngleRestriction`, then `enlarge(offsetWidth)`;
    /// * 45 degree (ShapeSearchTree45Degree.java:502-515): `boundingOctagon`, but
    ///   `boundingBox` when that octagon is really a box — Java's own comment: "to avoid small
    ///   corner cutoffs when taking the offset as an octagon" — then **`offset`**, not
    ///   `enlarge`, and finally `boundingOctagon` again;
    /// * 90 degree (ShapeSearchTree90Degree.java:448-458): `boundingBox`, then `offset`.
    ///
    /// `enlarge` and `offset` are not the same operation on a diagonal border line: `enlarge`
    /// moves it by `offsetWidth` along its normal, `offset` moves its x-coordinate by
    /// `offsetWidth`. That is the whole difference between the base tree's
    /// `Oct[...,-741,-259,...]` and the 45-degree tree's `Oct[...,-800,-200,...]` for the same
    /// 100x100 pad inflated by 100 (verified against the JVM).
    fn calculate_drill_item_tree_shapes(
        &self,
        item: &Item,
        ctx: &ItemCtx<'_>,
    ) -> Vec<Option<TileShape>> {
        let count = item.tile_shape_count(ctx);
        (0..count)
            .map(|i| {
                // ShapeSearchTree.java:877-880.
                let current_shape = drill_item_shape(item, i, ctx)
                    .or_else(|| self.drill_hole_obstacle(item, ctx))?;
                let layer = item.shape_layer(i, ctx);
                // ShapeSearchTree.java:893-896.
                let offset_width =
                    self.clearance_compensation_value(
                        item.header().clearance_class(),
                        layer,
                        ctx.rules,
                    ) + self.drill_hole_clearance_delta(item, &current_shape, layer, ctx);
                let offset_width = f64::from(offset_width);
                Some(match self.angle {
                    // ShapeSearchTree90Degree.java:448-458.
                    AngleRestriction::NinetyDegree => {
                        TileShape::Box(current_shape.bounding_box().offset(offset_width))
                    }
                    // ShapeSearchTree45Degree.java:502-515.
                    AngleRestriction::FortyFiveDegree => {
                        let octagon = current_shape.bounding_octagon()?;
                        let tile = if TileShape::Octagon(octagon).is_int_box() {
                            TileShape::Box(current_shape.bounding_box())
                        } else {
                            TileShape::Octagon(octagon)
                        };
                        let offset = tile.offset(offset_width);
                        TileShape::Octagon(offset.bounding_octagon()?)
                    }
                    // ShapeSearchTree.java:885-892, 900.
                    AngleRestriction::None => {
                        let tile = match ctx.rules.trace_angle_restriction {
                            AngleRestriction::NinetyDegree => {
                                TileShape::Box(current_shape.bounding_box())
                            }
                            AngleRestriction::FortyFiveDegree => {
                                TileShape::Octagon(current_shape.bounding_octagon()?)
                            }
                            AngleRestriction::None => current_shape.bounding_tile(),
                        };
                        tile.enlarge(offset_width)
                    }
                })
            })
            .collect()
    }

    /// Port of `ShapeSearchTree.drillHoleObstacle` (ShapeSearchTree.java:1012-1028): the
    /// synthesised hole obstacle for a copper layer on which a drilled item has no pad.
    ///
    /// Java's `board == null || board.rules == null` guard is unrepresentable here.
    fn drill_hole_obstacle(&self, item: &Item, ctx: &ItemCtx<'_>) -> Option<Shape> {
        if ctx.rules.get_hole_clearance() <= 0 {
            return None;
        }
        let padstack = drill_item_padstack(item, ctx)?;
        let drill_radius = padstack.drill_radius();
        if drill_radius <= 0.0 {
            return None;
        }
        // ShapeSearchTree.java:1023-1026: a rational centre is rounded to the nearest grid point.
        let center = match drill_item_center(item, ctx) {
            Point::Int(p) => p,
            other => other.to_float().round(),
        };
        Some(Shape::Circle(Circle::new(
            center,
            drill_radius.ceil() as i32,
        )))
    }

    /// Port of `ShapeSearchTree.drillHoleClearanceDelta` (ShapeSearchTree.java:1035-1074): the
    /// extra inflation that keeps other-net copper `holeClearance` away from the drill *hole*
    /// rather than the copper pad.
    fn drill_hole_clearance_delta(
        &self,
        item: &Item,
        shape: &Shape,
        layer: usize,
        ctx: &ItemCtx<'_>,
    ) -> i32 {
        let hole_clearance = ctx.rules.get_hole_clearance();
        if hole_clearance <= 0 {
            return 0;
        }
        let Some(padstack) = drill_item_padstack(item, ctx) else {
            return 0;
        };
        let drill_radius = padstack.drill_radius();
        if drill_radius <= 0.0 {
            return 0;
        }
        // ShapeSearchTree.java:1048-1057.
        let copper_radius = if padstack.hole_only {
            drill_radius
        } else {
            let from_shape = shape.border_distance(&drill_item_center(item, ctx).to_float());
            if from_shape > 0.0 {
                from_shape
            } else {
                match padstack.get_shape(layer as i32) {
                    None => drill_radius,
                    Some(pad_shape) => pad_shape.border_distance(&fr_geometry::FloatPoint::ZERO),
                }
            }
        };
        // ShapeSearchTree.java:1058-1061.
        let clearance_class = if self.compensated_clearance_class > 0 {
            self.compensated_clearance_class
        } else {
            BoardRules::default_clearance_class()
        };
        let copper_clearance = ctx.rules.clearance_matrix.get_value(
            item.header().clearance_class(),
            clearance_class,
            layer,
            false,
        );
        // ShapeSearchTree.java:1065-1073.
        let raw =
            (drill_radius + f64::from(hole_clearance) + f64::from(DRILL_HOLE_CLEARANCE_MARGIN)
                - copper_radius
                - f64::from(copper_clearance))
            .ceil();
        (raw as i32).max(0)
    }

    /// Port of `ShapeSearchTree.calculateTreeShapes(ObstacleArea)` (ShapeSearchTree.java:908-938)
    /// and its two overrides, which post-process every piece with `boundingOctagon`
    /// (ShapeSearchTree45Degree.java:521-530) or `boundingBox`
    /// (ShapeSearchTree90Degree.java:464-473).
    ///
    /// `ctx.max_tree_shape_width` is Java's `maxTreeShapeWidth` (ShapeSearchTree.java:916-920),
    /// which reads `board.communication`.
    fn calculate_obstacle_area_tree_shapes(
        &self,
        item: &Item,
        ctx: &ItemCtx<'_>,
    ) -> Vec<Option<TileShape>> {
        // ShapeSearchTree.java:912-915: a failed division answers an empty array.
        let Some(convex_shapes) = obstacle_area_split_to_convex(item, ctx) else {
            return Vec::new();
        };
        // ShapeSearchTree.java:925-927: the offset is computed inside the loop but does not
        // depend on it.
        let offset_width = f64::from(self.clearance_compensation_value(
            item.header().clearance_class(),
            item.first_layer(ctx),
            ctx.rules,
        ));
        convex_shapes
            .iter()
            .flat_map(|piece| {
                piece
                    .enlarge(offset_width)
                    .divide_into_sections(ctx.max_tree_shape_width)
            })
            .map(|piece| Some(self.regularise(piece)))
            .collect()
    }

    /// Port of `ShapeSearchTree.calculateTreeShapes(BoardOutline)` (ShapeSearchTree.java:940-990)
    /// and its two overrides (ShapeSearchTree45Degree.java:532-541,
    /// ShapeSearchTree90Degree.java:475-484).
    ///
    /// Two modes, exactly as `BoardOutline.tileShapeCount` has: with the keepout generated, the
    /// convex pieces of the keepout area repeated once per layer; without it, a
    /// `halfWidth`-wide band around every border line of every outline polygon, again once per
    /// layer. Note that the loop nesting differs between the two branches — layer-major in both,
    /// but the keepout branch enlarges per layer while the line branch builds a fresh
    /// three-line `Polyline` per border line.
    fn calculate_board_outline_tree_shapes(
        &self,
        item: &Item,
        ctx: &ItemCtx<'_>,
    ) -> Vec<Option<TileShape>> {
        let Item::BoardOutline(outline) = item else {
            unreachable!("calculate_board_outline_tree_shapes is only reached for BoardOutline")
        };
        let layer_count = ctx.rules.layer_structure().count();
        let clearance_class = item.header().clearance_class();
        let mut result: Vec<Option<TileShape>> = Vec::new();
        if outline.keepout_outside_outline_generated() {
            // ShapeSearchTree.java:945-964.
            let Some(convex_shapes) = outline.keepout_convex_pieces(ctx) else {
                return Vec::new();
            };
            for layer_index in 0..layer_count {
                let offset_width = f64::from(self.clearance_compensation_value(
                    clearance_class,
                    layer_index,
                    ctx.rules,
                ));
                for piece in convex_shapes {
                    result.push(Some(self.regularise(piece.enlarge(offset_width))));
                }
            }
        } else {
            // ShapeSearchTree.java:966-987: only the line shapes of the outline are inserted.
            let half_width = outline.get_half_width();
            for layer_index in 0..layer_count {
                let cmp_value =
                    self.clearance_compensation_value(clearance_class, layer_index, ctx.rules);
                for shape_index in 0..outline.shape_count() {
                    let Some(outline_shape) = outline.get_shape(shape_index) else {
                        continue;
                    };
                    let ops = outline_shape.as_ops();
                    let border_line_count = ops.border_line_count();
                    if border_line_count == 0 {
                        continue;
                    }
                    let mut previous = ops.border_line(border_line_count - 1);
                    for i in 0..border_line_count {
                        let current = ops.border_line(i);
                        let next = ops.border_line((i + 1) % border_line_count);
                        let shape = match (previous, current, next) {
                            // ShapeSearchTree.java:982 calls `tmpPolyline.offsetShape(...)`, the
                            // **non-virtual** `Polyline.offsetShape` — not `this.offsetShape`, so
                            // the 90-degree tree's `offsetBox` override does *not* apply here.
                            (Some(a), Some(b), Some(c)) => Polyline::from_lines(vec![a, b, c])
                                .ok()
                                .and_then(|polyline| {
                                    polyline.offset_shape(half_width + cmp_value, 0)
                                }),
                            // Java constructs `new Polyline(currentLineArr)` unconditionally and
                            // would throw on a `null` element; a `PolylineError` is Plan 1's
                            // `Result` for the same input (plan-1-handoff.md #12) and lands here
                            // as the `null` tree shape it would have produced.
                            _ => None,
                        };
                        // The 45-degree / 90-degree overrides map over the *whole* result of
                        // `super.calculateTreeShapes(outline)`, this branch included
                        // (ShapeSearchTree45Degree.java:534-540,
                        // ShapeSearchTree90Degree.java:477-483). A band around a border line is
                        // a `Simplex` unless the line happens to run in one of the tree's own
                        // directions, so this is not a no-op for a skewed outline.
                        result.push(shape.map(|shape| self.regularise(shape)));
                        previous = current;
                    }
                }
            }
        }
        result
    }

    /// Port of `ShapeSearchTree.calculateTreeShapes(PolylineTrace)`
    /// (ShapeSearchTree.java:992-1004). Neither subclass overrides it — the 90-degree tree's
    /// boxes come from its `offsetShape` override instead (ShapeSearchTree90Degree.java:486-490).
    fn calculate_trace_tree_shapes(
        &self,
        trace: &PolylineTrace,
        ctx: &ItemCtx<'_>,
    ) -> Vec<Option<TileShape>> {
        let offset_width = self.compensated_half_width(trace, ctx.rules);
        (0..trace.tile_shape_count())
            .map(|i| self.offset_shape(trace.polyline(), offset_width, i))
            .collect()
    }

    /// The subclass post-processing the area and outline overrides apply to every piece:
    /// `boundingOctagon()` for the 45-degree tree (ShapeSearchTree45Degree.java:526,537),
    /// `boundingBox()` for the 90-degree tree (ShapeSearchTree90Degree.java:469,480), and
    /// nothing for the base class.
    ///
    /// Java guards each with `if (result[i] != null)`; a `None` never reaches here because the
    /// base bodies these wrap return no `null` elements.
    fn regularise(&self, shape: TileShape) -> TileShape {
        match self.angle {
            AngleRestriction::None => shape,
            AngleRestriction::FortyFiveDegree => {
                shape.bounding_octagon().map_or(shape, TileShape::Octagon)
            }
            AngleRestriction::NinetyDegree => TileShape::Box(shape.bounding_box()),
        }
    }

    /// Port of the package-private `ShapeSearchTree.offsetShape` (ShapeSearchTree.java:1079-1081)
    /// and its one override, `ShapeSearchTree90Degree.offsetShape`
    /// (ShapeSearchTree90Degree.java:486-490): the shape a trace's `no`-th segment occupies in
    /// this tree.
    pub fn offset_shape(
        &self,
        polyline: &Polyline,
        half_width: i32,
        no: usize,
    ) -> Option<TileShape> {
        match self.angle {
            AngleRestriction::NinetyDegree => {
                polyline.offset_box(half_width, no).map(TileShape::Box)
            }
            _ => polyline.offset_shape(half_width, no),
        }
    }

    /// Port of `ShapeSearchTree.offsetShapes` (ShapeSearchTree.java:1086-1088) and its one
    /// override, `ShapeSearchTree90Degree.offsetShapes` (ShapeSearchTree90Degree.java:492-503).
    ///
    /// The 90-degree body re-derives the clamping and the shape count that `Polyline.offsetShapes`
    /// does for itself, then calls `offsetBox` per segment; `Polyline::offset_shapes_between`
    /// already carries the base version of that arithmetic, so both arms agree on how many
    /// shapes come back.
    pub fn offset_shapes(
        &self,
        polyline: &Polyline,
        half_width: i32,
        from_no: usize,
        to_no: usize,
    ) -> Vec<TileShape> {
        match self.angle {
            AngleRestriction::NinetyDegree => {
                // ShapeSearchTree90Degree.java:495-502. `fromNo = Math.max(fromNo, 0)` is
                // automatic for a `usize`.
                let to_no = to_no.min(polyline.lines().len().saturating_sub(1));
                (from_no..to_no.saturating_sub(1))
                    .map(|j| {
                        TileShape::Box(polyline.offset_box(half_width, j).expect(
                            "ShapeSearchTree90Degree.offsetShapes: `j + 1 <= lines.len() - 2` \
                             holds after the clamp, so the line segment always exists",
                        ))
                    })
                    .collect()
            }
            _ => polyline.offset_shapes_between(half_width, from_no, to_no),
        }
    }
}

/// `DrillItem.getShape(int)` across the two drill variants (Via.java:114-135,
/// Pin.java:213-292).
fn drill_item_shape(item: &Item, index: usize, ctx: &ItemCtx<'_>) -> Option<Shape> {
    match item {
        Item::Via(via) => via.get_shape(index, ctx),
        Item::Pin(pin) => pin.get_shape(index, ctx),
        _ => unreachable!("drill_item_shape is only reached for Via and Pin"),
    }
}

/// `DrillItem.getPadstack()` across the two drill variants (Via.java:98-100,
/// Pin.java:124-135).
fn drill_item_padstack<'a>(item: &Item, ctx: &ItemCtx<'a>) -> Option<&'a Padstack> {
    match item {
        Item::Via(via) => via.get_padstack(ctx),
        Item::Pin(pin) => pin.get_padstack(ctx),
        _ => unreachable!("drill_item_padstack is only reached for Via and Pin"),
    }
}

/// `DrillItem.getCenter()` across the two drill variants (Via.java:102-104,
/// Pin.java:157-211).
fn drill_item_center(item: &Item, ctx: &ItemCtx<'_>) -> Point {
    match item {
        Item::Via(via) => via.get_center(),
        Item::Pin(pin) => pin.get_center(ctx),
        _ => unreachable!("drill_item_center is only reached for Via and Pin"),
    }
}

/// `ObstacleArea.splitToConvex()` across the four area variants (ObstacleArea.java:320-326).
fn obstacle_area_split_to_convex<'a>(item: &'a Item, ctx: &ItemCtx<'_>) -> Option<&'a [TileShape]> {
    match item {
        Item::ObstacleArea(a) => a.split_to_convex(ctx),
        Item::ConductionArea(a) => a.split_to_convex(ctx),
        Item::ViaObstacleArea(a) => a.split_to_convex(ctx),
        Item::ComponentObstacleArea(a) => a.split_to_convex(ctx),
        _ => unreachable!("obstacle_area_split_to_convex is only reached for the area variants"),
    }
}

// -------------------------------------------------------------------------------------------
// Insertion and removal
// -------------------------------------------------------------------------------------------

impl ShapeSearchTree {
    /// Port of `ShapeTree.insert(ShapeTree.Storable)` (ShapeTree.java:32-42) specialised to an
    /// `Item`, with the lazy shape fill Java performs inside `Item.getTreeShape`
    /// (Item.java:212-238) folded in.
    ///
    /// Java's `insert(Storable)` calls `obj.treeShapeCount(this)`, which calls the private
    /// `Item.getPrecalculatedTreeShapes` (Item.java:228-238), which computes and *stores*
    /// `calculateTreeShapes(this)` on a miss. So a cold item is warmed by this call, and a warm
    /// one is reused untouched — which is why `SearchTreeManager.reinsertTreeItems`
    /// (SearchTreeManager.java:186-200) has to call `clearDerivedData()` before re-inserting.
    ///
    /// Java's `shapeCount <= 0` early return (ShapeTree.java:33-35) happens **before**
    /// `setSearchTreeEntries`, so a zero-shape item keeps whatever entry array it already had;
    /// this reproduces that (the `ShapeTree::insert_tiles` obligation note).
    pub fn insert_item(&mut self, item: &mut Item, ctx: &ItemCtx<'_>) {
        self.fill_tree_shapes(item, ctx);
        let shapes = item
            .header()
            .get_precalculated_tree_shapes(self.id)
            .expect("fill_tree_shapes always leaves a shape array for this tree")
            .to_vec();
        // ShapeTree.java:33-35.
        if shapes.is_empty() {
            return;
        }
        let entries = self
            .tree
            .insert_tiles_opt(TreeObject::Item(item.id()), &shapes);
        item.set_tree_entries(self.id, entries);
    }

    /// Removes an item's entries from this tree: `SearchTreeManager.remove`'s per-tree half
    /// (SearchTreeManager.java:51-59), which is `ShapeTree.remove(Leaf[])`
    /// (ShapeTree.java:96-104) on the possibly-holed entry array.
    ///
    /// Clearing the item's stored entries is *not* done here: Java clears them once, after the
    /// loop over every tree (SearchTreeManager.java:60), so
    /// [`SearchTreeManager::remove`](super::SearchTreeManager::remove) owns that step.
    pub fn remove_item(&mut self, item: &mut Item) {
        if let Some(entries) = item.get_search_tree_entries(self.id) {
            let entries = entries.to_vec();
            self.tree.remove_opt(&entries);
        }
    }

    /// Port of `ShapeTree.insert(ShapeTree.Storable)` (ShapeTree.java:32-42) specialised to an
    /// **expansion room** — `AutorouteEngine.addCompleteRoom`'s
    /// `this.autorouteSearchTree.insert(completedRoom)`
    /// (`autoroute/maze/AutorouteEngine.java:534`).
    ///
    /// `CompleteFreeSpaceExpansionRoom implements SearchTreeObject`
    /// (`autoroute/expansion/CompleteFreeSpaceExpansionRoom.java:19-20`), so rooms and board
    /// items share this tree and one ordered result set (plan-rulings.md #2, discharged by
    /// Plan 6 Task 2). A room answers `treeShapeCount(tree) == 1` (`:62-64`) and
    /// `getTreeShape(tree, 0) == getShape()` (`:66-69`), so this inserts exactly one leaf at
    /// shape index 0.
    ///
    /// `None` is Java's `null` `Leaf`: the room's shape has no bound in this tree's directions
    /// (ShapeTree.java:51-55). The caller stores the answer and hands it back to
    /// [`ShapeSearchTree::remove_room`] — the port keeps the entries on the object instead of
    /// the tree calling `setSearchTreeEntries` back into it (the `not ported: Storable` note on
    /// [`ShapeTree`]), which is the same inversion [`ShapeSearchTree::insert_item`] uses.
    ///
    /// A room inserted here is answered by the `*_with_rooms` queries
    /// ([`Self::overlapping_tree_entries_with_rooms`],
    /// [`Self::overlapping_objects_with_rooms`]), which take the [`RoomLookup`] that resolves
    /// its shape and layer. The room-free overloads pass [`NoRooms`] and therefore still panic
    /// on a room leaf — deliberately, because a board-level caller that reaches one has queried
    /// a tree it does not own. (Plan 6 Task 4 discharged the obligation this note used to
    /// record.) The low-level [`ShapeTree::overlaps`](crate::datastructures::ShapeTree::overlaps)
    /// is unaffected — it never looks inside the object key.
    pub fn insert_room(&mut self, room: RoomId, shape: &TileShape) -> Option<LeafId> {
        let bounds = self.tree.bounding_shape(shape)?;
        Some(self.tree.insert_leaf(TreeObject::Room(room), 0, bounds))
    }

    /// Removes an expansion room's entry from this tree —
    /// `CompleteFreeSpaceExpansionRoom.removeFromTree`'s `shapeTree.remove(this.treeEntries)`
    /// (`autoroute/expansion/CompleteFreeSpaceExpansionRoom.java:56-59`), for the one-element
    /// entry array a room has.
    ///
    /// `None` is Java's `null` element, skipped exactly as [`ShapeTree::remove_leaf_opt`] skips
    /// it (MinAreaTree.java:121-123) — a room whose shape had no bound was never inserted.
    pub fn remove_room(&mut self, leaf: Option<LeafId>) {
        self.tree.remove_leaf_opt(leaf);
    }

    /// Java's private `Item.getPrecalculatedTreeShapes` (Item.java:228-238): fill this tree's
    /// shape cache on the item if it is empty.
    fn fill_tree_shapes(&self, item: &mut Item, ctx: &ItemCtx<'_>) {
        if item
            .header()
            .get_precalculated_tree_shapes(self.id)
            .is_none()
        {
            let shapes = self.calculate_tree_shapes(item, ctx);
            item.set_precalculated_tree_shapes(self.id, shapes);
        }
    }

    /// Port of `Item.getTreeShape(ShapeTree, int)` (Item.java:212-226) **including its lazy
    /// recompute**: the cached shape when there is one, and otherwise the value
    /// `getPrecalculatedTreeShapes` would have computed and stored (Item.java:227-238).
    ///
    /// Java stores what it recomputes; this cannot, because the item is borrowed immutably —
    /// so a cold cache costs a `calculateTreeShapes` per call instead of one per item.
    /// [`crate::Board::item_tree_shape`] is the `&mut` variant that does store, and every
    /// `&mut self` board method uses it. The only way to reach a cold cache at all is an item
    /// whose `clearDerivedData()` ran without a re-insert — `Item.changeClearanceClassIndex`
    /// with clearance compensation off (Item.java:944-949) is the one board-level path that
    /// does that.
    ///
    /// `None` for Java's two `null` returns: an index past the end even after the recompute,
    /// and a `null` element (a drill layer with no pad and no synthesised hole obstacle,
    /// ShapeSearchTree.java:882).
    pub fn get_tree_shape<'a>(
        &self,
        item: &'a Item,
        index: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<Cow<'a, TileShape>> {
        // Item.java:218-221: an out-of-range index drops the derived data and recomputes once.
        // With nothing else cached to drop, that is just the recompute below.
        if let Some(shapes) = item.header().get_precalculated_tree_shapes(self.id)
            && index < shapes.len()
        {
            return shapes[index].as_ref().map(Cow::Borrowed);
        }
        let mut shapes = self.calculate_tree_shapes(item, ctx);
        if index >= shapes.len() {
            return None;
        }
        shapes.swap_remove(index).map(Cow::Owned)
    }

    /// The tree shape of an object already in this tree, recomputing it if the item's cache was
    /// dropped since insertion — see [`Self::get_tree_shape`], which this is
    /// `currentObject.getTreeShape(this, index)` (e.g. ShapeSearchTree.java:499).
    fn tree_shape_of<'a>(
        &self,
        entry: TreeEntry<TreeObject>,
        items: &'a impl ItemLookup,
        rooms: &'a impl RoomLookup,
        ctx: &ItemCtx<'_>,
    ) -> Cow<'a, TileShape> {
        match entry.object {
            TreeObject::Item(id) => {
                let item = items.item(id).unwrap_or_else(|| {
                    panic!("ShapeSearchTree: item {id} has a leaf but is not in the item list")
                });
                self.get_tree_shape(item, entry.shape_index, ctx)
                    .unwrap_or_else(|| {
                        // Java hands the `null` straight to `intersects`/`enlarge` and throws a
                        // NullPointerException; a leaf without a shape is the same inconsistency.
                        panic!(
                            "ShapeSearchTree: item {id} has a leaf for shape {} in tree {:?} but \
                             no shape for it — Java NPEs here too",
                            entry.shape_index, self.id
                        )
                    })
            }
            // `CompleteFreeSpaceExpansionRoom.getTreeShape`
            // (autoroute/expansion/CompleteFreeSpaceExpansionRoom.java:66-69), which ignores
            // both of its arguments and answers the room's own shape. `fr-board` cannot name a
            // room, so the shape comes from the caller's [`RoomLookup`] — [`NoRooms`] for every
            // board-level caller, `fr-router`'s room store for `SortedRoomNeighbours`.
            TreeObject::Room(id) => Cow::Borrowed(rooms.room_tree_shape(id).unwrap_or_else(|| {
                panic!(
                    "ShapeSearchTree: expansion room {id:?} has a leaf in tree {:?} but the \
                     supplied RoomLookup does not resolve it — Java NPEs here too",
                    self.id
                )
            })),
        }
    }
}

// -------------------------------------------------------------------------------------------
// Queries
// -------------------------------------------------------------------------------------------

impl ShapeSearchTree {
    /// Port of `ShapeSearchTree.overlappingObjects(ConvexShape, int, int[], Set)`
    /// (ShapeSearchTree.java:355-364) and the two-argument convenience overload
    /// (:370-374), which passes `new int[0]`.
    ///
    /// Java's `Set<SearchTreeObject>` is a `TreeSet`, so it iterates in `SearchTreeObject`
    /// order; [`BTreeSet<TreeObject>`] is the same order (rooms first, then items by
    /// **descending** id — `crate::ids::TreeObject`'s `Ord`). Verified against the JVM: a probe
    /// box over three items answers `4 3 2`.
    ///
    /// `layer` is `Option<usize>`: `None` is Java's "if layer < 0, the layer is ignored".
    pub fn overlapping_objects(
        &self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        items: &impl ItemLookup,
        ctx: &ItemCtx<'_>,
    ) -> BTreeSet<TreeObject> {
        self.overlapping_objects_with_rooms(shape, layer, ignore_net_nos, items, &NoRooms, ctx)
    }

    /// [`Self::overlapping_objects`] over a tree that also holds expansion rooms — the same
    /// method, with the [`RoomLookup`] that resolves a [`TreeObject::Room`] leaf.
    ///
    /// Java needs no such twin: its leaves hold the `SearchTreeObject` itself. Added in plan-6
    /// Task 4 for `SortedRoomNeighbours`, which queries the autoroute tree after the engine has
    /// been inserting complete rooms into it.
    #[allow(clippy::too_many_arguments)]
    pub fn overlapping_objects_with_rooms(
        &self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        items: &impl ItemLookup,
        rooms: &impl RoomLookup,
        ctx: &ItemCtx<'_>,
    ) -> BTreeSet<TreeObject> {
        self.overlapping_tree_entries_with_rooms(shape, layer, ignore_net_nos, items, rooms, ctx)
            .into_iter()
            .map(|entry| entry.object)
            .collect()
    }

    /// Port of `ShapeSearchTree.overlappingTreeEntries(ConvexShape, int, int[], Collection)`
    /// (ShapeSearchTree.java:390-434) and the three-argument overload (:380-383), which passes
    /// `new int[0]`.
    ///
    /// Java appends into a caller-supplied `LinkedList`; the port returns the list instead, in
    /// the same order — that order is the iteration order of `MinAreaTree.overlaps`'s
    /// `TreeSet<Leaf>`, i.e. `(object, shape index)`, which
    /// [`ShapeTree::overlaps`](crate::datastructures::ShapeTree::overlaps) already produces.
    /// Verified against the JVM: `4/0 4/1 4/2 3/0 2/0`.
    ///
    /// Java's `bounds == null` branch (ShapeSearchTree.java:400-403) warns and returns without
    /// touching the collection, which is the empty vector here.
    pub fn overlapping_tree_entries(
        &self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        items: &impl ItemLookup,
        ctx: &ItemCtx<'_>,
    ) -> Vec<TreeEntry<TreeObject>> {
        self.overlapping_tree_entries_with_rooms(shape, layer, ignore_net_nos, items, &NoRooms, ctx)
    }

    /// [`Self::overlapping_tree_entries`] over a tree that also holds expansion rooms — the same
    /// method, with the [`RoomLookup`] that resolves a [`TreeObject::Room`] leaf.
    ///
    /// This is the entry point `autoroute.expansion.SortedRoomNeighbours.calculateNeighbours`
    /// (`SortedRoomNeighbours.java:201`) uses: by then `AutorouteEngine.addCompleteRoom`
    /// (`AutorouteEngine.java:534`) has put complete rooms in the tree, so the query answers
    /// rooms as well as items and both have to be resolvable.
    #[allow(clippy::too_many_arguments)]
    pub fn overlapping_tree_entries_with_rooms(
        &self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        items: &impl ItemLookup,
        rooms: &impl RoomLookup,
        ctx: &ItemCtx<'_>,
    ) -> Vec<TreeEntry<TreeObject>> {
        let Some(bounds) = self.tree.bounding_shape(shape) else {
            return Vec::new();
        };
        // ShapeSearchTree.java:405: `shape instanceof IntOctagon` — a **type** test, not
        // `TileShape.isIntOctagon()`, which is the geometric "can be converted to one" predicate
        // and is true for an `IntBox` as well (IntBox.java:32). The shortcut below must key on
        // the type, so this is `matches!(.., TileShape::Octagon(_))`.
        let is_45_degree = matches!(shape, TileShape::Octagon(_));
        self.tree
            .overlaps(&bounds)
            .into_iter()
            .filter(|entry| {
                if self.ignore_object(*entry, layer, ignore_net_nos, items, rooms, ctx) {
                    return false;
                }
                let current_shape = self.tree_shape_of(*entry, items, rooms, ctx);
                // ShapeSearchTree.java:421-427: for two octagons the bounds test already
                // decided it, so Java skips the intersection check "for performance reasons".
                // `currentShape instanceof IntOctagon` is again a type test.
                if is_45_degree && matches!(*current_shape, TileShape::Octagon(_)) {
                    return true;
                }
                current_shape.intersects(shape)
            })
            .collect()
    }

    /// The shared filter of both query families (ShapeSearchTree.java:410-417 and the
    /// identical :475-482).
    ///
    /// Note Java's loop: the object is ignored if it is *not* an obstacle to **any** of the
    /// ignored net numbers, i.e. if it sits on one of them (`Item.isObstacle(int)` is
    /// `!containsNet(n)`, Item.java:161-164).
    fn ignore_object(
        &self,
        entry: TreeEntry<TreeObject>,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        items: &impl ItemLookup,
        rooms: &impl RoomLookup,
        ctx: &ItemCtx<'_>,
    ) -> bool {
        match entry.object {
            TreeObject::Item(id) => {
                let Some(item) = items.item(id) else {
                    return true;
                };
                if let Some(layer) = layer
                    && item.shape_layer(entry.shape_index, ctx) != layer
                {
                    return true;
                }
                ignore_net_nos
                    .iter()
                    .any(|net_no| !item.is_obstacle_for_net(*net_no))
            }
            // `CompleteFreeSpaceExpansionRoom.shapeLayer` / `isObstacle(int)`
            // (CompleteFreeSpaceExpansionRoom.java:71-84): both are constants — the room's own
            // layer, and `true` for **every** net. So a room is ignored only by layer; the net
            // loop below can never ignore one, because Java's test is
            // `!currentObject.isObstacle(n)` and a room's answer is always `true`.
            TreeObject::Room(id) => {
                let room_layer = rooms.room_shape_layer(id).unwrap_or_else(|| {
                    panic!(
                        "ShapeSearchTree: expansion room {id:?} has a leaf in tree {:?} but the \
                         supplied RoomLookup does not resolve it — Java NPEs here too",
                        self.id
                    )
                });
                layer.is_some_and(|layer| room_layer != layer)
            }
        }
    }

    /// Port of the five-argument `ShapeSearchTree.overlappingTreeEntriesWithClearance`
    /// (ShapeSearchTree.java:443-507): every entry that would clash with a new item of shape
    /// `shape`, clearance class `clearance_class_index` and layer `layer`.
    ///
    /// The algorithm, kept exactly:
    ///
    /// 1. enlarge the query bounds by `(int)(1.2 * clearanceMatrix.maxValue(clearanceClassIndex,
    ///    layer))` and collect every leaf overlapping *that* (ShapeSearchTree.java:462-468). The
    ///    1.2 is Java's own fudge for enlargement not being symmetric.
    /// 2. filter by layer and ignored nets exactly as [`Self::overlapping_tree_entries`] does,
    ///    and sort what survives into a `TreeSet<EntrySortedByClearance>` keyed on the clearance
    ///    to `clearance_class_index` and then the entry id (:472-489).
    /// 3. walk that set in order, enlarging *both* shapes by half the current clearance "to
    ///    create symmetry", and keep the entries whose enlarged shape still intersects
    ///    (:490-506). The enlarged query shape is recomputed only when the half clearance
    ///    changes, and starts as `shape` itself — not `shape.enlarge(0)`, which for an `IntBox`
    ///    would already be an octagon.
    ///
    /// `entry_counter` is Java's **static** `ShapeSearchTree.lastGeneratedEntryId`
    /// (ShapeSearchTree.java:55), which the port makes a per-`SearchTreeManager` `u64`
    /// (global-constraints.md, quirks register): a process-global counter shared by every board
    /// is not reproducible, and Java's wrap at `Integer.MAX_VALUE`
    /// (ShapeSearchTree.java:1144-1148) can invert the tie-break mid-query, which a `u64` never
    /// does. Within one query the effect is identical — the ids are consecutive, so equal
    /// clearances keep the order the leaves were visited in.
    ///
    /// Java's unbounded-shape branch falls back to `board.getBoundingBox()`
    /// (ShapeSearchTree.java:458-461), which is `ctx.bounding_box`.
    // The argument list mirrors Java's, plus the two the port has to pass explicitly
    // where Java reads `this.board`: the item list and the board context.
    #[allow(clippy::too_many_arguments)]
    pub fn overlapping_tree_entries_with_clearance(
        &self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        clearance_class_index: usize,
        items: &impl ItemLookup,
        ctx: &ItemCtx<'_>,
        entry_counter: &mut u64,
    ) -> Vec<TreeEntry<TreeObject>> {
        let matrix: &ClearanceMatrix = &ctx.rules.clearance_matrix;
        let bounds = self
            .tree
            .bounding_shape(shape)
            // ShapeSearchTree.java:458-461.
            .unwrap_or(RegularTileShape::Box(*ctx.bounding_box));
        // ShapeSearchTree.java:462: `maxValue` clamps a negative layer to 0 itself
        // (ClearanceMatrix.java:208-211), which is what `layer.unwrap_or(0)` does.
        let max_clearance =
            (1.2 * f64::from(matrix.max_value(clearance_class_index, layer.unwrap_or(0)))) as i32;
        let offset_bounds = offset_regular(bounds, f64::from(max_clearance));

        // ShapeSearchTree.java:470-489.
        let mut sorted_items: BTreeSet<EntrySortedByClearance> = BTreeSet::new();
        for entry in self.tree.overlaps(&offset_bounds) {
            if self.ignore_object(entry, layer, ignore_net_nos, items, &NoRooms, ctx) {
                continue;
            }
            let TreeObject::Item(id) = entry.object else {
                continue;
            };
            let item = items.item(id).expect("ignore_object rejects unknown items");
            // ShapeSearchTree.java:484-485. A negative layer makes `getValue` return 0
            // (ClearanceMatrix.java:133-161), which `layer_clearance` reproduces.
            let current_clearance = match layer {
                Some(layer) => matrix.get_value(
                    clearance_class_index,
                    item.header().clearance_class(),
                    layer,
                    true,
                ),
                None => 0,
            };
            *entry_counter += 1;
            sorted_items.insert(EntrySortedByClearance {
                clearance: current_clearance,
                entry_id: *entry_counter,
                entry,
            });
        }

        // ShapeSearchTree.java:490-506.
        let mut current_half_clearance = 0;
        let mut current_offset_shape = shape.clone();
        let mut result = Vec::new();
        for sorted in sorted_items {
            let tmp_half_clearance = sorted.clearance / 2;
            if tmp_half_clearance != current_half_clearance {
                current_half_clearance = tmp_half_clearance;
                current_offset_shape = shape.enlarge(f64::from(current_half_clearance));
            }
            let tmp_shape = self.tree_shape_of(sorted.entry, items, &NoRooms, ctx);
            let tmp_offset_shape = tmp_shape.enlarge(f64::from(current_half_clearance));
            if current_offset_shape.intersects(&tmp_offset_shape) {
                result.push(sorted.entry);
            }
        }
        result
    }

    /// Port of the four-argument `ShapeSearchTree.overlappingTreeEntriesWithClearance`
    /// (ShapeSearchTree.java:514-524): on a compensated tree the stored shapes already carry the
    /// clearance, so the plain overlap query is the answer.
    // The argument list mirrors Java's, plus the two the port has to pass explicitly
    // where Java reads `this.board`: the item list and the board context.
    #[allow(clippy::too_many_arguments)]
    pub fn overlapping_tree_entries_with_clearance_auto(
        &self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        clearance_class_index: usize,
        items: &impl ItemLookup,
        ctx: &ItemCtx<'_>,
        entry_counter: &mut u64,
    ) -> Vec<TreeEntry<TreeObject>> {
        if self.is_clearance_compensation_used() {
            self.overlapping_tree_entries(shape, layer, ignore_net_nos, items, ctx)
        } else {
            self.overlapping_tree_entries_with_clearance(
                shape,
                layer,
                ignore_net_nos,
                clearance_class_index,
                items,
                ctx,
                entry_counter,
            )
        }
    }

    /// Port of `ShapeSearchTree.overlappingObjectsWithClearance`
    /// (ShapeSearchTree.java:530-549), which is the four-argument entry-set query collapsed to
    /// its objects.
    // The argument list mirrors Java's, plus the two the port has to pass explicitly
    // where Java reads `this.board`: the item list and the board context.
    #[allow(clippy::too_many_arguments)]
    pub fn overlapping_objects_with_clearance(
        &self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        clearance_class_index: usize,
        items: &impl ItemLookup,
        ctx: &ItemCtx<'_>,
        entry_counter: &mut u64,
    ) -> BTreeSet<TreeObject> {
        self.overlapping_tree_entries_with_clearance_auto(
            shape,
            layer,
            ignore_net_nos,
            clearance_class_index,
            items,
            ctx,
            entry_counter,
        )
        .into_iter()
        .map(|entry| entry.object)
        .collect()
    }

    /// Port of `ShapeSearchTree.overlappingItemsWithClearance`
    /// (ShapeSearchTree.java:557-569): the same query with the expansion rooms dropped.
    ///
    /// Java returns a `TreeSet<Item>`, which iterates in `Item.compareTo` order — **descending**
    /// id (quirk #44). The port returns a `Vec<ItemId>` in that order rather than a `BTreeSet`,
    /// whose `Ord` on `ItemId` would be ascending; it is still a set (the objects came from
    /// one), and the order is the load-bearing part. Verified against the JVM: `5 4 3 2`.
    // renamed: the return type is `Vec<ItemId>`, not `Set<Item>` — the port's trees key on ids,
    // and Java's set order is descending, which `BTreeSet<ItemId>` cannot express.
    // The argument list mirrors Java's, plus the two the port has to pass explicitly
    // where Java reads `this.board`: the item list and the board context.
    #[allow(clippy::too_many_arguments)]
    pub fn overlapping_items_with_clearance(
        &self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        clearance_class_index: usize,
        items: &impl ItemLookup,
        ctx: &ItemCtx<'_>,
        entry_counter: &mut u64,
    ) -> Vec<ItemId> {
        self.overlapping_objects_with_clearance(
            shape,
            layer,
            ignore_net_nos,
            clearance_class_index,
            items,
            ctx,
            entry_counter,
        )
        .into_iter()
        .filter_map(|object| match object {
            TreeObject::Item(id) => Some(id),
            TreeObject::Room(_) => None,
        })
        .collect()
    }

    /// Port of `ShapeSearchTree.validateEntries` (ShapeSearchTree.java:1120-1130): every entry
    /// of `item` must sit at the index it claims.
    ///
    /// Java reads `currentLeaf.shapeIndexInObject`, which NPEs on the `null` elements a holed
    /// entry array carries; the port skips them, because a `None` entry is a shape with no leaf
    /// rather than an inconsistent one. Java's `FRLogger.warn` is dropped.
    pub fn validate_entries(&self, item: &Item) -> bool {
        let Some(entries) = item.get_search_tree_entries(self.id) else {
            return true;
        };
        entries
            .iter()
            .enumerate()
            .all(|(i, entry)| entry.is_none_or(|leaf| self.tree.leaf_entry(leaf).shape_index == i))
    }
}

/// Java's `(RegularTileShape) bounds.offset(maxClearance)` (ShapeSearchTree.java:467): both
/// `IntBox.offset` and `IntOctagon.offset` return their own type, so the cast always holds.
fn offset_regular(bounds: RegularTileShape, dist: f64) -> RegularTileShape {
    match bounds {
        RegularTileShape::Box(b) => RegularTileShape::Box(b.offset(dist)),
        RegularTileShape::Octagon(o) => RegularTileShape::Octagon(o.offset(dist)),
    }
}

// -------------------------------------------------------------------------------------------
// In-place entry surgery (the "used internally for performance improvement" family)
// -------------------------------------------------------------------------------------------

impl ShapeSearchTree {
    /// `ShapeTree.insert(Storable, int)` (ShapeTree.java:45-60) for one index of an item whose
    /// precalculated shapes have *already* been written — which is what every method below does
    /// before it inserts, and why Java's own comment says "correct the precalculated tree shapes
    /// first, because it is used in this.insert".
    fn insert_index(
        &mut self,
        id: ItemId,
        shapes: &[Option<TileShape>],
        index: usize,
    ) -> Option<LeafId> {
        // ShapeTree.java:46-49.
        let shape = shapes.get(index)?.as_ref()?;
        // ShapeTree.java:51-55.
        let bounds = self.tree.bounding_shape(shape)?;
        Some(self.tree.insert_leaf(TreeObject::Item(id), index, bounds))
    }

    /// Java's `leaf.shapeIndexInObject = newIndex` / `leaf.object = toTrace` in-place re-key,
    /// through [`ShapeTree::set_leaf_entry`](crate::datastructures::ShapeTree::set_leaf_entry).
    /// A `null` leaf is skipped, as Java's field write would have thrown.
    fn rekey(&mut self, leaf: Option<LeafId>, id: ItemId, shape_index: usize) {
        if let Some(leaf) = leaf {
            self.tree
                .set_leaf_entry(leaf, TreeObject::Item(id), shape_index);
        }
    }

    /// Port of `ShapeSearchTree.changeEntries` (ShapeSearchTree.java:120-164): replace the tree
    /// entries of `trace` from `keep_at_start_count` up to `newShapeCount - 1 -
    /// keep_at_end_count` with the shapes of `new_polyline`, keeping the rest.
    ///
    /// Java's step order is load-bearing and reproduced exactly: keep the head entries, remove
    /// the middle leaves, re-key the tail entries to their new indices *in place* (the leaves
    /// stay where they are in the tree), write the whole new shape array, and only then insert
    /// the middle.
    pub fn change_entries(
        &mut self,
        trace: &mut PolylineTrace,
        new_polyline: &Polyline,
        keep_at_start_count: usize,
        keep_at_end_count: usize,
        rules: &BoardRules,
    ) {
        let id = trace.hdr.id();
        // ShapeSearchTree.java:124-132.
        let compensated_half_width = self.compensated_half_width(trace, rules);
        let changed_shapes = self.offset_shapes(
            new_polyline,
            compensated_half_width,
            keep_at_start_count,
            new_polyline.lines().len() - 1 - keep_at_end_count,
        );
        // `Item.treeShapeCount(this)` (Item.java:203-210) reading the cache, which the caller
        // has already filled by inserting the trace.
        let old_shape_count = trace
            .hdr
            .get_precalculated_tree_shapes(self.id)
            .map_or(0, <[Option<TileShape>]>::len);
        let new_shape_count = changed_shapes.len() + keep_at_start_count + keep_at_end_count;
        let old_entries: Vec<Option<LeafId>> = trace
            .hdr
            .get_tree_entries(self.id)
            .map(<[Option<LeafId>]>::to_vec)
            .unwrap_or_default();
        let old_shapes: Vec<Option<TileShape>> = trace
            .hdr
            .get_precalculated_tree_shapes(self.id)
            .map(<[Option<TileShape>]>::to_vec)
            .unwrap_or_default();

        let mut new_leaves: Vec<Option<LeafId>> = vec![None; new_shape_count];
        let mut new_shapes: Vec<Option<TileShape>> = vec![None; new_shape_count];

        // ShapeSearchTree.java:138-141.
        for i in 0..keep_at_start_count {
            new_leaves[i] = old_entries[i];
            new_shapes[i].clone_from(&old_shapes[i]);
        }
        // ShapeSearchTree.java:142-144. `saturating_sub`, not `-`: Java's bound is a signed
        // `int`, so `keepAtEndCount > oldShapeCount` makes `i < oldShapeCount - keepAtEndCount`
        // false on the first test and simply skips the loop. A `usize` subtraction would
        // underflow instead.
        for old_entry in old_entries
            .iter()
            .take(old_shape_count.saturating_sub(keep_at_end_count))
            .skip(keep_at_start_count)
        {
            self.tree.remove_leaf_opt(*old_entry);
        }
        // ShapeSearchTree.java:145-152. `newShapeCount - keepAtEndCount` cannot go negative
        // (`newShapeCount` is defined as `changedShapes.length + keepAtStartCount +
        // keepAtEndCount`), but `oldShapeCount - keepAtEndCount + i` can: Java then indexes
        // `oldEntries` with a negative subscript and throws `ArrayIndexOutOfBoundsException`.
        // The subtraction is done in `i64` so the port raises on the same step in release
        // builds as in debug ones, instead of wrapping a `usize`.
        let old_tail_base = old_shape_count as i64 - keep_at_end_count as i64;
        for i in 0..keep_at_end_count {
            let new_index = new_shape_count - keep_at_end_count + i;
            let old_index_signed = old_tail_base + i as i64;
            assert!(
                old_index_signed >= 0,
                "ShapeSearchTree.changeEntries: keepAtEndCount ({keep_at_end_count}) exceeds \
                 oldShapeCount ({old_shape_count}); Java throws ArrayIndexOutOfBoundsException \
                 on oldEntries[{old_index_signed}] (ShapeSearchTree.java:148)"
            );
            let old_index = old_index_signed as usize;
            new_leaves[new_index] = old_entries[old_index];
            self.rekey(new_leaves[new_index], id, new_index);
            new_shapes[new_index].clone_from(&old_shapes[old_index]);
        }
        // ShapeSearchTree.java:156-158: correct the precalculated tree shapes first.
        for (offset, shape) in changed_shapes.into_iter().enumerate() {
            new_shapes[keep_at_start_count + offset] = Some(shape);
        }
        trace
            .hdr
            .set_precalculated_tree_shapes(self.id, new_shapes.clone());

        // ShapeSearchTree.java:160-162. `saturating_sub` for the same reason as :142 above,
        // though here the bound provably cannot go negative.
        for (i, leaf) in new_leaves
            .iter_mut()
            .enumerate()
            .take(new_shape_count.saturating_sub(keep_at_end_count))
            .skip(keep_at_start_count)
        {
            *leaf = self.insert_index(id, &new_shapes, i);
        }
        trace.hdr.set_tree_entries(self.id, new_leaves);
    }

    /// Port of `ShapeSearchTree.mergeEntriesInFront` (ShapeSearchTree.java:170-239): move
    /// `from_trace`'s entries to the front of `to_trace`'s, joined by the link shapes cut from
    /// `joined_polyline`.
    pub fn merge_entries_in_front(
        &mut self,
        from_trace: &mut PolylineTrace,
        to_trace: &mut PolylineTrace,
        joined_polyline: &Polyline,
        from_entry_no: usize,
        to_entry_no: usize,
        rules: &BoardRules,
    ) {
        let to_id = to_trace.hdr.id();
        // ShapeSearchTree.java:176.
        let change_order = from_trace.first_corner() == to_trace.first_corner();
        let from_shape_count_minus_1 = from_trace.tile_shape_count() - 1;
        // ShapeSearchTree.java:181-186.
        let remove_no = if change_order {
            0
        } else {
            from_shape_count_minus_1
        };
        let from_entries: Vec<Option<LeafId>> = from_trace
            .hdr
            .get_tree_entries(self.id)
            .map(<[Option<LeafId>]>::to_vec)
            .unwrap_or_default();
        let to_entries: Vec<Option<LeafId>> = to_trace
            .hdr
            .get_tree_entries(self.id)
            .map(<[Option<LeafId>]>::to_vec)
            .unwrap_or_default();
        let from_shapes: Vec<Option<TileShape>> = from_trace
            .hdr
            .get_precalculated_tree_shapes(self.id)
            .map(<[Option<TileShape>]>::to_vec)
            .unwrap_or_default();
        let to_shapes: Vec<Option<TileShape>> = to_trace
            .hdr
            .get_precalculated_tree_shapes(self.id)
            .map(<[Option<TileShape>]>::to_vec)
            .unwrap_or_default();
        // ShapeSearchTree.java:189-190.
        self.tree.remove_leaf_opt(from_entries[remove_no]);
        self.tree.remove_leaf_opt(to_entries[0]);

        // ShapeSearchTree.java:191-198.
        let link_shapes = self.offset_shapes(
            joined_polyline,
            self.compensated_half_width(to_trace, rules),
            from_entry_no,
            to_entry_no,
        );
        let new_shape_count = from_entries.len() + link_shapes.len() + to_entries.len() - 2;
        let old_to_shape_count = to_entries.len();
        let mut new_leaves: Vec<Option<LeafId>> = vec![None; new_shape_count];
        let mut new_shapes: Vec<Option<TileShape>> = vec![None; new_shape_count];

        // ShapeSearchTree.java:205-216: transfer `fromTrace`'s entries, reversed when the two
        // traces meet head-to-head.
        for i in 0..from_shape_count_minus_1 {
            let from_no = if change_order {
                from_shape_count_minus_1 - i
            } else {
                i
            };
            new_shapes[i].clone_from(&from_shapes[from_no]);
            new_leaves[i] = from_entries[from_no];
            self.rekey(new_leaves[i], to_id, i);
        }
        // ShapeSearchTree.java:217-222.
        for i in 1..old_to_shape_count {
            let current_ind = from_shape_count_minus_1 + link_shapes.len() + i - 1;
            new_shapes[current_ind].clone_from(&to_shapes[i]);
            new_leaves[current_ind] = to_entries[i];
            self.rekey(new_leaves[current_ind], to_id, current_ind);
        }
        // ShapeSearchTree.java:226-230.
        for (i, shape) in link_shapes.iter().enumerate() {
            new_shapes[from_shape_count_minus_1 + i] = Some(shape.clone());
        }
        to_trace
            .hdr
            .set_precalculated_tree_shapes(self.id, new_shapes.clone());
        // ShapeSearchTree.java:233-236.
        for i in 0..link_shapes.len() {
            let current_ind = from_shape_count_minus_1 + i;
            new_leaves[current_ind] = self.insert_index(to_id, &new_shapes, current_ind);
        }
        to_trace.hdr.set_tree_entries(self.id, new_leaves);
    }

    /// Port of `ShapeSearchTree.mergeEntriesAtEnd` (ShapeSearchTree.java:245-312): the mirror
    /// image of [`Self::merge_entries_in_front`], appending `from_trace`'s entries after
    /// `to_trace`'s.
    pub fn merge_entries_at_end(
        &mut self,
        from_trace: &mut PolylineTrace,
        to_trace: &mut PolylineTrace,
        joined_polyline: &Polyline,
        from_entry_no: usize,
        to_entry_no: usize,
        rules: &BoardRules,
    ) {
        let to_id = to_trace.hdr.id();
        // ShapeSearchTree.java:251.
        let change_order = from_trace.last_corner() == to_trace.last_corner();
        let from_entries: Vec<Option<LeafId>> = from_trace
            .hdr
            .get_tree_entries(self.id)
            .map(<[Option<LeafId>]>::to_vec)
            .unwrap_or_default();
        let to_entries: Vec<Option<LeafId>> = to_trace
            .hdr
            .get_tree_entries(self.id)
            .map(<[Option<LeafId>]>::to_vec)
            .unwrap_or_default();
        let from_shapes: Vec<Option<TileShape>> = from_trace
            .hdr
            .get_precalculated_tree_shapes(self.id)
            .map(<[Option<TileShape>]>::to_vec)
            .unwrap_or_default();
        let to_shapes: Vec<Option<TileShape>> = to_trace
            .hdr
            .get_precalculated_tree_shapes(self.id)
            .map(<[Option<TileShape>]>::to_vec)
            .unwrap_or_default();

        // ShapeSearchTree.java:257-265.
        let to_shape_count_minus_1 = to_trace.tile_shape_count() - 1;
        self.tree
            .remove_leaf_opt(to_entries[to_shape_count_minus_1]);
        let remove_no = if change_order {
            from_trace.tile_shape_count() - 1
        } else {
            0
        };
        self.tree.remove_leaf_opt(from_entries[remove_no]);

        // ShapeSearchTree.java:266-276.
        let link_shapes = self.offset_shapes(
            joined_polyline,
            self.compensated_half_width(to_trace, rules),
            from_entry_no,
            to_entry_no,
        );
        let new_shape_count = from_entries.len() + link_shapes.len() + to_entries.len() - 2;
        let mut new_leaves: Vec<Option<LeafId>> = vec![None; new_shape_count];
        let mut new_shapes: Vec<Option<TileShape>> = vec![None; new_shape_count];

        // ShapeSearchTree.java:279-282: the kept head of `toTrace` keeps its indices, so Java
        // does *not* re-key those leaves.
        for i in 0..to_shape_count_minus_1 {
            new_shapes[i].clone_from(&to_shapes[i]);
            new_leaves[i] = to_entries[i];
        }
        // ShapeSearchTree.java:284-296.
        for i in 1..from_entries.len() {
            let current_ind = to_shape_count_minus_1 + link_shapes.len() + i - 1;
            let from_no = if change_order {
                from_entries.len() - i - 1
            } else {
                i
            };
            new_shapes[current_ind].clone_from(&from_shapes[from_no]);
            new_leaves[current_ind] = from_entries[from_no];
            self.rekey(new_leaves[current_ind], to_id, current_ind);
        }
        // ShapeSearchTree.java:300-304.
        for (i, shape) in link_shapes.iter().enumerate() {
            new_shapes[to_shape_count_minus_1 + i] = Some(shape.clone());
        }
        to_trace
            .hdr
            .set_precalculated_tree_shapes(self.id, new_shapes.clone());
        // ShapeSearchTree.java:307-310.
        for i in 0..link_shapes.len() {
            let current_ind = to_shape_count_minus_1 + i;
            new_leaves[current_ind] = self.insert_index(to_id, &new_shapes, current_ind);
        }
        to_trace.hdr.set_tree_entries(self.id, new_leaves);
    }

    /// Port of `ShapeSearchTree.reuseEntriesAfterCutout` (ShapeSearchTree.java:318-349): hand
    /// `from_trace`'s head entries to `start_piece` and its tail entries to `end_piece`, minting
    /// only the two leaves at the new cut.
    ///
    /// Java nulls the transferred slots in `fromTrace`'s own array as it goes
    /// (ShapeSearchTree.java:327,344) so that the later `SearchTreeManager.remove(fromTrace)`
    /// does not delete leaves that now belong to the two pieces — the reason
    /// [`ShapeTree::remove_opt`](crate::datastructures::ShapeTree::remove_opt) exists. The port
    /// writes those `None`s back onto `from_trace` for the same reason.
    ///
    /// The two pieces' precalculated shapes must already be in place: Java's
    /// `insert(startPiece, ...)` reads `startPiece.getTreeShape(this, index)`, which computes
    /// them from the piece's own polyline on the spot.
    pub fn reuse_entries_after_cutout(
        &mut self,
        from_trace: &mut PolylineTrace,
        start_piece: &mut PolylineTrace,
        end_piece: &mut PolylineTrace,
        ctx: &ItemCtx<'_>,
    ) {
        let start_id = start_piece.hdr.id();
        let end_id = end_piece.hdr.id();
        // ShapeSearchTree.java:320.
        let start_len = start_piece.polyline().lines().len() - 2;
        let end_len = end_piece.polyline().lines().len() - 2;
        let mut from_entries: Vec<Option<LeafId>> = from_trace
            .hdr
            .get_tree_entries(self.id)
            .map(<[Option<LeafId>]>::to_vec)
            .unwrap_or_default();
        let mut start_leaves: Vec<Option<LeafId>> = vec![None; start_len];
        let mut end_leaves: Vec<Option<LeafId>> = vec![None; end_len];

        // ShapeSearchTree.java:323-328.
        for i in 0..start_len - 1 {
            start_leaves[i] = from_entries[i];
            self.rekey(start_leaves[i], start_id, i);
            from_entries[i] = None;
        }
        // ShapeSearchTree.java:329-330: the last entry of the start piece is new.
        let start_shapes = self.tree_shapes_for(start_piece, ctx);
        start_leaves[start_len - 1] = self.insert_index(start_id, &start_shapes, start_len - 1);

        // ShapeSearchTree.java:337: the first entry of the end piece is new.
        let end_shapes = self.tree_shapes_for(end_piece, ctx);
        end_leaves[0] = self.insert_index(end_id, &end_shapes, 0);
        // ShapeSearchTree.java:339-345.
        for (i, leaf) in end_leaves.iter_mut().enumerate().skip(1) {
            let from_index = from_entries.len() - end_len + i;
            *leaf = from_entries[from_index];
            if let Some(leaf) = *leaf {
                self.tree.set_leaf_entry(leaf, TreeObject::Item(end_id), i);
            }
            from_entries[from_index] = None;
        }

        from_trace.hdr.set_tree_entries(self.id, from_entries);
        start_piece.hdr.set_tree_entries(self.id, start_leaves);
        end_piece.hdr.set_tree_entries(self.id, end_leaves);
    }

    /// Java's `startPiece.getTreeShape(this, index)` for a piece that has not been inserted yet
    /// (Item.java:212-238): compute the shapes and store them, then hand them back.
    fn tree_shapes_for(
        &self,
        trace: &mut PolylineTrace,
        ctx: &ItemCtx<'_>,
    ) -> Vec<Option<TileShape>> {
        if trace.hdr.get_precalculated_tree_shapes(self.id).is_none() {
            let shapes = self.calculate_trace_tree_shapes(trace, ctx);
            trace.hdr.set_precalculated_tree_shapes(self.id, shapes);
        }
        trace
            .hdr
            .get_precalculated_tree_shapes(self.id)
            .expect("just filled")
            .to_vec()
    }

    /// Port of `ShapeSearchTree.changeItemShape` (ShapeSearchTree.java:852-869): replace one
    /// stored shape of `item` and re-insert only that leaf.
    ///
    /// The remove-then-insert pair at ShapeSearchTree.java:856/867 is the reason
    /// [`LeafId`] carries a generation counter: a LIFO free list
    /// hands the new leaf the slot the old one just released.
    pub fn change_item_shape(&mut self, item: &mut Item, shape_index: usize, new_shape: TileShape) {
        let id = item.id();
        let old_entries: Vec<Option<LeafId>> = item
            .get_search_tree_entries(self.id)
            .map(<[Option<LeafId>]>::to_vec)
            .unwrap_or_default();
        let old_shapes: Vec<Option<TileShape>> = item
            .header()
            .get_precalculated_tree_shapes(self.id)
            .map(<[Option<TileShape>]>::to_vec)
            .unwrap_or_default();
        let mut new_leaves: Vec<Option<LeafId>> = vec![None; old_entries.len()];
        let mut new_shapes: Vec<Option<TileShape>> = vec![None; old_entries.len()];
        // ShapeSearchTree.java:856.
        self.tree.remove_leaf_opt(old_entries[shape_index]);
        // ShapeSearchTree.java:857-865.
        for i in 0..new_shapes.len() {
            if i == shape_index {
                new_shapes[i] = Some(new_shape.clone());
            } else {
                new_shapes[i].clone_from(&old_shapes[i]);
                new_leaves[i] = old_entries[i];
            }
        }
        item.set_precalculated_tree_shapes(self.id, new_shapes.clone());
        // ShapeSearchTree.java:867.
        new_leaves[shape_index] = self.insert_index(id, &new_shapes, shape_index);
        item.set_tree_entries(self.id, new_leaves);
    }

    /// Port of `ShapeSearchTree.reduceTraceShapeAtTiePin` (ShapeSearchTree.java:817-846): cut
    /// the pin's shape out of the trace's first or last tile so the autorouter can reach the pin
    /// with a different net.
    ///
    /// Java returns early when the trace touches the pin at neither end
    /// (ShapeSearchTree.java:828-830) and when the overlap is not two-dimensional (:833-835).
    /// A failed cutout — `TileShape.cutout` answering `None` — leaves the trace alone, which is
    /// the same outcome as an all-degenerate piece list.
    pub fn reduce_trace_shape_at_tie_pin(
        &mut self,
        tie_pin: &crate::items::Pin,
        trace: &mut PolylineTrace,
        ctx: &ItemCtx<'_>,
    ) {
        let Some(pin_shape) = tie_pin
            .get_tree_shape_on_layer(self.id, trace.get_layer(), ctx)
            .cloned()
        else {
            return;
        };
        let pin_center = tie_pin.get_center(ctx);
        // ShapeSearchTree.java:821-830.
        let (trace_shape_no, compare_corner) = if trace.first_corner() == Some(pin_center.clone()) {
            (0, trace.polyline().corner_approx(1))
        } else if trace.last_corner() == Some(pin_center) {
            let no = trace.corner_count() - 2;
            (no, trace.polyline().corner_approx(no))
        } else {
            return;
        };
        let Some(trace_shape) = trace
            .hdr
            .get_precalculated_tree_shapes(self.id)
            .and_then(|shapes| shapes.get(trace_shape_no).and_then(Option::as_ref).cloned())
        else {
            return;
        };
        // ShapeSearchTree.java:832-835.
        if trace_shape.intersection(&pin_shape).dimension() < 2 {
            return;
        }
        let Some(shape_pieces) = trace_shape.cutout(&pin_shape) else {
            return;
        };
        // ShapeSearchTree.java:837-844: the first two-dimensional piece wins, unless a later one
        // contains the corner the trace continues towards.
        let mut new_trace_shape = TileShape::Simplex(fr_geometry::Simplex::EMPTY);
        let mut found = false;
        for piece in shape_pieces {
            if piece.dimension() == 2 {
                let contains_corner =
                    compare_corner.is_some_and(|corner| piece.contains_float(&corner));
                if !found || contains_corner {
                    new_trace_shape = piece;
                    found = true;
                }
            }
        }
        let mut item = Item::Trace(trace.clone());
        self.change_item_shape(&mut item, trace_shape_no, new_trace_shape);
        if let Item::Trace(updated) = item {
            *trace = updated;
        }
    }
}

// The two methods `fr-board` cannot carry, because both take and return
// `autoroute.expansion.IncompleteFreeSpaceExpansionRoom`: they landed in plan-6 Task 3 as an
// **extension trait in `fr-router`** (`crates/fr-router/src/autoroute/tree_ext.rs`,
// `AutorouteSearchTreeExt`), which keeps this crate free of expansion rooms — the `RoutingBoardExt`
// precedent (plan-2 ruling 4). The three angle regimes are dispatched there on
// [`ShapeSearchTree::angle`], exactly as `SearchTreeManager.getAutorouteTree`
// (SearchTreeManager.java:147-161) dispatches on the subclass, and the private
// `restrainShape`/`calcOutsideRestrainedShape`/`calcInsideRestrainedShape`/
// `obstacleSegmentTouchesInside`/`signedLineDistance` helpers went with them.
// renamed: `ShapeSearchTree.completeShape` (ShapeSearchTree.java:580-693) -> `fr_router::autoroute::tree_ext::AutorouteSearchTreeExt::complete_shape`.
// renamed: `ShapeSearchTree45Degree.completeShape` (ShapeSearchTree45Degree.java:95-281) -> the 45-degree arm of the same trait method.
// renamed: `ShapeSearchTree90Degree.completeShape` (ShapeSearchTree90Degree.java:38-191) -> the 90-degree arm of the same trait method.
// renamed: `ShapeSearchTree.divideLargeRoom` (ShapeSearchTree.java:1095-1118) -> `AutorouteSearchTreeExt::divide_large_room`.
// renamed: `ShapeSearchTree45Degree.divideLargeRoom` (ShapeSearchTree45Degree.java:288-298) -> the 45-degree arm of the same trait method.
//
// not ported: the private diagnostic helpers of both subclasses — `describeBounds`,
// `isCompleteShapeDebugAnchor`, `traceCompleteShapeFilter`, `traceCompleteShapeCandidate`,
// `traceCompleteShapeDecision`, `obstacleId` and `obstacleNets`
// (ShapeSearchTree45Degree.java:543-647, ShapeSearchTree90Degree.java:324-432). Every one is an
// `FRLogger.trace` payload for a single hard-coded room the Java author was debugging
// (`isCompleteShapeDebugAnchor` tests net 77 / net 84 at literal coordinates), and `fr-board`
// must not depend on `tracing` (global-constraints.md). They belong to `completeShape`, so Plan 6
// inherits nothing but their absence.
