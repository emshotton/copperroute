use crate::ids::PadstackId;

use super::logical_part::LogicalParts;
use super::package::{Package, Packages};
use super::padstack::{Padstack, Padstacks};

pub trait DrillItemPadstackLookup {
    fn any_drill_item_uses_padstack(&self, padstack: PadstackId) -> bool;
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoardLibrary {
    pub padstacks: Padstacks,
    pub packages: Packages,
    pub logical_parts: LogicalParts,
    via_padstacks: Option<Vec<PadstackId>>,
}

impl BoardLibrary {
    pub fn new(padstacks: Padstacks, packages: Packages) -> BoardLibrary {
        BoardLibrary {
            padstacks,
            packages,
            logical_parts: LogicalParts::new(),
            via_padstacks: None,
        }
    }

    pub fn via_padstack_count(&self) -> usize {
        self.via_padstacks.as_ref().map_or(0, Vec::len)
    }

    pub fn get_via_padstack(&self, no: usize) -> Option<PadstackId> {
        self.via_padstacks.as_ref()?.get(no).copied()
    }

    pub fn get_via_padstack_by_name(&self, name: &str) -> Option<PadstackId> {
        let via_padstacks = self.via_padstacks.as_ref()?;
        via_padstacks
            .iter()
            .copied()
            .find(|id| self.padstacks.get(*id).is_some_and(|p| p.name == name))
    }

    pub fn get_via_padstacks(&self) -> Vec<PadstackId> {
        self.via_padstacks.clone().unwrap_or_default()
    }

    pub fn set_via_padstacks(&mut self, padstacks: Vec<PadstackId>) {
        self.via_padstacks = Some(padstacks);
    }

    pub fn add_via_padstack(&mut self, padstack: PadstackId) -> bool {
        let resolved = self.padstacks.get(padstack);
        debug_assert!(
            resolved.is_some(),
            "BoardLibrary.add_via_padstack: PadstackId must resolve in self.padstacks (BoardLibrary.java:91 assumes a live Padstack reference)"
        );
        if let Some(name) = resolved.map(|p| p.name.clone())
            && self.get_via_padstack_by_name(&name).is_some()
        {
            return false;
        }
        self.via_padstacks
            .get_or_insert_with(Vec::new)
            .push(padstack);
        true
    }

    pub fn remove_via_padstack(&mut self, padstack: PadstackId) -> bool {
        let Some(list) = self.via_padstacks.as_mut() else {
            return false;
        };
        match list.iter().position(|id| *id == padstack) {
            Some(index) => {
                list.remove(index);
                true
            }
            None => false,
        }
    }

    pub fn get_mirrored_via_padstack(&self, via_padstack: PadstackId) -> Option<PadstackId> {
        let layer_count = self.padstacks.board_layer_structure.layers.len() as i32;
        let via = self
            .padstacks
            .get(via_padstack)
            .expect("get_mirrored_via_padstack: PadstackId must be valid");
        if via.from_layer() == 0 && via.to_layer() == layer_count - 1 {
            return Some(via_padstack);
        }
        let new_from_layer = layer_count - via.to_layer() - 1;
        let new_to_layer = layer_count - via.from_layer() - 1;
        let list = self.via_padstacks.as_ref()?;
        list.iter().copied().find(|id| {
            let candidate = self
                .padstacks
                .get(*id)
                .expect("get_mirrored_via_padstack: PadstackId must be valid");
            candidate.from_layer() == new_from_layer && candidate.to_layer() == new_to_layer
        })
    }

    pub fn is_used(
        &self,
        padstack: PadstackId,
        board_items: &impl DrillItemPadstackLookup,
    ) -> bool {
        if board_items.any_drill_item_uses_padstack(padstack) {
            return true;
        }
        for no in 1..=self.packages.count() {
            let package = self.packages.get(no);
            for pin_index in 0..package.pin_count() as i32 {
                if let Some(pin) = package.get_pin(pin_index)
                    && pin.padstack_no == padstack
                {
                    return true;
                }
            }
        }
        false
    }

    pub fn get_padstack(&self, id: PadstackId) -> Option<&Padstack> {
        self.padstacks.get(id)
    }

    pub fn get_package(&self, no: usize) -> &Package {
        self.packages.get(no)
    }
}

impl Default for BoardLibrary {
    fn default() -> Self {
        BoardLibrary::new(Padstacks::default(), Packages::default())
    }
}

#[cfg(test)]
mod tests {
    use super::super::package::PackagePin;
    use super::*;
    use crate::structure::{Layer, LayerStructure};
    use fr_geometry::{IntBox, Shape, TileShape};

    fn layer_structure(count: usize) -> LayerStructure {
        LayerStructure::new(
            (0..count)
                .map(|i| Layer::new(format!("L{i}"), true))
                .collect(),
        )
    }

    fn box_shape(x0: i32, y0: i32, x1: i32, y1: i32) -> Shape {
        Shape::Tile(TileShape::Box(IntBox::from_coords(x0, y0, x1, y1)))
    }

    struct NoDrillItems;
    impl DrillItemPadstackLookup for NoDrillItems {
        fn any_drill_item_uses_padstack(&self, _padstack: PadstackId) -> bool {
            false
        }
    }

    fn library_with_padstacks(
        layer_count: usize,
        via_count: usize,
    ) -> (BoardLibrary, Vec<PadstackId>) {
        let mut padstacks = Padstacks::new(layer_structure(layer_count));
        let mut ids = Vec::new();
        for i in 0..via_count {
            let id = padstacks.add_layer_range(box_shape(0, 0, 10, 10), i as i32, i as i32);
            ids.push(id);
        }
        (BoardLibrary::new(padstacks, Packages::new()), ids)
    }

