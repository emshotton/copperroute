//! Normalization-only adjacency scratch. Topology edits must invalidate it before another query.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use crate::ids::ItemId;

use super::Board;

#[derive(Debug, Default)]
// Mutex keeps Board: Sync while shared queries populate the scratch map.
pub(super) struct ContactCache(Option<Mutex<BTreeMap<ItemId, Arc<BTreeSet<ItemId>>>>>);

// Scratch data must not travel with snapshots or affect board equality.
impl Clone for ContactCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for ContactCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

pub(super) enum Contacts {
    Fresh(BTreeSet<ItemId>),
    Cached(Arc<BTreeSet<ItemId>>),
}

impl Deref for Contacts {
    type Target = BTreeSet<ItemId>;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Fresh(contacts) => contacts,
            Self::Cached(contacts) => contacts,
        }
    }
}

struct Scope<'a>(&'a mut Board);

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        self.0.contact_cache.0 = None;
    }
}

impl Board {
    // Keep this private: callers can otherwise mutate public board fields without invalidation.
    pub(super) fn with_cached_contacts<R>(&mut self, operation: impl FnOnce(&mut Board) -> R) -> R {
        self.contact_cache.0 = Some(Mutex::default());
        let scope = Scope(self);
        operation(scope.0)
    }

    pub(super) fn cycle_contacts(&self, id: ItemId) -> Contacts {
        let Some(cache) = &self.contact_cache.0 else {
            return Contacts::Fresh(self.normal_contacts(id));
        };
        let hit = cache.lock().unwrap().get(&id).cloned();
        if let Some(contacts) = hit {
            debug_assert_eq!(*contacts, self.normal_contacts(id));
            return Contacts::Cached(contacts);
        }
        let contacts = Arc::new(self.normal_contacts(id));
        cache.lock().unwrap().insert(id, Arc::clone(&contacts));
        Contacts::Cached(contacts)
    }

