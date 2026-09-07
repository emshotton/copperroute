use crate::ids::TreeId;
use crate::items::{Item, ItemCtx, PolylineTrace};
use crate::rules::BoardRules;
use crate::structure::AngleRestriction;

use super::ShapeSearchTree;

#[derive(Debug, Clone, PartialEq)]
pub struct SearchTreeManager {
    default_tree: ShapeSearchTree,
    compensated: Vec<ShapeSearchTree>,
    clearance_compensation_used: bool,
    next_entry_id: u64,
    next_tree_id: u32,
}

impl SearchTreeManager {
    pub fn new() -> SearchTreeManager {
        SearchTreeManager {
            default_tree: ShapeSearchTree::new(TreeId(0), AngleRestriction::None, 0),
            compensated: Vec::new(),
            clearance_compensation_used: false,
            next_entry_id: 0,
            next_tree_id: 1,
        }
    }

    pub fn get_default_tree(&self) -> &ShapeSearchTree {
        &self.default_tree
    }

    pub fn get_default_tree_mut(&mut self) -> &mut ShapeSearchTree {
        &mut self.default_tree
    }

    pub fn trees_mut(&mut self) -> impl Iterator<Item = &mut ShapeSearchTree> {
        std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut())
    }

    pub fn trees(&self) -> impl Iterator<Item = &ShapeSearchTree> {
        std::iter::once(&self.default_tree).chain(self.compensated.iter())
    }

    pub fn entry_counter_mut(&mut self) -> &mut u64 {
        &mut self.next_entry_id
    }

    pub fn default_tree_and_counter_mut(&mut self) -> (&ShapeSearchTree, &mut u64) {
        (&self.default_tree, &mut self.next_entry_id)
    }

    pub fn entry_counter(&self) -> u64 {
        self.next_entry_id
    }

    pub fn insert(&mut self, item: &mut Item, ctx: &ItemCtx<'_>) {
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.insert_item(item, ctx);
        }
        item.set_on_the_board(true);
    }

    pub fn remove(&mut self, item: &mut Item) {
        if !item.is_on_the_board() {
            return;
        }
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.remove_item(item);
        }
        item.clear_tree_entries();
        item.set_on_the_board(false);
    }

    pub fn validate_entries(&self, item: &Item) -> bool {
        let mut result = true;
        for tree in self.trees() {
            if !tree.validate_entries(item) {
                result = false;
            }
        }
        result
    }

    pub fn is_clearance_compensation_used(&self) -> bool {
        self.clearance_compensation_used
    }

    pub fn set_clearance_compensation_used(
        &mut self,
        value: bool,
        items: &mut [&mut Item],
        ctx: &ItemCtx<'_>,
    ) {
        if self.clearance_compensation_used == value {
            return;
        }
        self.clearance_compensation_used = value;
        self.remove_all_board_items(items);
        self.compensated.clear();
        let compensated_clearance_class = usize::from(value);
        self.default_tree = ShapeSearchTree::new(
            TreeId(self.next_tree_id),
            AngleRestriction::None,
            compensated_clearance_class,
        );
        self.next_tree_id += 1;
        self.insert_all_board_items(items, ctx);
    }

    pub fn clearance_value_changed(&mut self, items: &mut [&mut Item], ctx: &ItemCtx<'_>) {
        let default_class = self.default_tree.compensated_clearance_class();
        self.compensated
            .retain(|tree| tree.compensated_clearance_class() == default_class);
        if self.clearance_compensation_used {
            self.remove_all_board_items(items);
            self.insert_all_board_items(items, ctx);
        }
    }

    pub fn clearance_class_removed(&mut self, no: usize) {
        if no == self.default_tree.compensated_clearance_class() {
            return;
        }
        self.compensated
            .retain(|tree| tree.compensated_clearance_class() != no);
    }

    pub fn reset_compensated_trees(&mut self) {
        self.compensated.clear();
    }

    pub fn get_autoroute_tree(
        &mut self,
        clearance_class_index: usize,
        items: &mut [&mut Item],
        ctx: &ItemCtx<'_>,
    ) -> &mut ShapeSearchTree {
        if self.default_tree.compensated_clearance_class() == clearance_class_index {
            return &mut self.default_tree;
        }
        if let Some(index) = self
            .compensated
            .iter()
            .position(|tree| tree.compensated_clearance_class() == clearance_class_index)
        {
            return &mut self.compensated[index];
        }

        let mut tree = ShapeSearchTree::new(
            TreeId(self.next_tree_id),
            ctx.rules.trace_angle_restriction,
            clearance_class_index,
        );
        self.next_tree_id += 1;
        for item in items.iter_mut() {
            if p7t14b_fp_ledger() {
                eprintln!("TINS8 id={}", item.id().0);
            }
            tree.insert_item(item, ctx);
        }
        self.compensated.push(tree);
        self.compensated.last_mut().expect("just pushed")
    }

    pub fn reinsert_tree_shapes(&mut self, items: &mut [&mut Item], ctx: &ItemCtx<'_>) {
        self.remove_all_board_items(items);
        self.insert_all_board_items(items, ctx);
    }

    fn remove_all_board_items(&mut self, items: &mut [&mut Item]) {
        for item in items.iter_mut() {
            self.remove(item);
        }
    }

    fn insert_all_board_items(&mut self, items: &mut [&mut Item], ctx: &ItemCtx<'_>) {
        for item in items.iter_mut() {
            item.clear_derived_data();
            self.insert(item, ctx);
        }
    }

    pub fn merge_entries_in_front(
        &mut self,
        from_trace: &mut PolylineTrace,
        to_trace: &mut PolylineTrace,
        joined_polyline: &copper_geometry::Polyline,
        from_entry_no: usize,
        to_entry_no: usize,
        rules: &BoardRules,
    ) {
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.merge_entries_in_front(
                from_trace,
                to_trace,
                joined_polyline,
                from_entry_no,
                to_entry_no,
                rules,
            );
        }
    }

    pub fn merge_entries_at_end(
        &mut self,
        from_trace: &mut PolylineTrace,
        to_trace: &mut PolylineTrace,
        joined_polyline: &copper_geometry::Polyline,
        from_entry_no: usize,
        to_entry_no: usize,
        rules: &BoardRules,
    ) {
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.merge_entries_at_end(
                from_trace,
                to_trace,
                joined_polyline,
                from_entry_no,
                to_entry_no,
                rules,
            );
        }
    }

    pub fn change_entries(
        &mut self,
        trace: &mut PolylineTrace,
        new_polyline: &copper_geometry::Polyline,
        keep_at_start_count: usize,
        keep_at_end_count: usize,
        rules: &BoardRules,
    ) {
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.change_entries(
                trace,
                new_polyline,
                keep_at_start_count,
                keep_at_end_count,
                rules,
            );
        }
    }

    pub fn reuse_entries_after_cutout(
        &mut self,
        from_trace: &mut PolylineTrace,
        start_piece: &mut PolylineTrace,
        end_piece: &mut PolylineTrace,
        ctx: &ItemCtx<'_>,
    ) {
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.reuse_entries_after_cutout(from_trace, start_piece, end_piece, ctx);
        }
    }
}

impl Default for SearchTreeManager {
    fn default() -> Self {
        Self::new()
    }
}

fn p7t14b_fp_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T14B_FP").is_some());
    *ON
}
