//! Port of `board/searchtree/ShapeTraceEntries.java`: the auxiliary structure the shove
//! algorithm uses to find where traces cross the border of a shape it wants to clear, and to
//! build the substitute traces that go around it.
//!
//! Nothing in Plan 2 calls this — its callers are `TraceShover`, `ForcedViaInserter` and
//! `DrillItemMover`, all Plan 7. It is ported here because Plan 2's task list places it with the
//! board, and because `cutoutTrace` is the one method of the class the *model* owns: it removes
//! and re-inserts board items.
//!
//! # The linked list
//!
//! Java threads its `EntryPoint`s on a hand-rolled singly linked list (`listAnchor` plus a `next`
//! field, ShapeTraceEntries.java:36,796) and splices it in four different places. The port keeps
//! the nodes in a `Vec` arena and makes `next` an `Option<usize>` index into it, which reproduces
//! the splices exactly — including `popPiece`'s detached sub-list, whose nodes stay alive in the
//! arena the way Java's stay alive through the returned references.

use fr_geometry::{FloatPoint, Polyline, TileShape};

use crate::ids::ItemId;
use crate::items::{Item, ItemHeader, PolylineTrace};
use crate::structure::{FixedState, ShapeEntrySide};

use super::{Board, item_ctx};

/// Java `ShapeTraceEntries.c_offset_add` (ShapeTraceEntries.java:28).
const C_OFFSET_ADD: f64 = 1.0;

/// Port of the private `ShapeTraceEntries.EntryPoint` (ShapeTraceEntries.java:789-805): where one
/// trace crosses the border of the offset shape.
#[derive(Debug, Clone, PartialEq)]
struct EntryPoint {
    /// Java `final PolylineTrace trace` (:791), as its board id.
    trace: ItemId,
    /// `entry.trace.netNumbers`, which Java reads straight off the trace reference (e.g.
    /// :525,532,540,580,611). Cached on the node so the list surgery needs no board lookup.
    net_nos: Vec<i32>,
    /// Java `final int traceLineNo` (:792).
    trace_line_no: usize,
    /// Java `final FloatPoint entryApprox` (:793).
    entry_approx: FloatPoint,
    /// Java `int edgeIndex` (:794). Signed and unbounded because
    /// `rotateEntryListAroundAnchor` adds the border-line count to it (:777).
    edge_index: i32,
    /// Java `int stackLevel` (:795), `-1` until calculated (:803).
    stack_level: i32,
    /// Java `EntryPoint next` (:796), as an arena index.
    next: Option<usize>,
}

/// Port of `ShapeTraceEntries` (`board/searchtree/ShapeTraceEntries.java`).
///
/// not ported: `ShapeTraceEntries.board` (ShapeTraceEntries.java:34) — the board back-pointer.
/// Every method that read it takes a `&Board` (or `&mut Board`) instead.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeTraceEntries {
    /// Java `public final Collection<Via> shoveViaList` (ShapeTraceEntries.java:29).
    pub shove_via_list: Vec<ItemId>,
    /// Java `private final TileShape shape` (:30).
    shape: TileShape,
    /// Java `private final int layer` (:31).
    layer: usize,
    /// Java `private final int[] ownNetNos` (:32).
    own_net_nos: Vec<i32>,
    /// Java `private final int clearanceClassIndex` (:33).
    clearance_class_index: usize,
    /// Java `private ShapeEntrySide fromSide` (:35); `null` is `None`.
    from_side: Option<ShapeEntrySide>,
    /// The `EntryPoint` arena; Java allocates each node on the heap.
    entries: Vec<EntryPoint>,
    /// Java `private EntryPoint listAnchor` (:36).
    list_anchor: Option<usize>,
    /// Java `private int tracePieceCount` (:37).
    trace_piece_count: i32,
    /// Java `private int maxStackLevel` (:38).
    max_stack_level: i32,
    /// Java `private boolean shapeContainsTraceTails` (:39).
    shape_contains_trace_tails: bool,
    /// Java `private Item foundObstacle` (:40).
    found_obstacle: Option<ItemId>,
}

impl ShapeTraceEntries {
    /// Port of the `ShapeTraceEntries(TileShape, int, int[], int, ShapeEntrySide, RoutingBoard)`
    /// constructor (ShapeTraceEntries.java:46-63).
    pub fn new(
        shape: TileShape,
        layer: usize,
        own_net_nos: Vec<i32>,
        clearance_class_index: usize,
        from_side: Option<ShapeEntrySide>,
    ) -> ShapeTraceEntries {
        ShapeTraceEntries {
            shove_via_list: Vec::new(),
            shape,
            layer,
            own_net_nos,
            clearance_class_index,
            from_side,
            entries: Vec::new(),
            list_anchor: None,
            trace_piece_count: 0,
            max_stack_level: 0,
            shape_contains_trace_tails: false,
            found_obstacle: None,
        }
    }

