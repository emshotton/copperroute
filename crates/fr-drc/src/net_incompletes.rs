use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use fr_board::{Board, DelaunayCorner, ItemId, ItemKind, PlanarDelaunayTriangulation};
use fr_geometry::{FloatPoint, Signum};

use crate::airline::AirLine;

#[derive(Debug, Clone, PartialEq)]
pub struct NetIncompletes {
                pub incompletes: Vec<AirLine>,
        net_number: i32,
        draw_marker_radius: f64,
            length_violation: f64,
        connected_group_count: usize,
}

impl NetIncompletes {
                                pub fn new(net_number: i32, net_items: &[ItemId], board: &Board) -> NetIncompletes {
        let draw_marker_radius = f64::from(board.rules.get_min_trace_half_width()) * 2.0;

        let mut this = NetIncompletes {
            incompletes: Vec::new(),
            net_number,
            draw_marker_radius,
            length_violation: 0.0,
            connected_group_count: 0,
        };

        let filtered: Vec<ItemId> = net_items
            .iter()
            .copied()
            .filter(|&id| {
                if board.is_tail(id) {
                    return false;
                }
                let Some(item) = board.get_item(id) else {
                    debug_assert!(false, "net item {id:?} is not on the board");
                    return false;
                };
                let exempt = matches!(
                    item.kind(),
                    ItemKind::ConductionArea | ItemKind::Pin | ItemKind::Via
                );
                exempt || !board.normal_contacts(id).is_empty()
            })
            .collect();

        let (mut grouped_net_items, connected_sets) =
            calculate_net_items(net_number, &filtered, board);

        let unique_connected_sets: BTreeSet<&BTreeSet<ItemId>> = connected_sets.iter().collect();
        this.connected_group_count = unique_connected_sets.len();

        if grouped_net_items.len() <= 1 {
            this.connected_group_count = grouped_net_items.len();
            return this;
        }

        let mut corners: Vec<DelaunayCorner> = Vec::new();
        for net_item in &grouped_net_items {
            for point in board.ratsnest_corners(net_item.item) {
                corners.push(DelaunayCorner::new(net_item.item, point));
            }
        }
        let triangulation = PlanarDelaunayTriangulation::new(&corners);

        let index_of: BTreeMap<ItemId, usize> = grouped_net_items
            .iter()
            .enumerate()
            .map(|(index, net_item)| (net_item.item, index))
            .collect();
        let mut sorted_edges: BTreeSet<Edge> = BTreeSet::new();
        for line in triangulation.get_edge_lines() {
            let (Some(start), Some(end)) = (line.start_object, line.end_object) else {
                continue;
            };
            let (Some(&from_item), Some(&to_item)) = (index_of.get(&start), index_of.get(&end))
            else {
                debug_assert!(
                    false,
                    "a triangulation corner names an item the net does not have"
                );
                continue;
            };
            sorted_edges.insert(Edge {
                from_item,
                from_corner: line.start_point.to_float(),
                to_item,
                to_corner: line.end_point.to_float(),
                length_square: line.length_square(),
            });
        }

        for edge in sorted_edges {
            let from_set = grouped_net_items[edge.from_item].set_id;
            let to_set = grouped_net_items[edge.to_item].set_id;
            if from_set == to_set {
                continue;
            }
            this.incompletes.push(AirLine::new(
                net_number,
                grouped_net_items[edge.from_item].item,
                edge.from_corner,
                grouped_net_items[edge.to_item].item,
                edge.to_corner,
            ));
            join_connected_sets(&mut grouped_net_items, from_set, to_set);
        }

        this.calc_length_violation(board);
        this
    }

        pub fn count(&self) -> usize {
        self.incompletes.len()
    }

                pub fn get_connected_group_count(&self) -> usize {
        self.connected_group_count
    }

        pub fn get_marker_radius(&self) -> f64 {
        self.draw_marker_radius
    }

            pub fn get_length_violation(&self) -> f64 {
        self.length_violation
    }

            pub fn get_net_number(&self) -> i32 {
        self.net_number
    }

