//! Port of the check half of `board.actions.DrillItemMover` (`board/actions/DrillItemMover.java`).

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::searchtree::ShapeSearchTree;
use fr_board::{ItemId, TimeLimit, TreeId};
use fr_geometry::{IntPoint, Point, TileShape, Vector};

use crate::board_ext::forced_pad_router::{CheckDrillResult, ForcedPadRouter};

/// Port of `board.actions.DrillItemMover` (DrillItemMover.java:26-326) — the check half.
///
/// Java's class is `final` with a private constructor and nothing but static methods; the port is
/// a unit struct with associated functions and Java's `RoutingBoard board` parameter kept in
/// place, because `Board` cannot be a field here (plan-2 ruling 11: no board back-pointers).
pub struct DrillItemMover;

impl DrillItemMover {
    /// Port of `DrillItemMover.check(DrillItem, Vector, int, int, Collection<Item>, RoutingBoard,
    /// TimeLimit)` (DrillItemMover.java:34-103): "checks, if `drillItem` can be translated by
    /// `vector` by shoving obstacle traces and vias aside, so that no clearance violations
    /// occur."
    ///
    /// `ignore_items` is `Option<&mut Vec<ItemId>>` because Java's `null` and Java's non-null
    /// argument behave differently: a non-null collection has the drill item **appended to it**
    /// at `:63` and the caller sees that, while `null` is replaced by a fresh list (`:57-62`).
    /// Every live call site passes a freshly allocated empty list, so the aliasing is never
    /// observable — but it is transcribed rather than smoothed over.
    ///
    /// `false` where a `drill_item` id names something that is not a drill item: Java's parameter
    /// is typed `DrillItem`, so the case cannot arise there.
    ///
    /// The per-layer loop reaches `ForcedPadRouter.checkForcedPad` (`:86-100`), which calls this
    /// method back (`ForcedPadRouter.java:269-278`): Java's dependency here is a **cycle**, so
    /// Task 9 landed this side with the call site as an `added in Task 10:` marker and Task 10
    /// replaced it with the real call. `drill_item_mover_check_answers_the_arm_task_nine_left_unimplemented`
    /// in `tests/forced_via.rs` is the test that pins the closed cycle against the JVM.
    pub fn check(
        board: &mut Board,
        drill_item: ItemId,
        vector: &Vector,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        ignore_items: Option<&mut Vec<ItemId>>,
        time_limit: Option<&TimeLimit>,
    ) -> bool {
        // DrillItemMover.java:43-45.
        if time_limit.is_some_and(TimeLimit::is_exceeded) {
            return false;
        }
        let Some(item) = board.get_item(drill_item) else {
            return false;
        };
        if !item.is_drill_item() {
            return false;
        }
        // :46-48.
        if item.is_shove_fixed(&board.rules) {
            return false;
        }

        // :50-56. "Check, that drillitem is only connected to traces."
        for contact in board.normal_contacts(drill_item) {
            let is_shovable_contact = board
                .get_item(contact)
                .is_some_and(|it| it.is_trace() || matches!(it, Item::ConductionArea(_)));
            if !is_shovable_contact {
                return false;
            }
        }

        // :57-63. Java's `null` argument becomes a fresh list; a supplied one is appended to.
        let mut owned_ignore_items: Vec<ItemId>;
        let effective_ignore_items: &mut Vec<ItemId> = match ignore_items {
            Some(list) => list,
            None => {
                owned_ignore_items = Vec::new();
                &mut owned_ignore_items
            }
        };
        effective_ignore_items.push(drill_item);

        // :64-68.
        let item = board.get_item(drill_item).expect("checked above");
        let attach_allowed = match item {
            Item::Via(via) => via.attach_allowed,
            _ => false,
        };
        let net_numbers = item.net_nos().to_vec();
        let clearance_class_index = item.clearance_class();
        let center = drill_item_center(board, drill_item).expect("a drill item has a centre");
        let (first_layer, last_layer) = {
            let ctx = board.ctx();
            (item.first_layer(&ctx), item.last_layer(&ctx))
        };
        // :69. The **default** tree, not the engine's compensated one.
        let tree = board.trees.get_default_tree().id();
        let orthogonal_mode = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;

        // :70-101.
        for current_layer in first_layer..=last_layer {
            let current_ind = current_layer - first_layer;
            // :74-77.
            let Some(current_shape) = board.item_tree_shape(drill_item, tree, current_ind) else {
                continue;
            };
            // :78-84.
            let new_shape = current_shape.translate_by(vector);
            // totalized: `DrillItemMover.check`'s `newShape.boundingOctagon()` (`:83`) -> a
            // skipped layer. Java's `boundingOctagon` never answers null for a non-empty shape,
            // and `newShape` is a live tree shape translated by a vector, so it cannot be empty.
            // Unreachable — no register row.
            let current_tile_shape = if orthogonal_mode {
                TileShape::Box(new_shape.bounding_box())
            } else {
                match new_shape.bounding_octagon() {
                    Some(octagon) => TileShape::Octagon(octagon),
                    None => continue,
                }
            };
            // :85.
            let from_side = ShapeEntrySide::from_point(&center, &current_tile_shape);
            // :86-100. `checkOnlyFront` is `true` here — this is the "moving drill items" case
            // its javadoc names, and the only caller that passes it.
            if ForcedPadRouter::check_forced_pad(
                board,
                &current_tile_shape,
                &from_side,
                current_layer,
                &net_numbers,
                clearance_class_index,
                attach_allowed,
                Some(effective_ignore_items),
                max_recursion_depth,
                max_via_recursion_depth,
                true,
                time_limit,
            ) == CheckDrillResult::NotDrillable
            {
                return false;
            }
        }
        // :102.
        true
    }

