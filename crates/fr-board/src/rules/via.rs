//! Via definitions and via rules.
//!
//! Java: `rules/ViaInfo.java`, `rules/ViaInfos.java`, `rules/ViaRule.java`.

use std::cmp::Ordering;
use std::fmt;

use crate::ids::{PadstackId, ViaInfoId};

use super::PadstackLookup;

/// Port of `ViaInfo` (`rules/ViaInfo.java`): a padstack + clearance class + attach-to-SMD
/// setting, usable for routing.
///
/// not ported: `ViaInfo.boardRules` (ViaInfo.java:15) — the back-pointer is read only by
/// `ViaInfo.printInfo` (ViaInfo.java:95-99), to name the clearance class in a GUI info panel.
///
/// not ported: `ViaInfo.printInfo` (ViaInfo.java:86-107) — `ItemInfoPrinter.Printable`, GUI
/// only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViaInfo {
    /// `ViaInfo.name` (ViaInfo.java:16).
    name: String,
    /// `ViaInfo.padstack` (ViaInfo.java:17), as an index into the board library's padstacks.
    padstack: PadstackId,
    /// `ViaInfo.clearanceClassIndex` (ViaInfo.java:18).
    clearance_class_index: usize,
    /// `ViaInfo.attachSmdAllowed` (ViaInfo.java:19).
    attach_smd_allowed: bool,
}

impl ViaInfo {
    /// Port of the `ViaInfo(String, Padstack, int, boolean, BoardRules)` constructor
    /// (ViaInfo.java:22-33), minus the `BoardRules` back-pointer.
    pub fn new(
        name: impl Into<String>,
        padstack: PadstackId,
        clearance_class_index: usize,
        drill_to_smd_allowed: bool,
    ) -> ViaInfo {
        ViaInfo {
            name: name.into(),
            padstack,
            clearance_class_index,
            attach_smd_allowed: drill_to_smd_allowed,
        }
    }

    /// Port of `ViaInfo.getName` (ViaInfo.java:36-38).
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Port of `ViaInfo.setName` (ViaInfo.java:41-43).
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Port of `ViaInfo.getPadstack` (ViaInfo.java:51-53).
    pub fn get_padstack(&self) -> PadstackId {
        self.padstack
    }

    /// Port of `ViaInfo.setPadstack` (ViaInfo.java:56-58).
    pub fn set_padstack(&mut self, padstack: PadstackId) {
        self.padstack = padstack;
    }

    /// Port of `ViaInfo.getClearanceClassIndex` (ViaInfo.java:61-63).
    pub fn get_clearance_class_index(&self) -> usize {
        self.clearance_class_index
    }

    /// Port of `ViaInfo.setClearanceClassIndex` (ViaInfo.java:66-68).
    pub fn set_clearance_class_index(&mut self, clearance_class_index: usize) {
        self.clearance_class_index = clearance_class_index;
    }

    /// Port of `ViaInfo.attachSmdAllowed` (ViaInfo.java:71-73).
    pub fn attach_smd_allowed(&self) -> bool {
        self.attach_smd_allowed
    }

    /// Port of `ViaInfo.setAttachSmdAllowed` (ViaInfo.java:76-78).
    pub fn set_attach_smd_allowed(&mut self, attach_smd_allowed: bool) {
        self.attach_smd_allowed = attach_smd_allowed;
    }

    /// Port of `ViaInfo.compareTo` (ViaInfo.java:81-83): `name.compareTo(other.name)` — case
    /// *sensitive*, unlike `Net.compareTo`.
    ///
    /// Not an `Ord` impl: two via infos with the same name but different padstacks compare Equal
    /// here while `PartialEq` is structural, which would break the `Ord`/`Eq` contract. Java's
    /// `String.compareTo` compares UTF-16 code units, so this uses `str`'s byte-wise `cmp` —
    /// the two orders agree on all of ASCII and differ only where a code point above U+FFFF is
    /// compared against one in U+E000..U+FFFF, which cannot occur in a DSN identifier.
    pub fn compare_to(&self, other: &ViaInfo) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl fmt::Display for ViaInfo {
    // renamed: ViaInfo.toString -> Display::fmt (ViaInfo.java:46-48 returns `name`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Port of `ViaInfos` (`rules/ViaInfos.java`): the list of via definitions usable for routing.
///
/// not ported: `ViaInfos.printInfo` (ViaInfos.java:67-87) — `ItemInfoPrinter.Printable`, GUI
/// only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViaInfos {
    /// `ViaInfos.list` (ViaInfos.java:16).
    list: Vec<ViaInfo>,
}

impl ViaInfos {
    /// An empty via-info list (Java uses the field initialiser at ViaInfos.java:16).
    pub fn new() -> ViaInfos {
        ViaInfos::default()
    }

