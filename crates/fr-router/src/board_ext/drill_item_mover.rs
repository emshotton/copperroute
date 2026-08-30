//! Port of `board.actions.DrillItemMover` (`board/actions/DrillItemMover.java`).

use fr_board::board::ShapeTraceEntries;
use fr_board::datastructures::StopCheck;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::searchtree::ShapeSearchTree;
use fr_board::{BoardError, ItemId, TimeLimit, TreeId};
use fr_geometry::{IntOctagon, IntPoint, Point, ShapeOps, TileShape, Vector};

use crate::board_ext::forced_pad_router::{CheckDrillResult, ForcedPadRouter};

/// Port of `board.actions.DrillItemMover` (DrillItemMover.java:26-326).
///
/// Task 9 landed `check` and `try_shove_via_points`; **controller ruling AA**'s Task 10b added
/// the mutating `insert` (`:110-167`) and `shove_vias` (`:173-249`), which is the pair
/// `ForcedPadRouter.forcedPad` and `TraceShover.insert` both need.
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
    /// Task 9 landed this side with the call site deferred and Task 10 replaced it with the real
    /// call. `drill_item_mover_check_answers_the_arm_task_nine_left_unimplemented`
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

    /// Port of `DrillItemMover.insert(DrillItem, Vector, int, int, IntOctagon, RoutingBoard)`
    /// (DrillItemMover.java:110-167): "translates `drillItem` by `vector` by shoving obstacle
    /// traces and vias aside, so that no clearance violations occur. If `tidyRegion != null`, it
    /// will be joined by the bounding octagons of the translated shapes."
    ///
    /// The mutating twin of [`Self::check`], and — unlike it — it has **no `TimeLimit`
    /// parameter**: Java's comment at `shoveVias:225` says why ("no time limit here because the
    /// item database is already changed"). The [`StopCheck`] it does take is plan-6 ruling 6's,
    /// and reaches only `fr-board`'s cancellable walks below `forcedPad`; it is not a second
    /// `isStopRequested` site.
    ///
    /// `tidy_region` is transcribed although Java never reads it back: `:144-146` reassigns the
    /// **parameter**, which is a local, and nothing below it looks at the value. Both live callers
    /// (`shoveVias:244` and `RoutingBoard`'s mover) pass `null`. Kept so a future caller that
    /// wants the region finds the accumulation already in Java's place.
    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        board: &mut Board,
        drill_item: ItemId,
        vector: &Vector,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        tidy_region: Option<IntOctagon>,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        let Some(item) = board.get_item(drill_item) else {
            return Ok(false);
        };
        if !item.is_drill_item() {
            return Ok(false);
        }
        // :117-119.
        if item.is_shove_fixed(&board.rules) {
            return Ok(false);
        }
        // :121-127.
        let attach_allowed = match item {
            Item::Via(via) => via.attach_allowed,
            _ => false,
        };
        // Java re-reads `drillItem.netNumbers` and `clearanceClassIndex()` per iteration
        // (`:152-153`); neither can change under `forcedPad`, which is handed the drill item in
        // `ignoreItems` and never touches it, so reading them once here is the same values.
        let net_numbers = item.net_nos().to_vec();
        let clearance_class_index = item.clearance_class();
        let ignore_items = vec![drill_item];
        let (first_layer, last_layer) = {
            let ctx = board.ctx();
            (item.first_layer(&ctx), item.last_layer(&ctx))
        };
        // :128. The **default** tree, not the engine's compensated one.
        let tree = board.trees.get_default_tree().id();
        let orthogonal_mode = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        let mut tidy_region = tidy_region;

        // :129-164.
        for current_layer in first_layer..=last_layer {
            let current_ind = current_layer - first_layer;
            // :132-136. Re-read per iteration, as Java does: `forcedPad` below mutates the board.
            let Some(current_shape) = board.item_tree_shape(drill_item, tree, current_ind) else {
                continue;
            };
            // :137-143.
            let new_shape = current_shape.translate_by(vector);
            // totalized: `DrillItemMover.insert`'s `newShape.boundingOctagon()` (`:142`) -> a
            // skipped layer, exactly as `check`'s `:83` above. Java's `boundingOctagon` never
            // answers null for a non-empty shape, and `newShape` is a live tree shape translated
            // by a vector. Unreachable — no register row.
            let current_tile_shape = if orthogonal_mode {
                TileShape::Box(new_shape.bounding_box())
            } else {
                match new_shape.bounding_octagon() {
                    Some(octagon) => TileShape::Octagon(octagon),
                    None => continue,
                }
            };
            // :144-146. Java reassigns its own parameter; nothing reads it afterwards.
            if let Some(region) = tidy_region {
                // totalized: `DrillItemMover.insert`'s `currentTileShape.boundingOctagon()`
                // (`:145`) -> the region left as it was. Non-null in Java for any non-empty tile
                // shape. Unreachable — no register row.
                if let Some(octagon) = current_tile_shape.bounding_octagon() {
                    tidy_region = Some(region.union(&octagon));
                }
            }
            // :147. The centre is re-read per iteration too, and does not move until `:165`.
            let Some(center) = drill_item_center(board, drill_item) else {
                return Ok(false);
            };
            let from_side = ShapeEntrySide::from_point(&center, &current_tile_shape);
            // :148-159.
            if !ForcedPadRouter::forced_pad(
                board,
                &current_tile_shape,
                &from_side,
                current_layer,
                &net_numbers,
                clearance_class_index,
                attach_allowed,
                Some(&ignore_items),
                max_recursion_depth,
                max_via_recursion_depth,
                stop,
            )? {
                return Ok(false);
            }
            // :160-163. Note this is the **untranslated** shape's bounding box, not
            // `currentTileShape`'s.
            let current_bounding_box = current_shape.bounding_box();
            for j in 0..4 {
                // `PolylineShape.cornerApprox` (PolylineShape.java:68-70) is `corner(no).toFloat()`.
                let corner = current_bounding_box.corner(j).to_float();
                board.join_changed_area(&corner, current_layer);
            }
        }
        // :165-166.
        board.move_item_by(drill_item, vector)?;
        Ok(true)
    }

    /// Port of `DrillItemMover.shoveVias(TileShape, ShapeEntrySide, int, int[], int,
    /// Collection<Item>, int, int, boolean, RoutingBoard)` (DrillItemMover.java:173-249): "shoves
    /// vias out of `obstacleShape`. Returns false, if the database is damaged, so that an undo is
    /// necessary afterwards."
    ///
    /// # `false` means "damaged", not "did not shove"
    ///
    /// Three of the four early exits answer **`true`** — `:192-194` when `storeItems` refuses,
    /// `:198-200` when nothing shovable overlaps, and `:206-208` when the via-recursion budget is
    /// spent — and so does the `continue` at `:241-243` for a via no candidate centre worked for.
    /// The **only** `false` is `:244-246`, where [`Self::insert`] itself failed after the board
    /// had already been changed. Callers read it that way: `forcedPad:363-374` and
    /// `TraceShover.insert:435-447` both abandon the whole shove on `false`.
    ///
    /// `ignore_items` is `Option<&[ItemId]>` where Java takes a `Collection<Item>`: this one is
    /// **read only** (`:196` removes from the entries' own via list, `:220-223` copies it into a
    /// fresh `LinkedList` per candidate), unlike [`Self::check`]'s, which quirk #175 records as
    /// having a visible side effect. The fresh copy per candidate is exactly why: it is what
    /// stops `check`'s `:63` append from reaching this method's caller.
    #[allow(clippy::too_many_arguments)]
    pub fn shove_vias(
        board: &mut Board,
        obstacle_shape: &TileShape,
        from_side: &ShapeEntrySide,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        ignore_items: Option<&[ItemId]>,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        copper_sharing_allowed: bool,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        // :184-190. The **default** tree, not the engine's compensated one.
        let mut shape_entries = ShapeTraceEntries::new(
            obstacle_shape.clone(),
            layer,
            net_numbers.to_vec(),
            clearance_class_index,
            Some(*from_side),
        );
        let obstacles = board.overlapping_items_with_clearance(
            obstacle_shape,
            Some(layer),
            &[],
            clearance_class_index,
        );
        // :192-194. `storeItems` refusing answers **true** here — this method only reports
        // database damage, and refusing to store is the caller's problem, not damage.
        if !shape_entries.store_items(board, &obstacles, false, copper_sharing_allowed) {
            return Ok(true);
        }
        // :195-200.
        if let Some(ignored) = ignore_items {
            shape_entries
                .shove_via_list
                .retain(|id| !ignored.contains(id));
        }
        if shape_entries.shove_via_list.is_empty() {
            return Ok(true);
        }
        // :201.
        let shape_radius = 0.5 * obstacle_shape.bounding_box().min_width();

        // :202-247.
        for current_via in shape_entries.shove_via_list.clone() {
            // :203-205.
            if board
                .get_item(current_via)
                .is_none_or(|item| item.shares_net_no(net_numbers))
            {
                continue;
            }
            // :206-208. Again **true**: the budget is spent, not the database.
            if max_via_recursion_depth <= 0 {
                return Ok(true);
            }
            // :209-216.
            let try_via_centers = Self::try_shove_via_points(
                board,
                obstacle_shape,
                layer,
                current_via,
                clearance_class_index,
                true,
            );
            // totalized: `DrillItemMover.shoveVias`'s `(IntPoint) currentVia.getCenter()`
            // (`:215`) -> `false`, i.e. "the database is damaged". Java casts and throws a
            // `ClassCastException` for a via whose centre is a `RationalPoint`; every via on a
            // board has an `IntPoint` centre, because `BasicBoard.insertVia` is only ever handed
            // one. No register row.
            let Some(current_via_center) = drill_item_center(board, current_via) else {
                return Ok(false);
            };
            let via_max_width = via_shape_max_width(board, current_via, layer);
            let max_dist = 0.5 * via_max_width + shape_radius;
            let max_dist_square = max_dist * max_dist;
            let check_via_center = current_via_center.to_float();

            // :217-240. `relCoor` is Java's, declared outside the loop and read after it; the
            // break on success means the value read is always the successful candidate's.
            let mut new_via_center: Option<IntPoint> = None;
            let mut rel_coor: Option<Vector> = None;
            for (i, try_via_center) in try_via_centers.iter().enumerate() {
                if i == 0
                    || check_via_center.distance_square(&try_via_center.to_float())
                        <= max_dist_square
                {
                    // :220-223. A **fresh** list per candidate, so quirk #175's append inside
                    // `check` never reaches this method's caller.
                    let mut local_ignore_items: Vec<ItemId> =
                        ignore_items.map(<[ItemId]>::to_vec).unwrap_or_default();
                    let delta = Point::Int(*try_via_center).difference_by(&current_via_center);
                    rel_coor = Some(delta.clone());
                    // :225-234. "No time limit here because the item database is already
                    // changed." — Java passes `null` for the `TimeLimit`, and so does the port.
                    let shove_ok = Self::check(
                        board,
                        current_via,
                        &delta,
                        max_recursion_depth,
                        max_via_recursion_depth - 1,
                        Some(&mut local_ignore_items),
                        None,
                    );
                    if shove_ok {
                        new_via_center = Some(*try_via_center);
                        break;
                    }
                }
            }
            // :241-243.
            if new_via_center.is_none() {
                continue;
            }
            // :244-246. The one arm that answers `false`.
            let rel_coor = rel_coor.expect("a candidate that answered `shoveOk` set `relCoor`");
            if !Self::insert(
                board,
                current_via,
                &rel_coor,
                max_recursion_depth,
                max_via_recursion_depth - 1,
                None,
                stop,
            )? {
                return Ok(false);
            }
        }
        // :248.
        Ok(true)
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

/// `0.5 * currentVia.getShapeOnLayer(layer).boundingBox().maxWidth()`'s inner half
/// (DrillItemMover.java:213 and TraceShover.java:325): the drill item's padstack shape on this
/// layer, or `0.0` where Java would throw a `NullPointerException`.
///
/// Java's list is a `List<Via>` and `ShapeTraceEntries.storeItems` (ShapeTraceEntries.java:192-196)
/// pushes only `Via`s into it, so the `_ => 0.0` arm asserts an invariant that lives one crate
/// away; see task-9-report.md §11's N2 for why it stays data-driven rather than a panic.
pub(crate) fn via_shape_max_width(board: &Board, id: ItemId, layer: usize) -> f64 {
    let ctx = board.ctx();
    match board.get_item(id) {
        Some(Item::Via(via)) => via
            .get_shape_on_layer(layer, &ctx)
            .map_or(0.0, |shape| shape.bounding_box().max_width()),
        Some(Item::Pin(pin)) => pin
            .get_shape_on_layer(layer, &ctx)
            .map_or(0.0, |shape| shape.bounding_box().max_width()),
        _ => 0.0,
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

// The deferral roster for `board/actions/DrillItemMover.java` is empty: every method of the class
// is ported. `check` and `tryShoveViaPoints` landed in Task 9, `checkForcedPad`'s call site closed
// in Task 10, and `insert` / `shoveVias` above are controller ruling AA's Task 10b.
