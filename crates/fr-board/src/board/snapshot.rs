//! `#[derive(Clone)]` with no `transient` concept, so left alone it would copy every one of them
//! That would matter here only if the port's `#[derive(Clone)]` — which preserves the *original*
//! direction a `#[derive(Hash)]` or a `HashMap` key would depend on — while the converse is only
use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};

use fr_geometry::{Area, PolylineShapeRef, Shape, Vector};

use crate::ids::ItemId;
use crate::items::{Item, ObstacleAreaData};

use super::{Board, item_ctx};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UndoJournal {
    pub(crate) created: BTreeSet<ItemId>,
    pub(crate) saved: BTreeSet<ItemId>,
    pub(crate) deleted: Vec<ItemId>,
}

struct HashWriter<'a, H: Hasher>(&'a mut H);

impl<H: Hasher> std::fmt::Write for HashWriter<'_, H> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

fn hash_debug<H: Hasher, T: std::fmt::Debug + ?Sized>(value: &T, hasher: &mut H) {
    write!(HashWriter(hasher), "{value:?}").expect("HashWriter never fails");
    0xffu8.hash(hasher);
}

fn hash_vector<H: Hasher>(vector: &Vector, hasher: &mut H) {
    match vector {
        Vector::Int(v) => {
            0u8.hash(hasher);
            v.hash(hasher);
        }
        Vector::Rational(_) => {
            1u8.hash(hasher);
            hash_debug(vector, hasher);
        }
    }
}

fn hash_polyline_shape<H: Hasher>(shape: &PolylineShapeRef, hasher: &mut H) {
    match shape {
        PolylineShapeRef::Tile(tile) => {
            0u8.hash(hasher);
            tile.hash(hasher);
        }
        PolylineShapeRef::Polygon(polygon) => {
            1u8.hash(hasher);
            polygon.corners().hash(hasher);
        }
    }
}

fn hash_shape<H: Hasher>(shape: &Shape, hasher: &mut H) {
    match shape {
        Shape::Tile(tile) => {
            0u8.hash(hasher);
            tile.hash(hasher);
        }
        Shape::Polygon(polygon) => {
            1u8.hash(hasher);
            polygon.corners().hash(hasher);
        }
        Shape::Circle(circle) => {
            2u8.hash(hasher);
            circle.hash(hasher);
        }
    }
}

fn hash_area<H: Hasher>(area: &Area, hasher: &mut H) {
    match area {
        Area::Shape(shape) => {
            0u8.hash(hasher);
            hash_shape(shape, hasher);
        }
        Area::Polyline(polyline_area) => {
            1u8.hash(hasher);
            hash_polyline_shape(polyline_area.get_border(), hasher);
            polyline_area.get_holes().len().hash(hasher);
            for hole in polyline_area.get_holes() {
                hash_polyline_shape(hole, hasher);
            }
        }
    }
}

fn hash_obstacle_area<H: Hasher>(area: &ObstacleAreaData, hasher: &mut H) {
    area.name().hash(hasher);
    hash_area(area.get_relative_area(), hasher);
    area.get_layer().hash(hasher);
    hash_vector(area.get_translation(), hasher);
    area.get_rotation_in_degree().to_bits().hash(hasher);
    area.get_side_changed().hash(hasher);
}

impl Board {
    pub fn deep_copy(&self) -> Board {
        let mut copy = self.clone();

        copy.normalize_suppressed_net_nos.clear();
        copy.revision = 0;
        copy.changed_area = None;
        copy.shove_failing_obstacle = None;
        copy.shove_failing_layer = 0;

        copy.clear_autoroute_scratch();
        copy.finish_autoroute();
        copy.undo_journal = None;
        copy
    }

    fn clear_autoroute_scratch(&mut self) {
        for item in self.items.values_mut() {
            item.clear_autoroute_info();
        }
    }

    pub fn begin_undo_journal(&mut self) {
        self.undo_journal = Some(UndoJournal::default());
    }

    pub fn discard_undo_journal(&mut self) {
        self.undo_journal = None;
    }