    /// Port of `DrillItemMover.tryShoveViaPoints` (DrillItemMover.java:256-325): "calculates
    /// possible new locations for a via to shove outside `obstacleShape`. If `extendedCheck` is
    /// true, more than 1 possible new location is calculated."
    ///
    /// Java's javadoc says the function "is used here and in TraceShover.check" — those two, plus
    /// `ForcedPadRouter.checkForcedPad`, are its only callers.
    pub fn try_shove_via_points(
        board: &mut Board,
        obstacle_shape: &TileShape,
        layer: usize,
        via: ItemId,
        clearance_class_index: usize,
        extended_check: bool,
    ) -> Vec<IntPoint> {
        // :263-267.
        let tree = board.trees.get_default_tree().id();
        let Some(mut current_via_shape) = tree_shape_on_layer(board, via, tree, layer) else {
            return Vec::new();
        };
        // :268.
        let is_int_octagon = obstacle_shape.is_int_octagon();
        // :269-270.
        let Some(via_clearance_class) = board.get_item(via).map(Item::clearance_class) else {
            return Vec::new();
        };
        let clearance_value =
            f64::from(board.clearance_value(clearance_class_index, via_clearance_class, layer));
        let orthogonal_mode = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        let compensation_used = board
            .trees
            .get_default_tree()
            .is_clearance_compensation_used();

        // :271-286.
        let mut shove_distance;
        if orthogonal_mode || is_int_octagon {
            shove_distance = 0.5 * current_via_shape.bounding_box().max_width();
            if !compensation_used {
                shove_distance += clearance_value;
            }
        } else {
            // "a different algorithm is used for calculating the new via centers"
            shove_distance = 0.0;
            if !compensation_used {
                // "enlarge obstacleShape and currentViaShape by half of the clearance value to
                // synchronize with the check algorithm in
                // ShapeSearchTree.overlapping_tree_entries_with_clearance"
                shove_distance += 0.5 * clearance_value;
            }
        }
        // :288-290. "The additional constant 2 is an empirical value for the tolerance in case of
        // diagonal shoving."
        shove_distance += 2.0;

        // :292-323.
        let Some(Point::Int(current_via_center)) = drill_item_center(board, via) else {
            // Java casts `via.getCenter()` to `IntPoint` (`:292`) and throws otherwise.
            return Vec::new();
        };
        if orthogonal_mode {
            let current_offset_box = obstacle_shape.bounding_box().offset(shove_distance);
            let try_count = if extended_check { 2 } else { 1 };
            current_offset_box.nearest_border_projections(&current_via_center, try_count)
        } else if is_int_octagon {
            let Some(bounding_octagon) = obstacle_shape.bounding_octagon() else {
                return Vec::new();
            };
            let current_offset_octagon = bounding_octagon.enlarge(shove_distance);
            let try_count = if extended_check { 4 } else { 1 };
            current_offset_octagon.nearest_border_projections(&current_via_center, try_count)
        } else {
            let current_offset_shape = obstacle_shape.enlarge(shove_distance);
            if !compensation_used {
                current_via_shape = current_via_shape.enlarge(0.5 * clearance_value);
            }
            let try_count = if extended_check { 4 } else { 1 };
            let shove_deltas = current_offset_shape
                .nearest_relative_outside_locations(&current_via_shape, try_count);
            shove_deltas
                .into_iter()
                .map(|delta| {
                    let current_delta = Point::Int(delta.round()).difference_by(&Point::ZERO);
                    // Java casts the translated point back to `IntPoint` (`:321`).
                    match Point::Int(current_via_center).translate_by(&current_delta) {
                        Point::Int(p) => p,
                        Point::Rational(p) => p.to_float().round(),
                    }
                })
                .collect()
        }
    }
}