    /// Port of `ViaInfos.add` (ViaInfos.java:22-28): appends `via_info` unless its name is
    /// already taken, in which case it returns false and changes nothing.
    pub fn add(&mut self, via_info: ViaInfo) -> bool {
        if self.name_exists(via_info.get_name()) {
            return false;
        }
        self.list.push(via_info);
        true
    }

    /// Port of `ViaInfos.count` (ViaInfos.java:31-33).
    pub fn count(&self) -> usize {
        self.list.len()
    }

    /// Port of `ViaInfos.get(int)` (ViaInfos.java:36-39). Java asserts the index is in range and
    /// would then throw; the port panics on the same out-of-range index.
    pub fn get(&self, index: ViaInfoId) -> &ViaInfo {
        &self.list[index.0]
    }

    /// Mutable counterpart of [`Self::get`]; Java's `get` already hands back a mutable object.
    pub fn get_mut(&mut self, index: ViaInfoId) -> &mut ViaInfo {
        &mut self.list[index.0]
    }

    /// Port of `ViaInfos.get(String)` (ViaInfos.java:42-49): the via with exactly this name
    /// (`equals`, case sensitive), or `None`.
    pub fn get_by_name(&self, name: &str) -> Option<&ViaInfo> {
        self.get_no(name).map(|id| self.get(id))
    }

    /// The index form of [`Self::get_by_name`]. Not a Java method: Java's callers keep the
    /// object reference, while this port needs the [`ViaInfoId`].
    pub fn get_no(&self, name: &str) -> Option<ViaInfoId> {
        self.list.iter().position(|v| v.name == name).map(ViaInfoId)
    }

    /// Every via info, in index order. Not a Java method — Java iterates `list` directly
    /// (ViaInfos.java:43,53,74).
    pub fn iter(&self) -> std::slice::Iter<'_, ViaInfo> {
        self.list.iter()
    }

    /// Port of `ViaInfos.nameExists` (ViaInfos.java:52-59).
    pub fn name_exists(&self, name: &str) -> bool {
        self.get_no(name).is_some()
    }

    /// Port of `ViaInfos.remove` (ViaInfos.java:62-64): removes the via with the given index,
    /// returning false (and changing nothing) if the index is out of range — Java's `false` for
    /// "was not contained in the list".
    ///
    /// **Renumbering hazard.** Java's `ViaRule`s hold `ViaInfo` *object references*
    /// (ViaRule.java:21), so a removal here leaves them pointing at the removed object. This
    /// port's `ViaRule`s hold indices, so a removal shifts every later [`ViaInfoId`] and silently
    /// re-points them at the wrong via. The only non-GUI caller is
    /// `io/specctra/RulesReader.java:340-350`, which removes a via only to add a replacement
    /// with the same name; whoever ports that file (Plan 3) must renumber, or replace in place.
    /// (`gui/windows/routing/WindowEditVias.java:197` also calls it, but the GUI is out of scope
    /// for this port.)
    /// See the `docs/java-quirks.md` obligation-register row "`ViaInfoId` renumbering across
    /// `ViaInfos.remove`".
    pub fn remove(&mut self, index: ViaInfoId) -> bool {
        if index.0 >= self.list.len() {
            return false;
        }
        self.list.remove(index.0);
        true
    }
}

/// Port of `ViaRule` (`rules/ViaRule.java`): an ordered list of vias usable for routing; vias
/// near the front are preferred.
///
/// not ported: `ViaRule.swap` (ViaRule.java:92-104) — its only caller is
/// `gui/windows/routing/WindowViaRule.java:134`; no `board`/`autoroute`/`drc`/`io` code calls
/// it.
///
/// not ported: `ViaRule.printInfo` (ViaRule.java:107-129) — `ItemInfoPrinter.Printable`, GUI
/// only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViaRule {
    /// `ViaRule.name` (ViaRule.java:20); public and final in Java too.
    pub name: String,
    /// `ViaRule.list` (ViaRule.java:21), as indices into [`ViaInfos`].
    vias: Vec<ViaInfoId>,
}

impl ViaRule {
    /// Port of the `ViaRule(String)` constructor (ViaRule.java:24-26).
    pub fn new(name: impl Into<String>) -> ViaRule {
        ViaRule {
            name: name.into(),
            vias: Vec::new(),
        }
    }