                                            pub fn calc_length_violation(&mut self, board: &Board) -> bool {
        let Some(net) = board.rules.nets.get(self.net_number) else {
            self.length_violation = 0.0;
            return false;
        };
        let net_class = board.rules.net_classes.get(net.get_net_class());
        let max_length = net_class.get_maximum_trace_length();
        let min_length = net_class.get_minimum_trace_length();
        if max_length <= 0.0 && min_length <= 0.0 {
            self.length_violation = 0.0;
            return false;
        }
        let mut new_violation = 0.0;
        let trace_length = board.net_trace_length(self.net_number);
        if max_length > 0.0 && trace_length > max_length {
            new_violation = trace_length - max_length;
        }
        if min_length > 0.0 && trace_length < min_length && self.incompletes.is_empty() {
            new_violation = trace_length - min_length;
        }
        let old_violation = self.length_violation;
        self.length_violation = new_violation;
        (new_violation - old_violation).abs() > 0.1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NetItem {
    item: ItemId,
    set_id: usize,
}

fn calculate_net_items(
    net_number: i32,
    item_list: &[ItemId],
    board: &Board,
) -> (Vec<NetItem>, Vec<BTreeSet<ItemId>>) {
    let mut unique_items: BTreeSet<ItemId> = item_list.iter().copied().collect();
    let mut result: Vec<NetItem> = Vec::new();
    let mut connected_sets: Vec<BTreeSet<ItemId>> = Vec::new();

    while let Some(&start_item) = unique_items.iter().next() {
        let current_connected_set = board.connected_set(start_item, net_number, false);

        let items_in_component: Vec<ItemId> = current_connected_set
            .iter()
            .rev()
            .copied()
            .filter(|id| unique_items.contains(id))
            .collect();

        if items_in_component.is_empty() {
            unique_items.remove(&start_item);
            continue;
        }

        let set_id = connected_sets.len();
        for &id in &items_in_component {
            result.push(NetItem { item: id, set_id });
        }
        for id in &items_in_component {
            unique_items.remove(id);
        }
        connected_sets.push(current_connected_set);
    }

    (result, connected_sets)
}

fn join_connected_sets(net_items: &mut [NetItem], from_set: usize, to_set: usize) {
    for net_item in net_items.iter_mut() {
        if net_item.set_id == from_set {
            net_item.set_id = to_set;
        }
    }
}

#[derive(Debug, Clone)]
struct Edge {
    from_item: usize,
    from_corner: FloatPoint,
    to_item: usize,
    to_corner: FloatPoint,
            length_square: f64,
}

impl PartialEq for Edge {
    fn eq(&self, other: &Edge) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Edge {}

impl PartialOrd for Edge {
    fn partial_cmp(&self, other: &Edge) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Edge {
                                fn cmp(&self, other: &Edge) -> Ordering {
        let mut result = self.length_square - other.length_square;
        if result == 0.0 {
            result = self.from_corner.x - other.from_corner.x;
            if result == 0.0 {
                result = self.from_corner.y - other.from_corner.y;
            }
            if result == 0.0 {
                result = self.to_corner.x - other.to_corner.x;
            }
            if result == 0.0 {
                result = self.to_corner.y - other.to_corner.y;
            }
        }
        match Signum::as_int_f64(result) {
            -1 => Ordering::Less,
            1 => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }
}

#[cfg(test)]
mod tests {
                use super::*;
    use fr_board::prelude::*;
    use fr_geometry::{IntBox, IntPoint, Point, Polyline};

    const BOUNDING_BOX: IntBox = IntBox {
        ll: IntPoint {
            x: -100_000,
            y: -100_000,
        },
        ur: IntPoint {
            x: 100_000,
            y: 100_000,
        },
    };

    fn layers() -> LayerStructure {
        LayerStructure::new(vec![Layer::new("front", true)])
    }

            fn bare_board() -> Board {
        let ls = layers();
        let matrix = ClearanceMatrix::get_default_instance(&ls, 200);
        let mut rules = BoardRules::new(ls, matrix);
        rules.create_default_net_class();
        let default_class = rules.get_default_net_class();
        let mut board = Board::new(
            Vec::new(),
            1,
            BOUNDING_BOX,
            rules,
            BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
            Components::new(),
            Communication::default(),
        );
        board.rules.nets.add("N1", 1, false, default_class);
        board
    }

    fn insert_trace(board: &mut Board, from: (i32, i32), to: (i32, i32)) -> ItemId {
        board
            .insert_trace_without_cleaning(
                Polyline::from_points(&[Point::new(from.0, from.1), Point::new(to.0, to.1)]),
                0,
                30,
                vec![1],
                1,
                FixedState::Unfixed,
            )
            .expect("the synthetic trace is neither degenerate nor closed")
    }

    #[test]
    fn net_items_are_ordered_descending_within_a_component() {
        let mut board = bare_board();
        let ids: Vec<ItemId> = (0..4)
            .map(|i| insert_trace(&mut board, (i * 1000, 0), ((i + 1) * 1000, 0)))
            .collect();
        assert_eq!(ids, [2, 3, 4, 5].map(ItemId));

        let (net_items, sets) = calculate_net_items(1, &ids, &board);
        assert_eq!(
            net_items.iter().map(|n| n.item).collect::<Vec<_>>(),
            [5, 4, 3, 2].map(ItemId),
        );
        assert!(net_items.iter().all(|n| n.set_id == 0));
        assert_eq!(sets.len(), 1);
    }

    #[test]
    fn seeds_are_taken_in_ascending_id_order() {
        let mut board = bare_board();
        let a1 = insert_trace(&mut board, (0, 0), (1000, 0));
        let a2 = insert_trace(&mut board, (1000, 0), (2000, 0));
        let b1 = insert_trace(&mut board, (50_000, 0), (51_000, 0));
        let b2 = insert_trace(&mut board, (51_000, 0), (52_000, 0));
        assert_eq!([a1, a2, b1, b2], [2, 3, 4, 5].map(ItemId));

        let (net_items, sets) = calculate_net_items(1, &[a1, a2, b1, b2], &board);
        assert_eq!(
            net_items.iter().map(|n| n.item).collect::<Vec<_>>(),
            [3, 2, 5, 4].map(ItemId),
        );
        assert_eq!(
            net_items.iter().map(|n| n.set_id).collect::<Vec<_>>(),
            [0, 0, 1, 1],
        );
        assert_eq!(sets.len(), 2);
    }

    #[test]
    fn joining_two_sets_repoints_every_member_of_the_from_set() {
        let mut net_items = vec![
            NetItem {
                item: ItemId(2),
                set_id: 0,
            },
            NetItem {
                item: ItemId(3),
                set_id: 0,
            },
            NetItem {
                item: ItemId(4),
                set_id: 1,
            },
        ];
        join_connected_sets(&mut net_items, 0, 1);
        assert!(net_items.iter().all(|n| n.set_id == 1));
    }

    fn edge(from_item: usize, from: (f64, f64), to_item: usize, to: (f64, f64)) -> Edge {
        let from_corner = FloatPoint::new(from.0, from.1);
        let to_corner = FloatPoint::new(to.0, to.1);
        Edge {
            from_item,
            from_corner,
            to_item,
            to_corner,
            length_square: to_corner.distance_square(&from_corner),
        }
    }

    #[test]
    fn an_exact_five_way_tie_drops_the_second_edge() {
        let first = edge(0, (0.0, 0.0), 1, (10.0, 0.0));
        let second = edge(2, (0.0, 0.0), 3, (10.0, 0.0));
        assert_eq!(first.cmp(&second), Ordering::Equal);
        assert_eq!(first, second);
        assert_ne!(
            (first.from_item, first.to_item),
            (second.from_item, second.to_item),
        );

        let mut set = BTreeSet::new();
        assert!(set.insert(first.clone()));
        assert!(!set.insert(second));
        assert_eq!(set.len(), 1);
        let kept = set.iter().next().expect("one element");
        assert_eq!(
            (kept.from_item, kept.to_item),
            (first.from_item, first.to_item)
        );
    }

    #[test]
    fn a_nan_edge_swallows_or_is_swallowed_depending_on_insertion_order() {
        let short = edge(0, (0.0, 0.0), 1, (10.0, 0.0));
        let long = edge(2, (0.0, 0.0), 3, (20.0, 0.0));
        let nan = edge(4, (f64::INFINITY, 0.0), 5, (f64::INFINITY, 0.0));
        assert!(nan.length_square.is_nan());
        assert_eq!(nan.cmp(&short), Ordering::Equal);
        assert_eq!(short.cmp(&nan), Ordering::Equal);
        assert_eq!(short.cmp(&long), Ordering::Less);

        let mut nan_first = BTreeSet::new();
        assert!(nan_first.insert(nan.clone()));
        assert!(!nan_first.insert(short.clone()));
        assert!(!nan_first.insert(long.clone()));
        assert_eq!(nan_first.len(), 1);

        let mut nan_last = BTreeSet::new();
        assert!(nan_last.insert(short));
        assert!(nan_last.insert(long));
        assert!(!nan_last.insert(nan));
        assert_eq!(nan_last.len(), 2);

    }

    #[test]
    fn edges_are_ordered_by_length_then_by_the_four_coordinates() {
        let short = edge(0, (0.0, 0.0), 1, (1.0, 0.0));
        let long = edge(0, (0.0, 0.0), 1, (10.0, 0.0));
        assert_eq!(short.cmp(&long), Ordering::Less);

        let left = edge(0, (0.0, 0.0), 1, (0.0, 5.0));
        let right = edge(0, (1.0, 0.0), 1, (1.0, 5.0));
        assert_eq!(left.length_square, right.length_square);
        assert_eq!(left.cmp(&right), Ordering::Less);

        let lower = edge(0, (1.0, 0.0), 1, (1.0, 5.0));
        let upper = edge(0, (1.0, 1.0), 1, (1.0, 6.0));
        assert_eq!(lower.cmp(&upper), Ordering::Less);

        let to_left = edge(0, (0.0, 0.0), 1, (3.0, 4.0));
        let to_right = edge(0, (0.0, 0.0), 1, (4.0, 3.0));
        assert_eq!(to_left.length_square, to_right.length_square);
        assert_eq!(to_left.cmp(&to_right), Ordering::Less);
    }
}