    /// Port of `ShapeTraceEntries.stackDepth` (ShapeTraceEntries.java:285-287).
    pub fn stack_depth(&self) -> i32 {
        self.max_stack_level
    }

    /// Port of `ShapeTraceEntries.substituteTraceCount` (ShapeTraceEntries.java:290-292).
    pub fn substitute_trace_count(&self) -> i32 {
        self.trace_piece_count
    }

    /// Port of `ShapeTraceEntries.traceTailsInShape` (ShapeTraceEntries.java:298-300).
    pub fn trace_tails_in_shape(&self) -> bool {
        self.shape_contains_trace_tails
    }

    /// Port of `ShapeTraceEntries.getFoundObstacle` (ShapeTraceEntries.java:315-317).
    pub fn get_found_obstacle(&self) -> Option<ItemId> {
        self.found_obstacle
    }

    /// Port of `ShapeTraceEntries.storeItems` (ShapeTraceEntries.java:177-220): sort the traces
    /// and vias of `item_ids` into this structure, and report whether they can all be shoved.
    //
    // Java bug: the first `continue` reads
    // `if (!isPadCheck && currentItem instanceof ViaObstacleArea || currentItem instanceof
    // ComponentObstacleArea)` (ShapeTraceEntries.java:180-183). `&&` binds tighter than `||`, so
    // a `ComponentObstacleArea` is skipped **unconditionally**, while a `ViaObstacleArea` is only
    // skipped when this is not a pad check — almost certainly not what the author meant
    // (`!isPadCheck && (a || b)`). Reproduced; see docs/java-quirks.md.
    pub fn store_items(
        &mut self,
        board: &Board,
        item_ids: &[ItemId],
        is_pad_check: bool,
        copper_sharing_allowed: bool,
    ) -> bool {
        let ctx = board.ctx();
        for id in item_ids {
            let Some(item) = board.get_item(*id) else {
                continue;
            };
            // ShapeTraceEntries.java:180-183, precedence reproduced.
            if (!is_pad_check && matches!(item, Item::ViaObstacleArea(_)))
                || matches!(item, Item::ComponentObstacleArea(_))
            {
                continue;
            }
            let contains_own_net = item.shares_net_no(&self.own_net_nos);
            // ShapeTraceEntries.java:185-187.
            if let Item::ConductionArea(area) = item
                && (contains_own_net || !area.get_is_obstacle())
            {
                continue;
            }
            // ShapeTraceEntries.java:188-191.
            if item.is_shove_fixed(&board.rules) && !contains_own_net {
                self.found_obstacle = Some(*id);
                return false;
            }
            match item {
                // ShapeTraceEntries.java:192-196.
                Item::Via(_) => {
                    if is_pad_check || !contains_own_net {
                        self.shove_via_list.push(*id);
                    }
                }
                Item::Trace(_) => {
                    if !self.store_trace(board, *id) {
                        return false;
                    }
                }
                // ShapeTraceEntries.java:201-215.
                _ => {
                    if contains_own_net {
                        if !copper_sharing_allowed {
                            self.found_obstacle = Some(*id);
                            return false;
                        }
                        let drillable_pin = match item {
                            Item::Pin(pin) => pin.drill_allowed(&ctx),
                            _ => false,
                        };
                        if is_pad_check && !drillable_pin {
                            self.found_obstacle = Some(*id);
                            return false;
                        }
                    } else {
                        self.found_obstacle = Some(*id);
                        return false;
                    }
                }
            }
        }
        // ShapeTraceEntries.java:217-219.
        self.search_from_side();
        self.resort();
        self.calculate_stack_levels()
    }

