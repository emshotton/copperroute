//! The board library: padstacks, packages, and logical parts, plus the subset of padstacks
//! usable for routing vias.
//!
//! Java: `core/library/BoardLibrary.java`.

use crate::ids::PadstackId;

use super::logical_part::LogicalParts;
use super::package::{Package, Packages};
use super::padstack::{Padstack, Padstacks};

/// The facts [`BoardLibrary::is_used`] needs from the board's item list.
///
/// Java's `BoardLibrary.isUsed` (BoardLibrary.java:129-151) walks `BasicBoard.itemList` looking
/// for a `DrillItem` (a pin or a via) whose padstack is this one. `DrillItem`/`Board` belong to
/// a later task; this trait lets the board-item half of `is_used` be expressed now and
/// satisfied once `Board` exists, the same way `rules::PadstackLookup` lets the rules layer
/// depend on this task's `Padstack` before it existed.
pub trait DrillItemPadstackLookup {
    /// True if some drill item (pin or via) on the board uses `padstack`.
    fn any_drill_item_uses_padstack(&self, padstack: PadstackId) -> bool;
}

/// Port of `BoardLibrary` (`core/library/BoardLibrary.java`): a board library of packages and
/// padstacks.
///
/// Java's `viaPadstacks` field (BoardLibrary.java:24) is `null` until the first
/// [`Self::add_via_padstack`]/[`Self::set_via_padstacks`] call; the port keeps that
/// null-vs-empty distinction as `Option<Vec<PadstackId>>` rather than the task brief's plain
/// `Vec<usize>`; (Java wins over the brief) — see [`Self::remove_via_padstack`] and
/// [`Self::get_mirrored_via_padstack`], which both observably NPE in Java on the `null` state
/// (`docs/java-quirks.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct BoardLibrary {
    /// `BoardLibrary.padstacks` (BoardLibrary.java:15).
    pub padstacks: Padstacks,
    /// `BoardLibrary.packages` (BoardLibrary.java:16).
    pub packages: Packages,
    /// `BoardLibrary.logicalParts` (BoardLibrary.java:19): gate-swap/pin-swap info for the
    /// Specctra DSN format.
    pub logical_parts: LogicalParts,
    /// `BoardLibrary.viaPadstacks` (BoardLibrary.java:24, a `List<Padstack>`, `null` until
    /// first used): the subset of `padstacks` usable in routing for inserting vias.
    via_padstacks: Option<Vec<PadstackId>>,
}

impl BoardLibrary {
    /// Port of the `BoardLibrary(Padstacks, Packages)` constructor (BoardLibrary.java:27-31).
    pub fn new(padstacks: Padstacks, packages: Packages) -> BoardLibrary {
        BoardLibrary {
            padstacks,
            packages,
            logical_parts: LogicalParts::new(),
            via_padstacks: None,
        }
    }

    /// Port of `BoardLibrary.viaPadstackCount` (BoardLibrary.java:36-42).
    pub fn via_padstack_count(&self) -> usize {
        self.via_padstacks.as_ref().map_or(0, Vec::len)
    }

    /// Port of `BoardLibrary.getViaPadstack(int)` (BoardLibrary.java:44-50): the via padstack at
    /// position `no` in the via-padstack list (0-based — a list position, not a [`PadstackId`]),
    /// or `None` when unset or out of range.
    pub fn get_via_padstack(&self, no: usize) -> Option<PadstackId> {
        self.via_padstacks.as_ref()?.get(no).copied()
    }

    /// Port of `BoardLibrary.getViaPadstack(String)` (BoardLibrary.java:53-63): the via padstack
    /// named `name` (`equals`, case-sensitive — unlike `Padstacks.get(String)`), or `None`.
    pub fn get_via_padstack_by_name(&self, name: &str) -> Option<PadstackId> {
        let via_padstacks = self.via_padstacks.as_ref()?;
        via_padstacks
            .iter()
            .copied()
            .find(|id| self.padstacks.get(*id).is_some_and(|p| p.name == name))
    }