/// `DrillItem.getCenter()` (DrillItem.java:98-104) for whichever of the two drill items `id`
/// names; `None` for anything else.
pub(crate) fn drill_item_center(board: &Board, id: ItemId) -> Option<Point> {
    let ctx = board.ctx();
    match board.get_item(id)? {
        Item::Via(via) => Some(via.get_center()),
        Item::Pin(pin) => Some(pin.get_center(&ctx)),
        _ => None,
    }
}

/// `DrillItem.getTreeShapeOnLayer(ShapeSearchTree, int)` (DrillItem.java:240-249) with Java's
/// cold-cache half intact: `Item.getTreeShape` (Item.java:212-226) drops the item's derived data
/// and recomputes when nothing is cached for `tree`, which `Board::item_tree_shape` reproduces
/// and `fr-board`'s `&self` twin `get_tree_shape_on_layer` deliberately does not (plan-6 ruling
/// 10 forbids `fr-router` from calling the `&self` form at all).
pub(crate) fn tree_shape_on_layer(
    board: &mut Board,
    id: ItemId,
    tree: TreeId,
    layer: usize,
) -> Option<TileShape> {
    let (from_layer, to_layer) = {
        let ctx = board.ctx();
        let item = board.get_item(id)?;
        (item.first_layer(&ctx), item.last_layer(&ctx))
    };
    // DrillItem.java:242-246: Java warns and returns null out of range.
    if layer < from_layer || layer > to_layer {
        return None;
    }
    board.item_tree_shape(id, tree, layer - from_layer)
}

/// The default search tree, by id — `board.searchTreeManager.getDefaultTree()` in a form the
/// borrow checker accepts alongside `&mut Board`.
pub(crate) fn tree_by_id(board: &Board, tree: TreeId) -> &ShapeSearchTree {
    board
        .trees
        .trees()
        .find(|candidate| candidate.id() == tree)
        .unwrap_or_else(|| panic!("board_ext: no search tree with id {tree:?}"))
}

// =================================================================================================
// The deferral roster for `board/actions/DrillItemMover.java`
// =================================================================================================
//
// added in Task 10b: `DrillItemMover.insert` (DrillItemMover.java:110-167) — translates the drill item for real and joins the changed area. Task 9 marked it `added in Plan 7:`; **controller ruling AA** moves it into a new Task 10b with the rest of the `ForcedViaInserter.insert` chain.
// added in Task 10b: `DrillItemMover.shoveVias` (DrillItemMover.java:173-249) — reached from `TraceShover.insert:435` and from `ForcedPadRouter.forcedPad:364`, and it calls `DrillItemMover.insert`. Task 9 marked it `added in Plan 7:`; **controller ruling AA** moves it into a new Task 10b with the rest of the `ForcedViaInserter.insert` chain.