    pub(super) fn invalidate_cached_contacts(&mut self) {
        if let Some(cache) = &mut self.contact_cache.0 {
            cache.get_mut().unwrap().clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use fr_geometry::{Area, IntBox, IntVector, Point, Polyline, Shape, TileShape};

    fn board() -> Board {
        let layers = LayerStructure::new(vec![Layer::new("Top", true), Layer::new("Bottom", true)]);
        let mut rules = BoardRules::new(
            layers.clone(),
            ClearanceMatrix::get_default_instance(&layers, 10),
        );
        rules.create_default_net_class();
        let class = rules.get_default_net_class();
        rules.nets.add("N1", 1, false, class);
        Board::new(
            Vec::new(),
            0,
            IntBox::from_coords(0, 0, 1000, 1000),
            rules,
            BoardLibrary::new(Padstacks::new(layers), Packages::new()),
            Components::new(),
            Communication::default(),
        )
    }

    fn trace(board: &mut Board, from: (i32, i32), to: (i32, i32)) -> ItemId {
        board
            .insert_trace_without_cleaning(
                Polyline::from_points(&[Point::new(from.0, from.1), Point::new(to.0, to.1)]),
                0,
                10,
                vec![1],
                0,
                FixedState::Unfixed,
            )
            .unwrap()
    }

    fn assert_fresh(board: &Board) {
        for id in board.items_in_board_order() {
            assert_eq!(*board.cycle_contacts(id), board.normal_contacts(id));
        }
    }

    #[test]
    fn contacts_follow_insert_remove_geometry_and_rollback() {
        let mut board = board();
        let first = trace(&mut board, (100, 100), (300, 100));
        board.with_cached_contacts(|board| {
            assert_fresh(board);
            let second = trace(board, (300, 100), (300, 300));
            assert_fresh(board);
            board.begin_undo_journal();
            let snapshot = board.deep_copy();
            assert!(snapshot.contact_cache.0.is_none());
            board.save_for_undo(second);
            board.replace_trace_geometry(
                second,
                Polyline::from_points(&[Point::new(500, 500), Point::new(500, 700)]),
            );
            assert_fresh(board);
            assert!(!board.cycle_contacts(first).contains(&second));
            board.undo_from_snapshot(snapshot);
            assert!(board.contact_cache.0.is_some());
            assert!(board.normal_contacts(first).contains(&second));
            assert_fresh(board);
            board.begin_undo_journal();
            let snapshot = board.deep_copy();
            board.change_trace(
                second,
                Polyline::from_points(&[Point::new(500, 500), Point::new(500, 700)]),
            );
            assert_fresh(board);
            board.undo_from_snapshot(snapshot);
            assert!(board.normal_contacts(first).contains(&second));
            assert_fresh(board);
            board.begin_undo_journal();
            let snapshot = board.deep_copy();
            board
                .move_item_by(second, &IntVector::new(100, 0).into())
                .unwrap();
            assert_fresh(board);
            board.undo_from_snapshot(snapshot);
            assert!(board.normal_contacts(first).contains(&second));
            assert_fresh(board);
            board.get_item_mut(second).unwrap().header_mut().net_nos = vec![2];
            assert_fresh(board);
            board.remove_item(first);
            assert_fresh(board);
        });
        assert!(board.contact_cache.0.is_none());
    }

    #[test]
    #[cfg(debug_assertions)]
    fn failed_hit_validation_does_not_poison_cache() {
        let mut board = board();
        let first = trace(&mut board, (100, 100), (300, 100));
        trace(&mut board, (300, 100), (300, 300));
        board.with_cached_contacts(|board| {
            board
                .contact_cache
                .0
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .insert(first, Arc::new(BTreeSet::new()));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                board.cycle_contacts(first);
            }));
            assert!(result.is_err());
            assert!(!board.contact_cache.0.as_ref().unwrap().is_poisoned());
            board.invalidate_cached_contacts();
            assert_fresh(board);
        });
    }

    #[test]
    fn changing_tree_entries_invalidates_contacts() {
        let mut board = board();
        let id = trace(&mut board, (100, 100), (300, 100));
        board.with_cached_contacts(|board| {
            board.cycle_contacts(id);
            let Item::Trace(trace) = board.get_item(id).unwrap() else {
                unreachable!()
            };
            let lines = trace.polyline().clone();
            assert!(board.change_trace_entries(id, &lines, 0, 0));
            assert!(
                board
                    .contact_cache
                    .0
                    .as_ref()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .is_empty()
            );
            assert_fresh(board);
        });
    }

    #[test]
    fn merging_tree_entries_in_front_invalidates_contacts() {
        assert_merge_invalidates_contacts(true);
    }

    #[test]
    fn merging_tree_entries_at_end_invalidates_contacts() {
        assert_merge_invalidates_contacts(false);
    }

    fn assert_merge_invalidates_contacts(in_front: bool) {
        let mut board = board();
        let first = trace(&mut board, (100, 100), (300, 100));
        let second = trace(&mut board, (300, 100), (300, 300));
        let joined = Polyline::from_points(&[
            Point::new(100, 100),
            Point::new(300, 100),
            Point::new(300, 300),
        ]);
        board.with_cached_contacts(|board| {
            assert_fresh(board);
            assert!(
                !board
                    .contact_cache
                    .0
                    .as_ref()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .is_empty()
            );
            assert!(if in_front {
                board.merge_trace_entries_in_front(first, second, &joined, 0, 3)
            } else {
                board.merge_trace_entries_at_end(second, first, &joined, 0, 3)
            });
            assert!(
                board
                    .contact_cache
                    .0
                    .as_ref()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .is_empty()
            );
        });
    }

    #[test]
    fn repeated_queries_share_contacts_but_new_scopes_do_not() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Board>();
        let mut board = board();
        let id = trace(&mut board, (100, 100), (300, 100));
        let saved = board.with_cached_contacts(|board| {
            let Contacts::Cached(first) = board.cycle_contacts(id) else {
                panic!("uncached")
            };
            let Contacts::Cached(second) = board.cycle_contacts(id) else {
                panic!("uncached")
            };
            assert!(Arc::ptr_eq(&first, &second));
            first
        });
        trace(&mut board, (300, 100), (300, 300));
        board.with_cached_contacts(|board| {
            assert_fresh(board);
            let Contacts::Cached(next) = board.cycle_contacts(id) else {
                panic!("uncached")
            };
            assert!(!Arc::ptr_eq(&saved, &next));
        });
    }

    #[test]
    fn normalization_discards_cache_at_every_stop_boundary() {
        let mut original = board();
        trace(&mut original, (100, 100), (300, 100));
        trace(&mut original, (300, 100), (300, 300));
        trace(&mut original, (300, 300), (100, 300));
        trace(&mut original, (100, 300), (100, 100));
        for limit in 1..50 {
            let mut board = original.clone();
            let polls = std::cell::Cell::new(0);
            let _ = board.normalize_all_traces_checked(&|| {
                polls.set(polls.get() + 1);
                polls.get() >= limit
            });
            assert!(board.contact_cache.0.is_none());
            board.with_cached_contacts(|board| assert_fresh(board));
        }
    }

    #[test]
    fn splitting_another_trace_and_combining_refresh_contacts() {
        let mut board = board();
        let main = trace(&mut board, (100, 100), (500, 100));
        let tap = trace(&mut board, (300, 100), (300, 300));
        board.with_cached_contacts(|board| {
            assert_fresh(board);
            assert_eq!(
                board.split_trace_checked(tap, None, &|| false).unwrap(),
                vec![tap]
            );
            assert!(!board.items.contains_key(&main));
            assert_fresh(board);
            board.remove_item(tap);
            assert_fresh(board);
            assert!(board.combine_traces(1).unwrap());
            assert_fresh(board);
        });
    }

    #[test]
    fn conduction_area_cycles_respect_net_policy() {
        let mut board = board();
        let id = trace(&mut board, (100, 100), (300, 100));
        trace(&mut board, (300, 100), (300, 300));
        trace(&mut board, (300, 300), (100, 300));
        board.insert_conduction_area(
            Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                50, 50, 150, 350,
            )))),
            0,
            vec![1],
            0,
            true,
            FixedState::Unfixed,
        );
        for ignore in [false, true, false] {
            let class = board.rules.get_default_net_class();
            board
                .rules
                .net_classes
                .get_mut(class)
                .set_ignore_cycles_with_areas(ignore);
            let fresh = board.is_trace_cycle(id);
            assert_eq!(fresh, !ignore);
            board.with_cached_contacts(|board| {
                assert_fresh(board);
                assert_eq!(board.is_trace_cycle(id), fresh);
                assert_eq!(board.is_trace_cycle(id), fresh);
            });
        }
    }

    #[test]
    fn vias_connect_layers_and_layer_changes_refresh_contacts() {
        let mut board = board();
        let top = trace(&mut board, (100, 100), (300, 100));
        let bottom = board
            .insert_trace_without_cleaning(
                Polyline::from_points(&[Point::new(100, 100), Point::new(300, 100)]),
                1,
                10,
                vec![1],
                0,
                FixedState::Unfixed,
            )
            .unwrap();
        let pad = Shape::Tile(TileShape::Box(IntBox::from_coords(-20, -20, 20, 20)));
        let padstack =
            board
                .library
                .padstacks
                .add("via", vec![Some(pad.clone()), Some(pad)], true, false);
        board.with_cached_contacts(|board| {
            assert_fresh(board);
            assert!(board.cycle_contacts(top).is_empty());
            board
                .insert_via(
                    padstack,
                    Point::new(100, 100),
                    vec![1],
                    0,
                    FixedState::Unfixed,
                    false,
                )
                .unwrap();
            let via = board
                .insert_via(
                    padstack,
                    Point::new(300, 100),
                    vec![1],
                    0,
                    FixedState::Unfixed,
                    false,
                )
                .unwrap();
            assert_fresh(board);
            assert!(board.is_trace_cycle(top));
            board.remove_item(via);
            assert_fresh(board);
            assert!(!board.is_trace_cycle(top));
            let mut moved = board.items[&bottom].clone();
            board.remove_item(bottom);
            let Item::Trace(trace) = &mut moved else {
                unreachable!()
            };
            trace.set_layer(0);
            moved.clear_derived_data();
            board.insert_item(moved);
            assert_fresh(board);
        });
    }

    #[test]
    fn positive_cycle_and_broken_cycle_match_fresh_queries() {
        let mut board = board();
        let first = trace(&mut board, (100, 100), (300, 100));
        trace(&mut board, (300, 100), (300, 300));
        trace(&mut board, (300, 300), (100, 300));
        let last = trace(&mut board, (100, 300), (100, 100));
        assert!(board.is_trace_cycle(first));
        board.with_cached_contacts(|board| {
            for _ in 0..2 {
                assert!(board.is_trace_cycle(first));
                assert_fresh(board);
            }
            board.remove_item(last);
            assert!(!board.is_trace_cycle(first));
            assert_fresh(board);
        });
        assert!(!board.is_trace_cycle(first));
    }

    #[test]
    fn unwind_discards_scratch() {
        let mut board = board();
        let id = trace(&mut board, (100, 100), (300, 100));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            board.with_cached_contacts(|board| {
                board.cycle_contacts(id);
                assert!(board.contact_cache.0.is_some());
                panic!("stop");
            });
        }));
        assert!(result.is_err());
        assert!(board.contact_cache.0.is_none());
        assert!(board.normalize_traces_checked(1, &|| true).is_err());
        assert!(board.contact_cache.0.is_none());
    }
}