    /// Port of `ViaRule.EMPTY` (ViaRule.java:18): the shared, never-modified empty rule named
    /// `"empty"`.
    ///
    /// Java exposes it as a `static final` singleton whose identity matters nowhere (its one
    /// user, `gui/interactive/MakeSpaceState.java:42`, only reads it); a `String` field cannot be
    /// built in a `const`, so the port hands out a fresh equal value instead.
    pub fn empty() -> ViaRule {
        ViaRule::new("empty")
    }

    /// Port of `ViaRule.appendVia` (ViaRule.java:29-31).
    pub fn append_via(&mut self, via: ViaInfoId) {
        self.vias.push(via);
    }

    /// Port of `ViaRule.removeVia` (ViaRule.java:34-36): removes the *first* occurrence of
    /// `via`, returning false if the rule did not contain it (Java's `List.remove(Object)`).
    pub fn remove_via(&mut self, via: ViaInfoId) -> bool {
        match self.vias.iter().position(|v| *v == via) {
            Some(index) => {
                self.vias.remove(index);
                true
            }
            None => false,
        }
    }

    /// Port of `ViaRule.viaCount` (ViaRule.java:39-41).
    pub fn via_count(&self) -> usize {
        self.vias.len()
    }

    /// Port of `ViaRule.getVia` (ViaRule.java:44-47). Java asserts the index is in range and
    /// would then throw; the port panics on the same out-of-range index.
    pub fn get_via(&self, index: usize) -> ViaInfoId {
        self.vias[index]
    }

    /// The rule's vias in preference order. Not a Java method — Java iterates `list` directly
    /// (ViaRule.java:56,66,79,93).
    pub fn iter(&self) -> std::slice::Iter<'_, ViaInfoId> {
        self.vias.iter()
    }

    /// Port of `ViaRule.contains` (ViaRule.java:55-62). Java compares with `==`, i.e. object
    /// identity; the index newtype is exactly that identity here.
    pub fn contains(&self, via_info: ViaInfoId) -> bool {
        self.vias.contains(&via_info)
    }

    /// Port of `ViaRule.containsPadstack` (ViaRule.java:65-72): true if any via in this rule uses
    /// `padstack`. Needs [`ViaInfos`] to resolve each index, which Java reaches through the
    /// `ViaInfo` object reference.
    pub fn contains_padstack(&self, padstack: PadstackId, via_infos: &ViaInfos) -> bool {
        self.vias
            .iter()
            .any(|v| via_infos.get(*v).get_padstack() == padstack)
    }

    /// Port of `ViaRule.getLayerRange` (ViaRule.java:78-86): the first via in this rule whose
    /// padstack starts on `from_layer` and ends on `to_layer`, or `None`.
    ///
    /// Java reads `currentInfo.getPadstack().fromLayer()`; the port resolves the via index
    /// through `via_infos` and the padstack index through `padstacks` (see [`PadstackLookup`]).
    pub fn get_layer_range(
        &self,
        from_layer: i32,
        to_layer: i32,
        via_infos: &ViaInfos,
        padstacks: &impl PadstackLookup,
    ) -> Option<ViaInfoId> {
        self.vias.iter().copied().find(|via| {
            let padstack = via_infos.get(*via).get_padstack();
            padstacks.padstack_from_layer(padstack) == from_layer
                && padstacks.padstack_to_layer(padstack) == to_layer
        })
    }
}

impl fmt::Display for ViaRule {
    // renamed: ViaRule.toString -> Display::fmt (ViaRule.java:50-52 returns `name`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for Task 4's `Padstacks`: padstack `n` spans layers `n ..= n + 1` and has a
    /// max width of `10 * (n + 1)`.
    struct TestPadstacks;

    impl PadstackLookup for TestPadstacks {
        fn padstack_from_layer(&self, padstack: PadstackId) -> i32 {
            padstack.0 as i32
        }
        fn padstack_to_layer(&self, padstack: PadstackId) -> i32 {
            padstack.0 as i32 + 1
        }
        fn padstack_shape_max_width(&self, padstack: PadstackId, _layer: i32) -> Option<f64> {
            Some(10.0 * (padstack.0 as f64 + 1.0))
        }
    }

    fn via_infos() -> ViaInfos {
        let mut infos = ViaInfos::new();
        infos.add(ViaInfo::new("via0", PadstackId(0), 1, false));
        infos.add(ViaInfo::new("via1", PadstackId(1), 1, true));
        infos
    }