    #[test]
    fn via_padstacks_order_after_add_and_remove_matches_java_vector_semantics() {
        let (mut library, ids) = library_with_padstacks(4, 3);
        assert_eq!(library.via_padstack_count(), 0);

        assert!(library.add_via_padstack(ids[0]));
        assert!(library.add_via_padstack(ids[1]));
        assert!(library.add_via_padstack(ids[2]));
        assert_eq!(library.via_padstack_count(), 3);
        assert_eq!(library.get_via_padstacks(), vec![ids[0], ids[1], ids[2]]);

        assert!(!library.add_via_padstack(ids[0]));
        assert_eq!(library.via_padstack_count(), 3);

        assert!(library.remove_via_padstack(ids[1]));
        assert_eq!(library.get_via_padstacks(), vec![ids[0], ids[2]]);
        assert_eq!(library.get_via_padstack(0), Some(ids[0]));
        assert_eq!(library.get_via_padstack(1), Some(ids[2]));

        assert!(!library.remove_via_padstack(ids[1]));
        assert_eq!(library.via_padstack_count(), 2);
    }

    #[test]
    fn get_via_padstack_by_name_is_case_sensitive() {
        let mut padstacks = Padstacks::new(layer_structure(2));
        let id = padstacks.add(
            "Via1",
            vec![Some(box_shape(0, 0, 10, 10)), None],
            true,
            false,
        );
        let mut library = BoardLibrary::new(padstacks, Packages::new());
        library.add_via_padstack(id);
        assert_eq!(library.get_via_padstack_by_name("Via1"), Some(id));
        assert_eq!(library.get_via_padstack_by_name("VIA1"), None);
    }

    #[test]
    fn the_via_padstack_list_is_guarded_before_any_via_padstack_was_ever_added() {
        let (mut library, ids) = library_with_padstacks(2, 1);
        assert_eq!(
            library.via_padstack_count(),
            0,
            "the list is still Java's null"
        );

        assert!(!library.remove_via_padstack(ids[0]));
        assert_eq!(library.get_mirrored_via_padstack(ids[0]), None);

        library.add_via_padstack(ids[0]);
        assert_eq!(library.via_padstack_count(), 1);
        assert!(library.remove_via_padstack(ids[0]));
        assert!(!library.remove_via_padstack(ids[0]));
    }

    #[test]
    fn get_mirrored_via_padstack_returns_self_when_spanning_the_whole_board() {
        let (library, ids) = library_with_padstacks(1, 1);
        assert_eq!(library.get_mirrored_via_padstack(ids[0]), Some(ids[0]));
    }

    #[test]
    fn get_mirrored_via_padstack_finds_the_layer_reversed_via() {
        let mut padstacks = Padstacks::new(layer_structure(4));
        let front = padstacks.add_layer_range(box_shape(0, 0, 10, 10), 0, 1);
        let back = padstacks.add_layer_range(box_shape(0, 0, 10, 10), 2, 3);
        let mut library = BoardLibrary::new(padstacks, Packages::new());
        library.add_via_padstack(front);
        library.add_via_padstack(back);
        assert_eq!(library.get_mirrored_via_padstack(front), Some(back));
        assert_eq!(library.get_mirrored_via_padstack(back), Some(front));
    }

    #[test]
    fn get_mirrored_via_padstack_none_when_no_match() {
        let mut padstacks = Padstacks::new(layer_structure(4));
        let front = padstacks.add_layer_range(box_shape(0, 0, 10, 10), 0, 1);
        let mut library = BoardLibrary::new(padstacks, Packages::new());
        library.add_via_padstack(front);
        assert_eq!(library.get_mirrored_via_padstack(front), None);
    }

    #[test]
    fn is_used_checks_package_pins_when_no_drill_item_matches() {
        let mut padstacks = Padstacks::new(layer_structure(2));
        let pad = padstacks.add_layer_range(box_shape(0, 0, 10, 10), 0, 0);
        let other = padstacks.add_layer_range(box_shape(0, 0, 10, 10), 1, 1);
        let mut packages = Packages::new();
        packages.add_pins(vec![PackagePin::new(
            "1",
            pad,
            fr_geometry::Vector::from(fr_geometry::IntVector::new(0, 0)),
            0.0,
        )]);
        let library = BoardLibrary::new(padstacks, packages);
        assert!(library.is_used(pad, &NoDrillItems));
        assert!(!library.is_used(other, &NoDrillItems));
    }

    #[test]
    fn is_used_true_when_a_drill_item_matches() {
        struct AlwaysUsed;
        impl DrillItemPadstackLookup for AlwaysUsed {
            fn any_drill_item_uses_padstack(&self, _padstack: PadstackId) -> bool {
                true
            }
        }
        let (library, ids) = library_with_padstacks(2, 1);
        assert!(library.is_used(ids[0], &AlwaysUsed));
    }

    #[test]
    fn default_board_library_is_empty() {
        let library = BoardLibrary::default();
        assert_eq!(library.via_padstack_count(), 0);
        assert_eq!(library.padstacks.count(), 0);
        assert_eq!(library.packages.count(), 0);
        assert_eq!(library.logical_parts.count(), 0);
    }

    #[test]
    fn get_padstack_and_get_package_delegate() {
        let (library, ids) = library_with_padstacks(2, 1);
        assert!(library.get_padstack(ids[0]).is_some());
        assert!(library.get_padstack(PadstackId(99)).is_none());
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn add_via_padstack_debug_asserts_the_id_resolves() {
        let (mut library, _ids) = library_with_padstacks(2, 0);
        library.add_via_padstack(PadstackId(99));
    }
}