    /// Port of `BoardLibrary.getViaPadstacks` (BoardLibrary.java:66-75): every via padstack
    /// usable for routing, in list order; empty when unset.
    pub fn get_via_padstacks(&self) -> Vec<PadstackId> {
        self.via_padstacks.clone().unwrap_or_default()
    }

    /// Port of `BoardLibrary.setViaPadstacks` (BoardLibrary.java:81-84): replaces the entire
    /// via-padstack list.
    pub fn set_via_padstacks(&mut self, padstacks: Vec<PadstackId>) {
        self.via_padstacks = Some(padstacks);
    }

    /// Port of `BoardLibrary.addViaPadstack` (BoardLibrary.java:90-101): appends `padstack` to
    /// the via-padstack list, lazily creating the list if this is the first addition. Returns
    /// `false` (unchanged) if a via padstack with the same name is already present.
    ///
    /// Java reads `padstack.name` directly off an object reference (BoardLibrary.java:91), so a
    /// `padstack` that does not resolve cannot occur there. This port's `padstack` is only an
    /// id, which *can* fail to resolve (an artifact of the index-based redesign, not a case
    /// Java has); a `debug_assert!` flags that as a caller bug rather than silently skipping the
    /// duplicate-name check the way an unresolved id otherwise would.
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

    /// Port of `BoardLibrary.removeViaPadstack` (BoardLibrary.java:104-106): removes the first
    /// via-padstack list entry identical to `padstack` (Java's identity-based
    /// `List.remove(Object)` — `Padstack` has no `equals` override, so a [`PadstackId`]
    /// comparison is the exact analogue), returning whether it was found and removed.
    ///
    /// not ported: the `BasicBoard board` parameter (BoardLibrary.java:104) — unused in the
    /// method body.
    ///
    /// Java bug: the real method does not null-check `viaPadstacks` before calling
    /// `.remove` on it, so calling this before any `addViaPadstack`/`setViaPadstacks` threw a
    /// `NullPointerException`. See `docs/java-quirks.md`.
    ///
    /// fixed: T6 (#43) — the register suggests initialising `viaPadstacks` to an empty `Vector` in
    /// both constructors, and this is that fix expressed where the port keeps the null-ness. The
    /// field stays `Option`, because `getViaPadstackCount` (`:97`) and `getViaPadstack` (`:100`)
    /// already read it as "null means none" and Plan 6 needs that distinction; what changes is
    /// that the two methods which dereferenced it unguarded now read it the same way. An absent
    /// list removes nothing and answers `false`, which is exactly what an empty `Vector` would.
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

    /// Port of `BoardLibrary.getMirroredViaPadstack` (BoardLibrary.java:112-126): the via
    /// padstack mirrored to the back side of the board, or `None` if no such via padstack
    /// exists. A via already spanning the whole board (`fromLayer() == 0`,
    /// `toLayer() == layerCount - 1`) mirrors to itself.
    ///
    /// Java bug: when `via_padstack` does not already span the whole board, Java iterates the
    /// `viaPadstacks` field directly with no null check, so this also `NullPointerException`ed on
    /// an unset via-padstack list. See `docs/java-quirks.md`.
    ///
    /// fixed: T6 (#43) — the same guard as [`Self::remove_via_padstack`]'s. It is the same
    /// register row and the same field, so fixing one and leaving the other would have left the
    /// row half true; an absent list has no mirrored via in it, which is `None`.
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

    /// Port of `BoardLibrary.isUsed` (BoardLibrary.java:129-151): true if `padstack` is used by
    /// some board drill item (via `board_items`, see [`DrillItemPadstackLookup`]) or by a pin in
    /// any package.
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

    /// Not a Java method: convenience delegating to [`Padstacks::get`], per the task brief's
    /// `get_padstack` shorthand.
    pub fn get_padstack(&self, id: PadstackId) -> Option<&Padstack> {
        self.padstacks.get(id)
    }

    /// Not a Java method: convenience delegating to [`Packages::get`], per the task brief's
    /// `get_package` shorthand. Panics out of range, matching `Packages::get`.
    pub fn get_package(&self, no: usize) -> &Package {
        self.packages.get(no)
    }
}