    #[cfg(test)]
    pub(crate) fn undo_journal(&self) -> Option<&UndoJournal> {
        self.undo_journal.as_ref()
    }

    pub(crate) fn save_for_undo(&mut self, id: ItemId) {
        if let Some(journal) = self.undo_journal.as_mut()
            && !journal.created.contains(&id)
        {
            journal.saved.insert(id);
        }
    }

    pub(crate) fn journal_insert(&mut self, id: ItemId) {
        if let Some(journal) = self.undo_journal.as_mut() {
            journal.created.insert(id);
        }
    }

    pub(crate) fn journal_remove(&mut self, id: ItemId) {
        if let Some(journal) = self.undo_journal.as_mut() {
            if journal.created.remove(&id) {
                return;
            }
            journal.saved.remove(&id);
            journal.deleted.push(id);
        }
    }

    pub fn undo_from_snapshot(&mut self, snapshot: Board) {
        let journal = self.undo_journal.take().unwrap_or_default();
        let mut snapshot_items = snapshot.items;

        self.components = snapshot.components;

        let cancelled: Vec<ItemId> = journal
            .created
            .iter()
            .chain(journal.saved.iter())
            .copied()
            .collect::<BTreeSet<ItemId>>()
            .into_iter()
            .rev()
            .collect();
        let restored: Vec<ItemId> = journal
            .saved
            .iter()
            .rev()
            .copied()
            .chain(journal.deleted.iter().copied())
            .collect();

        for id in &cancelled {
            if let Some(item) = self.items.get_mut(id) {
                self.trees.remove(item);
            }
        }

        for id in &journal.created {
            self.items.remove(id);
        }
        for id in &restored {
            if let Some(item) = snapshot_items.remove(id) {
                self.items.insert(*id, item);
            }
        }

        let ctx = item_ctx!(self);
        for id in &restored {
            let Some(item) = self.items.get_mut(id) else {
                continue;
            };
            item.clear_tree_entries();
            item.set_on_the_board(false);
            self.trees.insert(item, &ctx);
            item.clear_autoroute_info();
        }
    }

    fn finish_autoroute(&mut self) {}

    pub fn structural_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        let ctx = self.ctx();
        self.items.len().hash(&mut hasher);
        for (id, item) in self.items.iter().rev() {
            id.hash(&mut hasher);
            item.kind().hash(&mut hasher);
            item.net_nos().hash(&mut hasher);
            item.clearance_class().hash(&mut hasher);
            item.get_fixed_state().hash(&mut hasher);
            item.component_id().hash(&mut hasher);
            item.is_on_the_board().hash(&mut hasher);
            match item {
                Item::Trace(trace) => {
                    trace.get_layer().hash(&mut hasher);
                    trace.get_half_width().hash(&mut hasher);
                    trace.polyline().hash(&mut hasher);
                }
                Item::Via(via) => {
                    via.get_center().hash(&mut hasher);
                    via.get_padstack_id().hash(&mut hasher);
                    via.attach_allowed.hash(&mut hasher);
                    via.is_escape_via.hash(&mut hasher);
                    via.escape_via_smd_layer.hash(&mut hasher);
                }
                Item::Pin(pin) => {
                    pin.get_pin_index().hash(&mut hasher);
                    pin.get_changed_to().hash(&mut hasher);
                }
                Item::ObstacleArea(area) => hash_obstacle_area(&area.area, &mut hasher),
                Item::ViaObstacleArea(area) => hash_obstacle_area(&area.area, &mut hasher),
                Item::ComponentObstacleArea(area) => hash_obstacle_area(&area.area, &mut hasher),
                Item::ConductionArea(area) => {
                    hash_obstacle_area(&area.area, &mut hasher);
                    area.get_is_obstacle().hash(&mut hasher);
                    area.get_is_filled().hash(&mut hasher);
                }
                Item::ComponentOutline(outline) => {
                    hash_area(outline.get_area(&ctx), &mut hasher);
                    hash_vector(outline.get_translation(), &mut hasher);
                    outline.get_rotation_in_degree().to_bits().hash(&mut hasher);
                    outline.is_front().hash(&mut hasher);
                    outline.is_courtyard().hash(&mut hasher);
                    outline.is_fabrication().hash(&mut hasher);
                    outline.is_closed().hash(&mut hasher);
                }
                Item::BoardOutline(outline) => {
                    outline.shape_count().hash(&mut hasher);
                    for index in 0..outline.shape_count() {
                        match outline.get_shape(index) {
                            Some(shape) => hash_polyline_shape(shape, &mut hasher),
                            None => 0xfeu8.hash(&mut hasher),
                        }
                    }
                    outline
                        .keepout_outside_outline_generated()
                        .hash(&mut hasher);
                }
            }
        }
        hasher.finish()
    }

    pub fn diff_traces(&self, compare_to: &Board) -> usize {
        let mut trace_ids: BTreeSet<ItemId> = self.get_traces().into_iter().collect();
        let mut result = 0usize;
        for id in compare_to.get_traces() {
            if !trace_ids.remove(&id) {
                result += 1;
            }
        }
        result + trace_ids.len()
    }
}