    #[test]
    fn add_rejects_duplicate_names() {
        // ViaInfos.java:23-25.
        let mut infos = via_infos();
        assert_eq!(infos.count(), 2);
        assert!(!infos.add(ViaInfo::new("via0", PadstackId(5), 2, false)));
        assert_eq!(infos.count(), 2);
        assert_eq!(infos.get(ViaInfoId(0)).get_padstack(), PadstackId(0));
    }

    #[test]
    fn name_lookup_is_case_sensitive() {
        // ViaInfos.java:44 uses `equals`.
        let infos = via_infos();
        assert_eq!(infos.get_no("via1"), Some(ViaInfoId(1)));
        assert_eq!(infos.get_no("VIA1"), None);
        assert!(infos.name_exists("via0"));
        assert!(!infos.name_exists("via2"));
        assert_eq!(
            infos.get_by_name("via1").map(ViaInfo::get_name),
            Some("via1")
        );
    }

    #[test]
    fn remove_reports_whether_the_via_was_present() {
        // ViaInfos.java:62-64.
        let mut infos = via_infos();
        assert!(infos.remove(ViaInfoId(0)));
        assert_eq!(infos.count(), 1);
        // The renumbering hazard: what was index 1 is now index 0.
        assert_eq!(infos.get(ViaInfoId(0)).get_name(), "via1");
        assert!(!infos.remove(ViaInfoId(7)));
    }

    #[test]
    fn via_info_accessors_round_trip() {
        let mut info = ViaInfo::new("v", PadstackId(2), 3, false);
        assert_eq!(info.get_name(), "v");
        assert_eq!(info.get_padstack(), PadstackId(2));
        assert_eq!(info.get_clearance_class_index(), 3);
        assert!(!info.attach_smd_allowed());
        assert_eq!(info.to_string(), "v");

        info.set_name("w");
        info.set_padstack(PadstackId(4));
        info.set_clearance_class_index(5);
        info.set_attach_smd_allowed(true);
        assert_eq!(info.get_name(), "w");
        assert_eq!(info.get_padstack(), PadstackId(4));
        assert_eq!(info.get_clearance_class_index(), 5);
        assert!(info.attach_smd_allowed());
    }

    #[test]
    fn via_info_compare_to_is_case_sensitive() {
        // ViaInfo.java:82 uses `compareTo`, not `compareToIgnoreCase`.
        let a = ViaInfo::new("Via", PadstackId(0), 0, false);
        let b = ViaInfo::new("via", PadstackId(0), 0, false);
        assert_eq!(a.compare_to(&b), Ordering::Less);
        assert_eq!(a.compare_to(&a.clone()), Ordering::Equal);
    }

    #[test]
    fn via_rule_append_remove_and_contains() {
        let mut rule = ViaRule::new("default");
        assert_eq!(rule.name, "default");
        rule.append_via(ViaInfoId(0));
        rule.append_via(ViaInfoId(1));
        assert_eq!(rule.via_count(), 2);
        assert_eq!(rule.get_via(0), ViaInfoId(0));
        assert!(rule.contains(ViaInfoId(1)));
        assert!(!rule.contains(ViaInfoId(2)));

        assert!(rule.remove_via(ViaInfoId(0)));
        assert_eq!(rule.via_count(), 1);
        assert_eq!(rule.get_via(0), ViaInfoId(1));
        // ViaRule.java:35: removing something absent answers false.
        assert!(!rule.remove_via(ViaInfoId(0)));
    }

    #[test]
    fn contains_padstack_resolves_through_via_infos() {
        // ViaRule.java:65-72.
        let infos = via_infos();
        let mut rule = ViaRule::new("default");
        rule.append_via(ViaInfoId(1));
        assert!(rule.contains_padstack(PadstackId(1), &infos));
        assert!(!rule.contains_padstack(PadstackId(0), &infos));
    }

    #[test]
    fn get_layer_range_finds_the_first_matching_via() {
        // ViaRule.java:78-86.
        let infos = via_infos();
        let mut rule = ViaRule::new("default");
        rule.append_via(ViaInfoId(0));
        rule.append_via(ViaInfoId(1));

        assert_eq!(
            rule.get_layer_range(0, 1, &infos, &TestPadstacks),
            Some(ViaInfoId(0))
        );
        assert_eq!(
            rule.get_layer_range(1, 2, &infos, &TestPadstacks),
            Some(ViaInfoId(1))
        );
        assert_eq!(rule.get_layer_range(3, 4, &infos, &TestPadstacks), None);
    }
}