    /// Port of the private `ShapeTraceEntries.storeTrace` (ShapeTraceEntries.java:323-442).
    fn store_trace(&mut self, board: &Board, trace_id: ItemId) -> bool {
        let search_tree = board.trees.get_default_tree();
        let Some(item @ Item::Trace(trace)) = board.get_item(trace_id) else {
            return true;
        };
        let offset_shape = if search_tree.is_clearance_compensation_used() {
            // ShapeTraceEntries.java:326-329.
            let current_offset =
                f64::from(search_tree.compensated_half_width(trace, &board.rules)) + C_OFFSET_ADD;
            self.shape.offset(current_offset)
        } else {
            // ShapeTraceEntries.java:330-337: two steps, "for symmetry reasons".
            let cl_offset = f64::from(board.clearance_value(
                trace.hdr.clearance_class(),
                self.clearance_class_index,
                trace.get_layer(),
            )) + C_OFFSET_ADD;
            self.shape
                .offset(f64::from(trace.get_half_width()))
                .offset(cl_offset)
        };

        // ShapeTraceEntries.java:341-350.
        for entry_tuple in offset_shape.entrance_points(trace.polyline()) {
            let Some(border_line) = offset_shape.border_line(entry_tuple[1]) else {
                continue;
            };
            let entry_approx =
                trace.polyline().lines()[entry_tuple[0]].intersection_approx(&border_line);
            self.insert_entry_point(
                trace_id,
                trace.hdr.net_nos.clone(),
                entry_tuple[0],
                entry_tuple[1] as i32,
                entry_approx,
            );
        }

        // ShapeTraceEntries.java:355-439: an end point of the trace inside the shape.
        if !item.shares_net_no(&self.own_net_nos) {
            if !item.nets_normal() {
                return false;
            }
            let corners = [trace.first_corner(), trace.last_corner()];
            for (i, end_corner) in corners.into_iter().enumerate() {
                let Some(end_corner) = end_corner else {
                    continue;
                };
                if !offset_shape.contains(&end_corner) {
                    continue;
                }
                let contact_list = if i == 0 {
                    board.trace_start_contacts(trace_id)
                } else {
                    board.trace_end_contacts(trace_id)
                };
                let mut contact_count = 0;
                let mut store_end_corner = true;
                // ShapeTraceEntries.java:372-415.
                for contact_id in &contact_list {
                    let Some(contact_item) = board.get_item(*contact_id) else {
                        continue;
                    };
                    if !contact_item.is_routable() {
                        self.found_obstacle = Some(*contact_id);
                        return false;
                    }
                    match contact_item {
                        Item::Trace(contact_trace) => {
                            // ShapeTraceEntries.java:379-386.
                            //
                            // Java bug: the third disjunct is
                            // `contactItem.clearanceClassIndex() != contactTrace
                            // .clearanceClassIndex()` (ShapeTraceEntries.java:381), and
                            // `contactItem` *is* `contactTrace` — the pattern variable bound one
                            // line above. It compares an item with itself, so it is always false;
                            // the intent was plainly `trace.clearanceClassIndex()`, i.e. "the
                            // contact has a different clearance class from the trace being
                            // stored". Reproduced; see docs/java-quirks.md.
                            if (contact_item.is_shove_fixed(&board.rules)
                                || contact_trace.get_half_width() != trace.get_half_width()
                                || contact_item.clearance_class()
                                    != contact_trace.hdr.clearance_class())
                                && offset_shape.contains_inside(&end_corner)
                            {
                                self.found_obstacle = Some(*contact_id);
                                return false;
                            }
                        }
                        Item::Via(_) => {
                            // ShapeTraceEntries.java:387-412.
                            // ShapeTraceEntries.java:388: `via.getTileShapeOnLayer(layer)`,
                            // which recomputes on a cold cache (Item.java:212-226). Java has no
                            // `continue` here — a `null` shape NPEs at `.smallestRadius()` — so
                            // the `expect` reproduces that rather than silently skipping the
                            // `++contactCount` below.
                            let via_shape = board
                                .drill_item_tile_shape_on_layer_ref(*contact_id, self.layer)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "ShapeTraceEntries.storeTrace: via {contact_id} has no \
                                         shape on layer {} — Java NPEs here too \
                                         (ShapeTraceEntries.java:388-391)",
                                        self.layer
                                    )
                                });
                            let mut via_trace_diff = via_shape.smallest_radius()
                                - f64::from(
                                    search_tree.compensated_half_width(trace, &board.rules),
                                );
                            if !search_tree.is_clearance_compensation_used() {
                                let via_clearance = board.clearance_value(
                                    contact_item.clearance_class(),
                                    self.clearance_class_index,
                                    self.layer,
                                );
                                let trace_clearance = board.clearance_value(
                                    trace.hdr.clearance_class(),
                                    self.clearance_class_index,
                                    self.layer,
                                );
                                if trace_clearance > via_clearance {
                                    via_trace_diff += f64::from(via_clearance - trace_clearance);
                                }
                            }
                            if via_trace_diff < 0.0 {
                                self.found_obstacle = Some(*contact_id);
                                return false;
                            }
                            if via_trace_diff == 0.0 && !offset_shape.contains_inside(&end_corner) {
                                store_end_corner = false;
                            }
                        }
                        _ => {}
                    }
                    contact_count += 1;
                }
                // ShapeTraceEntries.java:416-435.
                if contact_count == 1 && store_end_corner {
                    if let Some(projection) = offset_shape.nearest_border_point(&end_corner)
                        && let Some(projection_side) =
                            offset_shape.contains_on_border_line_no(&projection)
                    {
                        let trace_line_segment_no = if i == 0 {
                            0
                        } else {
                            trace.polyline().lines().len() - 1
                        };
                        self.insert_entry_point(
                            trace_id,
                            trace.hdr.net_nos.clone(),
                            trace_line_segment_no,
                            projection_side as i32,
                            projection.to_float(),
                        );
                    }
                } else if contact_count == 0 && offset_shape.contains_inside(&end_corner) {
                    self.shape_contains_trace_tails = true;
                }
            }
        }
        // ShapeTraceEntries.java:440-441: the trace becomes the "found obstacle" even on success.
        self.found_obstacle = Some(trace_id);
        true
    }

    /// Port of the private `ShapeTraceEntries.searchFromSide`
    /// (ShapeTraceEntries.java:444-460).
    fn search_from_side(&mut self) {
        // ShapeTraceEntries.java:445-447.
        if let Some(from_side) = self.from_side
            && from_side.no >= 0
        {
            return;
        }
        let mut current = self.list_anchor;
        let mut current_fromside_no = 0;
        let mut current_entry_approx = None;
        while let Some(index) = current {
            let entry = &self.entries[index];
            // `Item.sharesNetNo(int[])` (Item.java:179-189) is the raw array intersection.
            if entry.net_nos.iter().any(|n| self.own_net_nos.contains(n)) {
                current_fromside_no = entry.edge_index;
                current_entry_approx = Some(entry.entry_approx);
                break;
            }
            current = entry.next;
        }
        self.from_side = Some(ShapeEntrySide::new(
            current_fromside_no,
            current_entry_approx,
        ));
    }

    /// Port of the private `ShapeTraceEntries.resort` (ShapeTraceEntries.java:463-573): rotate
    /// the entry list so it starts in the middle of `fromSide`, then drop the redundant middle
    /// entries of each connected set.
    fn resort(&mut self) {
        let edge_count = self.shape.border_line_count() as i32;
        let Some(mut from_side) = self.from_side else {
            return;
        };
        // ShapeTraceEntries.java:465-468.
        if from_side.no < 0 || from_side.no >= edge_count {
            return;
        }
        let Some(compare_corner1) = self.shape.corner_approx(from_side.no as usize) else {
            return;
        };
        let compare_corner2 = if from_side.no == edge_count - 1 {
            self.shape.corner_approx(0)
        } else {
            self.shape.corner_approx(from_side.no as usize + 1)
        };
        let Some(compare_corner2) = compare_corner2 else {
            return;
        };
        let Some(from_side_border_line) = self.shape.border_line(from_side.no as usize) else {
            return;
        };
        // ShapeTraceEntries.java:478-487.
        let mut from_point_dist = 0.0;
        let mut from_point_projection = None;
        if let Some(border_intersection) = from_side.border_intersection {
            let projection = border_intersection.projection_approx(&from_side_border_line);
            from_point_projection = Some(projection);
            from_point_dist = projection.distance_square(&compare_corner1);
            if from_point_dist >= compare_corner1.distance_square(&compare_corner2) {
                from_side = ShapeEntrySide::new(from_side.no, None);
                from_point_projection = None;
            }
        }
        self.from_side = Some(from_side);

        // ShapeTraceEntries.java:490-515.
        let mut current = self.list_anchor;
        while let Some(index) = current {
            let entry = &self.entries[index];
            if entry.edge_index > from_side.no {
                break;
            }
            if entry.edge_index == from_side.no {
                match (from_side.border_intersection, from_point_projection) {
                    (Some(_), Some(from_point_projection)) => {
                        let current_projection =
                            entry.entry_approx.projection_approx(&from_side_border_line);
                        if current_projection.distance_square(&compare_corner1) >= from_point_dist
                            && current_projection.distance_square(&from_point_projection)
                                <= current_projection.distance_square(&compare_corner1)
                        {
                            break;
                        }
                    }
                    _ => {
                        if entry.entry_approx.distance_square(&compare_corner2)
                            <= entry.entry_approx.distance_square(&compare_corner1)
                        {
                            break;
                        }
                    }
                }
            }
            current = entry.next;
        }
        // ShapeTraceEntries.java:516-518.
        if let Some(index) = current
            && Some(index) != self.list_anchor
        {
            self.rotate_entry_list_around_anchor(index, edge_count);
        }

        // ShapeTraceEntries.java:521-552: keep only the first and the last entry of each run of
        // entries belonging to the same connected set.
        let Some(anchor) = self.list_anchor else {
            return;
        };
        let mut prev = anchor;
        let mut prev_net_nos = self.trace_net_nos(prev);
        let mut current = self.entries[anchor].next;
        let (mut current_net_nos, mut next) = match current {
            Some(index) => (self.trace_net_nos(index), self.entries[index].next),
            None => (Vec::new(), None),
        };
        let mut before_prev: Option<usize> = None;
        while let Some(next_index) = next {
            let next_net_nos = self.trace_net_nos(next_index);
            if nets_equal(&prev_net_nos, &current_net_nos)
                && nets_equal(&current_net_nos, &next_net_nos)
            {
                self.entries[prev].next = Some(next_index);
            } else {
                before_prev = Some(prev);
                prev = current.expect("current is Some whenever next is");
                prev_net_nos = current_net_nos;
            }
            current_net_nos = next_net_nos;
            current = Some(next_index);
            next = self.entries[next_index].next;
        }

        // ShapeTraceEntries.java:554-564.
        if current.is_some() && nets_equal(&current_net_nos, &self.own_net_nos) {
            self.entries[prev].next = None;
            if nets_equal(&prev_net_nos, &self.own_net_nos) {
                match before_prev {
                    Some(before_prev) => self.entries[before_prev].next = None,
                    None => self.list_anchor = None,
                }
            }
        }

        // ShapeTraceEntries.java:566-572.
        for _ in 0..2 {
            let Some(anchor) = self.list_anchor else {
                break;
            };
            if !nets_equal_exact(&self.trace_net_nos(anchor), &self.own_net_nos) {
                break;
            }
            self.list_anchor = self.entries[anchor].next;
        }
    }

    /// Port of the private `ShapeTraceEntries.calculateStackLevels`
    /// (ShapeTraceEntries.java:575-665): assign a shove nesting level to every entry, and report
    /// whether the entries really nest.
    fn calculate_stack_levels(&mut self) -> bool {
        let Some(anchor) = self.list_anchor else {
            return true;
        };
        let mut current_entry = Some(anchor);
        let mut current_net_numbers = self.trace_net_nos(anchor);
        // ShapeTraceEntries.java:581-587.
        let mut current_level = i32::from(!nets_equal(&current_net_numbers, &self.own_net_nos));

        while let Some(entry_index) = current_entry {
            // ShapeTraceEntries.java:590-599.
            if self.entries[entry_index].stack_level < 0 {
                self.trace_piece_count += 1;
                self.entries[entry_index].stack_level = current_level;
                if current_level > self.max_stack_level {
                    if self.max_stack_level > 1 {
                        self.found_obstacle = Some(self.entries[entry_index].trace);
                    }
                    self.max_stack_level = current_level;
                }
            }

            // ShapeTraceEntries.java:602-622.
            let mut check_entry = self.entries[entry_index].next;
            let mut index_of_next_foreign_set = 0;
            let mut index_of_last_occurrence_of_set = 0;
            let mut next_index = 0;
            let mut last_own_entry = None;
            let mut first_foreign_entry = None;
            while let Some(check_index) = check_entry {
                next_index += 1;
                let check_net_nos = self.trace_net_nos(check_index);
                if nets_equal(&check_net_nos, &current_net_numbers) {
                    index_of_last_occurrence_of_set = next_index;
                    last_own_entry = Some(check_index);
                    self.entries[check_index].stack_level = self.entries[entry_index].stack_level;
                } else if index_of_next_foreign_set == 0 {
                    index_of_next_foreign_set = next_index;
                    first_foreign_entry = Some(check_index);
                }
                check_entry = self.entries[check_index].next;
            }

            // ShapeTraceEntries.java:625-658.
            if next_index != 0 {
                let next_entry;
                if index_of_next_foreign_set != 0
                    && index_of_next_foreign_set < index_of_last_occurrence_of_set
                {
                    let Some(first_foreign) = first_foreign_entry else {
                        return false;
                    };
                    next_entry = first_foreign;
                    if self.entries[next_entry].stack_level >= 0 {
                        // ShapeTraceEntries.java:629-632: the stack property fails.
                        return false;
                    }
                    current_level += 1;
                } else if index_of_last_occurrence_of_set != 0 {
                    next_entry = last_own_entry.expect("set when the index is non-zero");
                } else {
                    let Some(first_foreign) = first_foreign_entry else {
                        return false;
                    };
                    next_entry = first_foreign;
                    if self.entries[next_entry].stack_level >= 0 {
                        current_level -= 1;
                        if self.entries[next_entry].stack_level != current_level {
                            return false;
                        }
                    }
                }
                current_net_numbers = self.trace_net_nos(next_entry);
                // ShapeTraceEntries.java:648-655: drop the entries in between.
                self.entries[entry_index].next = Some(next_entry);
                current_entry = Some(next_entry);
            } else {
                current_entry = None;
            }
        }
        // ShapeTraceEntries.java:660-663.
        current_level == 1
    }

    /// Port of the private `ShapeTraceEntries.popPiece` (ShapeTraceEntries.java:672-726): the
    /// first and last entry of the next piece at the maximal stack level.
    fn pop_piece(&mut self) -> Option<(usize, usize)> {
        let anchor = self.list_anchor?;
        let mut first = Some(anchor);
        let mut prev_first = None;
        // ShapeTraceEntries.java:682-689.
        while let Some(index) = first {
            if self.entries[index].stack_level == self.max_stack_level {
                break;
            }
            prev_first = Some(index);
            first = self.entries[index].next;
        }
        let first = first?;
        let first_trace = self.entries[first].trace;
        let first_net_nos = self.trace_net_nos(first);
        // ShapeTraceEntries.java:692-701.
        let mut last = first;
        let mut after_last = self.entries[first].next;
        while let Some(index) = after_last {
            if self.entries[index].stack_level != self.max_stack_level
                || !nets_equal_exact(&self.trace_net_nos(index), &first_net_nos)
            {
                break;
            }
            last = index;
            after_last = self.entries[index].next;
        }
        // ShapeTraceEntries.java:705-709.
        match prev_first {
            Some(prev) => self.entries[prev].next = after_last,
            None => self.list_anchor = after_last,
        }
        // ShapeTraceEntries.java:712-719.
        self.max_stack_level = 0;
        let mut current = self.list_anchor;
        while let Some(index) = current {
            if self.entries[index].stack_level > self.max_stack_level {
                self.max_stack_level = self.entries[index].stack_level;
            }
            current = self.entries[index].next;
        }
        self.trace_piece_count -= 1;
        // ShapeTraceEntries.java:721-724: an own-net piece is ignored and popped again.
        let _ = first_trace;
        if nets_equal_exact(&first_net_nos, &self.own_net_nos) {
            return self.pop_piece();
        }
        Some((first, last))
    }

    /// Port of `ShapeTraceEntries.nextSubstituteTracePiece`
    /// (ShapeTraceEntries.java:226-282): the next trace that goes around the shape instead of
    /// through it, or `None` at the end of the list.
    ///
    /// The returned trace is **not** inserted into the board; Java's caller does that. It carries
    /// a fresh id from the board's generator, which is why this takes `&mut Board`.
    pub fn next_substitute_trace_piece(&mut self, board: &mut Board) -> Option<PolylineTrace> {
        let (first, last) = self.pop_piece()?;
        let current_trace_id = self.entries[first].trace;
        let Some(Item::Trace(current_trace)) = board.get_item(current_trace_id) else {
            return None;
        };
        let search_tree = board.trees.get_default_tree();
        // ShapeTraceEntries.java:235-245.
        let offset_shape = if search_tree.is_clearance_compensation_used() {
            let current_offset =
                f64::from(search_tree.compensated_half_width(current_trace, &board.rules))
                    + C_OFFSET_ADD;
            self.shape.offset(current_offset)
        } else {
            let cl_offset = f64::from(board.clearance_value(
                current_trace.hdr.clearance_class(),
                self.clearance_class_index,
                self.layer,
            )) + C_OFFSET_ADD;
            self.shape
                .offset(f64::from(current_trace.get_half_width()))
                .offset(cl_offset)
        };
        let edge_count = self.shape.border_line_count() as i32;
        let edge_diff = self.entries[last].edge_index - self.entries[first].edge_index;
        if edge_diff < 0 {
            return None;
        }

        // ShapeTraceEntries.java:251-266.
        let piece_line_count = (edge_diff + 3) as usize;
        let mut piece_lines = Vec::with_capacity(piece_line_count);
        let start_line = current_trace.polyline().lines()[self.entries[first].trace_line_no];
        let Some(Item::Trace(last_trace)) = board.get_item(self.entries[last].trace) else {
            return None;
        };
        let end_line = last_trace.polyline().lines()[self.entries[last].trace_line_no];
        piece_lines.push(start_line);
        let mut current_edge_no = self.entries[first].edge_index.rem_euclid(edge_count);
        for _ in 1..piece_line_count - 1 {
            piece_lines.push(offset_shape.border_line(current_edge_no as usize)?);
            if current_edge_no == edge_count - 1 {
                current_edge_no = 0;
            } else {
                current_edge_no += 1;
            }
        }
        piece_lines.push(end_line);
        // ShapeTraceEntries.java:267-271.
        let Ok(piece_polyline) = Polyline::from_lines(piece_lines) else {
            return self.next_substitute_trace_piece(board);
        };
        if piece_polyline.is_empty() {
            return self.next_substitute_trace_piece(board);
        }
        // ShapeTraceEntries.java:272-281.
        let Some(Item::Trace(current_trace)) = board.get_item(current_trace_id) else {
            return None;
        };
        let half_width = current_trace.get_half_width();
        let net_nos = current_trace.hdr.net_nos.clone();
        let clearance_class = current_trace.hdr.clearance_class();
        let layer = self.layer;
        let layer_count = board.get_layer_count();
        let id = board.new_item_id();
        Some(PolylineTrace::new(
            ItemHeader::new(id, net_nos, clearance_class, 0, FixedState::Unfixed),
            piece_polyline,
            layer,
            half_width,
            Some(layer_count),
        ))
    }

    /// Port of `ShapeTraceEntries.cutoutTraces` (ShapeTraceEntries.java:306-312).
    pub fn cutout_traces(&self, board: &mut Board, item_ids: &[ItemId]) {
        for id in item_ids {
            let is_foreign_trace = board
                .get_item(*id)
                .is_some_and(|item| item.is_trace() && !item.shares_net_no(&self.own_net_nos));
            if is_foreign_trace {
                Self::cutout_trace(board, *id, &self.shape, self.clearance_class_index);
            }
        }
    }

    /// Port of the static `ShapeTraceEntries.cutoutTrace`
    /// (ShapeTraceEntries.java:66-107): cut `shape` (enlarged by the trace's half width and
    /// clearance) out of the trace, replacing it with the pieces that survive.
    ///
    /// Java's `trace.isOnTheBoard()` warning path (:67-70) returns without doing anything.
    pub fn cutout_trace(
        board: &mut Board,
        trace_id: ItemId,
        shape: &TileShape,
        clearance_class_index: usize,
    ) {
        let Some(item @ Item::Trace(trace)) = board.get_item(trace_id) else {
            return;
        };
        // ShapeTraceEntries.java:67-70.
        if !item.is_on_the_board() {
            return;
        }
        let search_tree = board.trees.get_default_tree();
        let offset_shape = if search_tree.is_clearance_compensation_used() {
            // ShapeTraceEntries.java:74-77.
            let current_offset =
                f64::from(search_tree.compensated_half_width(trace, &board.rules)) + C_OFFSET_ADD;
            shape.offset(current_offset)
        } else {
            // ShapeTraceEntries.java:78-84.
            let cl_offset = f64::from(board.clearance_value(
                trace.hdr.clearance_class(),
                clearance_class_index,
                trace.get_layer(),
            )) + C_OFFSET_ADD;
            shape
                .offset(f64::from(trace.get_half_width()))
                .offset(cl_offset)
        };
        let trace_lines = trace.polyline().clone();
        let Ok(pieces) = offset_shape.cutout_polyline(&trace_lines) else {
            return;
        };
        // ShapeTraceEntries.java:87-90: nothing was cut off.
        if pieces.len() == 1 && pieces[0] == trace_lines {
            return;
        }
        // ShapeTraceEntries.java:91-94.
        let fast_path = pieces.len() == 2
            && pieces[0]
                .first_corner()
                .is_some_and(|c| offset_shape.is_outside(&c))
            && pieces[1]
                .last_corner()
                .is_some_and(|c| offset_shape.is_outside(&c));
        if fast_path {
            let start_piece = pieces[0].clone();
            let end_piece = pieces[1].clone();
            Self::fast_cutout_trace(board, trace_id, start_piece, end_piece);
        } else {
            // ShapeTraceEntries.java:96-105.
            let layer = trace.get_layer();
            let half_width = trace.get_half_width();
            let net_nos = trace.hdr.net_nos.clone();
            let clearance_class = trace.hdr.clearance_class();
            board.remove_item(trace_id);
            for piece in pieces {
                board.insert_trace_without_cleaning(
                    piece,
                    layer,
                    half_width,
                    net_nos.clone(),
                    clearance_class,
                    FixedState::Unfixed,
                );
            }
        }
    }

    /// Port of the private `ShapeTraceEntries.fastCutoutTrace`
    /// (ShapeTraceEntries.java:110-151): the performance path, which hands the old trace's tree
    /// leaves to the two new pieces instead of recomputing them.
    ///
    /// Java's `board.itemList.saveForUndo(trace)` (:113) and its two observer notifications
    /// (:147-150) are dropped. Note the two new traces go into `itemList` **directly**
    /// (:126,141), not through `insertItem`, so no revision bump and no tree insert — the tree
    /// entries arrive from `reuseEntriesAfterCutout` instead.
    fn fast_cutout_trace(
        board: &mut Board,
        trace_id: ItemId,
        start_piece: Polyline,
        end_piece: Polyline,
    ) {
        let Some(Item::Trace(trace)) = board.get_item(trace_id) else {
            return;
        };
        let layer = trace.get_layer();
        let half_width = trace.get_half_width();
        let net_nos = trace.hdr.net_nos.clone();
        let clearance_class = trace.hdr.clearance_class();
        let layer_count = board.get_layer_count();

        // added in Plan 6: `board.additionalUpdateAfterChange(trace)`
        // (ShapeTraceEntries.java:112).
        let start_id = board.new_item_id();
        let mut start_trace = PolylineTrace::new(
            ItemHeader::new(
                start_id,
                net_nos.clone(),
                clearance_class,
                0,
                FixedState::Unfixed,
            ),
            start_piece,
            layer,
            half_width,
            Some(layer_count),
        );
        start_trace.hdr.set_on_the_board(true);
        let end_id = board.new_item_id();
        let mut end_trace = PolylineTrace::new(
            ItemHeader::new(end_id, net_nos, clearance_class, 0, FixedState::Unfixed),
            end_piece,
            layer,
            half_width,
            Some(layer_count),
        );
        end_trace.hdr.set_on_the_board(true);

        // ShapeTraceEntries.java:144.
        let mut from_item = board
            .items
            .remove(&trace_id)
            .expect("fast_cutout_trace: present, just checked");
        {
            let Item::Trace(from_trace) = &mut from_item else {
                unreachable!("fast_cutout_trace: checked above")
            };
            let ctx = item_ctx!(board);
            board.trees.reuse_entries_after_cutout(
                from_trace,
                &mut start_trace,
                &mut end_trace,
                &ctx,
            );
        }
        board.items.insert(trace_id, from_item);
        board.items.insert(start_id, Item::Trace(start_trace));
        board.items.insert(end_id, Item::Trace(end_trace));
        // ShapeTraceEntries.java:145.
        board.remove_item(trace_id);
    }

    /// Port of the private `ShapeTraceEntries.insertEntryPoint`
    /// (ShapeTraceEntries.java:728-762): insert into the list, sorted by edge index and then by
    /// the projection along that edge.
    fn insert_entry_point(
        &mut self,
        trace: ItemId,
        net_nos: Vec<i32>,
        trace_line_no: usize,
        edge_index: i32,
        entry_approx: FloatPoint,
    ) {
        let new_index = self.entries.len();
        self.entries.push(EntryPoint {
            trace,
            net_nos,
            trace_line_no,
            entry_approx,
            edge_index,
            stack_level: -1,
            next: None,
        });
        let mut current_prev: Option<usize> = None;
        let mut current_next = self.list_anchor;
        while let Some(next_index) = current_next {
            let next_edge_index = self.entries[next_index].edge_index;
            if next_edge_index > edge_index {
                break;
            }
            if next_edge_index == edge_index {
                // ShapeTraceEntries.java:739-751.
                let border_line_count = self.shape.border_line_count();
                let prev_corner = self.shape.corner_approx(edge_index as usize);
                let next_corner = if edge_index as usize == border_line_count.saturating_sub(1) {
                    self.shape.corner_approx(0)
                } else {
                    self.shape.corner_approx(edge_index as usize + 1)
                };
                if let (Some(prev_corner), Some(next_corner)) = (prev_corner, next_corner)
                    && prev_corner.scalar_product(&entry_approx, &next_corner)
                        <= prev_corner
                            .scalar_product(&self.entries[next_index].entry_approx, &next_corner)
                {
                    break;
                }
            }
            current_prev = Some(next_index);
            current_next = self.entries[next_index].next;
        }
        self.entries[new_index].next = current_next;
        match current_prev {
            Some(prev) => self.entries[prev].next = Some(new_index),
            None => self.list_anchor = Some(new_index),
        }
    }

    /// Port of the private `ShapeTraceEntries.rotateEntryListAroundAnchor`
    /// (ShapeTraceEntries.java:765-783).
    fn rotate_entry_list_around_anchor(&mut self, new_anchor: usize, edge_count: i32) {
        // Walk to the tail of the list starting at `new_anchor`.
        let mut current = Some(new_anchor);
        let mut prev = new_anchor;
        while let Some(index) = current {
            prev = index;
            current = self.entries[index].next;
        }
        self.entries[prev].next = self.list_anchor;
        // ShapeTraceEntries.java:773-780: everything before the new anchor gets `edgeCount`
        // added, so it sorts after the middle of `fromSide`.
        let mut current = self.list_anchor;
        while let Some(index) = current {
            if index == new_anchor {
                break;
            }
            self.entries[index].edge_index += edge_count;
            prev = index;
            current = self.entries[index].next;
        }
        self.entries[prev].next = None;
        self.list_anchor = Some(new_anchor);
    }

    /// The net numbers of the trace an entry belongs to — Java's `entry.trace.netNumbers`.
    fn trace_net_nos(&self, entry_index: usize) -> Vec<i32> {
        self.entries[entry_index].net_nos.clone()
    }
}

/// Port of the private static `ShapeTraceEntries.netNosEqual`
/// (ShapeTraceEntries.java:153-170): same length, and every element of the first array occurs in
/// the second.
fn nets_equal(net_nos1: &[i32], net_nos2: &[i32]) -> bool {
    if net_nos1.len() != net_nos2.len() {
        return false;
    }
    net_nos1.iter().all(|a| net_nos2.contains(a))
}

/// `Item.netsEqual(int[])` (Item.java:1189-1200), which `popPiece` and `resort` use where
/// `netNosEqual` is not what Java calls (ShapeTraceEntries.java:566,569,697,721).
///
/// It differs from [`nets_equal`] in one place: `Item.containsNet` rejects a net number `<= 0`
/// (Item.java:150-152), so two items on net 0 are *not* `netsEqual`.
fn nets_equal_exact(net_nos1: &[i32], net_nos2: &[i32]) -> bool {
    if net_nos1.len() != net_nos2.len() {
        return false;
    }
    net_nos2.iter().all(|n| *n > 0 && net_nos1.contains(n))
}