#[cfg(test)]
mod tests {
    use fr_geometry::{IntBox, Point, Polyline, PolylineShapeRef, TileShape};

    use crate::ids::ItemId;
    use crate::library::{BoardLibrary, Packages, Padstacks};
    use crate::rules::{BoardRules, ClearanceMatrix};
    use crate::structure::{Components, FixedState, Layer, LayerStructure};
    use crate::{Board, Communication};

    fn board() -> Board {
        let layers = LayerStructure::new(vec![Layer::new("Top", true)]);
        let clearance_matrix = ClearanceMatrix::get_default_instance(&layers, 10);
        let mut rules = BoardRules::new(layers.clone(), clearance_matrix);
        rules.create_default_net_class();
        let outline = vec![PolylineShapeRef::Tile(TileShape::Box(IntBox::from_coords(
            0, 0, 1000, 1000,
        )))];
        Board::new(
            outline,
            0,
            IntBox::from_coords(0, 0, 1000, 1000),
            rules,
            BoardLibrary::new(Padstacks::new(layers), Packages::new()),
            Components::new(),
            Communication::default(),
        )
    }

    fn insert_trace(board: &mut Board, net_number: i32, x1: i32, x2: i32) -> ItemId {
        board
            .insert_trace_without_cleaning(
                Polyline::from_points(&[Point::new(x1, 100), Point::new(x2, 100)]),
                0,
                10,
                vec![net_number],
                0,
                FixedState::Unfixed,
            )
            .expect("a straight two-corner trace")
    }

    #[test]
    fn deep_copy_is_independent() {
        let mut board = board();
        insert_trace(&mut board, 1, 100, 500);
        let mut copy = board.deep_copy();

        assert_eq!(copy.items, board.items);
        assert_eq!(copy.structural_hash(), board.structural_hash());

        let trace = board.get_traces()[0];
        copy.remove_item(trace);

        assert_ne!(copy, board);
        assert!(board.get_item(trace).is_some());
        assert!(copy.get_item(trace).is_none());

        let ctx = board.ctx();
        let shape = board
            .get_item(trace)
            .expect("the trace")
            .get_tile_shape(board.default_tree_id(), 0, &ctx)
            .expect("its tile shape");
        let object = crate::ids::TreeObject::Item(trace);
        assert!(board.overlapping_objects(&shape, Some(0)).contains(&object));
        assert!(!copy.overlapping_objects(&shape, Some(0)).contains(&object));
    }

    #[test]
    fn deep_copy_clears_autoroute_scratch() {
        let mut board = board();
        let trace = insert_trace(&mut board, 1, 100, 500);
        board
            .get_item_mut(trace)
            .expect("the trace")
            .get_autoroute_info();
        assert!(
            board
                .get_item(trace)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_some()
        );

        let copy = board.deep_copy();
        assert!(
            copy.get_item(trace)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_none()
        );
        assert!(
            board
                .get_item(trace)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_some()
        );
    }

