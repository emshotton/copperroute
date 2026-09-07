use std::collections::HashMap;

use copper_geometry::TileShape;

use crate::datastructures::LeafId;
use crate::ids::{ConnectionId, DrillId, ItemId, ObstacleRoomId, TreeId};
use crate::rules::{BoardRules, Nets};
use crate::structure::FixedState;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutorouteInfo {
    pub start_info: bool,
    pub precalculated_connection: Option<ConnectionId>,
    pub expansion_rooms: Vec<Option<ObstacleRoomId>>,
    pub autoroute_drill_info: Option<DrillId>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeEntries {
    pub leaves: Option<Vec<Option<LeafId>>>,
    pub shapes: Option<Vec<Option<TileShape>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemHeader {
    id: ItemId,

    pub net_nos: Vec<i32>,

    clearance_class: usize,

    fixed_state: FixedState,

    component_id: i32,

    on_the_board: bool,

    pub tree_entries: HashMap<TreeId, TreeEntries>,

    pub autoroute_info: Option<Box<AutorouteInfo>>,

    pub smallest_clearance: f64,
}

impl ItemHeader {
    pub fn new(
        id: ItemId,
        net_nos: Vec<i32>,
        clearance_class: usize,
        component_id: i32,
        fixed_state: FixedState,
    ) -> ItemHeader {
        ItemHeader {
            id,
            net_nos,
            clearance_class,
            fixed_state,
            component_id,
            on_the_board: false,
            tree_entries: HashMap::new(),
            autoroute_info: None,
            smallest_clearance: -1.0,
        }
    }

    pub fn id(&self) -> ItemId {
        self.id
    }

    pub fn net_count(&self) -> usize {
        self.net_nos.len()
    }

    pub fn get_net_number(&self, no: usize) -> i32 {
        self.net_nos[no]
    }

    pub fn contains_net(&self, net_number: i32) -> bool {
        if net_number <= 0 {
            return false;
        }
        self.net_nos.contains(&net_number)
    }

    pub fn shares_net_no(&self, net_nos: &[i32]) -> bool {
        self.net_nos.iter().any(|a| net_nos.contains(a))
    }

    pub fn nets_equal(&self, net_nos: &[i32]) -> bool {
        if self.net_nos.len() != net_nos.len() {
            return false;
        }
        net_nos.iter().all(|n| self.contains_net(*n))
    }

    pub fn nets_normal(&self) -> bool {
        self.net_nos.iter().all(|n| Nets::is_normal_net_number(*n))
    }

    pub fn assign_net_no(&mut self, net_number: i32, nets: &Nets) {
        if !Nets::is_normal_net_number(net_number) {
            return;
        }
        if net_number > nets.max_net_number() {
            return;
        }
        self.net_nos.clear();
        self.net_nos.push(net_number);
    }

    pub fn remove_from_net(&mut self, net_number: i32) -> bool {
        match self.net_nos.iter().position(|n| *n == net_number) {
            Some(index) => {
                self.net_nos.remove(index);
                true
            }
            None => false,
        }
    }

    pub fn get_fixed_state(&self) -> FixedState {
        self.fixed_state
    }

    pub fn set_fixed_state(&mut self, fixed_state: FixedState) {
        self.fixed_state = fixed_state;
    }

    pub fn unfix(&mut self) {
        if self.fixed_state != FixedState::SystemFixed {
            self.fixed_state = FixedState::Unfixed;
        }
    }

    pub fn is_user_fixed(&self) -> bool {
        self.fixed_state >= FixedState::UserFixed
    }

    pub fn is_shove_fixed(&self) -> bool {
        self.fixed_state >= FixedState::ShoveFixed
    }

    pub fn clearance_class(&self) -> usize {
        self.clearance_class
    }

    pub fn set_clearance_class(&mut self, index: usize, rules: &BoardRules) {
        if index >= rules.clearance_matrix.get_class_count() {
            return;
        }
        self.clearance_class = index;
    }

    pub fn get_component_id(&self) -> i32 {
        self.component_id
    }

    pub fn assign_component_id(&mut self, id: i32) {
        self.component_id = id;
    }

    pub fn is_on_the_board(&self) -> bool {
        self.on_the_board
    }

    pub fn set_on_the_board(&mut self, value: bool) {
        self.on_the_board = value;
    }

    pub fn get_tree_entries(&self, tree: TreeId) -> Option<&[Option<LeafId>]> {
        self.tree_entries
            .get(&tree)
            .and_then(|e| e.leaves.as_deref())
    }

    pub fn set_tree_entries(&mut self, tree: TreeId, leaves: Vec<Option<LeafId>>) {
        self.tree_entries.entry(tree).or_default().leaves = Some(leaves);
    }

    pub fn get_precalculated_tree_shapes(&self, tree: TreeId) -> Option<&[Option<TileShape>]> {
        self.tree_entries
            .get(&tree)
            .and_then(|e| e.shapes.as_deref())
    }

    pub fn set_precalculated_tree_shapes(&mut self, tree: TreeId, shapes: Vec<Option<TileShape>>) {
        self.tree_entries.entry(tree).or_default().shapes = Some(shapes);
    }

    pub fn clear_precalculated_tree_shapes(&mut self) {
        for entry in self.tree_entries.values_mut() {
            entry.shapes = None;
        }
    }

    pub fn clear_search_tree_entries(&mut self) {
        self.tree_entries.clear();
    }

    pub fn get_autoroute_info(&mut self) -> &mut AutorouteInfo {
        self.autoroute_info
            .get_or_insert_with(|| Box::new(AutorouteInfo::default()))
    }

    pub fn get_autoroute_info_pur(&self) -> Option<&AutorouteInfo> {
        self.autoroute_info.as_deref()
    }

    pub fn get_autoroute_info_pur_mut(&mut self) -> Option<&mut AutorouteInfo> {
        self.autoroute_info.as_deref_mut()
    }

    pub fn clear_autoroute_info(&mut self) {
        self.autoroute_info = None;
    }

    pub fn clear_derived_data(&mut self) {
        self.clear_precalculated_tree_shapes();
        self.autoroute_info = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::NetClassId;
    use crate::rules::ClearanceMatrix;
    use crate::structure::{Layer, LayerStructure};

    fn header(net_nos: Vec<i32>) -> ItemHeader {
        ItemHeader::new(ItemId(7), net_nos, 3, 0, FixedState::Unfixed)
    }

    fn nets_up_to(max: i32) -> Nets {
        let mut nets = Nets::new();
        for i in 1..=max {
            nets.add(format!("net{i}"), 1, false, NetClassId(0));
        }
        nets
    }

    fn layer_structure() -> LayerStructure {
        LayerStructure::new(vec![Layer::new("F.Cu", true), Layer::new("B.Cu", true)])
    }

    fn rules_with_classes(count: usize) -> BoardRules {
        let names: Vec<String> = (0..count).map(|i| format!("class{i}")).collect();
        let matrix = ClearanceMatrix::new(count, &layer_structure(), &names);
        BoardRules::new(layer_structure(), matrix)
    }

    #[test]
    fn assign_net_no_on_an_item_with_no_nets_gives_it_one() {
        let mut h = header(vec![]);
        h.assign_net_no(2, &nets_up_to(3));
        assert_eq!(h.net_nos, vec![2]);
    }

    #[test]
    fn assign_net_no_on_an_item_with_one_net_replaces_it() {
        let mut h = header(vec![1]);
        h.assign_net_no(3, &nets_up_to(3));
        assert_eq!(h.net_nos, vec![3]);
    }

    #[test]
    fn assign_net_no_replaces_the_whole_array() {
        let mut h = header(vec![1, 2]);
        h.assign_net_no(3, &nets_up_to(3));
        assert_eq!(h.net_nos, vec![3]);

        let mut h = header(vec![1, 2, 3]);
        h.assign_net_no(2, &nets_up_to(3));
        assert_eq!(h.net_nos, vec![2]);
    }

    #[test]
    fn assign_net_no_ignores_non_normal_net_numbers() {
        let nets = nets_up_to(3);
        for bad in [0, -1, i32::MIN, Nets::MAX_LEGAL_NET_NUMBER + 1] {
            let mut h = header(vec![1]);
            h.assign_net_no(bad, &nets);
            assert_eq!(h.net_nos, vec![1], "net number {bad} must be ignored");
        }
    }

    #[test]
    fn assign_net_no_ignores_a_net_number_above_the_boards_maximum() {
        let mut h = header(vec![1]);
        h.assign_net_no(4, &nets_up_to(3));
        assert_eq!(h.net_nos, vec![1]);
    }

    #[test]
    fn remove_from_net_removes_and_reports() {
        let mut h = header(vec![1, 2, 3]);
        assert!(h.remove_from_net(2));
        assert_eq!(h.net_nos, vec![1, 3]);
        assert!(!h.remove_from_net(2));
        assert_eq!(h.net_nos, vec![1, 3]);
    }

    #[test]
    fn remove_from_net_removes_the_first_duplicate() {
        let mut h = header(vec![5, 7, 5]);
        assert!(h.remove_from_net(5));
        assert_eq!(h.net_nos, vec![7, 5]);

        let mut h = header(vec![1, 5, 2, 5, 3]);
        assert!(h.remove_from_net(5));
        assert_eq!(h.net_nos, vec![1, 2, 5, 3]);
    }

    #[test]
    fn remove_from_net_on_a_single_net_item_empties_the_array() {
        let mut h = header(vec![9]);
        assert!(h.remove_from_net(9));
        assert!(h.net_nos.is_empty());
    }

    #[test]
    fn contains_net_rejects_non_positive_numbers_even_when_present() {
        let h = header(vec![0, -3, 4]);
        assert!(!h.contains_net(0));
        assert!(!h.contains_net(-3));
        assert!(h.contains_net(4));
    }

    #[test]
    fn shares_net_no_compares_raw_array_elements() {
        assert!(header(vec![0]).shares_net_no(&[0]));
        assert!(header(vec![1, 2]).shares_net_no(&[9, 2]));
        assert!(!header(vec![1, 2]).shares_net_no(&[9]));
        assert!(!header(vec![]).shares_net_no(&[]));
    }

    #[test]
    fn nets_equal_needs_equal_length_and_membership() {
        assert!(header(vec![1, 2]).nets_equal(&[2, 1]));
        assert!(!header(vec![1, 2]).nets_equal(&[1]));
        assert!(header(vec![]).nets_equal(&[]));
        assert!(!header(vec![0]).nets_equal(&[0]));
    }

    #[test]
    fn nets_normal_checks_every_number() {
        assert!(header(vec![1, 2]).nets_normal());
        assert!(header(vec![]).nets_normal());
        assert!(!header(vec![1, 0]).nets_normal());
        assert!(!header(vec![Nets::HIDDEN_NET_NUMBER]).nets_normal());
    }

    #[test]
    fn net_count_and_get_net_number() {
        let h = header(vec![4, 6]);
        assert_eq!(h.net_count(), 2);
        assert_eq!(h.get_net_number(0), 4);
        assert_eq!(h.get_net_number(1), 6);
    }

    #[test]
    #[should_panic]
    fn get_net_number_panics_out_of_range_like_java() {
        header(vec![4]).get_net_number(1);
    }

    #[test]
    fn is_user_fixed_is_true_from_user_fixed_upwards() {
        let cases = [
            (FixedState::Unfixed, false),
            (FixedState::ShoveFixed, false),
            (FixedState::UserFixed, true),
            (FixedState::SystemFixed, true),
        ];
        for (state, expected) in cases {
            let h = ItemHeader::new(ItemId(1), vec![], 0, 0, state);
            assert_eq!(h.is_user_fixed(), expected, "{state:?}");
        }
    }

    #[test]
    fn is_shove_fixed_is_true_from_shove_fixed_upwards() {
        let cases = [
            (FixedState::Unfixed, false),
            (FixedState::ShoveFixed, true),
            (FixedState::UserFixed, true),
            (FixedState::SystemFixed, true),
        ];
        for (state, expected) in cases {
            let h = ItemHeader::new(ItemId(1), vec![], 0, 0, state);
            assert_eq!(h.is_shove_fixed(), expected, "{state:?}");
        }
    }

    #[test]
    fn unfix_spares_system_fixed_items() {
        for state in [
            FixedState::Unfixed,
            FixedState::ShoveFixed,
            FixedState::UserFixed,
        ] {
            let mut h = ItemHeader::new(ItemId(1), vec![], 0, 0, state);
            h.unfix();
            assert_eq!(h.get_fixed_state(), FixedState::Unfixed);
        }
        let mut h = ItemHeader::new(ItemId(1), vec![], 0, 0, FixedState::SystemFixed);
        h.unfix();
        assert_eq!(h.get_fixed_state(), FixedState::SystemFixed);
    }

    #[test]
    fn set_fixed_state_round_trips() {
        let mut h = header(vec![]);
        h.set_fixed_state(FixedState::UserFixed);
        assert_eq!(h.get_fixed_state(), FixedState::UserFixed);
    }

    #[test]
    fn set_clearance_class_ignores_an_out_of_range_index() {
        let rules = rules_with_classes(4);
        let mut h = header(vec![]);
        h.set_clearance_class(2, &rules);
        assert_eq!(h.clearance_class(), 2);
        h.set_clearance_class(4, &rules);
        assert_eq!(h.clearance_class(), 2);
    }

    #[test]
    fn component_id_defaults_and_assigns() {
        let mut h = header(vec![]);
        assert_eq!(h.get_component_id(), 0);
        h.assign_component_id(12);
        assert_eq!(h.get_component_id(), 12);
    }

    #[test]
    fn on_the_board_starts_false() {
        let mut h = header(vec![]);
        assert!(!h.is_on_the_board());
        h.set_on_the_board(true);
        assert!(h.is_on_the_board());
    }

    #[test]
    fn smallest_clearance_starts_at_the_minus_one_sentinel() {
        assert_eq!(header(vec![]).smallest_clearance, -1.0);
    }

    #[test]
    fn id_is_what_the_constructor_was_given() {
        assert_eq!(header(vec![]).id(), ItemId(7));
    }

    #[test]
    fn tree_entries_are_absent_until_set() {
        let h = header(vec![]);
        assert_eq!(h.get_tree_entries(TreeId(0)), None);
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), None);
    }

    #[test]
    fn tree_entries_round_trip_per_tree() {
        let mut h = header(vec![]);
        h.set_tree_entries(TreeId(0), vec![None]);
        h.set_tree_entries(TreeId(1), vec![None, None]);
        assert_eq!(h.get_tree_entries(TreeId(0)).map(<[_]>::len), Some(1));
        assert_eq!(h.get_tree_entries(TreeId(1)).map(<[_]>::len), Some(2));
    }

    #[test]
    fn clear_precalculated_tree_shapes_keeps_the_leaves() {
        let mut h = header(vec![]);
        h.set_tree_entries(TreeId(0), vec![None]);
        h.set_precalculated_tree_shapes(TreeId(0), vec![]);
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), Some(&[][..]));
        h.clear_precalculated_tree_shapes();
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), None);
        assert_eq!(h.get_tree_entries(TreeId(0)), Some(&[None][..]));
    }

    #[test]
    fn an_empty_shape_list_is_not_the_same_as_no_shape_list() {
        let mut h = header(vec![]);
        h.set_precalculated_tree_shapes(TreeId(0), vec![]);
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), Some(&[][..]));
        assert_ne!(h.get_precalculated_tree_shapes(TreeId(0)), None);
    }

    #[test]
    fn clear_search_tree_entries_drops_everything() {
        let mut h = header(vec![]);
        h.set_tree_entries(TreeId(0), vec![None]);
        h.set_precalculated_tree_shapes(TreeId(0), vec![]);
        h.clear_search_tree_entries();
        assert_eq!(h.get_tree_entries(TreeId(0)), None);
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), None);
    }

    #[test]
    fn autoroute_info_is_created_on_demand_and_cleared() {
        let mut h = header(vec![]);
        assert_eq!(h.get_autoroute_info_pur(), None);
        h.get_autoroute_info();
        assert_eq!(h.get_autoroute_info_pur(), Some(&AutorouteInfo::default()));
        h.clear_autoroute_info();
        assert_eq!(h.get_autoroute_info_pur(), None);
    }

    #[test]
    fn clear_derived_data_drops_shapes_and_autoroute_info_but_not_leaves() {
        let mut h = header(vec![]);
        h.set_tree_entries(TreeId(0), vec![None]);
        h.set_precalculated_tree_shapes(TreeId(0), vec![]);
        h.get_autoroute_info();
        h.clear_derived_data();
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), None);
        assert_eq!(h.get_autoroute_info_pur(), None);
        assert_eq!(h.get_tree_entries(TreeId(0)), Some(&[None][..]));
    }
}