impl Default for BoardLibrary {
    /// Port of the no-arg `BoardLibrary()` constructor (BoardLibrary.java:34): `padstacks` and
    /// `packages` are `null` in Java until a caller assigns them directly (e.g.
    /// `KiCadJsonReader.java:337-338`); the port cannot represent that (the brief's interface
    /// has no `Option` wrapper), so `Default` builds an empty-but-valid `Padstacks`/`Packages`
    /// pair instead.
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
        // BoardLibrary.addViaPadstack (BoardLibrary.java:90-101) appends; removeViaPadstack
        // (BoardLibrary.java:104-106) is `List.remove(Object)`, which shifts later entries down
        // (a `Vector` is not a set: order is insertion order, minus the removed element).
        let (mut library, ids) = library_with_padstacks(4, 3);
        assert_eq!(library.via_padstack_count(), 0);

        assert!(library.add_via_padstack(ids[0]));
        assert!(library.add_via_padstack(ids[1]));
        assert!(library.add_via_padstack(ids[2]));
        assert_eq!(library.via_padstack_count(), 3);
        assert_eq!(library.get_via_padstacks(), vec![ids[0], ids[1], ids[2]]);

        // Re-adding an existing name is a no-op (BoardLibrary.java:91-93).
        assert!(!library.add_via_padstack(ids[0]));
        assert_eq!(library.via_padstack_count(), 3);

        // Removing the middle entry shifts the tail down; order of the rest is preserved.
        assert!(library.remove_via_padstack(ids[1]));
        assert_eq!(library.get_via_padstacks(), vec![ids[0], ids[2]]);
        assert_eq!(library.get_via_padstack(0), Some(ids[0]));
        assert_eq!(library.get_via_padstack(1), Some(ids[2]));

        // Removing something already gone answers false and changes nothing.
        assert!(!library.remove_via_padstack(ids[1]));
        assert_eq!(library.via_padstack_count(), 2);
    }

    #[test]
    fn get_via_padstack_by_name_is_case_sensitive() {
        // BoardLibrary.java:53-63 uses `.equals`, not `equalsIgnoreCase` (unlike
        // Padstacks.get(String)).
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

    /// Quirk #43, inverted. `viaPadstacks` is null until the first `addViaPadstack`, and both
    /// `removeViaPadstack` and `getMirroredViaPadstack` dereferenced it with no check.
    #[test]
    fn the_via_padstack_list_is_guarded_before_any_via_padstack_was_ever_added() {
        let (mut library, ids) = library_with_padstacks(2, 1);
        assert_eq!(
            library.via_padstack_count(),
            0,
            "the list is still Java's null"
        );

        // Removing from a list that does not exist removes nothing — what an empty `Vector`,
        // which is the register's suggested fix, would have answered.
        assert!(!library.remove_via_padstack(ids[0]));
        // The same row's other half: iterating a list that does not exist finds no mirror.
        assert_eq!(library.get_mirrored_via_padstack(ids[0]), None);

        // And the guard did not change the populated answers.
        library.add_via_padstack(ids[0]);
        assert_eq!(library.via_padstack_count(), 1);
        assert!(library.remove_via_padstack(ids[0]));
        assert!(!library.remove_via_padstack(ids[0]));
    }

    #[test]
    fn get_mirrored_via_padstack_returns_self_when_spanning_the_whole_board() {
        // BoardLibrary.java:113-116.
        let (library, ids) = library_with_padstacks(1, 1);
        assert_eq!(library.get_mirrored_via_padstack(ids[0]), Some(ids[0]));
    }

    #[test]
    fn get_mirrored_via_padstack_finds_the_layer_reversed_via() {
        // BoardLibrary.java:117-125: a 4-layer board, via on layers 0-1 mirrors to layers 2-3.
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
        // BoardLibrary.java:142-149.
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
        // Java cannot hit this: `padstack.name` (BoardLibrary.java:91) is a direct field read
        // off a live object reference, so a "does not resolve" `Padstack` cannot exist there.
        // This port's id-based redesign can be handed a stale/invalid `PadstackId`, which the
        // `debug_assert!` in `add_via_padstack` flags as a caller bug.
        let (mut library, _ids) = library_with_padstacks(2, 0);
        library.add_via_padstack(PadstackId(99));
    }
}