    #[test]
    fn deep_copy_clears_normalize_suppressed_net_nos() {
        let mut board = board();
        board.normalize_suppressed_net_nos.insert(3);
        let copy = board.deep_copy();
        assert!(copy.normalize_suppressed_net_nos.is_empty());
        assert!(board.normalize_suppressed_net_nos.contains(&3));
    }

    #[test]
    fn deep_copy_resets_transient_bookkeeping() {
        let mut board = board();
        let trace = insert_trace(&mut board, 1, 100, 500);
        assert_ne!(
            board.revision(),
            0,
            "the two inserts above must have advanced it"
        );

        board.start_marking_changed_area();
        assert!(board.changed_area.is_some());
        board.shove_failing_obstacle = Some(trace);
        board.shove_failing_layer = 3;

        let copy = board.deep_copy();

        assert_eq!(copy.revision(), 0);
        assert!(copy.changed_area.is_none());
        assert!(copy.shove_failing_obstacle.is_none());
        assert_eq!(copy.shove_failing_layer, 0);

        assert_ne!(board.revision(), 0);
        assert!(board.changed_area.is_some());
        assert_eq!(board.shove_failing_obstacle, Some(trace));
        assert_eq!(board.shove_failing_layer, 3);
    }

    #[test]
    fn hash_stable_across_clone() {
        let mut board = board();
        insert_trace(&mut board, 1, 100, 500);
        let clone = board.clone();
        assert_eq!(board.structural_hash(), clone.structural_hash());
        let copy = board.deep_copy();
        assert_eq!(board.structural_hash(), copy.structural_hash());
    }

    #[test]
    fn hash_equal_for_equal_boards_and_differs_after_trace_change() {
        let mut board_a = board();
        insert_trace(&mut board_a, 1, 100, 500);
        let mut board_b = board();
        insert_trace(&mut board_b, 1, 100, 500);
        assert_eq!(board_a.structural_hash(), board_b.structural_hash());

        insert_trace(&mut board_b, 2, 600, 900);
        assert_ne!(board_a.structural_hash(), board_b.structural_hash());
    }

    #[test]
    fn diff_traces_counts_ids_present_in_exactly_one_board() {
        let mut board_a = board();
        insert_trace(&mut board_a, 1, 100, 500);
        let trace_b = insert_trace(&mut board_a, 2, 600, 900);

        let mut board_b = board_a.clone();
        assert_eq!(board_a.diff_traces(&board_b), 0);

        board_b.remove_item(trace_b);
        assert_eq!(board_a.diff_traces(&board_b), 1);
        assert_eq!(board_b.diff_traces(&board_a), 1);

        insert_trace(&mut board_b, 3, 200, 300);
        assert_eq!(board_a.diff_traces(&board_b), 2);
    }

    #[test]
    fn undo_from_snapshot_cancels_the_inserted_items_and_restores_the_removed_ones() {
        let mut board = board();
        let kept = insert_trace(&mut board, 1, 100, 200);
        let removed = insert_trace(&mut board, 1, 300, 400);

        let snapshot = board.deep_copy();
        board.begin_undo_journal();
        let inserted = insert_trace(&mut board, 1, 500, 600);
        board.remove_item(removed);
        assert!(board.get_item(inserted).is_some());
        assert!(board.get_item(removed).is_none());

        board.undo_from_snapshot(snapshot);

        assert!(board.get_item(inserted).is_none());
        assert!(board.get_item(removed).is_some());
        assert!(board.get_item(kept).is_some());
        assert_eq!(board.undo_journal(), None);
    }

    #[test]
    fn undo_from_snapshot_puts_the_restored_item_back_in_the_live_trees() {
        let mut board = board();
        let trace = insert_trace(&mut board, 1, 300, 400);
        let ctx = board.ctx();
        let shape = board
            .get_item(trace)
            .expect("the trace")
            .get_tile_shape(board.default_tree_id(), 0, &ctx)
            .expect("its tile shape");
        let object = crate::ids::TreeObject::Item(trace);

        let snapshot = board.deep_copy();
        board.begin_undo_journal();
        board.remove_item(trace);
        assert!(!board.overlapping_objects(&shape, Some(0)).contains(&object));

        board.undo_from_snapshot(snapshot);

        assert!(board.overlapping_objects(&shape, Some(0)).contains(&object));
        assert!(board.get_item(trace).expect("the trace").is_on_the_board());
        assert!(board.validate_item(trace));
    }

    #[test]
    fn undo_from_snapshot_leaves_every_field_java_leaves_alone() {
        let mut board = board();
        let untouched = insert_trace(&mut board, 1, 100, 200);
        let removed = insert_trace(&mut board, 1, 300, 400);

        let snapshot = board.deep_copy();
        board.begin_undo_journal();

        let burned = board.new_item_id();
        board
            .get_item_mut(untouched)
            .expect("the trace")
            .get_autoroute_info();
        board.normalize_suppressed_net_nos.insert(7);
        board.shove_failing_obstacle = Some(removed);
        board.shove_failing_layer = 3;
        let revision_before_undo = board.revision();
        board.remove_item(removed);

        board.undo_from_snapshot(snapshot);

        assert_eq!(board.new_item_id().0, burned.0 + 1);
        assert!(board.normalize_suppressed_net_nos.contains(&7));
        assert_eq!(board.shove_failing_obstacle, Some(removed));
        assert_eq!(board.shove_failing_layer, 3);
        assert!(board.revision() > revision_before_undo);
        assert!(
            board
                .get_item(untouched)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_some()
        );
        assert!(
            board
                .get_item(removed)
                .expect("the restored trace")
                .get_autoroute_info_pur()
                .is_none()
        );
    }

    #[test]
    fn the_journal_records_removals_in_deletion_order_and_insertions_as_a_set() {
        let mut board = board();
        let a = insert_trace(&mut board, 1, 100, 200);
        let b = insert_trace(&mut board, 1, 300, 400);
        let c = insert_trace(&mut board, 1, 500, 600);

        board.begin_undo_journal();
        let created = insert_trace(&mut board, 1, 700, 800);
        board.remove_item(c);
        board.remove_item(a);
        board.remove_item(b);
        board.remove_item(created);

        let journal = board.undo_journal().expect("open");
        assert_eq!(journal.deleted, vec![c, a, b]);
        assert!(journal.created.is_empty());
        assert!(journal.saved.is_empty());
    }

    #[test]
    fn combining_two_traces_records_a_save_for_undo_and_the_undo_puts_the_short_one_back() {
        let mut board = board();
        let first = insert_trace(&mut board, 1, 100, 300);
        let second = insert_trace(&mut board, 1, 300, 600);

        let snapshot = board.deep_copy();
        board.begin_undo_journal();
        assert!(board.combine_trace(first).expect("combine"));

        let journal = board.undo_journal().expect("open");
        assert!(
            journal.saved.contains(&first),
            "the surviving trace was modified in place, so `undo` must cancel it and restore \
             the pre-attempt one"
        );
        assert_eq!(journal.deleted, vec![second]);
        let ctx = board.ctx();
        let combined_len = board
            .get_item(first)
            .expect("the survivor")
            .bounding_box(&ctx)
            .width();

        board.undo_from_snapshot(snapshot);

        assert!(board.get_item(second).is_some());
        let ctx = board.ctx();
        assert!(
            board
                .get_item(first)
                .expect("the survivor")
                .bounding_box(&ctx)
                .width()
                < combined_len,
            "the in-place modification is rolled back to the snapshot's geometry"
        );
        assert!(board.validate_item(first));
        assert!(board.validate_item(second));
    }

    #[test]
    fn discard_undo_journal_is_pop_snapshot_and_deep_copy_never_records() {
        let mut board = board();
        insert_trace(&mut board, 1, 100, 200);
        board.begin_undo_journal();
        assert!(board.undo_journal().is_some());

        let copy = board.deep_copy();
        assert_eq!(copy.undo_journal(), None);

        board.discard_undo_journal();
        assert_eq!(board.undo_journal(), None);
        let trace = insert_trace(&mut board, 1, 300, 400);
        board.remove_item(trace);
        assert_eq!(board.undo_journal(), None);
    }
}
